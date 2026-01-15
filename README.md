# vtrunkd - WireGuard multi-link bonding daemon (Rust)

vtrunkd is a Rust daemon that bonds multiple UDP paths into a single WireGuard tunnel
for higher reliability and aggregate throughput.

## Build

```bash
cargo build --release
```

## Configure

Generate a starter config:

```bash
vtrunkd config --output /etc/vtrunkd.yaml
```

Example configuration:

```yaml
network:
  mtu: 1420
  buffer_size: 65536
  interface: "utun3" # macOS uses utunX; use tun0 on Linux
  address: "10.10.0.2"
  netmask: "255.255.255.0"

wireguard:
  private_key: "<base64>"
  peer_public_key: "<base64>"
  preshared_key: null
  persistent_keepalive: 25
  bonding_mode: "aggregate" # bonding | aggregate | redundant | failover
  error_backoff_secs: 5
  health_check_interval_ms: 1000
  health_check_timeout_ms: 5000
  links:
    - name: "wifi"
      bind: "192.168.1.20:0"
      endpoint: "vps.example.com:51820"
      weight: 1
    - name: "lte"
      bind: "10.0.0.5:0"
      endpoint: "vps.example.com:51820"
      weight: 1
```

If a link has an `endpoint`, vtrunkd will initiate the handshake on startup. If all
endpoints are omitted, it waits for incoming traffic. `bonding_mode` controls how data
is sent across links: `aggregate` (striped/weighted, sums bandwidth), `bonding` (alias
for aggregate), `redundant` (send on all), or `failover` (highest weight first).

Health checks are simple ping/pong messages over the bonding sockets to detect dead
WANs even when the tunnel is idle. Both sides must run vtrunkd for this to work.

## Run

```bash
sudo ./target/release/vtrunkd --config /etc/vtrunkd.yaml --foreground
```
