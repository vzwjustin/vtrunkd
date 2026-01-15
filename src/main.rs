use clap::{Parser, Subcommand};
use tokio::signal;
use tracing::{error, info};
use std::path::PathBuf;
use std::os::fd::AsRawFd;

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

    let run_handle = tokio::spawn(async move {
        if let Err(e) = wireguard::run(config).await {
            error!("WireGuard error: {}", e);
        }
    });

    signal::ctrl_c().await?;
    info!("Received shutdown signal");

    run_handle.abort();
    run_handle.await.ok();

    info!("vtrunkd shutdown complete");
    Ok(())
}

fn daemonize() -> VtrunkdResult<()> {
    use nix::unistd::{fork, ForkResult, setsid, chdir, close};
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
            let dev_null = File::open("/dev/null")?;
            let _ = nix::unistd::dup2(dev_null.as_raw_fd(), 0)?;
            let _ = nix::unistd::dup2(dev_null.as_raw_fd(), 1)?;
            let _ = nix::unistd::dup2(dev_null.as_raw_fd(), 2)?;

            Ok(())
        }
    }
}
