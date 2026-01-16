#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use base64::{engine::general_purpose, Engine as _};
use boringtun::x25519::{PublicKey, StaticSecret};
use get_if_addrs::IfAddr;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

#[derive(Default)]
struct RunnerState {
    child: Mutex<Option<Child>>,
}

#[derive(Serialize)]
struct LocalAddr {
    name: String,
    addr: String,
}

#[derive(Deserialize)]
struct LinkInput {
    name: String,
    bind: String,
    weight: u32,
}

#[derive(Deserialize)]
struct ConfigParams {
    client_interface: String,
    client_address: String,
    server_address: String,
    netmask: String,
    mtu: u32,
    buffer_size: usize,
    bonding_mode: String,
    keepalive: u16,
    error_backoff_secs: u64,
    health_interval_ms: u64,
    health_timeout_ms: u64,
    health_enabled: bool,
    server_host: String,
    server_bind: String,
    server_port_base: u16,
    links: Vec<LinkInput>,
}

#[derive(Serialize)]
struct GeneratedConfigs {
    client_yaml: String,
    server_yaml: String,
    client_private_key: String,
    client_public_key: String,
    server_private_key: String,
    server_public_key: String,
}

#[derive(Deserialize)]
struct SshConfig {
    host: String,
    user: String,
    port: u16,
    key_path: String,
    use_root: bool,
}

#[derive(Deserialize)]
struct ProvisionOptions {
    install_vtrunkd: bool,
    install_service: bool,
}

#[derive(Serialize)]
struct Config {
    network: NetworkConfig,
    wireguard: WireGuardConfig,
}

#[derive(Serialize)]
struct NetworkConfig {
    mtu: u32,
    buffer_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    interface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    netmask: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    destination: Option<String>,
}

#[derive(Serialize)]
struct WireGuardConfig {
    private_key: String,
    peer_public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preshared_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    persistent_keepalive: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bonding_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_backoff_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_check_interval_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_check_timeout_ms: Option<u64>,
    links: Vec<WireGuardLinkConfig>,
}

#[derive(Serialize)]
struct WireGuardLinkConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight: Option<u32>,
}

#[tauri::command]
fn list_local_addrs() -> Result<Vec<LocalAddr>, String> {
    let mut seen = HashSet::new();
    let mut addrs = Vec::new();
    let interfaces = get_if_addrs::get_if_addrs().map_err(|e| e.to_string())?;
    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }
        match iface.addr {
            IfAddr::V4(addr) => {
                let ip = addr.ip.to_string();
                if seen.insert(ip.clone()) {
                    addrs.push(LocalAddr {
                        name: iface.name,
                        addr: ip,
                    });
                }
            }
            IfAddr::V6(addr) => {
                if addr.ip.is_unicast_link_local() {
                    continue;
                }
                let ip = addr.ip.to_string();
                if seen.insert(ip.clone()) {
                    addrs.push(LocalAddr {
                        name: iface.name,
                        addr: ip,
                    });
                }
            }
        }
    }
    Ok(addrs)
}

#[tauri::command]
fn generate_configs(params: ConfigParams) -> Result<GeneratedConfigs, String> {
    validate_params(&params)?;
    let (client_private_key, client_public_key) = generate_keypair();
    let (server_private_key, server_public_key) = generate_keypair();

    let (health_interval, health_timeout) = if params.health_enabled {
        (Some(params.health_interval_ms), Some(params.health_timeout_ms))
    } else {
        (None, None)
    };
    let keepalive = if params.keepalive == 0 {
        None
    } else {
        Some(params.keepalive)
    };
    let bonding_mode = params.bonding_mode.clone();

    let client_links = build_client_links(&params);
    let server_links = build_server_links(&params);

    let client_config = Config {
        network: NetworkConfig {
            mtu: params.mtu,
            buffer_size: params.buffer_size,
            interface: Some(params.client_interface),
            address: Some(params.client_address),
            netmask: Some(params.netmask.clone()),
            destination: None,
        },
        wireguard: WireGuardConfig {
            private_key: client_private_key.clone(),
            peer_public_key: server_public_key.clone(),
            preshared_key: None,
            persistent_keepalive: keepalive,
            bonding_mode: Some(bonding_mode.clone()),
            error_backoff_secs: Some(params.error_backoff_secs),
            health_check_interval_ms: health_interval,
            health_check_timeout_ms: health_timeout,
            links: client_links,
        },
    };

    let server_config = Config {
        network: NetworkConfig {
            mtu: params.mtu,
            buffer_size: params.buffer_size,
            interface: None,
            address: Some(params.server_address),
            netmask: Some(params.netmask),
            destination: None,
        },
        wireguard: WireGuardConfig {
            private_key: server_private_key.clone(),
            peer_public_key: client_public_key.clone(),
            preshared_key: None,
            persistent_keepalive: keepalive,
            bonding_mode: Some(bonding_mode),
            error_backoff_secs: Some(params.error_backoff_secs),
            health_check_interval_ms: health_interval,
            health_check_timeout_ms: health_timeout,
            links: server_links,
        },
    };

    let client_yaml = serde_yaml::to_string(&client_config).map_err(|e| e.to_string())?;
    let server_yaml = serde_yaml::to_string(&server_config).map_err(|e| e.to_string())?;

    Ok(GeneratedConfigs {
        client_yaml,
        server_yaml,
        client_private_key,
        client_public_key,
        server_private_key,
        server_public_key,
    })
}

#[tauri::command]
fn write_config(app: AppHandle, kind: String, yaml: String) -> Result<String, String> {
    let config_dir = app_config_dir(&app)?;
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let filename = match kind.as_str() {
        "client" => "client.yaml",
        "server" => "server.yaml",
        _ => return Err("Unsupported config kind".to_string()),
    };
    let path = config_dir.join(filename);
    fs::write(&path, yaml).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn start_vtrunkd(
    app: AppHandle,
    state: State<RunnerState>,
    binary_path: String,
    config_path: String,
) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|_| "State lock failed".to_string())?;
    if guard.is_some() {
        return Err("vtrunkd is already running".to_string());
    }

    let mut command = Command::new(if binary_path.is_empty() {
        "vtrunkd"
    } else {
        binary_path.as_str()
    });
    let mut child = command
        .arg("--config")
        .arg(&config_path)
        .arg("--foreground")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start vtrunkd: {}", e))?;

    if let Some(stdout) = child.stdout.take() {
        stream_logs(app.clone(), stdout, "vtrunkd-log");
    }
    if let Some(stderr) = child.stderr.take() {
        stream_logs(app.clone(), stderr, "vtrunkd-log");
    }

    *guard = Some(child);
    Ok(())
}

#[tauri::command]
fn stop_vtrunkd(state: State<RunnerState>) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|_| "State lock failed".to_string())?;
    if let Some(mut child) = guard.take() {
        child.kill().map_err(|e| e.to_string())?;
        let _ = child.wait();
        Ok(())
    } else {
        Err("vtrunkd is not running".to_string())
    }
}

#[tauri::command]
fn provision_vps(
    ssh: SshConfig,
    options: ProvisionOptions,
    server_yaml: String,
) -> Result<String, String> {
    let user = if ssh.use_root {
        "root".to_string()
    } else {
        ssh.user.trim().to_string()
    };
    if ssh.host.trim().is_empty() {
        return Err("SSH host is required".to_string());
    }
    if user.trim().is_empty() {
        return Err("SSH user is required".to_string());
    }
    if server_yaml.trim().is_empty() {
        return Err("Server config is empty".to_string());
    }

    let config_b64 = general_purpose::STANDARD.encode(server_yaml.as_bytes());
    let script = build_provision_script(&config_b64, &options);

    let target = format!("{}@{}", user, ssh.host);
    let mut cmd = Command::new("ssh");
    cmd.arg("-p")
        .arg(ssh.port.to_string())
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("ConnectTimeout=10");

    if !ssh.key_path.trim().is_empty() {
        cmd.arg("-i").arg(ssh.key_path.trim());
    }

    cmd.arg(target).arg("bash -s");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("SSH spawn failed: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| format!("SSH stdin failed: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("SSH failed: {}", e))?;

    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    if output.status.success() {
        Ok(combined.trim().to_string())
    } else {
        Err(combined.trim().to_string())
    }
}

fn app_config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path_resolver()
        .app_config_dir()
        .ok_or_else(|| "Unable to resolve app config directory".to_string())
}

fn stream_logs<R: std::io::Read + Send + 'static>(app: AppHandle, reader: R, event: &str) {
    let event_name = event.to_string();
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().flatten() {
            let _ = app.emit_all(&event_name, line);
        }
    });
}

fn validate_params(params: &ConfigParams) -> Result<(), String> {
    if params.links.is_empty() {
        return Err("At least one link is required".to_string());
    }
    if params.server_host.trim().is_empty() {
        return Err("Server host is required".to_string());
    }
    if params.server_bind.trim().is_empty() {
        return Err("Server bind address is required".to_string());
    }
    if params.server_port_base == 0 {
        return Err("Server base port must be between 1 and 65535".to_string());
    }
    if params.mtu == 0 || params.mtu > u16::MAX as u32 {
        return Err("MTU must be between 1 and 65535".to_string());
    }
    if params.buffer_size < params.mtu as usize {
        return Err("Buffer size must be at least MTU".to_string());
    }
    let total_ports = params.server_port_base as u32 + params.links.len() as u32 - 1;
    if total_ports > u16::MAX as u32 {
        return Err("Port range exceeds 65535".to_string());
    }
    if params.health_enabled && params.health_timeout_ms <= params.health_interval_ms {
        return Err("Health timeout must be greater than interval".to_string());
    }
    let allowed = ["aggregate", "redundant", "failover"];
    if !allowed.contains(&params.bonding_mode.as_str()) {
        return Err("Bonding mode must be aggregate, redundant, or failover".to_string());
    }
    for link in &params.links {
        if link.bind.trim().is_empty() {
            return Err("All links require a bind address".to_string());
        }
        if link.weight == 0 {
            return Err("Link weight must be greater than 0".to_string());
        }
    }
    Ok(())
}

fn generate_keypair() -> (String, String) {
    let mut private = [0u8; 32];
    OsRng.fill_bytes(&mut private);
    let secret = StaticSecret::from(private);
    let public = PublicKey::from(&secret);

    let private_b64 = general_purpose::STANDARD.encode(private);
    let public_b64 = general_purpose::STANDARD.encode(public.as_bytes());

    (private_b64, public_b64)
}

fn build_client_links(params: &ConfigParams) -> Vec<WireGuardLinkConfig> {
    params
        .links
        .iter()
        .enumerate()
        .map(|(index, link)| WireGuardLinkConfig {
            name: Some(link.name.clone()),
            bind: Some(link.bind.clone()),
            endpoint: Some(format_socket(&params.server_host, params.server_port_base + index as u16)),
            weight: Some(link.weight),
        })
        .collect()
}

fn build_server_links(params: &ConfigParams) -> Vec<WireGuardLinkConfig> {
    params
        .links
        .iter()
        .enumerate()
        .map(|(index, link)| WireGuardLinkConfig {
            name: Some(format!("server-{}-{}", index, link.name)),
            bind: Some(format_socket(&params.server_bind, params.server_port_base + index as u16)),
            endpoint: None,
            weight: Some(link.weight),
        })
        .collect()
}

fn format_socket(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{}]:{}", host, port)
    } else {
        format!("{}:{}", host, port)
    }
}

fn build_provision_script(config_b64: &str, options: &ProvisionOptions) -> String {
    let install_flag = if options.install_vtrunkd { "1" } else { "0" };
    let service_flag = if options.install_service { "1" } else { "0" };

    format!(
        "set -euo pipefail\n\
CONFIG_B64='{config_b64}'\n\
INSTALL_VTRUNKD='{install_flag}'\n\
INSTALL_SERVICE='{service_flag}'\n\
SUDO=\"\"\n\
if [ \"$(id -u)\" != \"0\" ]; then\n\
  SUDO=\"sudo\"\n\
fi\n\
\n\
write_config() {{\n\
  printf '%s' \"$CONFIG_B64\" | base64 -d | $SUDO tee /etc/vtrunkd.yaml >/dev/null\n\
}}\n\
\n\
install_deps() {{\n\
  if command -v apt-get >/dev/null 2>&1; then\n\
    $SUDO apt-get update -y\n\
    $SUDO apt-get install -y curl git build-essential pkg-config libssl-dev\n\
  elif command -v dnf >/dev/null 2>&1; then\n\
    $SUDO dnf install -y curl git gcc gcc-c++ make pkgconfig openssl-devel\n\
  elif command -v yum >/dev/null 2>&1; then\n\
    $SUDO yum install -y curl git gcc gcc-c++ make pkgconfig openssl-devel\n\
  else\n\
    echo 'Unsupported package manager' >&2\n\
    exit 1\n\
  fi\n\
}}\n\
\n\
install_rust() {{\n\
  if ! command -v cargo >/dev/null 2>&1; then\n\
    curl https://sh.rustup.rs -sSf | sh -s -- -y\n\
  fi\n\
  if [ -f \"$HOME/.cargo/env\" ]; then\n\
    . \"$HOME/.cargo/env\"\n\
  fi\n\
  export PATH=\"$HOME/.cargo/bin:$PATH\"\n\
}}\n\
\n\
install_vtrunkd() {{\n\
  if command -v vtrunkd >/dev/null 2>&1; then\n\
    return\n\
  fi\n\
  install_deps\n\
  install_rust\n\
  REPO_DIR=\"$HOME/.vtrunkd-build\"\n\
  if [ ! -d \"$REPO_DIR\" ]; then\n\
    git clone https://github.com/vzwjustin/vtrunkd.git \"$REPO_DIR\"\n\
  else\n\
    git -C \"$REPO_DIR\" pull --rebase\n\
  fi\n\
  cd \"$REPO_DIR\"\n\
  cargo build --release\n\
  $SUDO cp target/release/vtrunkd /usr/local/bin/vtrunkd\n\
}}\n\
\n\
install_service() {{\n\
  if ! command -v systemctl >/dev/null 2>&1; then\n\
    echo 'systemd not detected; skipping service install'\n\
    return\n\
  fi\n\
  $SUDO tee /etc/systemd/system/vtrunkd.service >/dev/null <<'UNIT'\n\
[Unit]\n\
Description=vtrunkd bonding daemon\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=simple\n\
ExecStart=/usr/local/bin/vtrunkd --config /etc/vtrunkd.yaml --foreground\n\
Restart=on-failure\n\
RestartSec=2\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n\
UNIT\n\
  $SUDO systemctl daemon-reload\n\
  $SUDO systemctl enable --now vtrunkd\n\
}}\n\
\n\
if [ \"$INSTALL_VTRUNKD\" = \"1\" ]; then\n\
  install_vtrunkd\n\
fi\n\
write_config\n\
if [ \"$INSTALL_SERVICE\" = \"1\" ]; then\n\
  install_service\n\
fi\n\
\n\
if command -v vtrunkd >/dev/null 2>&1; then\n\
  vtrunkd --version || true\n\
fi\n"
    )
}

fn main() {
    tauri::Builder::default()
        .manage(RunnerState::default())
        .invoke_handler(tauri::generate_handler![
            list_local_addrs,
            generate_configs,
            write_config,
            start_vtrunkd,
            stop_vtrunkd,
            provision_vps
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
