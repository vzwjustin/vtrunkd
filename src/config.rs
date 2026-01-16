use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_HEALTH_INTERVAL_MS: u64 = 1000;

use crate::error::{VtrunkdError, VtrunkdResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub network: NetworkConfig,
    pub wireguard: WireGuardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    pub mtu: u32,
    pub buffer_size: usize,
    pub interface: Option<String>,
    pub address: Option<String>,
    pub netmask: Option<String>,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireGuardConfig {
    pub private_key: String,
    pub peer_public_key: String,
    pub preshared_key: Option<String>,
    pub persistent_keepalive: Option<u16>,
    pub bonding_mode: Option<BondingMode>,
    pub error_backoff_secs: Option<u64>,
    pub health_check_interval_ms: Option<u64>,
    pub health_check_timeout_ms: Option<u64>,
    pub links: Vec<WireGuardLinkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireGuardLinkConfig {
    pub name: Option<String>,
    pub bind: Option<String>,
    pub endpoint: Option<String>,
    pub weight: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BondingMode {
    #[default]
    #[serde(alias = "bonding", alias = "bonded")]
    Aggregate,
    Redundant,
    Failover,
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
                bonding_mode: Some(BondingMode::Aggregate),
                error_backoff_secs: Some(5),
                health_check_interval_ms: Some(DEFAULT_HEALTH_INTERVAL_MS),
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

    if config.network.mtu > u16::MAX as u32 {
        return Err(VtrunkdError::InvalidConfig(
            "Network MTU exceeds u16::MAX".to_string(),
        ));
    }

    if config.network.buffer_size == 0 {
        return Err(VtrunkdError::InvalidConfig(
            "Network buffer_size cannot be 0".to_string(),
        ));
    }

    if config.network.buffer_size < config.network.mtu as usize {
        return Err(VtrunkdError::InvalidConfig(
            "Network buffer_size must be at least MTU size".to_string(),
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

    if let Some(timeout) = config.wireguard.health_check_timeout_ms {
        let interval = config
            .wireguard
            .health_check_interval_ms
            .unwrap_or(DEFAULT_HEALTH_INTERVAL_MS);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonding_mode_aliases_parse() {
        let aggregate: BondingMode = serde_yaml::from_str("bonding").unwrap();
        assert_eq!(aggregate, BondingMode::Aggregate);

        let aggregate2: BondingMode = serde_yaml::from_str("bonded").unwrap();
        assert_eq!(aggregate2, BondingMode::Aggregate);

        let redundant: BondingMode = serde_yaml::from_str("redundant").unwrap();
        assert_eq!(redundant, BondingMode::Redundant);
    }

    #[test]
    fn config_rejects_unknown_fields() {
        let yaml = r#"
network:
  mtu: 1420
  buffer_size: 65536
  extra: 123
wireguard:
  private_key: "key"
  peer_public_key: "peer"
  links:
    - endpoint: "example.com:51820"
"#;
        let parsed: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(parsed.is_err());
    }

    #[test]
    fn validate_config_rejects_timeout_le_interval() {
        let mut config = Config::default();
        config.wireguard.health_check_interval_ms = Some(1000);
        config.wireguard.health_check_timeout_ms = Some(1000);
        let result = validate_config(&config);
        assert!(matches!(result, Err(VtrunkdError::InvalidConfig(_))));
    }

    #[test]
    fn validate_config_rejects_timeout_le_default_interval() {
        let mut config = Config::default();
        config.wireguard.health_check_interval_ms = None;
        config.wireguard.health_check_timeout_ms = Some(DEFAULT_HEALTH_INTERVAL_MS);
        let result = validate_config(&config);
        assert!(matches!(result, Err(VtrunkdError::InvalidConfig(_))));
    }

    #[test]
    fn validate_config_rejects_buffer_smaller_than_mtu() {
        let mut config = Config::default();
        config.network.mtu = 1500;
        config.network.buffer_size = 1000;
        let result = validate_config(&config);
        assert!(matches!(result, Err(VtrunkdError::InvalidConfig(_))));
    }

    #[test]
    fn validate_config_rejects_mtu_too_large() {
        let mut config = Config::default();
        config.network.mtu = (u16::MAX as u32) + 1;
        let result = validate_config(&config);
        assert!(matches!(result, Err(VtrunkdError::InvalidConfig(_))));
    }
}
