import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

const linkTemplate = document.getElementById('link-template');
const linksContainer = document.getElementById('links');
const logEl = document.getElementById('log');
const clientConfigEl = document.getElementById('client-config');
const serverConfigEl = document.getElementById('server-config');
const clientPublicEl = document.getElementById('client-public');
const serverPublicEl = document.getElementById('server-public');
const runStatusEl = document.getElementById('run-status');
const tunnelModeEl = document.getElementById('tunnel-mode');
const linkCountEl = document.getElementById('link-count');
const serverHostDisplayEl = document.getElementById('server-host-display');

let links = [
  { name: 'wifi', bind: '', weight: 1 },
  { name: 'lte/5g', bind: '', weight: 1 }
];

function renderLinks() {
  linksContainer.innerHTML = '';
  links.forEach((link, index) => {
    const clone = linkTemplate.content.cloneNode(true);
    const card = clone.querySelector('.link-card');
    const nameInput = clone.querySelector('.link-name');
    const bindInput = clone.querySelector('.link-bind');
    const weightInput = clone.querySelector('.link-weight');
    const removeBtn = clone.querySelector('.link-remove');

    nameInput.value = link.name;
    bindInput.value = link.bind;
    weightInput.value = link.weight;

    nameInput.addEventListener('input', (event) => {
      links[index].name = event.target.value;
    });
    bindInput.addEventListener('input', (event) => {
      links[index].bind = event.target.value;
    });
    weightInput.addEventListener('input', (event) => {
      links[index].weight = Number(event.target.value || 1);
    });
    removeBtn.addEventListener('click', () => {
      links.splice(index, 1);
      renderLinks();
      refreshMetrics();
    });

    linksContainer.appendChild(card);
  });
}

function appendLog(message) {
  const next = `${logEl.textContent.trim()}`;
  const lines = next === 'Ready.' ? [] : next.split('\n');
  lines.push(message);
  const trimmed = lines.slice(-200);
  logEl.textContent = trimmed.join('\n');
}

function refreshMetrics() {
  const mode = document.getElementById('bonding-mode').value;
  tunnelModeEl.textContent = mode;
  linkCountEl.textContent = String(links.length);
  const host = document.getElementById('server-host').value.trim();
  serverHostDisplayEl.textContent = host || '-';
}

function readNumber(id) {
  return Number(document.getElementById(id).value || 0);
}

function readText(id) {
  return document.getElementById(id).value.trim();
}

function buildParams() {
  const healthEnabled = document.getElementById('health-enabled').checked;
  return {
    client_interface: readText('client-interface'),
    client_address: readText('client-address'),
    server_address: readText('server-address'),
    netmask: readText('netmask'),
    mtu: readNumber('mtu'),
    buffer_size: readNumber('buffer-size'),
    bonding_mode: readText('bonding-mode'),
    keepalive: readNumber('keepalive'),
    error_backoff_secs: readNumber('error-backoff'),
    health_interval_ms: readNumber('health-interval'),
    health_timeout_ms: readNumber('health-timeout'),
    health_enabled: healthEnabled,
    server_host: readText('server-host'),
    server_bind: readText('server-bind'),
    server_port_base: readNumber('server-port'),
    links: links.map((link) => ({
      name: link.name,
      bind: link.bind,
      weight: link.weight
    }))
  };
}

async function generateConfigs() {
  refreshMetrics();
  appendLog('Generating configs...');
  const params = buildParams();
  try {
    const result = await invoke('generate_configs', { params });
    clientConfigEl.value = result.client_yaml;
    serverConfigEl.value = result.server_yaml;
    clientPublicEl.textContent = result.client_public_key;
    serverPublicEl.textContent = result.server_public_key;
    appendLog('Configs generated.');
  } catch (err) {
    appendLog(`Error: ${err}`);
  }
}

async function provisionVps() {
  appendLog('Provisioning VPS...');
  const ssh = {
    host: readText('server-host'),
    user: readText('ssh-user'),
    port: readNumber('ssh-port'),
    key_path: readText('ssh-key'),
    use_root: document.getElementById('ssh-root').checked
  };
  const options = {
    install_vtrunkd: document.getElementById('install-vtrunkd').checked,
    install_service: document.getElementById('install-service').checked
  };
  try {
    const output = await invoke('provision_vps', {
      ssh,
      options,
      serverYaml: serverConfigEl.value
    });
    appendLog(output || 'Provisioning complete.');
  } catch (err) {
    appendLog(`Provisioning failed: ${err}`);
  }
}

async function startTunnel() {
  appendLog('Starting tunnel...');
  try {
    const clientYaml = clientConfigEl.value.trim();
    if (!clientYaml) {
      appendLog('Generate the client config first.');
      return;
    }
    const configPath = await invoke('write_config', {
      kind: 'client',
      yaml: clientYaml
    });
    const binaryPath = readText('binary-path') || 'vtrunkd';
    await invoke('start_vtrunkd', { binaryPath, configPath });
    runStatusEl.textContent = 'Status: running';
    runStatusEl.classList.add('running');
    appendLog(`Tunnel started using ${configPath}`);
  } catch (err) {
    appendLog(`Start failed: ${err}`);
  }
}

async function stopTunnel() {
  appendLog('Stopping tunnel...');
  try {
    await invoke('stop_vtrunkd');
    runStatusEl.textContent = 'Status: stopped';
    runStatusEl.classList.remove('running');
    appendLog('Tunnel stopped.');
  } catch (err) {
    appendLog(`Stop failed: ${err}`);
  }
}

async function autoDetect() {
  appendLog('Detecting local IPs...');
  try {
    const addresses = await invoke('list_local_addrs');
    if (!addresses.length) {
      appendLog('No suitable addresses detected.');
      return;
    }
    links = addresses.map((entry) => ({
      name: entry.name,
      bind: `${entry.addr}:0`,
      weight: 1
    }));
    renderLinks();
    refreshMetrics();
    appendLog(`Detected ${addresses.length} addresses.`);
  } catch (err) {
    appendLog(`Detection failed: ${err}`);
  }
}

/**
 * Wraps an async function with a loading state on the button.
 * @param {string} buttonId - The ID of the button to toggle.
 * @param {Function} asyncFn - The async function to execute.
 */
async function withLoading(buttonId, asyncFn) {
  const btn = document.getElementById(buttonId);
  if (!btn) return asyncFn();

  try {
    btn.classList.add('loading');
    btn.setAttribute('aria-busy', 'true');
    btn.disabled = true;
    await asyncFn();
  } finally {
    btn.classList.remove('loading');
    btn.removeAttribute('aria-busy');
    btn.disabled = false;
  }
}

function setupAnimations() {
  const elements = document.querySelectorAll('[data-animate]');
  elements.forEach((el, index) => {
    setTimeout(() => {
      el.classList.add('visible');
    }, 120 + index * 120);
  });
}

renderLinks();
refreshMetrics();
setupAnimations();

listen('vtrunkd-log', (event) => {
  appendLog(event.payload);
});

listen('vtrunkd-exit', (event) => {
  runStatusEl.textContent = 'Status: stopped';
  runStatusEl.classList.remove('running');
  appendLog(`vtrunkd exited: ${event.payload}`);
});

['bonding-mode', 'server-host'].forEach((id) => {
  document.getElementById(id).addEventListener('input', refreshMetrics);
});

document.getElementById('generate').addEventListener('click', () => withLoading('generate', generateConfigs));
document.getElementById('provision').addEventListener('click', () => withLoading('provision', provisionVps));
document.getElementById('start').addEventListener('click', () => withLoading('start', startTunnel));
document.getElementById('stop').addEventListener('click', () => withLoading('stop', stopTunnel));
document.getElementById('add-link').addEventListener('click', () => {
  links.push({ name: 'link', bind: '', weight: 1 });
  renderLinks();
  refreshMetrics();
});
document.getElementById('detect-links').addEventListener('click', () => withLoading('detect-links', autoDetect));
