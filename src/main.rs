use std::{
    collections::{BTreeSet, HashMap},
    env,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    process::Command,
    time::Duration,
};

const DEFAULT_PORTS: &[u16] = &[8282, 1080, 10808, 9050, 7890, 3128, 8080];
const KNOWN_HOSTS: &[Ipv4Addr] = &[
    Ipv4Addr::new(192, 168, 49, 1),
    Ipv4Addr::new(192, 168, 42, 129),
    Ipv4Addr::new(192, 168, 42, 1),
];

macro_rules! debug {
    ($enabled:expr, $($arg:tt)*) => {
        if $enabled {
            eprintln!("[micromodem] {}", format_args!($($arg)*));
        }
    };
}

#[derive(Debug, Clone)]
struct Interface {
    name: String,
    addresses: Vec<Ipv4Addr>,
    gateway: Option<Ipv4Addr>,
    score: u8,
}

#[derive(Debug, Clone)]
enum Kind {
    Socks5 { udp_associate: bool },
    HttpConnect,
    OpenTcp,
}

impl Kind {
    fn label(&self) -> String {
        match self {
            Self::Socks5 {
                udp_associate: true,
            } => "SOCKS5 (UDP ASSOCIATE accepted)".into(),
            Self::Socks5 {
                udp_associate: false,
            } => "SOCKS5 (TCP only / UDP ASSOCIATE rejected)".into(),
            Self::HttpConnect => "HTTP CONNECT proxy".into(),
            Self::OpenTcp => "open TCP (unrecognised protocol)".into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Finding {
    host: Ipv4Addr,
    port: u16,
    kind: Kind,
    source: String,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args
        .first()
        .map(String::as_str)
        .is_some_and(|a| a == "--help" || a == "-h")
    {
        print_help();
        return;
    }
    let command = args.first().map(String::as_str).unwrap_or("scan");
    if command == "gui" {
        run_gui();
        return;
    }
    if command != "scan" {
        eprintln!("unknown command: {command}");
        print_help();
        std::process::exit(2);
    }

    let json = args.iter().any(|a| a == "--json");
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let timeout = option_value(&args, "--timeout-ms")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(700);
    let host = option_value(&args, "--host").and_then(|v| v.parse::<Ipv4Addr>().ok());
    let port = option_value(&args, "--port").and_then(|v| v.parse::<u16>().ok());
    if option_value(&args, "--host").is_some() && host.is_none() {
        eprintln!("--host must be an IPv4 address");
        std::process::exit(2);
    }

    let timeout = Duration::from_millis(timeout);
    let (interfaces, findings) = run_scan(host, port, timeout, verbose);
    if json {
        println!("{}", to_json(&interfaces, &findings));
    } else {
        print_human(&interfaces, &findings);
    }
}

fn run_scan(
    host: Option<Ipv4Addr>,
    port: Option<u16>,
    timeout: Duration,
    verbose: bool,
) -> (Vec<Interface>, Vec<Finding>) {
    let interfaces = discover_interfaces();
    let targets = build_targets(&interfaces, host, port);
    debug!(verbose, "timeout: {} ms", timeout.as_millis());
    for interface in &interfaces {
        debug!(
            verbose,
            "candidate interface {} (score {}), addresses: {:?}, gateway: {:?}",
            interface.name,
            interface.score,
            interface.addresses,
            interface.gateway
        );
    }
    debug!(verbose, "probing {} endpoint(s)", targets.len());
    // A Wi-Fi interface can yield dozens of address/port combinations. Probe
    // a small batch concurrently so one filtered port cannot make the UI wait
    // for every individual TCP timeout.
    const PARALLEL_PROBES: usize = 8;
    let mut findings = Vec::new();
    for batch in targets.chunks(PARALLEL_PROBES) {
        let workers: Vec<_> = batch
            .iter()
            .map(|(host, port, source)| {
                let host = *host;
                let port = *port;
                let source = source.clone();
                std::thread::spawn(move || {
                    debug!(verbose, "probe {host}:{port} ({source})");
                    probe(host, port, timeout, verbose).map(|kind| Finding {
                        host,
                        port,
                        kind,
                        source,
                    })
                })
            })
            .collect();
        for worker in workers {
            if let Ok(Some(finding)) = worker.join() {
                findings.push(finding);
            }
        }
    }

    (interfaces, findings)
}

fn print_help() {
    println!(
        "Usage: micromodem [scan] [--host IPV4] [--port PORT] [--timeout-ms N] [--json] [-v|--verbose]\n       micromodem gui\n\nScan Android USB/RNDIS/Wi-Fi networks for SOCKS5 and HTTP proxies.\nEnvironment: MM_CANDIDATES=IP:PORT,... and MM_PORTS=PORT,..."
    );
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == option)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn discover_interfaces() -> Vec<Interface> {
    let mut addresses: HashMap<String, Vec<Ipv4Addr>> = HashMap::new();
    let output = Command::new("ip")
        .args(["-o", "-4", "addr", "show", "scope", "global"])
        .output();
    if let Ok(output) = output {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if let (Some(name), Some(cidr)) = (fields.get(1), fields.get(3))
                && let Some(ip) = cidr.split('/').next().and_then(|s| s.parse().ok())
                && is_private(ip)
            {
                addresses.entry((*name).into()).or_default().push(ip);
            }
        }
    }
    addresses
        .into_iter()
        .map(|(name, addresses)| {
            let gateway = route_gateway(&name);
            let score = interface_score(&name);
            Interface {
                name,
                addresses,
                gateway,
                score,
            }
        })
        .filter(|i| i.score > 0)
        .collect()
}

fn route_gateway(name: &str) -> Option<Ipv4Addr> {
    let output = Command::new("ip")
        .args(["route", "show", "dev", name])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            words
                .windows(2)
                .find(|w| w[0] == "via")
                .and_then(|w| w[1].parse().ok())
        })
}

fn interface_score(name: &str) -> u8 {
    let n = name.to_ascii_lowercase();
    if n.contains("rndis") {
        100
    } else if n.starts_with("usb") {
        95
    } else if n.starts_with("enx") {
        80
    } else if n.starts_with("wl") || n.contains("wifi") || n.contains("wlan") {
        70
    } else if n.starts_with("eth") {
        30
    } else {
        0
    }
}

fn is_private(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1])
}

fn build_targets(
    interfaces: &[Interface],
    specific_host: Option<Ipv4Addr>,
    specific_port: Option<u16>,
) -> Vec<(Ipv4Addr, u16, String)> {
    let ports = specific_port
        .map(|p| vec![p])
        .unwrap_or_else(configured_ports);
    let mut hosts: Vec<(Ipv4Addr, String)> = Vec::new();
    if let Some(host) = specific_host {
        hosts.push((host, "command line".into()));
    } else {
        for interface in interfaces {
            if let Some(gateway) = interface.gateway {
                hosts.push((gateway, format!("gateway on {}", interface.name)));
            }
            for &address in &interface.addresses {
                let [a, b, c, _] = address.octets();
                for last in [1, 2, 129] {
                    hosts.push((
                        Ipv4Addr::new(a, b, c, last),
                        format!("derived from {} ({address})", interface.name),
                    ));
                }
            }
        }
        for &host in KNOWN_HOSTS {
            hosts.push((host, "known Android tethering address".into()));
        }
    }
    for (host, port) in env::var("MM_CANDIDATES")
        .unwrap_or_default()
        .split(',')
        .filter_map(parse_candidate)
    {
        hosts.push((host, "MM_CANDIDATES".into()));
        if specific_host.is_none() && specific_port.is_none() { /* included below with the configured ports too */
        }
        let _ = port;
    }

    let explicit: Vec<(Ipv4Addr, u16)> = env::var("MM_CANDIDATES")
        .unwrap_or_default()
        .split(',')
        .filter_map(parse_candidate)
        .collect();
    let mut out = BTreeSet::new();
    let mut result = Vec::new();
    for (host, source) in hosts {
        for &port in &ports {
            if out.insert((host, port)) {
                result.push((host, port, source.clone()));
            }
        }
    }
    for (host, port) in explicit {
        if out.insert((host, port)) {
            result.push((host, port, "MM_CANDIDATES".into()));
        }
    }
    result
}

fn configured_ports() -> Vec<u16> {
    env::var("MM_PORTS")
        .ok()
        .map(|s| s.split(',').filter_map(|p| p.trim().parse().ok()).collect())
        .filter(|v: &Vec<u16>| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_PORTS.to_vec())
}

fn parse_candidate(s: &str) -> Option<(Ipv4Addr, u16)> {
    let (host, port) = s.trim().rsplit_once(':')?;
    Some((host.parse().ok()?, port.parse().ok()?))
}

fn connect(host: Ipv4Addr, port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let stream = TcpStream::connect_timeout(&SocketAddr::new(IpAddr::V4(host), port), timeout)
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

fn probe(host: Ipv4Addr, port: u16, timeout: Duration, verbose: bool) -> Option<Kind> {
    let stream = match connect(host, port, timeout) {
        Ok(stream) => stream,
        Err(error) => {
            debug!(verbose, "  TCP connect failed: {error}");
            return None;
        }
    };
    debug!(verbose, "  TCP connected; trying SOCKS5 greeting");
    if let Some(udp) = socks5(stream, host, port, timeout, verbose) {
        debug!(verbose, "  SOCKS5 confirmed; UDP ASSOCIATE accepted: {udp}");
        return Some(Kind::Socks5 { udp_associate: udp });
    }
    debug!(verbose, "  not SOCKS5; trying HTTP CONNECT");
    let mut stream = match connect(host, port, timeout) {
        Ok(stream) => stream,
        Err(error) => {
            debug!(verbose, "  reconnect for HTTP probe failed: {error}");
            return Some(Kind::OpenTcp);
        }
    };
    if let Err(error) = stream.write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\nProxy-Connection: close\r\n\r\n") {
        debug!(verbose, "  HTTP CONNECT write failed: {error}");
        return Some(Kind::OpenTcp);
    }
    let mut reply = [0_u8; 256];
    let n = stream.read(&mut reply).unwrap_or_else(|error| {
        debug!(verbose, "  HTTP CONNECT response unavailable: {error}");
        0
    });
    if String::from_utf8_lossy(&reply[..n]).starts_with("HTTP/") {
        debug!(verbose, "  HTTP CONNECT response received");
        Some(Kind::HttpConnect)
    } else {
        debug!(verbose, "  no recognised proxy protocol response");
        Some(Kind::OpenTcp)
    }
}

fn socks5(
    mut stream: TcpStream,
    host: Ipv4Addr,
    port: u16,
    timeout: Duration,
    verbose: bool,
) -> Option<bool> {
    if stream.write_all(&[5, 1, 0]).is_err() {
        return None;
    }
    let mut hello = [0; 2];
    if stream.read_exact(&mut hello).is_err() {
        return None;
    }
    if hello[0] != 5 || hello[1] == 0xff {
        debug!(
            verbose,
            "  SOCKS5 greeting rejected or response was not SOCKS5: {hello:?}"
        );
        return None;
    }
    if hello[1] != 0 {
        debug!(
            verbose,
            "  SOCKS5 requires authentication method {}; not probing UDP", hello[1]
        );
        return Some(false);
    }
    let mut udp = match connect(host, port, timeout) {
        Ok(stream) => stream,
        Err(error) => {
            debug!(verbose, "  SOCKS5 UDP probe reconnect failed: {error}");
            return Some(false);
        }
    };
    if udp.write_all(&[5, 1, 0]).is_err() || udp.read_exact(&mut hello).is_err() {
        return Some(false);
    }
    if hello != [5, 0] {
        return Some(false);
    }
    if udp.write_all(&[5, 3, 0, 1, 0, 0, 0, 0, 0, 0]).is_err() {
        return Some(false);
    }
    let mut header = [0; 4];
    if udp.read_exact(&mut header).is_err() || header[0] != 5 {
        return Some(false);
    }
    if header[1] != 0 {
        debug!(
            verbose,
            "  SOCKS5 server rejected UDP ASSOCIATE with reply code {}", header[1]
        );
        return Some(false);
    }
    let remaining = match header[3] {
        1 => 6,
        4 => 18,
        3 => {
            let mut n = [0; 1];
            if udp.read_exact(&mut n).is_err() {
                return Some(false);
            }
            n[0] as usize + 2
        }
        _ => return Some(false),
    };
    let mut discard = vec![0; remaining];
    Some(udp.read_exact(&mut discard).is_ok())
}

fn print_human(interfaces: &[Interface], findings: &[Finding]) {
    if interfaces.is_empty() {
        println!("No likely USB/RNDIS interface with a private IPv4 address found.");
    } else {
        for i in interfaces {
            println!(
                "Interface {} (score {}): addresses [{}]{}",
                i.name,
                i.score,
                i.addresses
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                i.gateway
                    .map(|g| format!(", gateway {g}"))
                    .unwrap_or_default()
            );
        }
    }
    if findings.is_empty() {
        println!("No proxy detected. Try --host 192.168.49.1 --port 8282 or set MM_CANDIDATES.");
    } else {
        for f in findings {
            println!("{}:{} — {} [{}]", f.host, f.port, f.kind.label(), f.source);
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
fn to_json(interfaces: &[Interface], findings: &[Finding]) -> String {
    format!("{{\"interfaces\":[{}],\"proxies\":[{}]}}", interfaces.iter().map(|i| format!("{{\"name\":\"{}\",\"addresses\":[{}],\"gateway\":{},\"score\":{}}}", esc(&i.name), i.addresses.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(","), i.gateway.map(|g| format!("\"{g}\"")).unwrap_or("null".into()), i.score)).collect::<Vec<_>>().join(","), findings.iter().map(|f| format!("{{\"host\":\"{}\",\"port\":{},\"kind\":\"{}\",\"udp_associate\":{},\"source\":\"{}\"}}", f.host, f.port, match f.kind { Kind::Socks5 {..} => "socks5", Kind::HttpConnect => "http_connect", Kind::OpenTcp => "open_tcp" }, matches!(f.kind, Kind::Socks5 { udp_associate: true }), esc(&f.source))).collect::<Vec<_>>().join(","))
}

fn run_gui() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![scan_networks])
        .run(tauri::generate_context!())
        .expect("error while running MicroModem");
}

#[tauri::command]
async fn scan_networks() -> String {
    tauri::async_runtime::spawn_blocking(|| {
        let (interfaces, findings) = run_scan(None, None, Duration::from_millis(700), false);
        to_json(&interfaces, &findings)
    })
    .await
    .unwrap_or_else(|error| format!("{{\"error\":\"scan worker failed: {error}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_configured_ipv4_endpoint() {
        assert_eq!(
            parse_candidate("192.168.49.1:8282"),
            Some((Ipv4Addr::new(192, 168, 49, 1), 8282))
        );
        assert_eq!(parse_candidate("not-an-address:8282"), None);
    }

    #[test]
    fn identifies_expected_usb_interface_names() {
        assert!(interface_score("rndis0") > interface_score("eth0"));
        assert!(interface_score("usb0") > interface_score("enp4s0"));
        assert!(interface_score("wlp2s0") > interface_score("eth0"));
    }
}
