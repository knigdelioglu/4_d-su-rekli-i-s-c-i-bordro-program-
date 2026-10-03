import { describe, expect, test } from 'bun:test';
import { generateKeyPairSync, verify } from 'node:crypto';
import { createPasswordHash } from '../netlify/lib/crypto';
import { handleLicenseRequest } from '../netlify/lib/service';
import { createStore, licenseStoreName, type BlobStore } from '../netlify/lib/storage';
import { AtomicBlobEmulator } from './blob-emulator';
import { FileAtomicBlobEmulator } from './file-blob-emulator';

const fixedNow = new Date('2026-10-03T12:00:00.000Z');
const deviceOne = 'a'.repeat(64);
const deviceTwo = 'b'.repeat(64);
const password = 'local-test-passphrase';
const credentials = {
  BORDRO_ADMIN_USERNAME: 'local-admin',
  BORDRO_ADMIN_PASSWORD_HASH: createPasswordHash(password, Buffer.from('test-salt-123456')),
  BORDRO_ADMIN_SESSION_SECRET: 'a'.repeat(48),
  BORDRO_LICENSE_KEY_ID: 'test-ed25519-key',
};

function dependencies(store: BlobStore) {
  const signing = generateKeyPairSync('ed25519');
  const deps = {
    store,
    env: { ...credentials, BORDRO_LICENSE_SIGNING_PRIVATE_KEY: signing.privateKey.export({ type: 'pkcs8', format: 'pem' }).toString() },
    now: () => new Date(fixedNow),
    clientIp: '127.0.0.1',
  };
  return { deps, publicKey: signing.publicKey };
}

function request(path: string, method = 'GET', body?: unknown, headers: Record<string, string> = {}) {
  return new Request(`https://licenses.test${path}`, {
    method,
    headers: {
      ...(body === undefined ? {} : { 'content-type': 'application/json' }),
      ...headers,
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}

async function login(deps: ReturnType<typeof dependencies>['deps']) {
  const response = await handleLicenseRequest(request('/api/license/login', 'POST', { username: 'local-admin', password }), deps);
  expect(response.status).toBe(200);
  const cookie = response.headers.get('set-cookie')!.split(';')[0];
  const { csrf } = await response.json() as { csrf: string };
  const admin = async (path: string, method = 'GET', body?: unknown) => handleLicenseRequest(
    request(path, method, body, { cookie, origin: 'https://licenses.test', ...(method === 'GET' ? {} : { 'x-csrf-token': csrf }) }),
    deps,
  );
  return { admin, csrf, cookie };
}

async function createLicense(admin: (path: string, method?: string, body?: unknown) => Promise<Response>, maxDevices = 2) {
  const response = await admin('/api/license/admin/licenses', 'POST', {
    institutionName: 'Test Kurumu',
    licenseName: 'Masaüstü lisansı',
    expiresAt: '2027-01-01T23:59:59.999Z',
    maxDevices,
  });
  expect(response.status).toBe(201);
  return response.json() as Promise<{ license: { id: string }; licenseKey: string }>;
}

async function requestActivation(deps: ReturnType<typeof dependencies>['deps'], licenseKey: string, deviceHash: string) {
  const response = await handleLicenseRequest(request('/api/license/activate', 'POST', { licenseKey, deviceHash }), deps);
  expect(response.status).toBe(202);
  return response.json() as Promise<{ requestId: string; status: string }>;
}

describe('Netlify Blobs license service', () => {
  test('production, preview, development, and test use distinct persistent site stores', () => {
    expect(licenseStoreName('production')).toBe('4d-license-production');
    expect(licenseStoreName('deploy-preview')).toBe('4d-license-preview');
    expect(licenseStoreName('branch-deploy')).toBe('4d-license-preview');
    expect(licenseStoreName('test')).toBe('4d-license-test');
    expect(licenseStoreName(undefined)).toBe('4d-license-development');
    const names: string[] = [];
    const factory = ((name: string) => { names.push(name); return new AtomicBlobEmulator(); }) as Parameters<typeof createStore>[0];
    createStore(factory, 'production');
    createStore(factory, 'deploy-preview');
    expect(names).toEqual(['4d-license-production', '4d-license-preview']);
  });

  test('admin endpoints fail closed without a server-side authenticated session', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const response = await handleLicenseRequest(request('/api/license/admin/licenses'), deps);
    expect(response.status).toBe(401);
    const loginResponse = await handleLicenseRequest(request('/api/license/login', 'POST', { username: 'nope', password: 'wrong' }), deps);
    expect(loginResponse.status).toBe(401);
    expect((await loginResponse.json()).error).toBe('invalid_credentials');
  });

  test('login uses scrypt, HttpOnly SameSite cookie and CSRF token without returning a session secret', async () => {
    const { deps } = dependencies(new AtomicBlobEmulator());
    const { admin, cookie, csrf } = await login(deps);
    const cookieHeader = (await handleLicenseRequest(request('/api/license/login', 'POST', { username: 'local-admin', password }), deps)).headers.get('set-cookie')!;
    expect(cookieHeader).toContain('HttpOnly');
    expect(cookieHeader).toContain('SameSite=Strict');
    expect(cookieHeader).not.toContain(password);
    expect(cookie).toContain('BORDRO_ADMIN_SESSION=');
    expect(csrf.length).toBeGreaterThan(20);
    const session = await admin('/api/license/admin/session');
    expect(session.status).toBe(200);
    expect((await session.json()).csrf).toBe(csrf);
    const noCsrf = await handleLicenseRequest(request('/api/license/admin/licenses', 'POST', {}, { cookie, origin: 'https://licenses.test' }), deps);
    expect(noCsrf.status).toBe(403);
  });

  test('manual activation, approval and signed device-bound grant survive local store reload', async () => {
    const store = await FileAtomicBlobEmulator.create();
    const { deps, publicKey } = dependencies(store);
    try {
      const { admin } = await login(deps);
      const created = await createLicense(admin);
      const activation = await requestActivation(deps, created.licenseKey, deviceOne);
      expect(activation.status).toBe('pending');
      const requests = await admin('/api/license/admin/requests');
      expect((await requests.json()).requests).toHaveLength(1);
      const approval = await admin(`/api/license/admin/requests/${encodeURIComponent(activation.requestId)}/approve`, 'POST', {});
      expect(approval.status).toBe(200);
      const restartedStore = await FileAtomicBlobEmulator.open(store.directory);
      const reloadedDeps = { ...deps, store: restartedStore };
      const issued = await handleLicenseRequest(request('/api/license/activation-status', 'POST', {
        requestId: activation.requestId,
        deviceHash: deviceOne,
      }), reloadedDeps);
      expect(issued.status).toBe(200);
      const grantResponse = await issued.json() as { decision: string; grant: string };
      expect(grantResponse.decision).toBe('authorized');
      const [, encodedPayload, encodedSignature] = grantResponse.grant.split('.');
      const payload = Buffer.from(encodedPayload, 'base64url');
      expect(verify(null, payload, publicKey, Buffer.from(encodedSignature, 'base64url'))).toBe(true);
      expect(JSON.parse(payload.toString()).deviceHash).toBe(deviceOne);
      const mismatch = await handleLicenseRequest(request('/api/license/activation-status', 'POST', {
        requestId: activation.requestId,
        deviceHash: deviceTwo,
      }), reloadedDeps);
      expect(mismatch.status).toBe(404);
    } finally {
      await store.dispose();
    }
  });

  test('atomic license CAS keeps simultaneous approvals inside the device limit', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const { admin } = await login(deps);
    const created = await createLicense(admin, 1);
    const first = await requestActivation(deps, created.licenseKey, deviceOne);
    const second = await requestActivation(deps, created.licenseKey, deviceTwo);
    const outcomes = await Promise.all([first, second].map((item) => admin(
      `/api/license/admin/requests/${encodeURIComponent(item.requestId)}/approve`, 'POST', {},
    )));
    expect(outcomes.map((response) => response.status).sort()).toEqual([200, 409]);
    const list = await admin('/api/license/admin/licenses');
    const payload = await list.json();
    expect(payload.licenses[0].devices).toHaveLength(1);
  });

  test('device replacement releases the old slot and admits the next manually approved device', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const { admin } = await login(deps);
    const created = await createLicense(admin, 1);
    const oldRequest = await requestActivation(deps, created.licenseKey, deviceOne);
    await admin(`/api/license/admin/requests/${encodeURIComponent(oldRequest.requestId)}/approve`, 'POST', {});
    const released = await admin(`/api/license/admin/licenses/${created.license.id}/devices/${deviceOne}/release`, 'POST', {});
    expect(released.status).toBe(200);
    const oldStatus = await handleLicenseRequest(request('/api/license/activation-status', 'POST', {
      requestId: oldRequest.requestId, deviceHash: deviceOne,
    }), deps);
    expect(oldStatus.status).toBe(403);
    const newRequest = await requestActivation(deps, created.licenseKey, deviceTwo);
    const approved = await admin(`/api/license/admin/requests/${encodeURIComponent(newRequest.requestId)}/approve`, 'POST', {});
    expect(approved.status).toBe(200);
    const listing = await admin('/api/license/admin/licenses');
    const data = await listing.json();
    expect(data.licenses[0].devices.map((device: { deviceHash: string }) => device.deviceHash)).toEqual([deviceTwo]);
  });

  test('approval fails closed before consuming a slot when signing is not configured', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const { admin } = await login(deps);
    const created = await createLicense(admin, 1);
    const activation = await requestActivation(deps, created.licenseKey, deviceOne);
    delete deps.env.BORDRO_LICENSE_SIGNING_PRIVATE_KEY;
    const response = await admin(`/api/license/admin/requests/${encodeURIComponent(activation.requestId)}/approve`, 'POST', {});
    expect(response.status).toBe(503);
    const listing = await admin('/api/license/admin/licenses');
    expect((await listing.json()).licenses[0].devices).toHaveLength(0);
  });

  test('extra personal or payroll fields are ignored and never persisted by activation', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const { admin } = await login(deps);
    const created = await createLicense(admin, 2);
    const response = await handleLicenseRequest(request('/api/license/activate', 'POST', {
      licenseKey: created.licenseKey,
      deviceHash: deviceOne,
      personnel: [{ tcNo: '10000000146', name: 'Personel Gizli Ad', salary: 50000 }],
      payrollBackup: 'must-not-be-stored',
    }), deps);
    expect(response.status).toBe(202);
    const saved = [...store.snapshot().values()].map((entry) => JSON.stringify(entry.data)).join('\n');
    expect(saved).not.toContain('10000000146');
    expect(saved).not.toContain('must-not-be-stored');
    expect(saved).not.toContain('Personel Gizli Ad');
  });

  test('revocation is an explicit denial and does not include the license key in admin records', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const { admin } = await login(deps);
    const created = await createLicense(admin, 1);
    const list = await admin('/api/license/admin/licenses');
    const serialized = await list.text();
    expect(serialized).not.toContain(created.licenseKey);
    const revoked = await admin(`/api/license/admin/licenses/${created.license.id}/revoke`, 'POST', {});
    expect(revoked.status).toBe(200);
    const check = await handleLicenseRequest(request('/api/license/check', 'POST', {
      licenseId: created.license.id,
      deviceHash: deviceOne,
    }), deps);
    expect(check.status).toBe(403);
    expect((await check.json()).decision).toBe('denied');
  });

  test('invalid license key and device-limit input fail without creating records', async () => {
    const store = new AtomicBlobEmulator();
    const { deps } = dependencies(store);
    const invalid = await handleLicenseRequest(request('/api/license/activate', 'POST', { licenseKey: 'unknown', deviceHash: deviceOne }), deps);
    expect(invalid.status).toBe(404);
    const { admin } = await login(deps);
    const invalidLimit = await admin('/api/license/admin/licenses', 'POST', {
      institutionName: 'Test', licenseName: 'Test', expiresAt: '2027-01-01T00:00:00.000Z', maxDevices: 0,
    });
    expect(invalidLimit.status).toBe(400);
    expect((await store.list({ prefix: 'license:' })).blobs).toHaveLength(0);
  });

  test('authenticated backup restores idempotently and verifies its checksum', async () => {
    const firstStore = new AtomicBlobEmulator();
    const first = dependencies(firstStore);
    const { admin: firstAdmin } = await login(first.deps);
    await createLicense(firstAdmin, 3);
    const backupResponse = await firstAdmin('/api/license/admin/backup');
    const backup = await backupResponse.json();
    const target = new AtomicBlobEmulator();
    const second = dependencies(target);
    const { admin: secondAdmin } = await login(second.deps);
    const restored = await secondAdmin('/api/license/admin/backup/restore', 'POST', { backup });
    expect(restored.status).toBe(200);
    const retry = await secondAdmin('/api/license/admin/backup/restore', 'POST', { backup });
    expect(retry.status).toBe(200);
    backup.checksum = '0'.repeat(64);
    const corrupted = await secondAdmin('/api/license/admin/backup/restore', 'POST', { backup });
    expect(corrupted.status).toBe(400);
    expect((await target.list({ prefix: 'license:' })).blobs).toHaveLength(1);
  });

  test('in-memory storage emulator enforces ETag conditional writes atomically', async () => {
    const store = new AtomicBlobEmulator();
    const initial = await store.setJSON('counter', { value: 0 }, { onlyIfNew: true });
    const current = await store.getWithMetadata('counter');
    const competing = await Promise.all([
      store.setJSON('counter', { value: 1 }, { onlyIfMatch: current!.etag }),
      store.setJSON('counter', { value: 2 }, { onlyIfMatch: current!.etag }),
    ]);
    expect(initial.modified).toBe(true);
    expect(competing.filter((item) => item.modified)).toHaveLength(1);
    expect((await store.getWithMetadata('counter'))?.data).not.toEqual({ value: 0 });
  });

  test('file-backed local emulator persists records across instances and arbitrates concurrent CAS', async () => {
    const first = await FileAtomicBlobEmulator.create();
    try {
      await first.setJSON('license:test', { status: 'active', seats: 0 }, { onlyIfNew: true });
      const second = await FileAtomicBlobEmulator.open(first.directory);
      const current = await first.getWithMetadata('license:test');
      const results = await Promise.all([
        first.setJSON('license:test', { status: 'active', seats: 1 }, { onlyIfMatch: current!.etag }),
        second.setJSON('license:test', { status: 'active', seats: 2 }, { onlyIfMatch: current!.etag }),
      ]);
      expect(results.filter((item) => item.modified)).toHaveLength(1);
      const reloaded = await FileAtomicBlobEmulator.open(first.directory);
      expect((await reloaded.getWithMetadata('license:test'))?.data).not.toEqual({ status: 'active', seats: 0 });
    } finally {
      await first.dispose();
    }
  });
});
