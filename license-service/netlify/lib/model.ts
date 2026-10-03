export type LicenseStatus = 'active' | 'revoked';
export type ActivationStatus = 'pending' | 'approving' | 'approved' | 'denied' | 'released';

export interface LicensedDevice {
  deviceHash: string;
  activatedAt: string;
  activationId: string;
}

export interface LicenseRecord {
  id: string;
  institutionName: string;
  licenseName: string;
  keyHash: string;
  status: LicenseStatus;
  createdAt: string;
  expiresAt: string;
  maxDevices: number;
  devices: LicensedDevice[];
  revokedAt?: string;
}

export interface ActivationHistoryItem {
  status: ActivationStatus;
  at: string;
  actor: 'device' | 'admin';
}

export interface ActivationRecord {
  id: string;
  licenseId: string;
  deviceHash: string;
  status: ActivationStatus;
  createdAt: string;
  decidedAt?: string;
  history: ActivationHistoryItem[];
}

export interface LicenseGrantClaims {
  v: 1;
  aud: '4d-bordro-desktop';
  kid: string;
  licenseId: string;
  deviceHash: string;
  issuedAt: string;
  lastOnlineAt: string;
  offlineUntil: string;
  licenseExpiresAt: string;
  status: 'active';
}

export const LICENSE_PREFIX = 'license:';
export const KEY_INDEX_PREFIX = 'key-index:';
export const ACTIVATION_PREFIX = 'activation:';

export function activationIdFor(licenseId: string, deviceHash: string): string {
  return `${ACTIVATION_PREFIX}${licenseId}:${deviceHash}`;
}

export function validDeviceHash(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}
