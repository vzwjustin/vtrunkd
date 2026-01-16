use std::io;
use thiserror::Error;

pub type VtrunkdResult<T> = Result<T, VtrunkdError>;

#[derive(Error, Debug)]
pub enum VtrunkdError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("System call failed: {0}")]
    SystemCall(String),

    #[error("Resource not found: {0}")]
    NotFound(String),
}

impl From<nix::Error> for VtrunkdError {
    fn from(err: nix::Error) -> Self {
        VtrunkdError::SystemCall(err.to_string())
    }
}

impl From<serde_yaml::Error> for VtrunkdError {
    fn from(err: serde_yaml::Error) -> Self {
        VtrunkdError::Config(format!("YAML parsing error: {}", err))
    }
}
