import assert from 'node:assert/strict';
import { constants } from 'node:fs';
import { access } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { resolve } from 'node:path';

const webdriverHost = '127.0.0.1';
const webdriverPort = 4444;
const nativeDriverPort = 4445;
const webdriverUrl = `http://${webdriverHost}:${webdriverPort}`;
const defaultAppPath = resolve(
  'target/debug',
  process.platform === 'win32' ? 'bordro-programi.exe' : 'bordro-programi',
);
const appPath = resolve(process.env.TAURI_WEBVIEW_APP ?? defaultAppPath);
const timeoutMs = 30_000;

if (process.platform !== 'linux' && process.platform !== 'win32') {
  throw new Error('Native Tauri WebView E2E currently supports Linux and Windows through tauri-driver.');
}

if (process.env.CI !== 'true') {
  throw new Error('Run this smoke test only in an isolated CI runner; it starts the app against its local application data directory.');
}

await access(appPath, process.platform === 'win32' ? constants.F_OK : constants.X_OK);

let driverExitCode = null;
let driverSpawnError = null;
let driverOutput = '';
const driver = spawn(
  'tauri-driver',
  ['--port', String(webdriverPort), '--native-port', String(nativeDriverPort)],
  { stdio: ['ignore', 'pipe', 'pipe'] },
);

for (const stream of [driver.stdout, driver.stderr]) {
  stream.setEncoding('utf8');
  stream.on('data', (chunk) => {
    driverOutput = `${driverOutput}${chunk}`.slice(-12_000);
  });
}
driver.on('exit', (code) => {
  driverExitCode = code ?? 1;
});
driver.on('error', (error) => {
  driverSpawnError = error;
});

async function command(path, { method = 'GET', body, requestTimeout = timeoutMs } = {}) {
  const response = await fetch(`${webdriverUrl}${path}`, {
    method,
    headers: body === undefined ? undefined : { 'content-type': 'application/json; charset=utf-8' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(requestTimeout),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok || payload?.value?.error) {
    throw new Error(
      `WebDriver ${method} ${path} failed (${response.status}): ${JSON.stringify(payload)}`,
    );
  }
  return payload?.value;
}

async function waitForDriver() {
  const deadline = Date.now() + 20_000;
  let lastError;
  while (Date.now() < deadline) {
    if (driverSpawnError) {
      throw new Error(`Could not start tauri-driver: ${driverSpawnError.message}`);
    }
    if (driverExitCode !== null) {
      throw new Error(`tauri-driver exited early with code ${driverExitCode}.`);
    }
    try {
      const status = await command('/status', { requestTimeout: 1_000 });
      if (status?.ready !== false) return;
      lastError = new Error('tauri-driver is not ready yet.');
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for tauri-driver: ${lastError?.message ?? 'no status response'}`);
}

async function readAppState(sessionId) {
  return command(`/session/${sessionId}/execute/sync`, {
    method: 'POST',
    body: {
      script: `return {
        title: document.title,
        rootText: document.getElementById('root')?.innerText ?? '',
        loading: Boolean(document.querySelector('[data-testid="data-loading-state"]')),
        storageError: document.querySelector('[data-testid="storage-error"]')?.innerText ?? null
      };`,
      args: [],
    },
    requestTimeout: 5_000,
  });
}

let sessionId;
let testError;
try {
  await waitForDriver();
  const session = await command('/session', {
    method: 'POST',
    body: {
      capabilities: {
        alwaysMatch: {
          browserName: 'wry',
          'tauri:options': {
            application: appPath,
            args: [],
            webviewOptions: {},
          },
        },
      },
    },
  });
  sessionId = session?.sessionId;
  assert.equal(typeof sessionId, 'string', 'tauri-driver should return a W3C session');

  await command(`/session/${sessionId}/timeouts`, {
    method: 'POST',
    body: { script: timeoutMs, pageLoad: timeoutMs, implicit: 0 },
  });

  const deadline = Date.now() + timeoutMs;
  let appState;
  while (Date.now() < deadline) {
    appState = await readAppState(sessionId);
    if (appState?.storageError) {
      throw new Error(`Desktop storage failed during app startup: ${appState.storageError}`);
    }
    if (appState?.title === '4/D Bordro' && !appState.loading && appState.rootText.trim()) break;
    await delay(300);
  }

  assert.equal(appState?.title, '4/D Bordro', 'native WebView should load the payroll app title');
  assert.equal(appState?.loading, false, 'native app should finish loading its persisted data');
  assert.ok(appState?.rootText.trim(), 'React app should render into the native WebView');

  const ipcResult = await command(`/session/${sessionId}/execute/async`, {
    method: 'POST',
    body: {
      script: `const done = arguments[arguments.length - 1];
        const invoke = window.__TAURI_INTERNALS__?.invoke;
        if (typeof invoke !== 'function') {
          done({ ok: false, error: 'Tauri invoke bridge is missing from the native WebView.' });
          return;
        }
        invoke('get_periods')
          .then((periods) => done({ ok: true, periods }))
          .catch((error) => done({ ok: false, error: String(error) }));`,
      args: [],
    },
  });

  assert.equal(ipcResult?.ok, true, `native get_periods IPC should succeed: ${ipcResult?.error ?? ''}`);
  assert.ok(Array.isArray(ipcResult?.periods), 'native get_periods IPC should return an array');
  console.log('Native Tauri WebView E2E passed: app rendered, loaded storage, and get_periods IPC returned an array.');
} catch (error) {
  testError = error;
} finally {
  if (sessionId) {
    await command(`/session/${sessionId}`, { method: 'DELETE', requestTimeout: 5_000 }).catch(() => {});
  }
  driver.kill('SIGTERM');
  await Promise.race([
    new Promise((resolveExit) => driver.once('exit', resolveExit)),
    delay(2_000),
  ]);
  if (driver.exitCode === null) driver.kill('SIGKILL');
}

if (testError) {
  const detail = driverOutput.trim();
  throw new Error(
    `Native Tauri WebView E2E failed: ${testError instanceof Error ? testError.message : String(testError)}${detail ? `\n\ntauri-driver output:\n${detail}` : ''}`,
    { cause: testError },
  );
}
