# MicroModem Server

This bundle runs the MicroModem data plane on Linux servers and Raspberry Pi
systems. It has no GUI and requires no Rust or Nix installation.

## Requirements

- Linux with `/dev/net/tun`
- Docker Engine with the Compose v2 plugin (`docker compose`)
- `iproute2`, `nftables`, and standard `sysctl` utilities on the host
- root access for network configuration
- `hostapd`-capable Wi-Fi hardware for access-point mode

The pinned tunnel image supports `amd64`, `arm64`, `arm/v7`, `arm/v6`, `386`,
and `riscv64`. The downstream image is built locally from Alpine for the
machine's native architecture.

## Start

```sh
cp gateway/.env.example gateway/.env
# Edit gateway/.env for the phone proxy and downstream interface.
./micromodem start
./micromodem status
```

Stop and remove only MicroModem's containers and network policy with:

```sh
./micromodem stop
```

MicroModem uses host networking and narrowly scoped policy-routing/firewall
rules because containers cannot expose a physical Wi-Fi adapter as an access
point through an ordinary Docker bridge.
