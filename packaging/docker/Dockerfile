FROM ghcr.io/heiher/hev-socks5-tunnel:2.17@sha256:48e2ab1caf3e35e2a49262d9cceef9aacea279448a3b6d2938e4d47cd3cf3003 AS tunnel

FROM alpine:3.22
RUN apk add --no-cache dnsmasq hostapd iproute2 nftables unbound
COPY --from=tunnel /usr/bin/hev-socks5-tunnel /usr/bin/hev-socks5-tunnel
COPY --from=tunnel /entrypoint.sh /tunnel-entrypoint.sh
COPY entrypoint.sh /entrypoint.sh
RUN chmod 0755 /entrypoint.sh /tunnel-entrypoint.sh

ENV TUN=mm-tun0 \
    MTU=1400 \
    CLIENT_MTU=1400 \
    CONFIG_ROUTES=0 \
    SOCKS5_PORT=8228 \
    SOCKS5_UDP_MODE=udp \
    DOWNSTREAM_MODE=ethernet \
    DOWNSTREAM_ADDR=10.77.0.1/24 \
    DOWNSTREAM_SUBNET=10.77.0.0/24 \
    DHCP_START=10.77.0.20 \
    DHCP_END=10.77.0.200 \
    DHCP_LEASE=12h \
    DNS_SERVERS=1.1.1.1,9.9.9.9 \
    WIFI_SSID=MicroModem \
    WIFI_CHANNEL=6 \
    WIFI_COUNTRY=US

HEALTHCHECK --interval=5s --timeout=2s --start-period=10s --retries=3 \
  CMD ip link show dev mm-tun0 >/dev/null 2>&1 || exit 1
ENTRYPOINT ["/entrypoint.sh"]
