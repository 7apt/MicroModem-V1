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
nix develop path:./nix
cargo build --release
./target/release/micromodem scan
```

Or build without entering a persistent development shell:

```bash
nix develop path:./nix --command cargo build --release
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
./run-gui
```

This opens a native, GPU-rendered Rust window with a lavender-and-dark-grey
dashboard and a **Scan networks** button. It uses egui/eframe directly and has
no browser engine, WebKit process, HTML, JavaScript, or local web server.

Use `./run-gui` during development instead of launching a binary under
`target/` directly. The launcher selects the newest debug/release build and
rebuilds it whenever the Rust source is newer.

## Suggested next layer

Use this command as a discovery component, then let a privileged service
consume its machine-readable `--json` output and *explicitly* choose a routing
policy. Native RNDIS routing is preferable for full TCP/UDP; a SOCKS5 endpoint
is useful for applications that support SOCKS and for a later TUN-to-SOCKS
adapter.

## Cellular gateway

The `gateway/` directory contains the first data-plane implementation. It
turns a detected Android SOCKS5 endpoint into a routed Ethernet handoff for a
router WAN port, or into a Wi-Fi access point. TCP and UDP are translated by
`hev-socks5-tunnel`; DHCP, policy routing, and firewall rules are managed
separately. DHCP gives clients the configured public DNS addresses directly,
so DNS follows the tunnel policy instead of the computer's default route.

Requirements are Linux, Docker Compose, nftables, `iproute2`, `/dev/net/tun`,
and root access. For Wi-Fi mode, the selected adapter and driver must support
nl80211 AP mode.

```bash
cp gateway/.env.example gateway/.env
# Edit SOCKS5_ADDR, SOCKS5_PORT, DOWNSTREAM_IF, and DOWNSTREAM_MODE.
sudo gateway/micromodem-gateway start
sudo gateway/micromodem-gateway status
sudo gateway/micromodem-gateway stop
```

Ethernet mode is intended for connection to a router's WAN port. Wi-Fi mode
runs `hostapd` in the downstream container. In both modes clients receive
addresses from `10.77.0.0/24` by default. The gateway installs a policy rule
only for packets arriving on the downstream interface, so host traffic and
the SOCKS5 control connection cannot recurse into the tunnel.

This is routed IP service, not a layer-2 bridge. Broadcast, multicast, and
inbound port forwarding through the Android/carrier network are not implied.

## Headless server and Raspberry Pi release

Build the architecture-independent server archive with:

```bash
nix build path:.#release
```

The archive and SHA-256 checksum are placed under `result/`. The target Linux
machine needs Docker Compose, iproute2, nftables, and `/dev/net/tun`, but does
not need Rust, Nix, GTK, or the desktop GUI. See `gateway/RELEASE.md` for target
installation and operation.

## Portable Linux GUI release

Build an x86_64 AppImage bundle together with the gateway assets:

```bash
./build-gui-release
```

The GUI release is built inside a Debian 12 container so it does not retain
Nix store paths and targets glibc 2.36 rather than the build host's libc.

Extract the archive from `dist/`, copy `gateway/.env.example` to
`gateway/.env`, and launch `./micromodem-gui`. The launcher keeps the writable
gateway configuration outside the AppImage and points the GUI at it.
