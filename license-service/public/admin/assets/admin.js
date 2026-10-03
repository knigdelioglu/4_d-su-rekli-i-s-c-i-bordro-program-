const loginPanel = document.querySelector('#login-panel');
const adminPanel = document.querySelector('#admin-panel');
const notice = document.querySelector('#notice');
const refreshButton = document.querySelector('#refresh-button');
let csrfToken = '';
let licenses = [];

function showNotice(message, kind = '') {
  notice.textContent = message;
  notice.className = `notice${kind ? ` ${kind}` : ''}`;
}

function clearNotice() {
  notice.textContent = '';
  notice.className = 'notice hidden';
}

async function api(path, { method = 'GET', body, headers = {} } = {}) {
  const response = await fetch(path, {
    method,
    credentials: 'same-origin',
    headers: {
      ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
      ...(method === 'GET' ? {} : { 'X-CSRF-Token': csrfToken }),
      ...headers,
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload.message || 'İstek tamamlanamadı.');
  return payload;
}

function formatDate(value) {
  return new Intl.DateTimeFormat('tr-TR', { dateStyle: 'medium' }).format(new Date(value));
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function actionButton(text, className, action) {
  const button = element('button', `button ${className}`, text);
  button.type = 'button';
  button.addEventListener('click', action);
  return button;
}

async function loadDashboard() {
  clearNotice();
  refreshButton.disabled = true;
  try {
    const [licenseData, requestData] = await Promise.all([
      api('/api/license/admin/licenses'),
      api('/api/license/admin/requests'),
    ]);
    licenses = licenseData.licenses;
    renderLicenses(licenses);
    renderRequests(requestData.requests);
  } catch (error) {
    showNotice(error.message, 'error');
    if (error.message.includes('oturumu')) showLogin();
  } finally {
    refreshButton.disabled = false;
  }
}

function renderRequests(requests) {
  const root = document.querySelector('#requests-list');
  root.replaceChildren();
  if (requests.length === 0) {
    root.className = 'empty-state';
    root.textContent = 'Bekleyen talep yok.';
    return;
  }
  root.className = '';
  for (const request of requests) {
    const license = licenses.find((item) => item.id === request.licenseId);
    const row = element('article', 'request');
    const top = element('div', 'row-top');
    const details = document.createElement('div');
    details.append(element('strong', '', license?.institutionName ?? 'Lisans kaydı bulunamadı'));
    details.append(element('p', 'meta', `${license?.licenseName ?? request.licenseId} · Talep ${formatDate(request.createdAt)}`));
    details.append(element('code', 'hash', request.deviceHash));
    const actions = element('div', 'actions');
    actions.append(actionButton('Onayla', 'primary', () => decideRequest(request.id, 'approve')));
    actions.append(actionButton('Reddet', 'danger', () => decideRequest(request.id, 'deny')));
    top.append(details, actions);
    row.append(top);
    root.append(row);
  }
}

function renderLicenses(items) {
  const root = document.querySelector('#licenses-list');
  root.replaceChildren();
  if (items.length === 0) {
    root.className = 'empty-state';
    root.textContent = 'Henüz lisans yok.';
    return;
  }
  root.className = '';
  for (const license of items) {
    const row = element('article', 'license');
    const top = element('div', 'row-top');
    const details = document.createElement('div');
    details.append(element('strong', '', `${license.institutionName} · ${license.licenseName}`));
    details.append(element('p', 'meta', `Son geçerlilik: ${formatDate(license.expiresAt)} · Cihaz hakkı: ${license.devices.length}/${license.maxDevices}`));
    const badge = element('span', `badge${license.status === 'revoked' ? ' revoked' : ''}`, license.status === 'active' ? 'Aktif' : 'İptal');
    top.append(details, badge);
    row.append(top);
    if (license.devices.length) {
      const list = element('ul', 'device-list');
      for (const device of license.devices) {
        const item = element('li', 'device-row');
        item.append(element('code', 'hash', device.deviceHash));
        item.append(actionButton('Cihaz hakkını boşa çıkar', 'secondary', () => releaseDevice(license.id, device.deviceHash)));
        list.append(item);
      }
      row.append(list);
    }
    if (license.status === 'active') {
      row.append(actionButton('Lisansı iptal et', 'danger', () => revokeLicense(license.id, license.institutionName)));
    }
    root.append(row);
  }
}

async function decideRequest(requestId, decision) {
  const verb = decision === 'approve' ? 'onaylamak' : 'reddetmek';
  if (!window.confirm(`Bu cihaz talebini ${verb} istiyor musunuz?`)) return;
  try {
    await api(`/api/license/admin/requests/${encodeURIComponent(requestId)}/${decision}`, { method: 'POST', body: {} });
    showNotice(decision === 'approve' ? 'Cihaz talebi onaylandı.' : 'Cihaz talebi reddedildi.', 'success');
    await loadDashboard();
  } catch (error) { showNotice(error.message, 'error'); }
}

async function releaseDevice(licenseId, deviceHash) {
  if (!window.confirm('Bu cihazın lisans hakkı kaldırılacak. Devam edilsin mi?')) return;
  try {
    await api(`/api/license/admin/licenses/${licenseId}/devices/${deviceHash}/release`, { method: 'POST', body: {} });
    showNotice('Cihaz hakkı boşa çıkarıldı.', 'success');
    await loadDashboard();
  } catch (error) { showNotice(error.message, 'error'); }
}

async function revokeLicense(licenseId, institutionName) {
  if (!window.confirm(`${institutionName} lisansı iptal edilecek. Bu lisansla etkinleşen cihazlar çevrimiçi kontrolde reddedilir. Devam edilsin mi?`)) return;
  try {
    await api(`/api/license/admin/licenses/${licenseId}/revoke`, { method: 'POST', body: {} });
    showNotice('Lisans iptal edildi.', 'success');
    await loadDashboard();
  } catch (error) { showNotice(error.message, 'error'); }
}

function showLogin() {
  csrfToken = '';
  loginPanel.classList.remove('hidden');
  adminPanel.classList.add('hidden');
  refreshButton.classList.add('hidden');
}

function showAdmin() {
  loginPanel.classList.add('hidden');
  adminPanel.classList.remove('hidden');
  refreshButton.classList.remove('hidden');
}

document.querySelector('#login-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  clearNotice();
  const form = new FormData(event.currentTarget);
  const submit = event.currentTarget.querySelector('button[type="submit"]');
  submit.disabled = true;
  try {
    const result = await api('/api/license/login', {
      method: 'POST',
      body: { username: form.get('username'), password: form.get('password') },
    });
    csrfToken = result.csrf;
    event.currentTarget.reset();
    showAdmin();
    await loadDashboard();
  } catch (error) { showNotice(error.message, 'error'); }
  finally { submit.disabled = false; }
});

document.querySelector('#create-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  clearNotice();
  const form = new FormData(event.currentTarget);
  const submit = event.currentTarget.querySelector('button[type="submit"]');
  submit.disabled = true;
  try {
    const result = await api('/api/license/admin/licenses', {
      method: 'POST',
      body: {
        institutionName: form.get('institutionName'),
        licenseName: form.get('licenseName'),
        expiresAt: new Date(`${form.get('expiresAt')}T23:59:59.999Z`).toISOString(),
        maxDevices: Number(form.get('maxDevices')),
      },
    });
    document.querySelector('#new-key-value').textContent = result.licenseKey;
    document.querySelector('#new-key').classList.remove('hidden');
    showNotice('Lisans oluşturuldu. Anahtarı bu ekrandan ayrılmadan güvenli biçimde saklayın.', 'success');
    event.currentTarget.reset();
    document.querySelector('#license-name').value = '4/D Bordro';
    document.querySelector('#max-devices').value = '1';
    await loadDashboard();
  } catch (error) { showNotice(error.message, 'error'); }
  finally { submit.disabled = false; }
});

document.querySelector('#copy-key').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(document.querySelector('#new-key-value').textContent);
    showNotice('Lisans anahtarı panoya kopyalandı.', 'success');
  } catch { showNotice('Pano erişimi başarısız. Anahtarı seçip kopyalayın.', 'error'); }
});

document.querySelector('#refresh-button').addEventListener('click', loadDashboard);
document.querySelector('#logout-button').addEventListener('click', async () => {
  try { await api('/api/license/logout', { method: 'POST', body: {} }); } catch { /* Local UI still clears the session. */ }
  showLogin();
  showNotice('Oturum kapatıldı.', 'success');
});

document.querySelector('#export-backup').addEventListener('click', async () => {
  try {
    const backup = await api('/api/license/admin/backup');
    const blob = new Blob([JSON.stringify(backup, null, 2)], { type: 'application/json' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `4d-lisans-yedegi-${new Date().toISOString().slice(0, 10)}.json`;
    link.click();
    URL.revokeObjectURL(link.href);
    showNotice('Lisans kayıt yedeği indirildi.', 'success');
  } catch (error) { showNotice(error.message, 'error'); }
});

document.querySelector('#restore-file').addEventListener('change', async (event) => {
  const file = event.currentTarget.files?.[0];
  if (!file) return;
  try {
    const backup = JSON.parse(await file.text());
    if (!window.confirm('Bu yedek kayıtları mevcut kayıtlarla çakışmayacak şekilde yükleyecek. Devam edilsin mi?')) return;
    await api('/api/license/admin/backup/restore', { method: 'POST', body: { backup } });
    showNotice('Yedek kayıtları geri yüklendi.', 'success');
    await loadDashboard();
  } catch (error) { showNotice(error.message || 'Yedek dosyası okunamadı.', 'error'); }
  finally { event.currentTarget.value = ''; }
});

async function restoreSession() {
  try {
    const result = await api('/api/license/admin/session');
    csrfToken = result.csrf;
    showAdmin();
    await loadDashboard();
  } catch { showLogin(); }
}

restoreSession();
