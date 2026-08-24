const $ = (id) => document.getElementById(id);
const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (c) => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' }[c]));

function render(target, items, empty, item) {
  $(target).innerHTML = items.length ? items.map(item).join('') : `<div class="empty">${empty}</div>`;
}

async function scan() {
  const button = $('scan');
  button.disabled = true;
  $('status').textContent = 'Scanning candidate networks and proxy protocols…';
  try {
    const raw = await window.__TAURI__.core.invoke('scan_networks');
    const data = JSON.parse(raw);
    render('interfaces', data.interfaces, 'No private USB/RNDIS or Wi-Fi interface was detected.', (item) => `
      <div class="row"><strong>${escapeHtml(item.name)} <span class="meta">score ${item.score}</span></strong>
      <code>${item.addresses.map(escapeHtml).join(', ')}</code>${item.gateway ? `<br><span class="meta">gateway </span><code>${escapeHtml(item.gateway)}</code>` : ''}</div>`);
    render('proxies', data.proxies, 'No recognised proxy endpoint was found.', (item) => `
      <div class="row"><strong class="good">${escapeHtml(item.kind)}</strong><code>${escapeHtml(item.host)}:${item.port}</code>
      ${item.kind === 'socks5' ? `<div class="meta">UDP ASSOCIATE: ${item.udp_associate ? 'accepted' : 'unavailable'}</div>` : ''}
      <div class="meta">${escapeHtml(item.source)}</div></div>`);
    $('status').textContent = `Scan completed at ${new Date().toLocaleTimeString()}.`;
  } catch (error) {
    $('status').textContent = `Scan failed: ${error}`;
  } finally {
    button.disabled = false;
  }
}

$('scan').addEventListener('click', scan);
