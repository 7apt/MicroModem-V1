#!/bin/sh
set -eu

: "${DOWNSTREAM_IF:?DOWNSTREAM_IF is required}"
: "${DOWNSTREAM_ADDR:=10.77.0.1/24}"
: "${DOWNSTREAM_SUBNET:=10.77.0.0/24}"
: "${CLIENT_MTU:=1400}"
: "${DHCP_START:=10.77.0.20}"
: "${DHCP_END:=10.77.0.200}"
: "${DHCP_LEASE:=12h}"
: "${DNS_SERVERS:=1.1.1.1,9.9.9.9}"
: "${DOWNSTREAM_MODE:=ethernet}"

gateway_addr=${DOWNSTREAM_ADDR%/*}

cat > /tmp/dnsmasq.conf <<EOF
interface=${DOWNSTREAM_IF}
bind-dynamic
except-interface=lo
dhcp-range=${DHCP_START},${DHCP_END},${DHCP_LEASE}
dhcp-option=option:router,${gateway_addr}
dhcp-option=option:dns-server,${gateway_addr}
dhcp-option=option:mtu,${CLIENT_MTU}
domain-needed
bogus-priv
port=0
EOF

cat > /tmp/unbound.conf <<EOF
server:
  interface: ${gateway_addr}
  port: 53
  do-ip4: yes
  do-ip6: no
  do-udp: yes
  do-tcp: yes
  access-control: ${DOWNSTREAM_SUBNET} allow
  outgoing-interface: ${gateway_addr}
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
for dns_server in $DNS_SERVERS; do
  printf '  forward-addr: %s\n' "$dns_server" >> /tmp/unbound.conf
done
IFS=$old_ifs

unbound -d -c /tmp/unbound.conf &
unbound_pid=$!

if [ "$DOWNSTREAM_MODE" = wifi ]; then
  : "${WIFI_SSID:?WIFI_SSID is required in wifi mode}"
  : "${WIFI_PASSPHRASE:?WIFI_PASSPHRASE is required in wifi mode}"
  if [ "${#WIFI_PASSPHRASE}" -lt 8 ] || [ "${#WIFI_PASSPHRASE}" -gt 63 ]; then
    echo "WIFI_PASSPHRASE must contain 8 to 63 characters" >&2
    exit 2
  fi
  cat > /tmp/hostapd.conf <<EOF
interface=${DOWNSTREAM_IF}
driver=nl80211
ssid=${WIFI_SSID}
country_code=${WIFI_COUNTRY:-US}
hw_mode=g
channel=${WIFI_CHANNEL:-6}
ieee80211n=1
wmm_enabled=1
auth_algs=1
wpa=2
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
wpa_passphrase=${WIFI_PASSPHRASE}
EOF
  hostapd /tmp/hostapd.conf &
  hostapd_pid=$!
  trap 'kill "$hostapd_pid" "$unbound_pid" 2>/dev/null || true' EXIT INT TERM
elif [ "$DOWNSTREAM_MODE" != ethernet ]; then
  echo "DOWNSTREAM_MODE must be ethernet or wifi" >&2
  exit 2
fi

trap 'kill "${hostapd_pid:-}" "$unbound_pid" 2>/dev/null || true' EXIT INT TERM
dnsmasq --keep-in-foreground --conf-file=/tmp/dnsmasq.conf &
dnsmasq_pid=$!
wait "$dnsmasq_pid"
