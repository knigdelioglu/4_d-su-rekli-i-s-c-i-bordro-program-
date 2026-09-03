import { BACKUP_FORMAT_VERSION } from '../../types/payroll';
import {
  parseLegacyPayrollStorage,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';
import {
  assertRecord,
  parseAndValidatePayrollPayload,
} from './payrollPayloadSchema';

type UnknownRecord = Record<string, unknown>;

const CURRENT_V2_ARRAY_FIELDS = [
  'donemler',
  'personeller',
  'puantajlar',
  'bordrolar',
  'taxOpenings',
  'sickLeaveRecords',
  'annualPayrollParameters',
  'zamAylari',
] as const;

function hasOwn(record: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}

function isRecord(value: unknown): value is UnknownRecord {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function parseJsonObject(json: string): UnknownRecord {
  const value: unknown = JSON.parse(json);
  assertRecord(value, '$');
  return value;
}

function getBackupVersion(parsed: UnknownRecord): number {
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

/**
 * Legacy classification only selects the explicit compatibility adapter. The
 * resulting canonical object is still required to pass the full V2 validator.
 */
function isLegacyCandidate(parsed: UnknownRecord): boolean {
  try {
    const version = getBackupVersion(parsed);
    if (!Array.isArray(parsed.donemler) || !Array.isArray(parsed.personeller)) return false;

    // A V2-shaped payload is only a legacy candidate when all fields that were
    // required by the V2 collection contract are present. Missing current V2
    // fields must not be repaired by this path.
    if (version === BACKUP_FORMAT_VERSION) {
      return CURRENT_V2_ARRAY_FIELDS.every((key) => Array.isArray(parsed[key])) &&
        hasOwn(parsed, 'exportedAt') &&
        typeof parsed.exportedAt === 'string' &&
        hasOwn(parsed, 'aktifDonemId') &&
        typeof parsed.aktifDonemId === 'string' &&
        hasOwn(parsed, 'kurumDegerleriMap') &&
        isRecord(parsed.kurumDegerleriMap);
    }

    return true;
  } catch {
    return false;
  }
}

function legacyValueOrDefault(
  parsed: UnknownRecord,
  key: string,
  defaultValue: unknown
): unknown {
  return hasOwn(parsed, key) ? parsed[key] : defaultValue;
}

function canonicalizeLegacyPersonel(value: unknown): unknown {
  if (!isRecord(value)) return value;

  const personel = { ...value };
  // These defaults mirror MigrationService::LegacyPersonel and keep the
  // supported V1 browser migration compatible with native migration.
  if (!hasOwn(personel, 'sgkSicilNo') || personel.sgkSicilNo === null) {
    personel.sgkSicilNo = '';
  }
  if (!hasOwn(personel, 'iban') || personel.iban === null) personel.iban = '';
  if (!hasOwn(personel, 'hizmetYili') || personel.hizmetYili === null) personel.hizmetYili = 1;
  return personel;
}

function firstPeriodId(value: unknown): string {
  if (!Array.isArray(value)) return '';
  const first = value[0];
  return isRecord(first) && typeof first.id === 'string' ? first.id : '';
}

function toCanonicalLegacyPayload(parsed: UnknownRecord): UnknownRecord {
  const periods = legacyValueOrDefault(parsed, 'donemler', []);
  const rawPayrolls = legacyValueOrDefault(parsed, 'bordrolar', []);
  const payrolls = Array.isArray(rawPayrolls)
    ? rawPayrolls.map((payroll) => {
        if (!isRecord(payroll)) return payroll;
        // V1 records did not have status. Only an absent field is defaulted;
        // explicit null/invalid values remain visible to the validator.
        return hasOwn(payroll, 'status')
          ? payroll
          : { ...payroll, status: 'CALCULATED' };
      })
    : rawPayrolls;

  const rawPersonnel = legacyValueOrDefault(parsed, 'personeller', []);
  const personnel = Array.isArray(rawPersonnel)
    ? rawPersonnel.map(canonicalizeLegacyPersonel)
    : rawPersonnel;

  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: legacyValueOrDefault(parsed, 'exportedAt', new Date().toISOString()),
    donemler: periods,
    aktifDonemId: legacyValueOrDefault(parsed, 'aktifDonemId', firstPeriodId(periods)),
    personeller: personnel,
    kurumDegerleriMap: legacyValueOrDefault(parsed, 'kurumDegerleriMap', {}),
    puantajlar: legacyValueOrDefault(parsed, 'puantajlar', []),
    bordrolar: payrolls,
    taxOpenings: legacyValueOrDefault(parsed, 'taxOpenings', []),
    sickLeaveRecords: legacyValueOrDefault(parsed, 'sickLeaveRecords', []),
    annualPayrollParameters: legacyValueOrDefault(parsed, 'annualPayrollParameters', []),
    zamAylari: legacyValueOrDefault(parsed, 'zamAylari', []),
  };
}

function parseLegacyBackupRecord(raw: UnknownRecord): PayrollStorageDto {
  if (!isLegacyCandidate(raw)) {
    throw new Error('Desteklenen legacy yedek yapısı bulunamadı.');
  }

  // This conversion is deliberately confined to the legacy adapter. No value
  // becomes authoritative until the canonical object passes the full schema.
  const encodedLegacy = parseLegacyPayrollStorage<unknown>(JSON.stringify(raw));
  if (!isRecord(encodedLegacy)) {
    throw new Error('Legacy yedek canonical nesne içermiyor.');
  }
  return parseAndValidatePayrollPayload(toCanonicalLegacyPayload(encodedLegacy));
}

/** Explicit structural/version predicate for legacy localStorage or imports. */
export function isSupportedLegacyBackupPayload(payload: string): boolean {
  try {
    parseLegacyBackupRecord(parseJsonObject(payload));
    return true;
  } catch {
    return false;
  }
}

/** Compatibility alias retained for the browser migration owner. */
export const isMigratableBackupPayload = isSupportedLegacyBackupPayload;

/**
 * Parses the authoritative IndexedDB snapshot. This path never calls the
 * legacy numeric compatibility adapter and never canonicalizes current data.
 */
export function parseCurrentBrowserSnapshot(json: string): PayrollStorageDto {
  const parsed: unknown = JSON.parse(json);
  return parseAndValidatePayrollPayload(parsed);
}

/** Parses and canonicalizes a structurally supported legacy backup. */
export function parseLegacyBackup(json: string): PayrollStorageDto {
  return parseLegacyBackupRecord(parseJsonObject(json));
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
 * legacy format. Legacy parsing is reachable only after raw version/shape
 * classification, never as a generic strict-parser catch-all.
 */
export function parseImportedBackup(json: string): PayrollStorageDto {
  const raw = parseJsonObject(json);
  const version = getBackupVersion(raw);

  if (version === BACKUP_FORMAT_VERSION) {
    const strictResult = tryParseCurrentBrowserSnapshot(json);
    if (strictResult.ok === false) {
      if (!isLegacyCandidate(raw)) throw strictResult.error;
      return parseLegacyBackupRecord(raw);
    }
    return strictResult.value;
  }

  if (isLegacyCandidate(raw)) return parseLegacyBackupRecord(raw);
  return parseCurrentBrowserSnapshot(json);
}
