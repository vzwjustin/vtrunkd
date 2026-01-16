use clap::{Parser, Subcommand};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info};

mod config;
mod error;
mod network;
mod wireguard;

use crate::error::VtrunkdResult;

#[derive(Parser)]
#[command(name = "vtrunkd")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Universal network link bonding and multichannel VPN daemon")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Run in foreground (don't daemonize)
    #[arg(short, long)]
    foreground: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate configuration file
    Config {
        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> VtrunkdResult<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.debug {
        "vtrunkd=debug"
    } else {
        "vtrunkd=info"
    };

    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();

    info!("Starting vtrunkd {}", env!("CARGO_PKG_VERSION"));

    match cli.command {
        Some(Commands::Config { output }) => {
            config::generate_default_config(&output)?;
            info!("Generated default configuration at {:?}", output);
            return Ok(());
        }
        None => {}
    }

    let config_path = cli
        .config
        .unwrap_or_else(|| PathBuf::from("/etc/vtrunkd.yaml"));
    let config = config::load_config(&config_path)?;

    if !cli.foreground {
        daemonize()?;
    }

    if let Err(e) = run_until_shutdown(wireguard::run(config), signal::ctrl_c()).await {
        error!("WireGuard error: {}", e);
        return Err(e);
    }

    info!("vtrunkd shutdown complete");
    Ok(())
}

async fn run_until_shutdown<R, S>(run_fut: R, shutdown: S) -> VtrunkdResult<()>
where
    R: std::future::Future<Output = VtrunkdResult<()>> + Send + 'static,
    S: std::future::Future<Output = std::io::Result<()>> + Send,
{
    let mut run_handle = tokio::spawn(run_fut);
    tokio::select! {
        result = &mut run_handle => {
            match result {
                Ok(Ok(())) => Err(error::VtrunkdError::Network(
                    "WireGuard task exited unexpectedly".to_string(),
                )),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(error::VtrunkdError::Network(format!(
                    "WireGuard task join error: {}",
                    e
                ))),
            }
        }
        shutdown_result = shutdown => {
            shutdown_result?;
            info!("Received shutdown signal");
            run_handle.abort();
            let _ = run_handle.await;
            Ok(())
        }
    }
}

fn daemonize() -> VtrunkdResult<()> {
    use nix::unistd::{chdir, close, fork, setsid, ForkResult};
    use std::fs::File;

    match unsafe { fork() }? {
        ForkResult::Parent { .. } => {
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Create new session
            setsid()?;

            // Change working directory
            chdir("/")?;

            // Close standard file descriptors
            close(0)?;
            close(1)?;
            close(2)?;

            // Redirect to /dev/null
            let dev_null_in = File::open("/dev/null")?;
            let dev_null_out = std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/null")?;

            let _ = nix::unistd::dup2(dev_null_in.as_raw_fd(), 0)?;
            let _ = nix::unistd::dup2(dev_null_out.as_raw_fd(), 1)?;
            let _ = nix::unistd::dup2(dev_null_out.as_raw_fd(), 2)?;

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_until_shutdown_errors_on_run_failure() {
        let run_fut = async { Err(error::VtrunkdError::Network("boom".to_string())) };
        let shutdown = std::future::pending::<std::io::Result<()>>();
        let result = run_until_shutdown(run_fut, shutdown).await;
        assert!(matches!(result, Err(error::VtrunkdError::Network(_))));
    }

    #[tokio::test]
    async fn run_until_shutdown_errors_on_unexpected_exit() {
        let run_fut = async { Ok(()) };
        let shutdown = std::future::pending::<std::io::Result<()>>();
        let result = run_until_shutdown(run_fut, shutdown).await;
        assert!(matches!(result, Err(error::VtrunkdError::Network(_))));
    }

    #[tokio::test]
    async fn run_until_shutdown_returns_ok_on_shutdown() {
        let run_fut = std::future::pending::<VtrunkdResult<()>>();
        let shutdown = async { Ok(()) };
        let result = run_until_shutdown(run_fut, shutdown).await;
        assert!(result.is_ok());
    }
}
