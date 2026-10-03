import {
  createHash,
  createHmac,
  createPrivateKey,
  randomBytes,
  scryptSync,
  sign,
  timingSafeEqual,
} from 'node:crypto';
import type { LicenseGrantClaims } from './model';

const base64url = (value: Buffer | string) => Buffer.from(value).toString('base64url');

export function hashLicenseKey(key: string): string {
  return createHash('sha256').update('4d-bordro-license-key-v1\0').update(key.trim()).digest('hex');
}

export function generateLicenseKey(): string {
  return `4D-${randomBytes(32).toString('base64url')}`;
}

export function verifyPassword(password: string, encodedHash: string): boolean {
  const [algorithm, salt, expected] = encodedHash.split('$');
  if (algorithm !== 'scrypt' || !salt || !expected) return false;
  try {
    const actualBuffer = scryptSync(password, Buffer.from(salt, 'base64url'), 32);
    const expectedBuffer = Buffer.from(expected, 'base64url');
    return expectedBuffer.length === actualBuffer.length && timingSafeEqual(actualBuffer, expectedBuffer);
  } catch {
    return false;
  }
}

export function createSessionToken(
  claims: { sub: string; exp: number; csrf: string },
  secret: string
): string {
  const payload = base64url(JSON.stringify(claims));
  const signature = createHmac('sha256', secret).update(payload).digest('base64url');
  return `${payload}.${signature}`;
}

export function readSessionToken(token: string, secret: string, now: number):
  | { sub: string; exp: number; csrf: string }
  | null {
  const [payload, signature, extra] = token.split('.');
  if (!payload || !signature || extra) return null;
  const expected = createHmac('sha256', secret).update(payload).digest();
  const received = Buffer.from(signature, 'base64url');
  if (received.length !== expected.length || !timingSafeEqual(received, expected)) return null;
  try {
    const claims = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')) as {
      sub?: unknown; exp?: unknown; csrf?: unknown;
    };
    if (typeof claims.sub !== 'string' || typeof claims.exp !== 'number' ||
        typeof claims.csrf !== 'string' || claims.exp <= now) return null;
    return claims as { sub: string; exp: number; csrf: string };
  } catch {
    return null;
  }
}

export function signGrant(claims: LicenseGrantClaims, privateKeyPem: string): string {
  const privateKey = readSigningKey(privateKeyPem);
  const payload = Buffer.from(JSON.stringify(claims));
  const signature = sign(null, payload, privateKey);
  return `v1.${base64url(payload)}.${base64url(signature)}`;
}

export function validateSigningKey(privateKeyPem: string): void {
  readSigningKey(privateKeyPem);
}

function readSigningKey(privateKeyPem: string) {
  const privateKey = createPrivateKey(privateKeyPem.replace(/\\n/g, '\n'));
  if (privateKey.asymmetricKeyType !== 'ed25519') {
    throw new Error('License signing key must use Ed25519.');
  }
  return privateKey;
}

export function createPasswordHash(password: string, salt = randomBytes(16)): string {
  const derived = scryptSync(password, salt, 32);
  return `scrypt$${base64url(salt)}$${base64url(derived)}`;
}
