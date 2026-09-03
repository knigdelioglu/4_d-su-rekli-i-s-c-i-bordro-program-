import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { setTimeout as wait } from 'node:timers/promises';
import assert from 'node:assert/strict';
import { chromium } from '@playwright/test';

const root = process.cwd();
const port = Number(process.env.NETLIFY_SMOKE_PORT || 4199);
const baseUrl = `http://127.0.0.1:${port}`;
const databaseName = '4d-bordro-programi';
const objectStoreName = 'snapshots';
const localNetlify = join(
  root,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'netlify.cmd' : 'netlify'
);
assert.ok(
  existsSync(localNetlify),
  'Netlify CLI bulunamadı; önce bun install --frozen-lockfile çalıştırılmalı.'
);
const command = localNetlify;
const args = ['dev', '--dir', 'dist', '--port', String(port), '--no-open'];
const netlifyStateDir = join(root, '.netlify');
const denoLockPath = join(root, 'deno.lock');
const hadNetlifyState = existsSync(netlifyStateDir);
const hadDenoLock = existsSync(denoLockPath);

function assertHeader(headers, name, expected, actualUrl) {
  const actual = headers.get(name);
  assert.equal(actual, expected, `${actualUrl} ${name} headerı beklenen değerde değil: ${actual}`);
}

async function waitForServer(url, childOutput) {
  const deadline = Date.now() + 120_000;
  let lastError = 'unknown';
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = `HTTP ${response.status}`;
    } catch (error) {
      lastError = String(error);
    }
    if (childOutput.exited) break;
    await wait(250);
  }
  throw new Error(`Netlify local server başlatılamadı (${lastError}).\n${childOutput.text()}`);
}

function readStoredPayroll(page) {
  return page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const openRequest = indexedDB.open('4d-bordro-programi', 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const request = database
            .transaction('snapshots', 'readonly')
            .objectStore('snapshots')
            .get('current');
          request.onerror = () => reject(request.error ?? new Error('Snapshot okunamadı.'));
          request.onsuccess = () => {
            const value = request.result;
            database.close();
            resolve(typeof value === 'string' ? JSON.parse(value) : null);
          };
        };
      })
  );
}

async function installCalculableFixture(page) {
  await page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName }) =>
      new Promise((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const transaction = database.transaction(storeName, 'readwrite');
          const objectStore = transaction.objectStore(storeName);
          const readRequest = objectStore.get('current');
          readRequest.onerror = () => reject(readRequest.error ?? new Error('Fixture okunamadı.'));
          readRequest.onsuccess = () => {
            try {
              const snapshot = JSON.parse(readRequest.result);
              snapshot.donemler = snapshot.donemler.map((period) => ({
                ...period,
                taxYear: period.yil,
                taxMonth: period.ay,
              }));
              const activeYear = snapshot.donemler[0]?.taxYear;
              const annual = snapshot.annualPayrollParameters.find(
                (parameters) => parameters.year === activeYear
              );
              if (!annual) throw new Error('Örnek fixture yıllık vergi parametresi içermiyor.');
              annual.sigortaGvYillikBrutAsgariUcretTavani = '396360';
              const writeRequest = objectStore.put(JSON.stringify(snapshot), 'current');
              writeRequest.onerror = () =>
                reject(writeRequest.error ?? new Error('Fixture yazılamadı.'));
            } catch (error) {
              reject(error);
            }
          };
          transaction.oncomplete = () => {
            database.close();
            resolve();
          };
          transaction.onerror = () =>
            reject(transaction.error ?? new Error('Fixture transaction başarısız.'));
        };
      }),
    { databaseName, objectStoreName }
  );
  await page.reload();
}

function isAllowedBrowserRequest(request, origin) {
  const url = new URL(request.url());
  const staticPath =
    url.pathname === '/' ||
    url.pathname === '/index.html' ||
    url.pathname.startsWith('/assets/') ||
    /^\/favicon(?:\.ico)?$/.test(url.pathname);
  return url.origin === origin && ['GET', 'HEAD'].includes(request.method()) && staticPath;
}

const childOutput = {
  chunks: [],
  exited: false,
  text() {
    return this.chunks.join('');
  },
};
const configHome = mkdtempSync(join(tmpdir(), 'payroll-netlify-config-'));
const server = spawn(command, args, {
  cwd: root,
  env: {
    ...process.env,
    XDG_CONFIG_HOME: configHome,
    NETLIFY_CLI_TELEMETRY_DISABLED: '1',
  },
  stdio: ['ignore', 'pipe', 'pipe'],
});
server.stdout.on('data', (chunk) => childOutput.chunks.push(String(chunk)));
server.stderr.on('data', (chunk) => childOutput.chunks.push(String(chunk)));
server.on('exit', () => {
  childOutput.exited = true;
});

let browser;
try {
  const distDir = resolve(root, 'dist');
  assert.ok(existsSync(join(distDir, 'index.html')), 'Netlify smoke öncesi dist/index.html bulunamadı.');
  await waitForServer(`${baseUrl}/`, childOutput);

  const wasmFile = readdirSync(join(distDir, 'assets')).find((file) => file.endsWith('.wasm'));
  assert.ok(wasmFile, 'dist/assets içinde WASM asset bulunamadı.');
  const wasmUrl = `${baseUrl}/assets/${wasmFile}`;
  const wasmResponse = await fetch(wasmUrl);
  assert.equal(wasmResponse.status, 200, `${wasmUrl} HTTP 200 dönmedi.`);
  assertHeader(wasmResponse.headers, 'content-type', 'application/wasm', wasmUrl);
  assertHeader(wasmResponse.headers, 'x-content-type-options', 'nosniff', wasmUrl);
  assertHeader(wasmResponse.headers, 'referrer-policy', 'strict-origin-when-cross-origin', wasmUrl);
  assertHeader(
    wasmResponse.headers,
    'permissions-policy',
    'camera=(), microphone=(), geolocation=(), payment=()',
    wasmUrl
  );
  assert.match(
    wasmResponse.headers.get('content-security-policy') || '',
    /script-src 'self' 'wasm-unsafe-eval'/,
    `${wasmUrl} CSP headerı WASM policy içermiyor.`
  );

  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  const origin = new URL(baseUrl).origin;
  const requestIssues = [];
  const runtimeIssues = [];
  page.on('request', (request) => {
    if (!isAllowedBrowserRequest(request, origin)) {
      requestIssues.push(`${request.method()} ${request.url()} [${request.resourceType()}]`);
    }
  });
  page.on('websocket', (socket) => requestIssues.push(`WebSocket ${socket.url()}`));
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeIssues.push(`console: ${message.text()}`);
  });
  page.on('pageerror', (error) => runtimeIssues.push(`pageerror: ${error.message}`));

  await page.goto(`${baseUrl}/`);
  await page.getByText('4/D Personel Kayıtları (0)').waitFor();
  const wasmBrowserResponse = page.waitForResponse(
    (response) => /\.wasm(?:\?|$)/.test(response.url()),
    { timeout: 30_000 }
  );
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByTitle('Örnek Veriyi Yeniden Yükle').click();
  assert.equal((await wasmBrowserResponse).status(), 200, 'Chromium WASM isteği başarılı değil.');
  await page.getByText(/4\/D Personel Kayıtları \(5\)/).waitFor();
  await installCalculableFixture(page);
  await page.getByRole('button', { name: '3. Bordro Hesaplama' }).click();
  await page.getByTestId('calculate-payroll-p-1').click();
  await page.getByText(/Ahmet Yılmaz bordrosu başarıyla hesaplandı\./).waitFor();

  const snapshot = await readStoredPayroll(page);
  const payroll = snapshot?.bordrolar?.find(
    (item) => item.personelId === 'p-1' && typeof item.netOdeme === 'string'
  );
  assert.ok(payroll, 'Netlify runtime hesaplama sonucu IndexedDB snapshotına yazılmadı.');
  assert.equal(typeof payroll.gelirToplam, 'string');
  assert.equal(typeof payroll.kesintiToplam, 'string');
  assert.equal(typeof payroll.netOdeme, 'string');
  assert.deepEqual(requestIssues, [], `Browser outbound request guard ihlali: ${requestIssues.join('; ')}`);
  assert.deepEqual(runtimeIssues, [], `Browser runtime/CSP hatası: ${runtimeIssues.join('; ')}`);
  await context.close();
  console.log(
    `test:netlify-smoke: PASS — gerçek Netlify local runtime headerları, CSP, WASM, IndexedDB ve hesaplama doğrulandı (${relative(root, join(distDir, 'assets', wasmFile))}).`
  );
} catch (error) {
  console.error(`test:netlify-smoke: FAIL — ${String(error)}`);
  console.error(childOutput.text());
  process.exitCode = 1;
} finally {
  if (browser) await browser.close().catch(() => undefined);
  server.kill('SIGTERM');
  rmSync(configHome, { recursive: true, force: true });
  if (!hadNetlifyState) rmSync(netlifyStateDir, { recursive: true, force: true });
  if (!hadDenoLock) rmSync(denoLockPath, { force: true });
}
