use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{VtrunkdError, VtrunkdResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub wireguard: WireGuardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub mtu: u32,
    pub buffer_size: usize,
    pub interface: Option<String>,
    pub address: Option<String>,
    pub netmask: Option<String>,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    pub private_key: String,
    pub peer_public_key: String,
    pub preshared_key: Option<String>,
    pub persistent_keepalive: Option<u16>,
    pub bonding_mode: Option<String>,
    pub error_backoff_secs: Option<u64>,
    pub health_check_interval_ms: Option<u64>,
    pub health_check_timeout_ms: Option<u64>,
    pub links: Vec<WireGuardLinkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardLinkConfig {
    pub name: Option<String>,
    pub bind: Option<String>,
    pub endpoint: Option<String>,
    pub weight: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            network: NetworkConfig {
                mtu: 1420,
                buffer_size: 65536,
                interface: None,
                address: None,
                netmask: None,
                destination: None,
            },
            wireguard: WireGuardConfig {
                private_key: "REPLACE_ME".to_string(),
                peer_public_key: "REPLACE_ME".to_string(),
                preshared_key: None,
                persistent_keepalive: Some(25),
                bonding_mode: Some("aggregate".to_string()),
                error_backoff_secs: Some(5),
                health_check_interval_ms: Some(1000),
                health_check_timeout_ms: Some(5000),
                links: vec![WireGuardLinkConfig {
                    name: Some("link-0".to_string()),
                    bind: Some("0.0.0.0:0".to_string()),
                    endpoint: Some("example.com:51820".to_string()),
                    weight: Some(1),
                }],
            },
        }
    }
}

pub fn load_config(path: &Path) -> VtrunkdResult<Config> {
    if !path.exists() {
        return Err(VtrunkdError::NotFound(format!(
            "Configuration file not found: {:?}",
            path
        )));
    }

    let contents = std::fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&contents)?;
    validate_config(&config)?;
    Ok(config)
}

pub fn generate_default_config(path: &Path) -> VtrunkdResult<()> {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

fn validate_config(config: &Config) -> VtrunkdResult<()> {
    if config.network.mtu == 0 {
        return Err(VtrunkdError::InvalidConfig(
            "Network MTU cannot be 0".to_string(),
        ));
    }

    if config.network.buffer_size == 0 {
        return Err(VtrunkdError::InvalidConfig(
            "Network buffer_size cannot be 0".to_string(),
        ));
    }

    if config.wireguard.private_key.is_empty() {
        return Err(VtrunkdError::InvalidConfig(
            "WireGuard private_key is required".to_string(),
        ));
    }

    if config.wireguard.peer_public_key.is_empty() {
        return Err(VtrunkdError::InvalidConfig(
            "WireGuard peer_public_key is required".to_string(),
        ));
    }

    if config.wireguard.links.is_empty() {
        return Err(VtrunkdError::InvalidConfig(
            "WireGuard links cannot be empty".to_string(),
        ));
    }

    if let Some(mode) = config.wireguard.bonding_mode.as_deref() {
        let mode = mode.to_ascii_lowercase();
        let is_valid = matches!(
            mode.as_str(),
            "aggregate" | "redundant" | "failover" | "bonding" | "bonded"
        );
        if !is_valid {
            return Err(VtrunkdError::InvalidConfig(format!(
                "Unsupported bonding_mode: {}",
                mode
            )));
        }
    }

    if let Some(backoff) = config.wireguard.error_backoff_secs {
        if backoff == 0 {
            return Err(VtrunkdError::InvalidConfig(
                "error_backoff_secs must be greater than 0".to_string(),
            ));
        }
    }

    if let Some(interval) = config.wireguard.health_check_interval_ms {
        if interval == 0 {
            return Err(VtrunkdError::InvalidConfig(
                "health_check_interval_ms must be greater than 0".to_string(),
            ));
        }
    }

    if let Some(timeout) = config.wireguard.health_check_timeout_ms {
        if timeout == 0 {
            return Err(VtrunkdError::InvalidConfig(
                "health_check_timeout_ms must be greater than 0".to_string(),
            ));
        }
    }

    if let (Some(interval), Some(timeout)) = (
        config.wireguard.health_check_interval_ms,
        config.wireguard.health_check_timeout_ms,
    ) {
        if timeout <= interval {
            return Err(VtrunkdError::InvalidConfig(
                "health_check_timeout_ms must be greater than health_check_interval_ms".to_string(),
            ));
        }
    }

    for link in &config.wireguard.links {
        if let Some(weight) = link.weight {
            if weight == 0 {
                return Err(VtrunkdError::InvalidConfig(
                    "WireGuard link weight must be greater than 0".to_string(),
                ));
            }
        }
    }

    Ok(())
}
