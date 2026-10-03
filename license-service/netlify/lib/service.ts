import {
  createHash,
  randomBytes,
  timingSafeEqual,
} from 'node:crypto';
import type { BlobStore } from './storage';
import { readEntry, readJSON } from './storage';
import {
  ACTIVATION_PREFIX,
  activationIdFor,
  KEY_INDEX_PREFIX,
  LICENSE_PREFIX,
  type ActivationRecord,
  type LicenseGrantClaims,
  type LicenseRecord,
  validDeviceHash,
} from './model';
import {
  createSessionToken,
  generateLicenseKey,
  hashLicenseKey,
  readSessionToken,
  signGrant,
  validateSigningKey,
  verifyPassword,
} from './crypto';

export interface ServiceEnv {
  BORDRO_ADMIN_USERNAME?: string;
  BORDRO_ADMIN_PASSWORD_HASH?: string;
  BORDRO_ADMIN_SESSION_SECRET?: string;
  BORDRO_LICENSE_KEY_ID?: string;
  BORDRO_LICENSE_SIGNING_PRIVATE_KEY?: string;
}

export interface ServiceContext {
  deploy?: { context?: string };
  ip?: string;
}

export interface ServiceDependencies {
  store: BlobStore;
  env: ServiceEnv;
  now?: () => Date;
  clientIp?: string;
}

const SESSION_TTL_SECONDS = 8 * 60 * 60;
const MAX_LOGIN_ATTEMPTS = 5;
const LOGIN_WINDOW_MS = 15 * 60 * 1000;
const CAS_RETRIES = 24;

function json(body: unknown, status = 200, headers: HeadersInit = {}): Response {
  const responseHeaders = new Headers(headers);
  responseHeaders.set('Content-Type', 'application/json; charset=utf-8');
  responseHeaders.set('Cache-Control', 'no-store');
  responseHeaders.set('X-Content-Type-Options', 'nosniff');
  return new Response(JSON.stringify(body), { status, headers: responseHeaders });
}

function error(status: number, code: string, message: string): Response {
  return json({ error: code, message }, status);
}

async function readBody(request: Request): Promise<Record<string, unknown> | null> {
  const contentType = request.headers.get('content-type') ?? '';
  if (!contentType.toLowerCase().includes('application/json')) return null;
  try {
    const value = await request.json();
    if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
    return value as Record<string, unknown>;
  } catch {
    return null;
  }
}

function sameSecret(left: string, right: string): boolean {
  const leftBytes = Buffer.from(left);
  const rightBytes = Buffer.from(right);
  return leftBytes.length === rightBytes.length && timingSafeEqual(leftBytes, rightBytes);
}

function isValidOrigin(request: Request): boolean {
  const origin = request.headers.get('origin');
  if (!origin) return true;
  try {
    return new URL(origin).origin === new URL(request.url).origin;
  } catch {
    return false;
  }
}

function parseCookie(request: Request, name: string): string | null {
  const cookie = request.headers.get('cookie') ?? '';
  for (const part of cookie.split(';')) {
    const [key, ...value] = part.trim().split('=');
    if (key === name) return decodeURIComponent(value.join('='));
  }
  return null;
}

function sessionCookie(token: string, secure: boolean): string {
  return `BORDRO_ADMIN_SESSION=${encodeURIComponent(token)}; HttpOnly; SameSite=Strict; Path=/api/license/admin; Max-Age=${SESSION_TTL_SECONDS}${secure ? '; Secure' : ''}`;
}

function clearSessionCookie(secure: boolean): string {
  return `BORDRO_ADMIN_SESSION=; HttpOnly; SameSite=Strict; Path=/api/license/admin; Max-Age=0${secure ? '; Secure' : ''}`;
}

async function requireAdmin(
  request: Request,
  deps: ServiceDependencies,
  mutation: boolean
): Promise<{ username: string; csrf: string } | Response> {
  const secret = deps.env.BORDRO_ADMIN_SESSION_SECRET;
  const username = deps.env.BORDRO_ADMIN_USERNAME;
  if (!secret || secret.length < 32 || !username) {
    return error(503, 'admin_not_configured', 'Yönetici kimlik doğrulaması yapılandırılmamış.');
  }
  if (!isValidOrigin(request)) return error(403, 'origin_rejected', 'İstek kaynağı doğrulanamadı.');
  const token = parseCookie(request, 'BORDRO_ADMIN_SESSION');
  const claims = token && readSessionToken(token, secret, Math.floor((deps.now?.() ?? new Date()).getTime() / 1000));
  if (!claims || !sameSecret(claims.sub, username)) {
    return error(401, 'unauthorized', 'Yönetici oturumu gerekli.');
  }
  if (mutation && !sameSecret(request.headers.get('x-csrf-token') ?? '', claims.csrf)) {
    return error(403, 'csrf_rejected', 'İstek doğrulaması başarısız. Sayfayı yenileyip yeniden giriş yapın.');
  }
  return { username, csrf: claims.csrf };
}

function validDate(value: unknown): value is string {
  return typeof value === 'string' && Number.isFinite(Date.parse(value));
}

function hashIp(ip: string): string {
  return createHash('sha256').update(ip).digest('hex');
}

interface LoginRateRecord {
  count: number;
  windowStartedAt: string;
}

async function allowLoginAttempt(store: BlobStore, ip: string, now: Date): Promise<boolean> {
  const key = `rate:admin-login:${hashIp(ip)}`;
  for (let attempt = 0; attempt < CAS_RETRIES; attempt += 1) {
    const current = await readEntry<LoginRateRecord>(store, key);
    if (current && now.getTime() - Date.parse(current.data.windowStartedAt) < LOGIN_WINDOW_MS &&
        current.data.count >= MAX_LOGIN_ATTEMPTS) return false;
    const expired = !current || now.getTime() - Date.parse(current.data.windowStartedAt) >= LOGIN_WINDOW_MS;
    const next = expired
      ? { count: 0, windowStartedAt: now.toISOString() }
      : current.data;
    const result = await store.setJSON(key, next, current ? { onlyIfMatch: current.etag } : { onlyIfNew: true });
    if (result.modified) return true;
  }
  return false;
}

async function recordLoginFailure(store: BlobStore, ip: string, now: Date): Promise<void> {
  const key = `rate:admin-login:${hashIp(ip)}`;
  for (let attempt = 0; attempt < CAS_RETRIES; attempt += 1) {
    const current = await readEntry<LoginRateRecord>(store, key);
    const expired = !current || now.getTime() - Date.parse(current.data.windowStartedAt) >= LOGIN_WINDOW_MS;
    const next = expired
      ? { count: 1, windowStartedAt: now.toISOString() }
      : { ...current.data, count: current.data.count + 1 };
    const result = await store.setJSON(key, next, current ? { onlyIfMatch: current.etag } : { onlyIfNew: true });
    if (result.modified) return;
  }
}

async function createLicense(
  store: BlobStore,
  body: Record<string, unknown>,
  now: Date
): Promise<{ record: LicenseRecord; key: string } | Response> {
  const institutionName = typeof body.institutionName === 'string' ? body.institutionName.trim() : '';
  const licenseName = typeof body.licenseName === 'string' ? body.licenseName.trim() : '';
  const maxDevices = body.maxDevices;
  const expiresAt = body.expiresAt;
  if (!institutionName || institutionName.length > 160 || !licenseName || licenseName.length > 100) {
    return error(400, 'invalid_license', 'Kurum ve lisans adı zorunludur.');
  }
  if (!Number.isInteger(maxDevices) || (maxDevices as number) < 1 || (maxDevices as number) > 100) {
    return error(400, 'invalid_device_limit', 'Cihaz hakkı 1 ile 100 arasında tam sayı olmalıdır.');
  }
  if (!validDate(expiresAt) || Date.parse(expiresAt) <= now.getTime()) {
    return error(400, 'invalid_expiry', 'Geçerlilik tarihi gelecekte olmalıdır.');
  }

  const key = generateLicenseKey();
  const keyHash = hashLicenseKey(key);
  const id = randomBytes(18).toString('base64url');
  const record: LicenseRecord = {
    id,
    institutionName,
    licenseName,
    keyHash,
    status: 'active',
    createdAt: now.toISOString(),
    expiresAt: new Date(expiresAt).toISOString(),
    maxDevices: maxDevices as number,
    devices: [],
  };
  const written = await store.setJSON(`${LICENSE_PREFIX}${id}`, record, { onlyIfNew: true });
  if (!written.modified) return error(503, 'storage_conflict', 'Lisans oluşturulamadı. Yeniden deneyin.');
  const indexed = await store.setJSON(`${KEY_INDEX_PREFIX}${keyHash}`, { licenseId: id }, { onlyIfNew: true });
  if (!indexed.modified) {
    await store.delete(`${LICENSE_PREFIX}${id}`);
    return error(503, 'storage_conflict', 'Lisans anahtarı oluşturulamadı. Yeniden deneyin.');
  }
  return { record, key };
}

function validGrantConfiguration(env: ServiceEnv): boolean {
  return Boolean(env.BORDRO_LICENSE_KEY_ID && env.BORDRO_LICENSE_SIGNING_PRIVATE_KEY);
}

function makeGrant(record: LicenseRecord, deviceHash: string, env: ServiceEnv, now: Date): string | null {
  if (!validGrantConfiguration(env)) return null;
  const expiry = Date.parse(record.expiresAt);
  const offlineMax = now.getTime() + 30 * 24 * 60 * 60 * 1000;
  const claims: LicenseGrantClaims = {
    v: 1,
    aud: '4d-bordro-desktop',
    kid: env.BORDRO_LICENSE_KEY_ID!,
    licenseId: record.id,
    deviceHash,
    issuedAt: now.toISOString(),
    lastOnlineAt: now.toISOString(),
    offlineUntil: new Date(Math.min(expiry, offlineMax)).toISOString(),
    licenseExpiresAt: record.expiresAt,
    status: 'active',
  };
  return signGrant(claims, env.BORDRO_LICENSE_SIGNING_PRIVATE_KEY!);
}

async function findLicenseByKey(store: BlobStore, key: string): Promise<LicenseRecord | null> {
  const keyHash = hashLicenseKey(key);
  const index = await readJSON<{ licenseId: string }>(store, `${KEY_INDEX_PREFIX}${keyHash}`);
  if (!index) return null;
  return readJSON<LicenseRecord>(store, `${LICENSE_PREFIX}${index.licenseId}`);
}

async function withLicenseCas<T>(
  store: BlobStore,
  licenseId: string,
  mutate: (record: LicenseRecord) => { next: LicenseRecord; value: T } | { reject: Response }
): Promise<T | Response> {
  for (let attempt = 0; attempt < CAS_RETRIES; attempt += 1) {
    const current = await readEntry<LicenseRecord>(store, `${LICENSE_PREFIX}${licenseId}`);
    if (!current) return error(404, 'license_not_found', 'Lisans bulunamadı.');
    const plan = mutate(current.data);
    if ('reject' in plan) return plan.reject;
    const result = await store.setJSON(`${LICENSE_PREFIX}${licenseId}`, plan.next, { onlyIfMatch: current.etag });
    if (result.modified) return plan.value;
  }
  return error(409, 'concurrent_update', 'Lisans aynı anda değiştirildi. İşlem tamamlanmadı; yeniden deneyin.');
}

async function issueActivation(
  store: BlobStore,
  body: Record<string, unknown>,
  now: Date
): Promise<Response> {
  const licenseKey = typeof body.licenseKey === 'string' ? body.licenseKey.trim() : '';
  const deviceHash = body.deviceHash;
  if (!licenseKey || licenseKey.length > 160 || !validDeviceHash(deviceHash)) {
    return error(400, 'invalid_activation', 'Lisans anahtarı veya cihaz kimliği biçimi geçersiz.');
  }
  const license = await findLicenseByKey(store, licenseKey);
  if (!license) return error(404, 'invalid_license_key', 'Lisans anahtarı geçersiz.');
  if (license.status !== 'active') return json({ decision: 'denied', reason: 'revoked' }, 403);
  if (Date.parse(license.expiresAt) <= now.getTime()) return json({ decision: 'denied', reason: 'expired' }, 403);

  const id = activationIdFor(license.id, deviceHash);
  for (let attempt = 0; attempt < CAS_RETRIES; attempt += 1) {
    const current = await readEntry<ActivationRecord>(store, id);
    if (current && current.data.status !== 'denied' && current.data.status !== 'released') {
      return json({ requestId: id.slice(ACTIVATION_PREFIX.length), status: current.data.status === 'approving' ? 'pending' : current.data.status });
    }
    const record: ActivationRecord = {
      id: id.slice(ACTIVATION_PREFIX.length),
      licenseId: license.id,
      deviceHash,
      status: 'pending',
      createdAt: now.toISOString(),
      history: [
        ...(current?.data.history ?? []),
        { status: 'pending', at: now.toISOString(), actor: 'device' },
      ],
    };
    const write = await store.setJSON(id, record, current ? { onlyIfMatch: current.etag } : { onlyIfNew: true });
    if (write.modified) return json({ requestId: record.id, status: 'pending' }, 202);
  }
  return error(409, 'activation_conflict', 'Talep eşzamanlı olarak değişti. Yeniden deneyin.');
}

async function activationStatus(
  store: BlobStore,
  body: Record<string, unknown>,
  env: ServiceEnv,
  now: Date
): Promise<Response> {
  const requestId = typeof body.requestId === 'string' ? body.requestId : '';
  const deviceHash = body.deviceHash;
  if (!requestId || requestId.length > 220 || !validDeviceHash(deviceHash)) {
    return error(400, 'invalid_status_request', 'Talep kimliği veya cihaz kimliği geçersiz.');
  }
  const activation = await readJSON<ActivationRecord>(store, `${ACTIVATION_PREFIX}${requestId}`);
  if (!activation || activation.deviceHash !== deviceHash) return error(404, 'request_not_found', 'Talep bulunamadı.');
  const license = await readJSON<LicenseRecord>(store, `${LICENSE_PREFIX}${activation.licenseId}`);
  if (!license || license.status !== 'active' || Date.parse(license.expiresAt) <= now.getTime()) {
    return json({ decision: 'denied', reason: license?.status === 'revoked' ? 'revoked' : 'expired' }, 403);
  }
  if (activation.status === 'pending' || activation.status === 'approving') return json({ decision: 'pending' });
  if (activation.status === 'denied' || activation.status === 'released') {
    return json({ decision: 'denied', reason: activation.status }, 403);
  }
  const device = license.devices.find((item) => item.deviceHash === deviceHash && item.activationId === activation.id);
  if (!device) return error(409, 'device_not_assigned', 'Cihaz hakkı ataması tamamlanmadı.');
  const grant = makeGrant(license, deviceHash, env, now);
  if (!grant) return error(503, 'signing_not_configured', 'Lisans imzalama sunucuda yapılandırılmamış.');
  return json({ decision: 'authorized', grant });
}

async function checkActiveLicense(
  store: BlobStore,
  body: Record<string, unknown>,
  env: ServiceEnv,
  now: Date
): Promise<Response> {
  const licenseId = typeof body.licenseId === 'string' ? body.licenseId : '';
  const deviceHash = body.deviceHash;
  if (!licenseId || licenseId.length > 120 || !validDeviceHash(deviceHash)) {
    return error(400, 'invalid_check', 'Lisans veya cihaz kimliği geçersiz.');
  }
  const license = await readJSON<LicenseRecord>(store, `${LICENSE_PREFIX}${licenseId}`);
  if (!license || license.status !== 'active') return json({ decision: 'denied', reason: 'revoked' }, 403);
  if (Date.parse(license.expiresAt) <= now.getTime()) return json({ decision: 'denied', reason: 'expired' }, 403);
  if (!license.devices.some((item) => item.deviceHash === deviceHash)) {
    return json({ decision: 'denied', reason: 'device_not_approved' }, 403);
  }
  const grant = makeGrant(license, deviceHash, env, now);
  if (!grant) return error(503, 'signing_not_configured', 'Lisans imzalama sunucuda yapılandırılmamış.');
  return json({ decision: 'authorized', grant });
}

async function listAdminLicenses(store: BlobStore): Promise<unknown[]> {
  const { blobs } = await store.list({ prefix: LICENSE_PREFIX });
  const records = await Promise.all(blobs.map(({ key }) => readJSON<LicenseRecord>(store, key)));
  return records.filter((item): item is LicenseRecord => Boolean(item)).map((item) => ({
    id: item.id,
    institutionName: item.institutionName,
    licenseName: item.licenseName,
    status: item.status,
    createdAt: item.createdAt,
    expiresAt: item.expiresAt,
    maxDevices: item.maxDevices,
    devices: item.devices,
  }));
}

async function listPendingRequests(store: BlobStore): Promise<ActivationRecord[]> {
  const { blobs } = await store.list({ prefix: ACTIVATION_PREFIX });
  const records = await Promise.all(blobs.map(({ key }) => readJSON<ActivationRecord>(store, key)));
  return records.filter((item): item is ActivationRecord => Boolean(item) && (item!.status === 'pending' || item!.status === 'approving'));
}

async function decideActivation(
  store: BlobStore,
  requestId: string,
  decision: 'approve' | 'deny',
  actor: string,
  env: ServiceEnv,
  now: Date
): Promise<Response> {
  if (decision === 'approve') {
    if (!env.BORDRO_LICENSE_KEY_ID || !env.BORDRO_LICENSE_SIGNING_PRIVATE_KEY) {
      return error(503, 'signing_not_configured', 'Lisans imzalama sunucuda yapılandırılmamış.');
    }
    try { validateSigningKey(env.BORDRO_LICENSE_SIGNING_PRIVATE_KEY); }
    catch { return error(503, 'signing_key_invalid', 'Lisans imzalama anahtarı geçersiz.'); }
  }
  const key = `${ACTIVATION_PREFIX}${requestId}`;
  for (let attempt = 0; attempt < CAS_RETRIES; attempt += 1) {
    let activation = await readEntry<ActivationRecord>(store, key);
    if (!activation) return error(404, 'request_not_found', 'Aktivasyon talebi bulunamadı.');
    if (activation.data.status === 'approved' && decision === 'approve') return json({ status: 'approved' });
    if (activation.data.status === 'pending' && decision === 'deny') {
      const denied: ActivationRecord = {
        ...activation.data,
        status: 'denied',
        decidedAt: now.toISOString(),
        history: [...activation.data.history, { status: 'denied', at: now.toISOString(), actor: 'admin' }],
      };
      const written = await store.setJSON(key, denied, { onlyIfMatch: activation.etag });
      if (written.modified) return json({ status: 'denied' });
      continue;
    }
    if (decision === 'deny') return error(409, 'request_already_decided', 'Talep onay işlemi sürüyor veya daha önce sonuçlandırılmış.');
    if (activation.data.status === 'pending') {
      const claimed: ActivationRecord = {
        ...activation.data,
        status: 'approving',
        history: [...activation.data.history, { status: 'approving', at: now.toISOString(), actor: 'admin' }],
      };
      const write = await store.setJSON(key, claimed, { onlyIfMatch: activation.etag });
      if (!write.modified || !write.etag) continue;
      activation = { data: claimed, etag: write.etag };
    } else if (activation.data.status !== 'approving') {
      return error(409, 'request_already_decided', 'Talep daha önce sonuçlandırılmış.');
    }
    if (decision === 'approve') {
      const assignment = await withLicenseCas(store, activation.data.licenseId, (license) => {
        if (license.status !== 'active') return { reject: error(409, 'license_inactive', 'İptal edilmiş lisansa cihaz eklenemez.') };
        if (Date.parse(license.expiresAt) <= now.getTime()) return { reject: error(409, 'license_expired', 'Süresi dolmuş lisansa cihaz eklenemez.') };
        const alreadyAssigned = license.devices.some((device) => device.deviceHash === activation.data.deviceHash);
        if (!alreadyAssigned && license.devices.length >= license.maxDevices) {
          return { reject: error(409, 'device_limit_reached', 'Cihaz hakkı dolu. Önce eski cihaz hakkını kaldırın.') };
        }
        const devices = alreadyAssigned ? license.devices : [
          ...license.devices,
          { deviceHash: activation.data.deviceHash, activatedAt: now.toISOString(), activationId: activation.data.id },
        ];
        return { next: { ...license, devices }, value: undefined };
      });
      if (assignment instanceof Response) {
        const reset: ActivationRecord = {
          ...activation.data,
          status: 'pending',
          history: [...activation.data.history, { status: 'pending', at: now.toISOString(), actor: 'admin' }],
        };
        await store.setJSON(key, reset, { onlyIfMatch: activation.etag });
        return assignment;
      }
    }
    const next: ActivationRecord = {
      ...activation.data,
      status: decision === 'approve' ? 'approved' : 'denied',
      decidedAt: now.toISOString(),
      history: [...activation.data.history, { status: decision === 'approve' ? 'approved' : 'denied', at: now.toISOString(), actor: 'admin' }],
    };
    const written = await store.setJSON(key, next, { onlyIfMatch: activation.etag });
    if (written.modified) return json({ status: next.status });
  }
  return error(409, 'concurrent_update', 'Talep aynı anda değiştirildi. Yeniden deneyin.');
}

async function releaseDevice(store: BlobStore, licenseId: string, deviceHash: string, now: Date): Promise<Response> {
  if (!validDeviceHash(deviceHash)) return error(400, 'invalid_device', 'Cihaz kimliği biçimi geçersiz.');
  const removed = await withLicenseCas(store, licenseId, (license) => {
    const devices = license.devices.filter((device) => device.deviceHash !== deviceHash);
    if (devices.length === license.devices.length) return { reject: error(404, 'device_not_found', 'Cihaz lisansa bağlı değil.') };
    return { next: { ...license, devices }, value: true };
  });
  if (removed instanceof Response) return removed;
  const activation = await readEntry<ActivationRecord>(store, activationIdFor(licenseId, deviceHash));
  if (activation && activation.data.status === 'approved') {
    await store.setJSON(activationIdFor(licenseId, deviceHash), {
      ...activation.data,
      status: 'released',
      decidedAt: now.toISOString(),
      history: [...activation.data.history, { status: 'released', at: now.toISOString(), actor: 'admin' }],
    }, { onlyIfMatch: activation.etag });
  }
  return json({ status: 'released' });
}

async function revokeLicense(store: BlobStore, licenseId: string, now: Date): Promise<Response> {
  const revoked = await withLicenseCas(store, licenseId, (record) => ({
    next: record.status === 'revoked' ? record : { ...record, status: 'revoked', revokedAt: now.toISOString() },
    value: true,
  }));
  return revoked instanceof Response ? revoked : json({ status: 'revoked' });
}

interface BackupDocument {
  schemaVersion: 1;
  generatedAt: string;
  entries: Array<{ key: string; value: unknown }>;
  checksum: string;
}

function stableJson(value: unknown): string {
  return JSON.stringify(value);
}

function backupChecksum(schemaVersion: number, entries: BackupDocument['entries']): string {
  return createHash('sha256').update(stableJson({ schemaVersion, entries })).digest('hex');
}

async function exportBackup(store: BlobStore, now: Date): Promise<BackupDocument> {
  const entries: BackupDocument['entries'] = [];
  for (const prefix of [LICENSE_PREFIX, KEY_INDEX_PREFIX, ACTIVATION_PREFIX]) {
    const { blobs } = await store.list({ prefix });
    for (const { key } of blobs) {
      const value = await readJSON<unknown>(store, key);
      if (value !== null) entries.push({ key, value });
    }
  }
  entries.sort((left, right) => left.key.localeCompare(right.key));
  return { schemaVersion: 1, generatedAt: now.toISOString(), entries, checksum: backupChecksum(1, entries) };
}

async function restoreBackup(store: BlobStore, body: Record<string, unknown>): Promise<Response> {
  const document = body.backup as BackupDocument | undefined;
  if (!document || document.schemaVersion !== 1 || !Array.isArray(document.entries) ||
      document.entries.length > 100_000 || typeof document.checksum !== 'string') {
    return error(400, 'invalid_backup', 'Yedek biçimi desteklenmiyor veya bozuk.');
  }
  const expected = backupChecksum(1, document.entries);
  if (!sameSecret(expected, document.checksum)) return error(400, 'backup_checksum_failed', 'Yedek bütünlük kontrolünden geçmedi.');
  const permitted = (key: string) => key.startsWith(LICENSE_PREFIX) || key.startsWith(KEY_INDEX_PREFIX) || key.startsWith(ACTIVATION_PREFIX);
  if (document.entries.some((entry) => !entry || typeof entry.key !== 'string' || !permitted(entry.key))) {
    return error(400, 'invalid_backup_keys', 'Yedekte izin verilmeyen kayıt anahtarı var.');
  }
  for (const entry of document.entries) {
    const result = await store.setJSON(entry.key, entry.value, { onlyIfNew: true });
    if (result.modified) continue;
    const existing = await readJSON<unknown>(store, entry.key);
    if (stableJson(existing) !== stableJson(entry.value)) {
      return error(409, 'restore_conflict', 'Depoda farklı kayıt bulundu. Yedek kısmen uygulanmış olabilir; aynı yedeği tekrar deneyin veya kayıtları dışa aktarın.');
    }
  }
  return json({ restored: document.entries.length });
}

export async function handleLicenseRequest(request: Request, deps: ServiceDependencies): Promise<Response> {
  const now = deps.now?.() ?? new Date();
  const url = new URL(request.url);
  const route = url.pathname.replace(/\/$/, '');
  const secure = request.url.startsWith('https:');

  if (request.method === 'POST' && route === '/api/license/login') {
    if (!isValidOrigin(request)) return error(403, 'origin_rejected', 'İstek kaynağı doğrulanamadı.');
    const body = await readBody(request);
    if (!body || typeof body.username !== 'string' || typeof body.password !== 'string') {
      return error(400, 'invalid_login', 'Kullanıcı adı ve parola gerekli.');
    }
    const ip = deps.clientIp ?? 'unknown';
    if (!(await allowLoginAttempt(deps.store, ip, now))) return error(429, 'login_rate_limited', 'Çok fazla giriş denemesi. 15 dakika sonra yeniden deneyin.');
    const validConfiguration = deps.env.BORDRO_ADMIN_USERNAME && deps.env.BORDRO_ADMIN_PASSWORD_HASH &&
      deps.env.BORDRO_ADMIN_SESSION_SECRET && deps.env.BORDRO_ADMIN_SESSION_SECRET.length >= 32;
    const validCredentials = Boolean(validConfiguration && sameSecret(body.username, deps.env.BORDRO_ADMIN_USERNAME!) &&
      verifyPassword(body.password, deps.env.BORDRO_ADMIN_PASSWORD_HASH!));
    if (!validCredentials) {
      await recordLoginFailure(deps.store, ip, now);
      return error(validConfiguration ? 401 : 503, validConfiguration ? 'invalid_credentials' : 'admin_not_configured',
        validConfiguration ? 'Kullanıcı adı veya parola hatalı.' : 'Yönetici kimlik doğrulaması yapılandırılmamış.');
    }
    const csrf = randomBytes(24).toString('base64url');
    const token = createSessionToken({ sub: body.username, exp: Math.floor(now.getTime() / 1000) + SESSION_TTL_SECONDS, csrf }, deps.env.BORDRO_ADMIN_SESSION_SECRET!);
    await deps.store.delete(`rate:admin-login:${hashIp(ip)}`);
    return json({ csrf }, 200, { 'Set-Cookie': sessionCookie(token, secure) });
  }

  if (request.method === 'POST' && route === '/api/license/logout') {
    return json({ ok: true }, 200, { 'Set-Cookie': clearSessionCookie(secure) });
  }

  if (request.method === 'POST' && route === '/api/license/activate') {
    const body = await readBody(request);
    return body ? issueActivation(deps.store, body, now) : error(400, 'invalid_json', 'JSON gövdesi gerekli.');
  }
  if (request.method === 'POST' && route === '/api/license/activation-status') {
    const body = await readBody(request);
    return body ? activationStatus(deps.store, body, deps.env, now) : error(400, 'invalid_json', 'JSON gövdesi gerekli.');
  }
  if (request.method === 'POST' && route === '/api/license/check') {
    const body = await readBody(request);
    return body ? checkActiveLicense(deps.store, body, deps.env, now) : error(400, 'invalid_json', 'JSON gövdesi gerekli.');
  }

  if (route.startsWith('/api/license/admin')) {
    const isMutation = request.method !== 'GET';
    const auth = await requireAdmin(request, deps, isMutation);
    if (auth instanceof Response) return auth;
    if (request.method === 'GET' && route === '/api/license/admin/session') return json({ csrf: auth.csrf });
    if (request.method === 'GET' && route === '/api/license/admin/licenses') {
      return json({ licenses: await listAdminLicenses(deps.store) });
    }
    if (request.method === 'POST' && route === '/api/license/admin/licenses') {
      const body = await readBody(request);
      if (!body) return error(400, 'invalid_json', 'JSON gövdesi gerekli.');
      const result = await createLicense(deps.store, body, now);
      return result instanceof Response ? result : json({ license: { ...result.record, keyHash: undefined }, licenseKey: result.key }, 201);
    }
    if (request.method === 'GET' && route === '/api/license/admin/requests') {
      return json({ requests: await listPendingRequests(deps.store) });
    }
    if (request.method === 'GET' && route === '/api/license/admin/backup') {
      return json(await exportBackup(deps.store, now));
    }
    if (request.method === 'POST' && route === '/api/license/admin/backup/restore') {
      const body = await readBody(request);
      return body ? restoreBackup(deps.store, body) : error(400, 'invalid_json', 'JSON gövdesi gerekli.');
    }
    const requestDecision = route.match(/^\/api\/license\/admin\/requests\/([A-Za-z0-9_%:-]+)\/(approve|deny)$/);
    if (request.method === 'POST' && requestDecision) {
      let requestId: string;
      try { requestId = decodeURIComponent(requestDecision[1]); }
      catch { return error(400, 'invalid_request_id', 'Talep kimliği geçersiz.'); }
      return decideActivation(deps.store, requestId, requestDecision[2] as 'approve' | 'deny', auth.username, deps.env, now);
    }
    const release = route.match(/^\/api\/license\/admin\/licenses\/([A-Za-z0-9_-]+)\/devices\/([a-f0-9]{64})\/release$/);
    if (request.method === 'POST' && release) return releaseDevice(deps.store, release[1], release[2], now);
    const revoke = route.match(/^\/api\/license\/admin\/licenses\/([A-Za-z0-9_-]+)\/revoke$/);
    if (request.method === 'POST' && revoke) return revokeLicense(deps.store, revoke[1], now);
    return error(404, 'not_found', 'Yönetim uç noktası bulunamadı.');
  }

  return error(404, 'not_found', 'Uç nokta bulunamadı.');
}
