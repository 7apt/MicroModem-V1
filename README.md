# MicroModem

`micromodem` is a small, dependency-free Linux utility for finding a proxy
served by an Android phone over USB/RNDIS tethering. It does not assume that a
service on a familiar port is SOCKS: it performs protocol handshakes.

## What it detects

* likely Android USB/RNDIS and Wi-Fi interfaces (USB is preferred; names such
  as `rndis0`, `usb0`, `enx…`, `wlan0`, and `wlp…` are recognised);
* private IPv4 addresses and the interface's route gateway;
* likely peer addresses, including `192.168.49.1`;
* SOCKS5, including whether the proxy accepts `UDP ASSOCIATE`;
* HTTP CONNECT proxies; and
* open TCP ports whose protocol could not be identified.

The scanner does not alter routes, DNS, or firewall rules. That separation is
intentional: discovery is safe to run automatically, while traffic steering
needs an explicit policy.

## Build and run

```bash
cargo build --release
./target/release/micromodem scan
```

For a known service:

```bash
./target/release/micromodem scan --host 192.168.49.1 --port 8282
```

For connection and handshake diagnostics, add `--verbose` (or `-v`). Debug
output is written to stderr, so `--json` remains safe to pipe into another
program:

```bash
./target/release/micromodem scan --json --verbose > discovery.json
```

Add endpoints or replace the port list without recompiling:

```bash
MM_CANDIDATES=192.168.49.1:8282,192.168.42.129:1080 \
MM_PORTS=8282,1080,8080 ./target/release/micromodem scan
```

`UDP ASSOCIATE accepted` establishes that the server implements the SOCKS5 UDP
command. It does not prove that UDP can reach every Internet destination; a
carrier, VPN, or proxy policy can still restrict it.

## Local GUI

Start the dashboard with:

```bash
./target/release/micromodem gui
```

This opens a native Tauri desktop window with a lavender-and-dark-grey
dashboard and a **Scan networks** button. The web UI has no network listener:
it invokes the Rust scanner through Tauri's command bridge.

## Suggested next layer

Use this command as a discovery component, then let a privileged service
consume its machine-readable `--json` output and *explicitly* choose a routing
policy. Native RNDIS routing is preferable for full TCP/UDP; a SOCKS5 endpoint
is useful for applications that support SOCKS and for a later TUN-to-SOCKS
adapter.
