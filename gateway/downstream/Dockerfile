FROM alpine:3.22

RUN apk add --no-cache dnsmasq hostapd unbound
COPY entrypoint.sh /entrypoint.sh
RUN chmod 0755 /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
