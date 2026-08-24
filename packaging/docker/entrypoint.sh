#!/bin/sh
set -eu

: "${SOCKS5_ADDR:?SOCKS5_ADDR is required}"
: "${DOWNSTREAM_IF:?DOWNSTREAM_IF is required}"
: "${DOWNSTREAM_ADDR:=10.77.0.1/24}"
: "${DOWNSTREAM_SUBNET:=10.77.0.0/24}"
: "${CLIENT_MTU:=${MTU:-1400}}"
: "${DOWNSTREAM_MODE:=ethernet}"

table_id=109
rule_priority=10900
resolver_rule_priority=10899
tun_if=${TUN:-mm-tun0}
gateway_addr=${DOWNSTREAM_ADDR%/*}
original_downstream_mtu=$(ip -o link show dev "$DOWNSTREAM_IF" 2>/dev/null | awk '{ for (i=1; i<=NF; i++) if ($i == "mtu") { print $(i+1); exit } }')

cleanup() {
  trap - EXIT INT TERM
  kill "${hostapd_pid:-}" "${dnsmasq_pid:-}" "${unbound_pid:-}" "${tunnel_pid:-}" 2>/dev/null || true
  nft delete table inet micromodem 2>/dev/null || true
  ip rule del priority "$rule_priority" 2>/dev/null || true
  ip rule del priority "$resolver_rule_priority" 2>/dev/null || true
  ip route flush table "$table_id" 2>/dev/null || true
  ip address del "$DOWNSTREAM_ADDR" dev "$DOWNSTREAM_IF" 2>/dev/null || true
  case "$original_downstream_mtu" in ''|*[!0-9]*) ;; *) ip link set dev "$DOWNSTREAM_IF" mtu "$original_downstream_mtu" 2>/dev/null || true;; esac
}
trap cleanup EXIT INT TERM

ip link show dev "$DOWNSTREAM_IF" >/dev/null 2>&1 || {
  echo "MicroModem: interface not found: $DOWNSTREAM_IF" >&2
  exit 2
}
upstream_if=$(ip route get "$SOCKS5_ADDR" | awk '{ for (i=1; i<=NF; i++) if ($i == "dev") { print $(i+1); exit } }')
[ -n "$upstream_if" ] || { echo "MicroModem: no route to $SOCKS5_ADDR" >&2; exit 2; }
[ "$upstream_if" != "$DOWNSTREAM_IF" ] || { echo "MicroModem: upstream and downstream are both $DOWNSTREAM_IF" >&2; exit 2; }

/tunnel-entrypoint.sh &
tunnel_pid=$!
count=0
while ! ip link show dev "$tun_if" >/dev/null 2>&1; do
  count=$((count + 1))
  [ "$count" -lt 100 ] || { echo "MicroModem: tunnel was not created" >&2; exit 2; }
  kill -0 "$tunnel_pid" 2>/dev/null || { wait "$tunnel_pid"; exit $?; }
  sleep 0.2
done

ip address flush dev "$DOWNSTREAM_IF" scope global
ip address replace "$DOWNSTREAM_ADDR" dev "$DOWNSTREAM_IF"
ip link set dev "$DOWNSTREAM_IF" mtu "$CLIENT_MTU"
ip link set dev "$DOWNSTREAM_IF" up
sysctl -w net.ipv4.ip_forward=1 >/dev/null
ip rule del priority "$rule_priority" 2>/dev/null || true
ip rule del priority "$resolver_rule_priority" 2>/dev/null || true
ip route flush table "$table_id" 2>/dev/null || true
ip route replace table "$table_id" "$DOWNSTREAM_SUBNET" dev "$DOWNSTREAM_IF" scope link
ip route replace table "$table_id" default dev "$tun_if"
ip rule add priority "$rule_priority" iif "$DOWNSTREAM_IF" lookup "$table_id"
ip rule add priority "$resolver_rule_priority" from "$gateway_addr/32" lookup "$table_id"

nft delete table inet micromodem 2>/dev/null || true
nft -f - <<EOF
table inet micromodem {
  chain forward {
    type filter hook forward priority -5; policy accept;
    iifname "$DOWNSTREAM_IF" oifname "$tun_if" tcp flags syn tcp option maxseg size set rt mtu
    iifname "$DOWNSTREAM_IF" oifname "$tun_if" counter accept
    iifname "$tun_if" oifname "$DOWNSTREAM_IF" counter accept
    iifname "$DOWNSTREAM_IF" drop
    oifname "$DOWNSTREAM_IF" drop
  }
  chain input {
    type filter hook input priority -5; policy accept;
    iifname "$DOWNSTREAM_IF" udp dport { 67, 68 } accept
    iifname "$DOWNSTREAM_IF" ip daddr $gateway_addr meta l4proto { tcp, udp } th dport 53 accept
    iifname "$DOWNSTREAM_IF" ip daddr $gateway_addr icmp type echo-request accept
    iifname "$DOWNSTREAM_IF" drop
  }
}
EOF

cat >/tmp/dnsmasq.conf <<EOF
interface=$DOWNSTREAM_IF
bind-dynamic
except-interface=lo
dhcp-range=${DHCP_START:-10.77.0.20},${DHCP_END:-10.77.0.200},${DHCP_LEASE:-12h}
dhcp-option=option:router,$gateway_addr
dhcp-option=option:dns-server,$gateway_addr
dhcp-option=option:mtu,$CLIENT_MTU
domain-needed
bogus-priv
port=0
EOF

cat >/tmp/unbound.conf <<EOF
server:
  interface: $gateway_addr
  port: 53
  do-ip4: yes
  do-ip6: no
  do-udp: yes
  do-tcp: yes
  access-control: $DOWNSTREAM_SUBNET allow
  outgoing-interface: $gateway_addr
  username: unbound
  directory: /tmp
  pidfile: /tmp/unbound.pid
  logfile: ""
  use-syslog: no
  verbosity: 1
  tcp-upstream: yes
forward-zone:
  name: "."
EOF
old_ifs=$IFS
IFS=,
for dns_server in ${DNS_SERVERS:-1.1.1.1,9.9.9.9}; do
  printf '  forward-addr: %s\n' "$dns_server" >>/tmp/unbound.conf
done
IFS=$old_ifs
unbound -d -c /tmp/unbound.conf &
unbound_pid=$!

if [ "$DOWNSTREAM_MODE" = wifi ]; then
  : "${WIFI_PASSPHRASE:?WIFI_PASSPHRASE is required in Wi-Fi mode}"
  cat >/tmp/hostapd.conf <<EOF
interface=$DOWNSTREAM_IF
driver=nl80211
ssid=${WIFI_SSID:-MicroModem}
country_code=${WIFI_COUNTRY:-US}
hw_mode=g
channel=${WIFI_CHANNEL:-6}
ieee80211n=1
wmm_enabled=1
wpa=2
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
wpa_passphrase=$WIFI_PASSPHRASE
EOF
  hostapd /tmp/hostapd.conf &
  hostapd_pid=$!
elif [ "$DOWNSTREAM_MODE" != ethernet ]; then
  echo "MicroModem: DOWNSTREAM_MODE must be ethernet or wifi" >&2
  exit 2
fi

dnsmasq --keep-in-foreground --conf-file=/tmp/dnsmasq.conf &
dnsmasq_pid=$!
echo "MicroModem appliance is routing $DOWNSTREAM_IF through $SOCKS5_ADDR:${SOCKS5_PORT:-8228}"
wait -n "$tunnel_pid" "$dnsmasq_pid" "$unbound_pid" ${hostapd_pid:+"$hostapd_pid"}
