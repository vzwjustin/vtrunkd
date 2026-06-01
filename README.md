# vtrunkd - WireGuard multi-link bonding daemon (Rust)

vtrunkd is a Rust daemon that bonds multiple UDP paths into a single WireGuard tunnel
for higher reliability and aggregate throughput. Kernel QUIC/TQUIC stays outside
the daemon: run QUIC against the peer's tunnel address and let the OS route those
packets through the bonded WireGuard interface.

## Features

- Multi-link bonding over UDP with TQUIC-derived aggregate, weighted, minrtt,
  ECF, BLEST, OWD, redundant, and failover scheduler modes.
- Weighted path selection per link.
- WireGuard tunnel implementation via boringtun.
- Carries arbitrary routed IP traffic, including kernel QUIC/TQUIC sockets.
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
  bonding_mode: "aggregate" # aggregate | roundrobin | weighted | minrtt | ecf | blest | owd | owd-ecf | redundant | failover
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
      endpoint: "vps.example.com:51821"
      weight: 1
```

If a link has an `endpoint`, vtrunkd will initiate the handshake on startup. If all
endpoints are omitted, it waits for incoming traffic. `bonding_mode` controls how data
is sent across links. TQUIC-compatible names are accepted: `aggregate`/`bonding`
(deficit weighted round-robin), `roundrobin`, `weighted`/`wrr`, `minrtt`,
`ecf`, `blest`, `owd`, `owd-ecf`, `redundant` (send on all), and `failover`
(highest weight first).

To route kernel QUIC through vtrunkd, bind or connect the QUIC socket to the
remote tunnel IP, for example `10.10.0.1`. vtrunkd does not open `IPPROTO_TQUIC`
sockets and does not call kernel `TQUIC_BOND_*` sockopts; bonding remains in
userspace on the WireGuard links.

Health checks are simple ping/pong messages over the bonding sockets to detect dead
WANs even when the tunnel is idle. Both sides must run vtrunkd for this to work.

## Configuration notes

- `buffer_size` must be at least the `mtu` size.
- `health_check_timeout_ms` must be greater than `health_check_interval_ms`.
- If `bind` is omitted, the socket binds to `0.0.0.0:0` or `[::]:0` based on the endpoint family.

## Client/server pairing

Both ends must run vtrunkd. It is not a drop-in peer for stock kernel WireGuard.

## Bonding mode guidance

- aggregate/bonding: deficit weighted round-robin; best when RTTs are similar.
- weighted/wrr: weighted round-robin using configured link weights.
- roundrobin/rr: rotate across usable links without weight bias.
- minrtt: prefer the path with the lowest measured health-check RTT.
- ecf: pick the path with the lowest estimated completion time using RTT and
  link weight as a capacity proxy.
- blest: prefer paths with lower estimated blocking delay.
- owd/owd-ecf: use RTT/2 as a one-way-delay approximation until explicit OWD
  measurement frames exist in vtrunkd.
- redundant: send on all links for reliability.
- failover: highest weight link active; others standby.

## Run

```bash
sudo ./target/release/vtrunkd --config /etc/vtrunkd.yaml --foreground
```

## macOS GUI (Control Room)

The desktop app in `gui/` generates client/server configs, provisions a Linux VPS over
SSH, and runs the client tunnel only while the app is active.

```bash
cd gui
npm install
npm run tauri dev
```

Notes:
- SSH provisioning expects key-based auth plus passwordless sudo (or root).
- The server uses one UDP port per client link (base port + link index).

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
- OpenWrt/GL.iNet (Flint 3) packaging and setup guides.
