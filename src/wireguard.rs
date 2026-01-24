use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose, Engine as _};
use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};
use tokio::net::{lookup_host, UdpSocket};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::{
    BondingMode, Config, WireGuardConfig, WireGuardLinkConfig, DEFAULT_HEALTH_INTERVAL_MS,
};
use crate::error::{VtrunkdError, VtrunkdResult};
use crate::network::TunnelDevice;

const WG_KEEPALIVE_LEN: usize = 32;
const BOND_MAGIC: [u8; 4] = *b"VTBD";
const BOND_PING: u8 = 1;
const BOND_PONG: u8 = 2;
const BOND_PACKET_LEN: usize = 13;
const DEFAULT_ERROR_BACKOFF_SECS: u64 = 5;

struct Link {
    name: String,
    socket: Arc<UdpSocket>,
    remote: Option<SocketAddr>,
    weight: u32,
    down_since: Option<Instant>,
    last_rx: Option<Instant>,
    last_ping_sent: Option<Instant>,
    last_rtt_ms: Option<u64>,
}

struct LinkManager {
    links: Vec<Link>,
    mode: BondingMode,
    error_backoff: Duration,
    health_timeout: Option<Duration>,
    next_index: usize,
    remaining_weight: u32,
}

struct NetPacket {
    link_index: usize,
    src: SocketAddr,
    data: Vec<u8>,
}

trait TunnelWriter {
    fn write_packet<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = VtrunkdResult<()>> + Send + 'a>>;
}

impl TunnelWriter for TunnelDevice {
    fn write_packet<'a>(
        &'a self,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = VtrunkdResult<()>> + Send + 'a>> {
        Box::pin(async move { TunnelDevice::write_packet(self, data).await })
    }
}

pub async fn run(config: Config) -> VtrunkdResult<()> {
    let wg_config = &config.wireguard;
    let bonding_mode = wg_config.bonding_mode.unwrap_or_default();
    let error_backoff = Duration::from_secs(
        wg_config
            .error_backoff_secs
            .unwrap_or(DEFAULT_ERROR_BACKOFF_SECS),
    );
    let health_interval = Duration::from_millis(
        wg_config
            .health_check_interval_ms
            .unwrap_or(DEFAULT_HEALTH_INTERVAL_MS),
    );
    let health_timeout = wg_config.health_check_timeout_ms.map(Duration::from_millis);

    let private_key = decode_key("private_key", &wg_config.private_key)?;
    let peer_public_key = decode_key("peer_public_key", &wg_config.peer_public_key)?;
    let preshared_key = match &wg_config.preshared_key {
        Some(value) => Some(decode_key("preshared_key", value)?),
        None => None,
    };

    let index = rand::random::<u32>();

    let mut tunnel = Tunn::new(
        StaticSecret::from(private_key),
        PublicKey::from(peer_public_key),
        preshared_key,
        wg_config.persistent_keepalive,
        index,
        None,
    );

    let device = TunnelDevice::new(&config.network)?;
    info!("WireGuard TUN device {} ready", device.name());
    info!(
        "WireGuard bonding mode {:?}, error backoff {}s",
        bonding_mode,
        error_backoff.as_secs()
    );
    if let Some(timeout) = health_timeout {
        info!(
            "WireGuard health checks every {}ms (timeout {}ms)",
            health_interval.as_millis(),
            timeout.as_millis()
        );
    }

    let (mut links, mut net_rx) = setup_links(
        wg_config,
        config.network.buffer_size,
        bonding_mode,
        error_backoff,
        health_timeout,
    )
    .await?;
    if links.links.is_empty() {
        return Err(VtrunkdError::InvalidConfig(
            "WireGuard links must include at least one entry".to_string(),
        ));
    }

    if links.has_endpoints() {
        send_handshake(&mut tunnel, &mut links).await?;
    }

    let mut tun_buf = vec![0u8; config.network.buffer_size];
    let mut out_buf = vec![0u8; std::cmp::max(config.network.buffer_size + 32, 148)];
    let mut wg_timer = tokio::time::interval(tokio::time::Duration::from_millis(250));
    let mut health_timer = tokio::time::interval(health_interval);
    let bond_epoch = Instant::now();

    loop {
        tokio::select! {
            result = device.read_packet(&mut tun_buf) => {
                let size = result?;
                if size == 0 {
                    continue;
                }
                match tunnel.encapsulate(&tun_buf[..size], &mut out_buf) {
                    TunnResult::WriteToNetwork(packet) => {
                        let payload = packet.to_vec();
                        links.send_packet(&payload).await?;
                    }
                    TunnResult::Done => {}
                    TunnResult::Err(e) => {
                        return Err(VtrunkdError::Network(format!("WireGuard encapsulate error: {:?}", e)));
                    }
                    TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {
                        debug!("Unexpected tunnel write during encapsulate");
                    }
                }
            }

            packet = net_rx.recv() => {
                let packet = match packet {
                    Some(packet) => packet,
                    None => break,
                };
                links.update_remote(packet.link_index, packet.src, Instant::now());
                handle_incoming(
                    &mut tunnel,
                    &device,
                    &mut links,
                    &mut out_buf,
                    bond_epoch,
                    packet,
                )
                .await?;
            }

            _ = wg_timer.tick() => {
                match tunnel.update_timers(&mut out_buf) {
                    TunnResult::WriteToNetwork(packet) => {
                        let payload = packet.to_vec();
                        links.send_packet(&payload).await?;
                    }
                    TunnResult::Done => {}
                    TunnResult::Err(e) => {
                        return Err(VtrunkdError::Network(format!("WireGuard timer error: {:?}", e)));
                    }
                    TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {}
                }
            }

            _ = health_timer.tick() => {
                if health_timeout.is_some() {
                    links.send_health_pings(bond_epoch).await?;
                }
            }
        }
    }

    Ok(())
}

async fn handle_incoming(
    tunnel: &mut Tunn,
    device: &impl TunnelWriter,
    links: &mut LinkManager,
    out_buf: &mut [u8],
    bond_epoch: Instant,
    packet: NetPacket,
) -> VtrunkdResult<()> {
    if links
        .handle_control_packet(packet.link_index, &packet.data, bond_epoch)
        .await?
    {
        return Ok(());
    }

    let mut result = tunnel.decapsulate(Some(packet.src.ip()), &packet.data, out_buf);

    loop {
        match result {
            TunnResult::WriteToNetwork(buffer) => {
                let payload = buffer.to_vec();
                links.send_packet(&payload).await?;
                result = tunnel.decapsulate(None, &[], out_buf);
            }
            TunnResult::WriteToTunnelV4(buffer, _) | TunnResult::WriteToTunnelV6(buffer, _) => {
                let payload = buffer.to_vec();
                device.write_packet(&payload).await?;
                return Ok(());
            }
            TunnResult::Done => return Ok(()),
            TunnResult::Err(e) => {
                warn!("WireGuard decapsulate error: {:?}", e);
                return Ok(());
            }
        }
    }
}

async fn send_handshake(tunnel: &mut Tunn, links: &mut LinkManager) -> VtrunkdResult<()> {
    let mut out_buf = vec![0u8; 2048];
    match tunnel.format_handshake_initiation(&mut out_buf, true) {
        TunnResult::WriteToNetwork(packet) => {
            let payload = packet.to_vec();
            links.send_packet(&payload).await?;
        }
        TunnResult::Done => {}
        TunnResult::Err(e) => {
            return Err(VtrunkdError::Network(format!(
                "WireGuard handshake error: {:?}",
                e
            )))
        }
        TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {}
    }
    Ok(())
}

async fn setup_links(
    wg_config: &WireGuardConfig,
    buffer_size: usize,
    mode: BondingMode,
    error_backoff: Duration,
    health_timeout: Option<Duration>,
) -> VtrunkdResult<(LinkManager, mpsc::Receiver<NetPacket>)> {
    let (tx, rx) = mpsc::channel(1024);
    let mut links = Vec::new();

    for (index, link_config) in wg_config.links.iter().enumerate() {
        let (socket, remote) = create_link_socket(link_config).await?;
        let name = link_config
            .name
            .clone()
            .unwrap_or_else(|| format!("link-{}", index));
        let log_name = name.clone();

        let socket = Arc::new(socket);
        let recv_socket = Arc::clone(&socket);
        let tx = tx.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; buffer_size];
            loop {
                match recv_socket.recv_from(&mut buf).await {
                    Ok((size, src)) => {
                        let payload = buf[..size].to_vec();
                        if tx
                            .send(NetPacket {
                                link_index: index,
                                src,
                                data: payload,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        error!("WireGuard socket recv error on {}: {}", log_name, err);
                        break;
                    }
                }
            }
        });

        links.push(Link {
            name,
            socket,
            remote,
            weight: link_config.weight.unwrap_or(1),
            down_since: None,
            last_rx: None,
            last_ping_sent: None,
            last_rtt_ms: None,
        });
    }

    Ok((
        LinkManager {
            links,
            mode,
            error_backoff,
            health_timeout,
            next_index: 0,
            remaining_weight: 0,
        },
        rx,
    ))
}

async fn create_link_socket(
    link_config: &WireGuardLinkConfig,
) -> VtrunkdResult<(UdpSocket, Option<SocketAddr>)> {
    let remote = match &link_config.endpoint {
        Some(endpoint) => Some(resolve_endpoint(endpoint).await?),
        None => None,
    };

    let bind_addr = match link_config.bind.as_deref() {
        Some(value) => parse_bind_addr(value)?,
        None => default_bind_addr(remote),
    };
    let socket = UdpSocket::bind(bind_addr).await?;

    Ok((socket, remote))
}

fn default_bind_addr(remote: Option<SocketAddr>) -> SocketAddr {
    match remote {
        Some(SocketAddr::V6(_)) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        _ => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
    }
}

fn parse_bind_addr(value: &str) -> VtrunkdResult<SocketAddr> {
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Ok(addr);
    }

    if let Ok(ip) = value.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, 0));
    }

    Err(VtrunkdError::InvalidConfig(format!(
        "Invalid WireGuard bind address: {}",
        value
    )))
}

async fn resolve_endpoint(value: &str) -> VtrunkdResult<SocketAddr> {
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Ok(addr);
    }

    let mut resolved = lookup_host(value)
        .await
        .map_err(|e| VtrunkdError::InvalidConfig(format!("Failed to resolve {}: {}", value, e)))?;

    resolved
        .next()
        .ok_or_else(|| VtrunkdError::InvalidConfig(format!("No addresses resolved for {}", value)))
}

fn decode_key(label: &str, value: &str) -> VtrunkdResult<[u8; 32]> {
    let decoded = general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| VtrunkdError::InvalidConfig(format!("Invalid base64 for {}", label)))?;
    if decoded.len() != 32 {
        return Err(VtrunkdError::InvalidConfig(format!(
            "Invalid {} length (expected 32 bytes, got {})",
            label,
            decoded.len()
        )));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

fn build_control_packet(message_type: u8, token: u64) -> [u8; BOND_PACKET_LEN] {
    let mut buf = [0u8; BOND_PACKET_LEN];
    buf[..4].copy_from_slice(&BOND_MAGIC);
    buf[4] = message_type;
    buf[5..].copy_from_slice(&token.to_be_bytes());
    buf
}

fn parse_control_packet(data: &[u8]) -> Option<(u8, u64)> {
    if data.len() != BOND_PACKET_LEN {
        return None;
    }
    if data[..4] != BOND_MAGIC {
        return None;
    }
    let message_type = data[4];
    let token = u64::from_be_bytes(data[5..13].try_into().ok()?);
    Some((message_type, token))
}

impl Link {
    fn is_available(
        &mut self,
        now: Instant,
        error_backoff: Duration,
        health_timeout: Option<Duration>,
    ) -> bool {
        if self.remote.is_none() {
            return false;
        }

        if let Some(timeout) = health_timeout {
            match (self.last_rx, self.last_ping_sent) {
                (Some(last_rx), _) => {
                    if now.duration_since(last_rx) > timeout {
                        if self.down_since.is_none() {
                            warn!("WireGuard {} marked down (no rx)", self.name);
                        }
                        self.down_since = Some(now);
                        return false;
                    }
                }
                (None, Some(last_ping)) => {
                    if now.duration_since(last_ping) > timeout {
                        if self.down_since.is_none() {
                            warn!("WireGuard {} marked down (no pong)", self.name);
                        }
                        self.down_since = Some(now);
                        return false;
                    }
                }
                (None, None) => {}
            }
        }

        if let Some(down_since) = self.down_since {
            if now.duration_since(down_since) < error_backoff {
                return false;
            }
        }

        true
    }

    fn record_rx(&mut self, now: Instant) {
        self.last_rx = Some(now);
        if self.down_since.take().is_some() {
            info!("WireGuard {} recovered (rx)", self.name);
        }
    }

    fn record_ping(&mut self, now: Instant) {
        self.last_ping_sent = Some(now);
    }

    fn record_rtt(&mut self, rtt_ms: u64) {
        self.last_rtt_ms = Some(rtt_ms);
    }

    fn record_send_ok(&mut self) {
        if self.down_since.take().is_some() {
            info!("WireGuard {} recovered", self.name);
        }
    }

    fn record_send_error(&mut self, now: Instant, err: &std::io::Error) {
        if self.down_since.is_none() {
            warn!("WireGuard {} marked down: {}", self.name, err);
        }
        self.down_since = Some(now);
    }
}

impl LinkManager {
    fn has_endpoints(&self) -> bool {
        self.links.iter().any(|link| link.remote.is_some())
    }

    fn update_remote(&mut self, index: usize, src: SocketAddr, now: Instant) {
        if let Some(link) = self.links.get_mut(index) {
            if link.remote != Some(src) {
                debug!("WireGuard {} remote updated to {}", link.name, src);
            }
            link.remote = Some(src);
            link.record_rx(now);
        }
    }

    async fn send_health_pings(&mut self, epoch: Instant) -> VtrunkdResult<()> {
        let token = epoch.elapsed().as_millis() as u64;
        let packet = build_control_packet(BOND_PING, token);
        let now = Instant::now();

        for index in 0..self.links.len() {
            if self.send_probe(index, &packet, now).await {
                self.links[index].record_ping(now);
            }
        }

        Ok(())
    }

    async fn handle_control_packet(
        &mut self,
        link_index: usize,
        data: &[u8],
        epoch: Instant,
    ) -> VtrunkdResult<bool> {
        let (message_type, token) = match parse_control_packet(data) {
            Some(parsed) => parsed,
            None => return Ok(false),
        };

        let now = Instant::now();
        match message_type {
            BOND_PING => {
                let response = build_control_packet(BOND_PONG, token);
                let _ = self.send_probe(link_index, &response, now).await;
            }
            BOND_PONG => {
                if let Some(link) = self.links.get_mut(link_index) {
                    let elapsed = epoch.elapsed().as_millis() as u64;
                    if elapsed >= token {
                        link.record_rtt(elapsed - token);
                    }
                }
            }
            _ => {}
        }

        Ok(true)
    }

    async fn send_packet(&mut self, packet: &[u8]) -> VtrunkdResult<()> {
        let packet_type = wg_packet_type(packet);
        let is_keepalive = packet_type == Some(4) && packet.len() == WG_KEEPALIVE_LEN;
        match packet_type {
            Some(1..=3) => self.send_all(packet).await?,
            Some(4) if is_keepalive => self.send_all(packet).await?,
            _ => match self.mode {
                BondingMode::Aggregate => self.send_round_robin(packet).await?,
                BondingMode::Redundant => self.send_all(packet).await?,
                BondingMode::Failover => self.send_failover(packet).await?,
            },
        }
        Ok(())
    }

    async fn send_all(&mut self, packet: &[u8]) -> VtrunkdResult<()> {
        let now = Instant::now();
        let mut sent = 0usize;
        for index in 0..self.links.len() {
            if self.send_to_link(index, packet, now).await {
                sent += 1;
            }
        }

        if sent == 0 {
            warn!("WireGuard has no remote endpoints to send to");
        }
        Ok(())
    }

    async fn send_round_robin(&mut self, packet: &[u8]) -> VtrunkdResult<()> {
        let now = Instant::now();
        let len = self.links.len();
        if len == 0 {
            return Ok(());
        }

        let mut attempts = 0usize;
        while attempts < len {
            let index = match self.next_weighted_index(now) {
                Some(index) => index,
                None => break,
            };
            if self.send_to_link(index, packet, now).await {
                return Ok(());
            }
            attempts += 1;
        }

        if !self.send_any(packet, now).await {
            warn!("WireGuard has no remote endpoints to send to");
        }
        Ok(())
    }

    async fn send_failover(&mut self, packet: &[u8]) -> VtrunkdResult<()> {
        let now = Instant::now();
        if let Some(index) = self.best_failover_index(now) {
            if self.send_to_link(index, packet, now).await {
                return Ok(());
            }
        }

        if !self.send_any(packet, now).await {
            warn!("WireGuard has no remote endpoints to send to");
        }
        Ok(())
    }

    fn next_weighted_index(&mut self, now: Instant) -> Option<usize> {
        if self.links.is_empty() {
            return None;
        }

        let len = self.links.len();
        let mut attempts = 0usize;
        while attempts < len {
            let index = self.next_index % len;
            let link = &mut self.links[index];
            if link.weight == 0 || !link.is_available(now, self.error_backoff, self.health_timeout)
            {
                self.advance_cursor(len);
                attempts += 1;
                continue;
            }

            if self.remaining_weight == 0 {
                self.remaining_weight = link.weight;
            }

            if self.remaining_weight > 0 {
                self.remaining_weight -= 1;
                if self.remaining_weight == 0 {
                    self.advance_cursor(len);
                }
                return Some(index);
            }

            self.advance_cursor(len);
            attempts += 1;
        }

        None
    }

    fn best_failover_index(&mut self, now: Instant) -> Option<usize> {
        let mut best: Option<(usize, u32)> = None;
        for (index, link) in self.links.iter_mut().enumerate() {
            if !link.is_available(now, self.error_backoff, self.health_timeout) {
                continue;
            }
            let weight = link.weight;
            match best {
                Some((_, best_weight)) if best_weight >= weight => {}
                _ => best = Some((index, weight)),
            }
        }
        best.map(|(index, _)| index)
    }

    async fn send_any(&mut self, packet: &[u8], now: Instant) -> bool {
        for index in 0..self.links.len() {
            if self.send_to_link(index, packet, now).await {
                return true;
            }
        }
        false
    }

    async fn send_to_link(&mut self, index: usize, packet: &[u8], now: Instant) -> bool {
        let remote = match self.links[index].remote {
            Some(remote) => remote,
            None => return false,
        };
        // Bolt optimization: Avoid unnecessary Arc::clone on hot path
        let send_result = self.links[index].socket.send_to(packet, remote).await;
        let link = &mut self.links[index];
        match send_result {
            Ok(_) => {
                link.record_send_ok();
                true
            }
            Err(err) => {
                link.record_send_error(now, &err);
                false
            }
        }
    }

    async fn send_probe(&mut self, index: usize, packet: &[u8], now: Instant) -> bool {
        let remote = match self.links[index].remote {
            Some(remote) => remote,
            None => return false,
        };
        // Bolt optimization: Avoid unnecessary Arc::clone on hot path
        let send_result = self.links[index].socket.send_to(packet, remote).await;
        let link = &mut self.links[index];
        match send_result {
            Ok(_) => {
                link.record_send_ok();
                true
            }
            Err(err) => {
                link.record_send_error(now, &err);
                false
            }
        }
    }

    fn advance_cursor(&mut self, len: usize) {
        self.next_index = (self.next_index + 1) % len;
        self.remaining_weight = 0;
    }
}

fn wg_packet_type(packet: &[u8]) -> Option<u32> {
    if packet.len() < 4 {
        return None;
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&packet[..4]);
    Some(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn control_packet_round_trip() {
        let token = 42u64;
        let packet = build_control_packet(BOND_PING, token);
        let parsed = parse_control_packet(&packet).expect("parse control packet");
        assert_eq!(parsed, (BOND_PING, token));
    }

    #[test]
    fn control_packet_rejects_bad_magic() {
        let mut packet = build_control_packet(BOND_PING, 1);
        packet[0] = b'X';
        assert!(parse_control_packet(&packet).is_none());
    }

    #[test]
    fn decode_key_rejects_wrong_length() {
        let result = decode_key("test", "AAAA");
        assert!(matches!(result, Err(VtrunkdError::InvalidConfig(_))));
    }

    #[test]
    fn wg_packet_type_reads_le() {
        let mut packet = Vec::new();
        packet.extend_from_slice(&3u32.to_le_bytes());
        packet.extend_from_slice(&[0u8; 8]);
        assert_eq!(wg_packet_type(&packet), Some(3));
    }

    #[test]
    fn parse_bind_addr_accepts_ip_only() {
        let addr = parse_bind_addr("127.0.0.1").expect("parse bind addr");
        let expected = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        assert_eq!(addr, expected);
    }

    #[test]
    fn default_bind_addr_prefers_ipv6_for_ipv6_remote() {
        let remote = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 51820);
        let bind_addr = default_bind_addr(Some(remote));
        let expected = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
        assert_eq!(bind_addr, expected);
    }

    #[tokio::test]
    async fn link_marks_down_after_missed_pong() {
        let now = Instant::now();
        let last_ping = now
            .checked_sub(Duration::from_secs(10))
            .expect("instant subtraction");
        let mut link = Link {
            name: "link-0".to_string(),
            socket: Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()),
            remote: Some("127.0.0.1:12345".parse().unwrap()),
            weight: 1,
            down_since: None,
            last_rx: None,
            last_ping_sent: Some(last_ping),
            last_rtt_ms: None,
        };

        let available = link.is_available(now, Duration::from_secs(1), Some(Duration::from_secs(3)));
        assert!(!available);
        assert!(link.down_since.is_some());
    }

    #[tokio::test]
    async fn handle_incoming_drops_invalid_packet() {
        struct TestDevice;

        impl TunnelWriter for TestDevice {
            fn write_packet<'a>(
                &'a self,
                _data: &'a [u8],
            ) -> Pin<Box<dyn Future<Output = VtrunkdResult<()>> + Send + 'a>> {
                Box::pin(async { Ok(()) })
            }
        }

        let mut tunnel = Tunn::new(
            StaticSecret::from([1u8; 32]),
            PublicKey::from([2u8; 32]),
            None,
            None,
            1,
            None,
        );

        let packet = NetPacket {
            link_index: 0,
            src: "127.0.0.1:12345".parse().unwrap(),
            data: vec![0u8; 1],
        };

        let mut links = LinkManager {
            links: Vec::new(),
            mode: BondingMode::Aggregate,
            error_backoff: Duration::from_secs(1),
            health_timeout: None,
            next_index: 0,
            remaining_weight: 0,
        };

        let mut out_buf = vec![0u8; 256];
        let probe = tunnel.decapsulate(Some(packet.src.ip()), &packet.data, &mut out_buf);
        assert!(matches!(probe, TunnResult::Err(_)));

        let mut tunnel = Tunn::new(
            StaticSecret::from([1u8; 32]),
            PublicKey::from([2u8; 32]),
            None,
            None,
            1,
            None,
        );
        let result = handle_incoming(
            &mut tunnel,
            &TestDevice,
            &mut links,
            &mut out_buf,
            Instant::now(),
            packet,
        )
        .await;
        assert!(result.is_ok());
    }
}
