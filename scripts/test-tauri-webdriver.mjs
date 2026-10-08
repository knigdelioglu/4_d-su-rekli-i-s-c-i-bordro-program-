import assert from 'node:assert/strict';
import { constants } from 'node:fs';
import { access, mkdtemp, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const webdriverHost = '127.0.0.1';
const webdriverPort = Number(process.env.TAURI_WEBDRIVER_PORT ?? 4445);
const webdriverUrl = `http://${webdriverHost}:${webdriverPort}`;
const defaultAppPath = resolve(
  'target/debug',
  process.platform === 'win32' ? 'bordro-programi.exe' : 'bordro-programi',
);
const appPath = resolve(process.env.TAURI_WEBVIEW_APP ?? defaultAppPath);
const timeoutMs = 30_000;

if (!['linux', 'win32', 'darwin'].includes(process.platform)) {
  throw new Error('Native Tauri WebView E2E currently supports Linux, Windows, and macOS.');
}

if (process.env.CI !== 'true') {
  throw new Error('Set CI=true to run this smoke test; it starts the app with a temporary, isolated application data directory.');
}

await access(appPath, process.platform === 'win32' ? constants.F_OK : constants.X_OK);
const isolatedProfile = await mkdtemp(join(tmpdir(), '4d-bordro-native-e2e-'));
const isolatedShare = join(isolatedProfile, 'share');

let appExitCode = null;
let appSpawnError = null;
let appOutput = '';
const app = spawn(appPath, [], {
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    APPDATA: isolatedProfile,
    HOME: isolatedProfile,
    LOCALAPPDATA: isolatedProfile,
    TAURI_WEBDRIVER_PORT: String(webdriverPort),
    XDG_DATA_HOME: isolatedShare,
  },
});

for (const stream of [app.stdout, app.stderr]) {
  stream.setEncoding('utf8');
  stream.on('data', (chunk) => {
    appOutput = `${appOutput}${chunk}`.slice(-12_000);
  });
}
app.on('exit', (code) => {
  appExitCode = code ?? 1;
});
app.on('error', (error) => {
  appSpawnError = error;
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

async function waitForWebDriver() {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (appSpawnError) {
      throw new Error(`Could not start the Tauri app: ${appSpawnError.message}`);
    }
    if (appExitCode !== null) {
      throw new Error(`The Tauri app exited early with code ${appExitCode}.`);
    }
    try {
      const status = await command('/status', { requestTimeout: 1_000 });
      if (status?.ready === true) return;
      lastError = new Error('The embedded WebDriver is waiting for the Tauri WebView.');
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for the embedded WebDriver: ${lastError?.message ?? 'no status response'}`);
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
  await waitForWebDriver();
  const session = await command('/session', {
    method: 'POST',
    body: {
      capabilities: {
        alwaysMatch: {
          'wdio:tauriServiceOptions': { windowLabel: 'main' },
        },
      },
    },
  });
  sessionId = session?.sessionId;
  assert.equal(typeof sessionId, 'string', 'embedded WebDriver should return a W3C session');

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
  app.kill('SIGTERM');
  await Promise.race([
    new Promise((resolveExit) => app.once('exit', resolveExit)),
    delay(2_000),
  ]);
  if (app.exitCode === null) app.kill('SIGKILL');
  await rm(isolatedProfile, { recursive: true, force: true });
}

if (testError) {
  const detail = appOutput.trim();
  throw new Error(
    `Native Tauri WebView E2E failed: ${testError instanceof Error ? testError.message : String(testError)}${detail ? `\n\nTauri app output:\n${detail}` : ''}`,
    { cause: testError },
  );
}
