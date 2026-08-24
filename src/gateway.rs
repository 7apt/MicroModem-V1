use std::{
    collections::HashMap,
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const DEFAULT_CONFIG: &[(&str, &str)] = &[
    ("SOCKS5_ADDR", ""),
    ("SOCKS5_PORT", "8228"),
    ("DOWNSTREAM_IF", ""),
    ("DOWNSTREAM_MODE", "wifi"),
    ("DOWNSTREAM_ADDR", "10.77.0.1/24"),
    ("DOWNSTREAM_SUBNET", "10.77.0.0/24"),
    ("TUN_MTU", "1400"),
    ("CLIENT_MTU", "1400"),
    ("DHCP_START", "10.77.0.20"),
    ("DHCP_END", "10.77.0.200"),
    ("DHCP_LEASE", "12h"),
    ("DNS_SERVERS", "1.1.1.1,9.9.9.9"),
    ("WIFI_SSID", "MicroModem"),
    ("WIFI_PASSPHRASE", ""),
    ("WIFI_CHANNEL", "6"),
    ("WIFI_COUNTRY", "US"),
    ("SOCKS5_UDP_MODE", "udp"),
    ("SOCKS5_USERNAME", ""),
    ("SOCKS5_PASSWORD", ""),
    ("LOG_LEVEL", "info"),
];

fn gateway_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("MICROMODEM_GATEWAY_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(binary_dir) = executable.parent()
    {
        for candidate in [
            binary_dir.join("gateway"),
            binary_dir.join("../share/micromodem/gateway"),
        ] {
            if candidate.join("micromodem-gateway").is_file() {
                return candidate;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("gateway")
}

fn config_path() -> PathBuf {
    gateway_dir().join(".env")
}

pub(crate) fn parse_config() -> HashMap<String, String> {
    let mut values: HashMap<String, String> = DEFAULT_CONFIG
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect();
    if let Ok(contents) = fs::read_to_string(config_path()) {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim();
                let unquoted = if value.len() >= 2
                    && ((value.starts_with('\'') && value.ends_with('\''))
                        || (value.starts_with('"') && value.ends_with('"')))
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                values.insert(key.trim().into(), unquoted.into());
            }
        }
    }
    values
}

fn output_text(output: Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let message = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => "Command completed.".into(),
    };
    if output.status.success() {
        Ok(message)
    } else {
        Err(message)
    }
}

fn safe_value(name: &str, value: &str) -> Result<(), String> {
    if value.contains(['\n', '\r', '\'', '\0']) {
        return Err(format!("{name} contains an unsupported character"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn save_gateway_config(
    socks5_addr: String,
    socks5_port: String,
    downstream_if: String,
    downstream_mode: String,
    wifi_ssid: String,
    wifi_passphrase: String,
    wifi_channel: String,
    wifi_country: String,
) -> Result<String, String> {
    socks5_addr
        .parse::<IpAddr>()
        .map_err(|_| "Proxy address must be an IPv4 or IPv6 address".to_string())?;
    socks5_port
        .parse::<u16>()
        .map_err(|_| "Proxy port must be between 1 and 65535".to_string())?;
    if downstream_if.is_empty()
        || !downstream_if
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
    {
        return Err("Downstream interface is invalid".into());
    }
    if downstream_mode != "ethernet" && downstream_mode != "wifi" {
        return Err("Downstream mode must be ethernet or wifi".into());
    }
    if downstream_mode == "wifi" && !(8..=63).contains(&wifi_passphrase.len()) {
        return Err("Wi-Fi password must contain 8 to 63 characters".into());
    }
    let channel = wifi_channel
        .parse::<u8>()
        .map_err(|_| "Wi-Fi channel is invalid".to_string())?;
    if channel == 0 || channel > 196 {
        return Err("Wi-Fi channel is invalid".into());
    }
    if wifi_country.len() != 2 || !wifi_country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Country must be a two-letter code".into());
    }
    for (name, value) in [
        ("proxy address", socks5_addr.as_str()),
        ("interface", downstream_if.as_str()),
        ("SSID", wifi_ssid.as_str()),
        ("Wi-Fi password", wifi_passphrase.as_str()),
    ] {
        safe_value(name, value)?;
    }

    let mut values = parse_config();
    for (key, value) in [
        ("SOCKS5_ADDR", socks5_addr),
        ("SOCKS5_PORT", socks5_port),
        ("DOWNSTREAM_IF", downstream_if),
        ("DOWNSTREAM_MODE", downstream_mode),
        ("WIFI_SSID", wifi_ssid),
        ("WIFI_PASSPHRASE", wifi_passphrase),
        ("WIFI_CHANNEL", wifi_channel),
        ("WIFI_COUNTRY", wifi_country.to_ascii_uppercase()),
    ] {
        values.insert(key.into(), value);
    }
    let contents = DEFAULT_CONFIG
        .iter()
        .map(|(key, _)| {
            format!(
                "{key}='{}'",
                values.get(*key).map(String::as_str).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(config_path(), contents).map_err(|error| format!("Could not save config: {error}"))?;
    Ok("Configuration saved.".into())
}

pub fn gateway_action(action: String) -> Result<String, String> {
    if !matches!(action.as_str(), "start" | "stop" | "restart") {
        return Err("Unsupported gateway action".into());
    }
    let script = gateway_dir().join("micromodem-gateway");
    Command::new("pkexec")
        // Nix development shells expose a store-path SHELL which pkexec
        // rejects unless it appears in /etc/shells. The gateway script is
        // POSIX sh, so pass the stable system shell to the auth helper.
        .env("SHELL", "/bin/sh")
        .arg(script)
        .arg(action)
        .output()
        .map_err(|error| format!("Could not open the administrator prompt: {error}"))
        .and_then(output_text)
}

pub fn gateway_status() -> Result<String, String> {
    Command::new(gateway_dir().join("micromodem-gateway"))
        .arg("status")
        .output()
        .map_err(|error| format!("Could not inspect gateway: {error}"))
        .and_then(output_text)
}

pub fn gateway_logs() -> Result<String, String> {
    let mut combined = String::new();
    for container in ["micromodem-tunnel", "micromodem-downstream"] {
        let output = Command::new("docker")
            .args(["logs", "--tail", "80", container])
            .output()
            .map_err(|error| format!("Could not read container logs: {error}"))?;
        combined.push_str(&format!("== {container} ==\n"));
        combined.push_str(&String::from_utf8_lossy(&output.stdout));
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        combined.push('\n');
    }
    Ok(combined)
}
