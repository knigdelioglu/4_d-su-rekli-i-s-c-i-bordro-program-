import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import {
  parseLegacyPayrollStorage,
  parsePayrollStorage,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';

type PartialPayload = Partial<PayrollStorageDto> & Record<string, unknown>;

const REQUIRED_V2_COLLECTIONS = [
  'puantajlar',
  'bordrolar',
  'taxOpenings',
  'sickLeaveRecords',
  'annualPayrollParameters',
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function parseJsonObject(json: string): Record<string, unknown> {
  const value: unknown = JSON.parse(json);
  if (!isRecord(value)) {
    throw new Error('Yedek JSON nesne içermiyor.');
  }
  return value;
}

function getBackupVersion(parsed: Record<string, unknown>): number {
  const versionValue = parsed.backupVersion;
  if (versionValue === undefined) return 1;
  if (typeof versionValue !== 'number' || !Number.isInteger(versionValue)) {
    throw new Error('Yedek sürümü geçersiz; backupVersion tam sayı olmalıdır.');
  }
  if (versionValue <= 0 || versionValue > BACKUP_FORMAT_VERSION) {
    throw new Error(`Desteklenmeyen yedek sürümü: ${versionValue}`);
  }
  return versionValue;
}

function validateBackupStructure(parsed: PartialPayload): number {
  const version = getBackupVersion(parsed);
  if (!Array.isArray(parsed.donemler) || !Array.isArray(parsed.personeller)) {
    throw new Error('Yedek dosyasında dönem veya personel listesi bulunamadı.');
  }
  if (
    version >= BACKUP_FORMAT_VERSION &&
    REQUIRED_V2_COLLECTIONS.some((key) => !Array.isArray(parsed[key]))
  ) {
    throw new Error('V2 yedek dosyasında tüm domain kayıt listeleri bulunmalıdır.');
  }
  return version;
}

function normalizeZamAylari(value: unknown): number[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(
    value.filter(
      (month): month is number =>
        typeof month === 'number' && Number.isInteger(month) && month >= 1 && month <= 12
    )
  )].sort((a, b) => a - b);
}

function toCanonicalBackupPayload(parsed: PartialPayload): PayrollStorageDto {
  const periods = parsed.donemler as PayrollStorageDto['donemler'];
  const bordrolar = (
    (parsed.bordrolar as Array<Partial<PayrollStorageDto['bordrolar'][number]>> | undefined) || []
  ).map(
    (bordro) => ({
      ...bordro,
      // V1 legacy records did not have a status. Normalize it once at the
      // explicit legacy/import boundary before the app uses the dataset.
      status: bordro.status || 'CALCULATED',
    })
  ) as PayrollStorageDto['bordrolar'];

  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    donemler: periods,
    aktifDonemId:
      typeof parsed.aktifDonemId === 'string'
        ? parsed.aktifDonemId
        : periods[0]?.id || '',
    personeller: parsed.personeller as PayrollStorageDto['personeller'],
    kurumDegerleriMap:
      (parsed.kurumDegerleriMap as PayrollStorageDto['kurumDegerleriMap'] | undefined) || {},
    puantajlar: (parsed.puantajlar as PayrollStorageDto['puantajlar'] | undefined) || [],
    bordrolar,
    taxOpenings: (parsed.taxOpenings as PayrollStorageDto['taxOpenings'] | undefined) || [],
    sickLeaveRecords:
      (parsed.sickLeaveRecords as PayrollStorageDto['sickLeaveRecords'] | undefined) || [],
    annualPayrollParameters:
      (parsed.annualPayrollParameters as PayrollStorageDto['annualPayrollParameters'] | undefined) || [],
    zamAylari: normalizeZamAylari(parsed.zamAylari),
  };
}

function isSupportedLegacyBackupRecord(parsed: Record<string, unknown>): boolean {
  try {
    const versionValue = parsed.backupVersion;
    if (
      versionValue !== undefined &&
      (typeof versionValue !== 'number' || !Number.isInteger(versionValue))
    ) {
      return false;
    }
    const version = typeof versionValue === 'number' ? versionValue : 1;
    if (version <= 0 || version > BACKUP_FORMAT_VERSION) return false;
    if (!Array.isArray(parsed.donemler) || !Array.isArray(parsed.personeller)) return false;
    return (
      version < BACKUP_FORMAT_VERSION ||
      REQUIRED_V2_COLLECTIONS.every((key) => Array.isArray(parsed[key]))
    );
  } catch {
    return false;
  }
}

/** Explicit structural/version predicate for legacy localStorage or imports. */
export function isSupportedLegacyBackupPayload(payload: string): boolean {
  try {
    return isSupportedLegacyBackupRecord(parseJsonObject(payload));
  } catch {
    return false;
  }
}

/** Compatibility alias retained for the browser migration owner. */
export const isMigratableBackupPayload = isSupportedLegacyBackupPayload;

/**
 * Parses the authoritative IndexedDB snapshot. This path is deliberately
 * strict and never calls the legacy numeric compatibility adapter.
 */
export function parseCurrentBrowserSnapshot(json: string): PayrollStorageDto {
  const parsed = parsePayrollStorage<PartialPayload>(json);
  validateBackupStructure(parsed);
  return toCanonicalBackupPayload(parsed);
}

/** Parses and canonicalizes a structurally supported legacy backup. */
export function parseLegacyBackup(json: string): PayrollStorageDto {
  const raw = parseJsonObject(json);
  if (!isSupportedLegacyBackupRecord(raw)) {
    throw new Error('Desteklenen legacy yedek yapısı bulunamadı.');
  }
  const parsed = parseLegacyPayrollStorage<PartialPayload>(json);
  validateBackupStructure(parsed);
  return toCanonicalBackupPayload(parsed);
}

type ParseAttempt<T> =
  | { ok: true; value: T }
  | { ok: false; error: unknown };

function tryParseCurrentBrowserSnapshot(json: string): ParseAttempt<PayrollStorageDto> {
  try {
    return { ok: true, value: parseCurrentBrowserSnapshot(json) };
  } catch (error) {
    return { ok: false, error };
  }
}

/**
 * User imports may use the strict current format or an explicitly supported
 * legacy format. Legacy parsing is reachable only after raw structure/version
 * classification, never as a generic strict-parser catch-all.
 */
export function parseImportedBackup(json: string): PayrollStorageDto {
  const raw = parseJsonObject(json);
  const version = getBackupVersion(raw);

  if (version === BACKUP_FORMAT_VERSION) {
    const strictResult = tryParseCurrentBrowserSnapshot(json);
    if (strictResult.ok) return strictResult.value;
    if ('error' in strictResult) {
      if (isSupportedLegacyBackupRecord(raw)) return parseLegacyBackup(json);
      throw strictResult.error;
    }
    throw new Error('Strict backup parser sonucu belirlenemedi.');
  }

  if (isSupportedLegacyBackupRecord(raw)) return parseLegacyBackup(json);
  return parseCurrentBrowserSnapshot(json);
}
