use crate::config::NetworkConfig;
use crate::error::{VtrunkdError, VtrunkdResult};
use tun::{Configuration, Layer};

pub struct TunnelDevice {
    name: String,
    device: tun::AsyncDevice,
}

impl TunnelDevice {
    pub fn new(config: &NetworkConfig) -> VtrunkdResult<Self> {
        let name = config
            .interface
            .clone()
            .unwrap_or_else(|| "tun0".to_string());
        let mut configuration = Configuration::default();
        configuration.tun_name(&name);
        configuration.layer(Layer::L3);
        configuration.mtu(config.mtu as u16);
        configuration.up();

        if let Some(address) = &config.address {
            let parsed: std::net::IpAddr = address.parse().map_err(|_| {
                VtrunkdError::InvalidConfig(format!("Invalid tun address: {}", address))
            })?;
            configuration.address(parsed);
        }

        if let Some(netmask) = &config.netmask {
            let parsed: std::net::IpAddr = netmask.parse().map_err(|_| {
                VtrunkdError::InvalidConfig(format!("Invalid tun netmask: {}", netmask))
            })?;
            configuration.netmask(parsed);
        }

        if let Some(destination) = &config.destination {
            let parsed: std::net::IpAddr = destination.parse().map_err(|_| {
                VtrunkdError::InvalidConfig(format!("Invalid tun destination: {}", destination))
            })?;
            configuration.destination(parsed);
        }

        let device = tun::create_as_async(&configuration).map_err(|e| {
            VtrunkdError::Network(format!("Failed to create TUN device: {}", e))
        })?;

        Ok(TunnelDevice { name, device })
    }

    pub async fn read_packet(&self, buf: &mut [u8]) -> VtrunkdResult<usize> {
        let size = self.device.recv(buf).await?;
        Ok(size)
    }

    pub async fn write_packet(&self, data: &[u8]) -> VtrunkdResult<()> {
        self.device.send(data).await?;
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
