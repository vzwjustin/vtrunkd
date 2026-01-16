# vtrunkd - WireGuard multi-link bonding daemon (Rust)

vtrunkd is a Rust daemon that bonds multiple UDP paths into a single WireGuard tunnel
for higher reliability and aggregate throughput.

## Features

- Multi-link bonding over UDP with aggregate/bonding, redundant, and failover modes.
- Weighted path selection per link.
- WireGuard tunnel implementation via boringtun.
- IPv4 and IPv6 endpoints with automatic default bind family selection.
- Health checks over bonding sockets to detect dead links.
- Strict YAML config validation to prevent invalid MTU/buffer/timeout settings.
- Robust handling of malformed traffic and clean shutdown behavior.

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
    - name: "lte/5g"
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

## Configuration notes

- `buffer_size` must be at least the `mtu` size.
- `health_check_timeout_ms` must be greater than `health_check_interval_ms`.
- If `bind` is omitted, the socket binds to `0.0.0.0:0` or `[::]:0` based on the endpoint family.

## Client/server pairing

Both ends must run vtrunkd. It is not a drop-in peer for stock kernel WireGuard.

## Bonding mode guidance

- aggregate/bonding: stripe packets across links; best when RTTs are similar.
- redundant: send on all links for reliability.
- failover: highest weight link active; others standby.

## Run

```bash
sudo ./target/release/vtrunkd --config /etc/vtrunkd.yaml --foreground
```

## Testing

```bash
cargo test
cargo clippy
```

Optional dependency scan:

```bash
cargo install cargo-audit
cargo audit
```

## Reports

Repository audit artifacts live in `reports/` (analysis, executive summary, and machine-readable findings).

## Roadmap

- Integration tests with a sandboxed TUN device and packet fixtures.
- Link health metrics (RTT, downtime, backoff events) and structured logging.
- Dynamic link reconfiguration without restart.
- Expanded routing and policy controls per link.
- Packaging for common service managers (systemd/launchd) and container images.
