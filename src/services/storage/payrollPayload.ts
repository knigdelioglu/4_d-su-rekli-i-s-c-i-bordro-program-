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

const LEGACY_PUANTAJ_OZETI_KEYS = ['Ç', 'T', 'G', 'İ', 'GÇ', 'GÇT', 'R'] as const;

const LEGACY_GELIR_OPTIONAL_DECIMAL_KEYS = [
  'tabanBrutAylik',
  'tediye',
  'tisIkramiyesi',
  'ekOdeme',
  'yemek',
  'birlestirilmisSosyalYardim',
  'vasitaYol',
  'giyimYardimi',
  'isPrimi',
  'geceCalismasiUcreti',
  'geceCalismasiTatiliUcreti',
  'hizmetZammi',
  'digerGelir',
] as const;

const LEGACY_KESINTI_OPTIONAL_DECIMAL_KEYS = [
  'isciSgkPrimi',
  'isciIssizlikPrimi',
  'gelirVergisi',
  'damgaVergisi',
  'sendikaAidati',
  'bes',
  'icra',
  'kisiBorcu',
  'dogumAskerlikBorclanmasi',
  'hayatSaglikSigortasi',
  'digerKesinti',
] as const;

const LEGACY_PEK_DEFAULT_DECIMAL_KEYS = [
  'hamPek',
  'devredenPekKullanilan',
  'primMatrahi',
  'altSinirTamamlamaFarki',
] as const;

const LEGACY_GV_DEFAULT_DECIMAL_KEYS = [
  'oncekiKumulatifGvMatrahi',
  'ayniAyOncekiKullanilanGvIstisnasi',
  'tahakkukOncesiKalanGvIstisnasi',
  'tahakkukSonrasiKalanGvIstisnasi',
  'dogumAskerlikGvIndirimi',
  'sigortaGvIndirimAdayi',
  'sigortaGvAylikLimiti',
  'sigortaGvYillikKalanLimiti',
  'uygulanabilirSigortaGvIndirimi',
] as const;

// MigrationService::import_payload uses AnnualPayrollParameters::default_for_2026
// for tax years that are absent from a legacy backup. Keep these as exact
// strings because this is persistence canonicalization, not calculation code.
const LEGACY_DEFAULT_ANNUAL_TAX_BRACKETS = [
  { limit: '190000', oran: '0.15' },
  { limit: '400000', oran: '0.20' },
  { limit: '1500000', oran: '0.27' },
  { limit: '5300000', oran: '0.35' },
  { limit: '1000000000000000', oran: '0.40' },
] as const;
const LEGACY_DEFAULT_ANNUAL_INSURANCE_CAP = '396360';

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
  // Native LegacyPayload stores this as Option<u32>; missing and explicit null
  // therefore both mean an unversioned legacy payload.
  if (versionValue === undefined || versionValue === null) return 1;
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
 * resulting canonical object is still required to pass the full current validator.
 */
function isLegacyCandidate(parsed: UnknownRecord): boolean {
  try {
    return getBackupVersion(parsed) < BACKUP_FORMAT_VERSION;
  } catch {
    return false;
  }
}

function legacyValueOrDefault(
  parsed: UnknownRecord,
  key: string,
  defaultValue: unknown
): unknown {
  return !hasOwn(parsed, key) || parsed[key] === null ? defaultValue : parsed[key];
}

function canonicalizeLegacyMissingFields(
  value: unknown,
  keys: readonly string[],
  defaultValue: unknown
): unknown {
  if (!isRecord(value)) return value;
  const record = { ...value };
  keys.forEach((key) => {
    if (!hasOwn(record, key)) record[key] = defaultValue;
  });
  return record;
}

function canonicalizeLegacyPuantajOzeti(value: unknown): unknown {
  return canonicalizeLegacyMissingFields(value, LEGACY_PUANTAJ_OZETI_KEYS, 0);
}

function canonicalizeLegacyDonem(value: unknown): unknown {
  if (!isRecord(value)) return value;
  const period = { ...value };
  if (!hasOwn(period, 'taxYear') || period.taxYear === null || typeof period.taxYear !== 'number') {
    period.taxYear = typeof period.yil === 'number' ? period.yil : new Date().getFullYear();
  }
  if (!hasOwn(period, 'taxMonth') || period.taxMonth === null || typeof period.taxMonth !== 'number') {
    const ay = typeof period.ay === 'number' ? period.ay : 1;
    period.taxMonth = ay + 1 > 12 ? 1 : ay + 1;
  }
  return period;
}

function legacyPaymentDate(periods: unknown, periodId: unknown): string {
  if (!Array.isArray(periods) || typeof periodId !== 'string') {
    return new Date().toISOString().slice(0, 10);
  }
  const period = periods.find(
    (candidate) => isRecord(candidate) && candidate.id === periodId
  );
  if (!isRecord(period) || typeof period.bitisTarihi !== 'string' || period.bitisTarihi.trim() === '') {
    return new Date().toISOString().slice(0, 10);
  }
  if (
    typeof period.taxYear !== 'number' ||
    !Number.isInteger(period.taxYear) ||
    typeof period.taxMonth !== 'number' ||
    !Number.isInteger(period.taxMonth) ||
    period.taxMonth < 1 ||
    period.taxMonth > 12
  ) {
    return period.bitisTarihi;
  }
  const taxPrefix = `${String(period.taxYear).padStart(4, '0')}-${String(period.taxMonth).padStart(2, '0')}-`;
  if (period.bitisTarihi.startsWith(taxPrefix)) return period.bitisTarihi;
  const monthEnd = new Date(Date.UTC(period.taxYear, period.taxMonth, 0));
  return Number.isNaN(monthEnd.getTime())
    ? period.bitisTarihi
    : monthEnd.toISOString().slice(0, 10);
}

function canonicalizeLegacyBordro(value: unknown, periods: unknown, version: number): unknown {
  if (!isRecord(value)) return value;

  const bordro = { ...value };
  // BordroKaydi.status has #[serde(default)] with BordroStatus::CALCULATED.
  if (!hasOwn(bordro, 'status')) bordro.status = 'CALCULATED';
  // V1/V2 had one record per person+period. It maps losslessly to the only
  // legal legacy accrual: one NORMAL node carrying the old record id/items.
  if (!hasOwn(bordro, 'accrualId') || bordro.accrualId === null || bordro.accrualId === '') {
    bordro.accrualId = typeof bordro.id === 'string' && bordro.id.trim() !== ''
      ? bordro.id
      : `${String(bordro.personelId || 'person')}_${String(bordro.donemId || 'period')}`;
  }
  // V1/V2 predate the multi-accrual contract and therefore have one
  // person+period NORMAL node. V3 already carries independent payment-event
  // identity; changing its type/sequence here would silently destroy a
  // retro/TEDIYE/NORMAL chain during V4 upgrade.
  if (version < 3) {
    bordro.accrualType = 'NORMAL';
    if (!hasOwn(bordro, 'paymentDate') || bordro.paymentDate === null || bordro.paymentDate === '') {
      bordro.paymentDate = legacyPaymentDate(periods, bordro.donemId);
    }
    bordro.sequence = 0;
  } else {
    if (!hasOwn(bordro, 'accrualType') || bordro.accrualType === null || bordro.accrualType === '') {
      bordro.accrualType = 'NORMAL';
    }
    if (!hasOwn(bordro, 'paymentDate') || bordro.paymentDate === null || bordro.paymentDate === '') {
      bordro.paymentDate = legacyPaymentDate(periods, bordro.donemId);
    }
    if (!hasOwn(bordro, 'sequence') || bordro.sequence === null || bordro.sequence === '') {
      bordro.sequence = 0;
    }
  }
  if (!hasOwn(bordro, 'accrualDescription')) {
    bordro.accrualDescription = hasOwn(bordro, 'notlar') ? bordro.notlar : null;
  }
  if (hasOwn(bordro, 'puantajOzeti')) {
    bordro.puantajOzeti = canonicalizeLegacyPuantajOzeti(bordro.puantajOzeti);
  }
  if (hasOwn(bordro, 'gelirler')) {
    bordro.gelirler = canonicalizeLegacyMissingFields(
      bordro.gelirler,
      LEGACY_GELIR_OPTIONAL_DECIMAL_KEYS,
      null
    );
  }
  if (hasOwn(bordro, 'kesintiler')) {
    bordro.kesintiler = canonicalizeLegacyMissingFields(
      bordro.kesintiler,
      LEGACY_KESINTI_OPTIONAL_DECIMAL_KEYS,
      null
    );
  }
  if (isRecord(bordro.pekDetay)) {
    bordro.pekDetay = canonicalizeLegacyMissingFields(
      bordro.pekDetay,
      LEGACY_PEK_DEFAULT_DECIMAL_KEYS,
      '0'
    );
  }
  if (isRecord(bordro.gvDetay)) {
    bordro.gvDetay = canonicalizeLegacyMissingFields(
      bordro.gvDetay,
      LEGACY_GV_DEFAULT_DECIMAL_KEYS,
      '0'
    );
  }
  return bordro;
}

function canonicalizeLegacyIsPrimiGroups(value: unknown): unknown {
  if (!Array.isArray(value)) return value;
  return value.map((item) => {
    if (!isRecord(item)) return item;
    return hasOwn(item, 'aktif') ? item : { ...item, aktif: true };
  });
}

function canonicalizeLegacyInstitutionSettings(value: unknown): unknown {
  if (!isRecord(value)) return value;

  return Object.fromEntries(
    Object.entries(value).map(([periodId, rawSettings]) => {
      if (!isRecord(rawSettings)) return [periodId, rawSettings];
      const settings = { ...rawSettings };
      // Native migration deserializes the value first, then assigns the map
      // key to donemId. Preserve invalid/missing types for the full validator.
      if (typeof settings.donemId === 'string') settings.donemId = periodId;
      if (hasOwn(settings, 'isPrimiGruplari')) {
        settings.isPrimiGruplari = canonicalizeLegacyIsPrimiGroups(settings.isPrimiGruplari);
      }
      return [periodId, settings];
    })
  );
}

function canonicalizeLegacyAnnualParameter(value: unknown): unknown {
  if (!isRecord(value)) return value;
  const parameters = { ...value };
  // AnnualPayrollParametersRepository fills the 2026 cap when a legacy
  // Option<Decimal> is missing/None and the row is read back.
  if (
    parameters.year === 2026 &&
    (!hasOwn(parameters, 'sigortaGvYillikBrutAsgariUcretTavani') ||
      parameters.sigortaGvYillikBrutAsgariUcretTavani === null)
  ) {
    parameters.sigortaGvYillikBrutAsgariUcretTavani = LEGACY_DEFAULT_ANNUAL_INSURANCE_CAP;
  }
  return parameters;
}

function defaultLegacyAnnualParameter(year: number): UnknownRecord {
  return {
    year,
    gelirVergisiDilimleri: LEGACY_DEFAULT_ANNUAL_TAX_BRACKETS.map((bracket) => ({ ...bracket })),
    sigortaGvYillikBrutAsgariUcretTavani: LEGACY_DEFAULT_ANNUAL_INSURANCE_CAP,
  };
}

function canonicalizeLegacyAnnualParameters(value: unknown, periods: unknown): unknown {
  if (!Array.isArray(value) || !Array.isArray(periods)) return value;

  const parameters = value.map(canonicalizeLegacyAnnualParameter);
  const importedTaxYears = new Set<number>();
  periods.forEach((period) => {
    if (!isRecord(period) || typeof period.taxYear !== 'number' || !Number.isInteger(period.taxYear)) {
      return;
    }
    importedTaxYears.add(period.taxYear);
  });

  const parameterYears = new Set<number>();
  parameters.forEach((parameter) => {
    if (!isRecord(parameter) || typeof parameter.year !== 'number' || !Number.isInteger(parameter.year)) {
      return;
    }
    parameterYears.add(parameter.year);
  });

  importedTaxYears.forEach((year) => {
    if (!parameterYears.has(year)) parameters.push(defaultLegacyAnnualParameter(year));
  });
  return parameters;
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

function legacyAmountIsNegative(value: unknown): boolean {
  return (typeof value === 'string' && value.trim().startsWith('-')) ||
    (typeof value === 'number' && value < 0);
}

function canonicalizeLegacyRetroBatches(value: unknown, allocations: unknown): unknown {
  if (!Array.isArray(value)) return value;
  return value.map((item) => {
    if (!isRecord(item)) return item;
    const batch = { ...item };
    const status = batch.status ?? 'DRAFT';
    if (!hasOwn(batch, 'status') || batch.status === null) batch.status = status;
    if (!hasOwn(batch, 'settlementStatus') || batch.settlementStatus === null) {
      const batchAllocations = Array.isArray(allocations)
        ? allocations.filter((allocation) =>
            isRecord(allocation) && allocation.batchId === batch.id
          )
        : [];
      const hasNegativeDelta = legacyAmountIsNegative(batch.totalGrossDelta) ||
        batchAllocations.some((allocation) =>
          isRecord(allocation) && legacyAmountIsNegative(allocation.deltaAmount)
        );
      batch.settlementStatus = hasNegativeDelta
        ? 'OVERPAYMENT'
        : status === 'FINALIZED'
          ? 'PAID'
          : 'UNSETTLED';
    }
    return batch;
  });
}

function firstPeriodId(value: unknown): string {
  if (!Array.isArray(value)) return '';
  const first = value[0];
  return isRecord(first) && typeof first.id === 'string' ? first.id : '';
}

function toCanonicalLegacyPayload(
  parsed: UnknownRecord,
  pruneDanglingReferences: boolean
): UnknownRecord {
  const version = getBackupVersion(parsed);
  const rawPeriods = legacyValueOrDefault(parsed, 'donemler', []);
  const periods = Array.isArray(rawPeriods)
    ? rawPeriods.map(canonicalizeLegacyDonem)
    : rawPeriods;

  const rawPersonnel = legacyValueOrDefault(parsed, 'personeller', []);
  const personnel = Array.isArray(rawPersonnel)
    ? rawPersonnel.map(canonicalizeLegacyPersonel)
    : rawPersonnel;

  const validPersonnelIds = new Set(
    Array.isArray(personnel)
      ? personnel.map((p) => (isRecord(p) && typeof p.id === 'string' ? p.id : '')).filter(Boolean)
      : []
  );
  const validPeriodIds = new Set(
    Array.isArray(periods)
      ? periods.map((p) => (isRecord(p) && typeof p.id === 'string' ? p.id : '')).filter(Boolean)
      : []
  );

  const rawPayrolls = legacyValueOrDefault(parsed, 'bordrolar', []);
  const payrolls = Array.isArray(rawPayrolls)
      ? rawPayrolls
        .filter(
          (payroll) =>
            !pruneDanglingReferences ||
            !isRecord(payroll) ||
            ((!validPersonnelIds.size || validPersonnelIds.has(String(payroll.personelId))) &&
              (!validPeriodIds.size || validPeriodIds.has(String(payroll.donemId))))
        )
        .map((payroll) => canonicalizeLegacyBordro(payroll, periods, version))
    : rawPayrolls;

  const rawPuantajlar = legacyValueOrDefault(parsed, 'puantajlar', []);
  const puantajlar = Array.isArray(rawPuantajlar)
      ? rawPuantajlar.filter(
        (pj) =>
          !pruneDanglingReferences ||
          !isRecord(pj) ||
          ((!validPersonnelIds.size || validPersonnelIds.has(String(pj.personelId))) &&
            (!validPeriodIds.size || validPeriodIds.has(String(pj.donemId))))
      )
    : rawPuantajlar;

  const rawTaxOpenings = legacyValueOrDefault(parsed, 'taxOpenings', []);
  const taxOpenings = Array.isArray(rawTaxOpenings)
      ? rawTaxOpenings.filter(
        (item) =>
          !pruneDanglingReferences ||
          !isRecord(item) ||
          ((!validPersonnelIds.size || validPersonnelIds.has(String(item.personnelId))) &&
            (!validPeriodIds.size || validPeriodIds.has(String(item.effectiveFromPeriodId))))
      )
    : rawTaxOpenings;

  const rawSickLeaves = legacyValueOrDefault(parsed, 'sickLeaveRecords', []);
  const sickLeaveRecords = Array.isArray(rawSickLeaves)
      ? rawSickLeaves.filter(
        (item) =>
          !pruneDanglingReferences ||
          !isRecord(item) ||
          !validPersonnelIds.size ||
          validPersonnelIds.has(String(item.personnelId))
      )
    : rawSickLeaves;

  const annualPayrollParameters = canonicalizeLegacyAnnualParameters(
    legacyValueOrDefault(parsed, 'annualPayrollParameters', []),
    periods
  );

  const compensationRevisions = legacyValueOrDefault(parsed, 'compensationRevisions', []);
  const compensationRevisionOverrides = legacyValueOrDefault(
    parsed,
    'compensationRevisionOverrides',
    []
  );
  const retroAllocations = legacyValueOrDefault(parsed, 'retroAllocations', []);
  const retroBatches = canonicalizeLegacyRetroBatches(
    legacyValueOrDefault(parsed, 'retroBatches', []),
    retroAllocations
  );

  let aktifDonemId = legacyValueOrDefault(parsed, 'aktifDonemId', firstPeriodId(periods));
  if (
    pruneDanglingReferences &&
    (typeof aktifDonemId !== 'string' ||
      (validPeriodIds.size > 0 && !validPeriodIds.has(aktifDonemId)))
  ) {
    aktifDonemId = firstPeriodId(periods);
  }

  return {
    backupVersion: BACKUP_FORMAT_VERSION,
    exportedAt: legacyValueOrDefault(parsed, 'exportedAt', new Date().toISOString()),
    donemler: periods,
    aktifDonemId,
    personeller: personnel,
    kurumDegerleriMap: canonicalizeLegacyInstitutionSettings(
      legacyValueOrDefault(parsed, 'kurumDegerleriMap', {})
    ),
    puantajlar,
    bordrolar: payrolls,
    taxOpenings,
    sickLeaveRecords,
    annualPayrollParameters,
    zamAylari: legacyValueOrDefault(parsed, 'zamAylari', []),
    compensationRevisions,
    compensationRevisionOverrides,
    retroBatches,
    retroAllocations,
  };
}

export function parseLegacyBackupRecord(raw: UnknownRecord): PayrollStorageDto {
  if (!isLegacyCandidate(raw)) {
    throw new Error('Desteklenen legacy yedek yapısı bulunamadı.');
  }

  // V2 was the previous exact-Decimal snapshot format. It is legacy with
  // respect to V4's accrual metadata, but it must not pass through the V1
  // numeric-repair adapter: a numeric V2 value is still an invalid Decimal.
  const version = getBackupVersion(raw);
  if (version >= 2 && (!hasOwn(raw, 'bordrolar') || !hasOwn(raw, 'personeller'))) {
    throw new Error('Legacy yedek bordro ve personel koleksiyonlarını içermelidir.');
  }

  // This conversion is deliberately confined to the legacy adapter. No value
  // becomes authoritative until the canonical object passes the full schema.
  const encodedLegacy = version === 2
    ? raw
    : parseLegacyPayrollStorage<unknown>(JSON.stringify(raw));
  if (!isRecord(encodedLegacy)) {
    throw new Error('Legacy yedek canonical nesne içermiyor.');
  }
  return parseAndValidatePayrollPayload(toCanonicalLegacyPayload(encodedLegacy, false));
}

/**
 * Emergency repair helper for damaged, pre-v3, or numeric-tainted payloads in browser storage.
 * Coerces numbers into Decimal strings, fills missing fields, and prunes dangling references.
 */
export function repairAndCanonicalizeBackup(raw: UnknownRecord): PayrollStorageDto {
  const converted = parseLegacyPayrollStorage<unknown>(JSON.stringify(raw));
  if (!isRecord(converted)) {
    throw new Error('Kurtarılacak veri geçerli bir nesne değil.');
  }
  return parseAndValidatePayrollPayload(toCanonicalLegacyPayload(converted, true));
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

/**
 * User imports may use the strict current format or an explicitly supported
 * legacy format. Legacy parsing is reachable only after raw version
 * classification, never as a generic strict-parser catch-all.
 */
export function parseImportedBackup(json: string): PayrollStorageDto {
  const raw = parseJsonObject(json);
  const version = getBackupVersion(raw);

  if (version === BACKUP_FORMAT_VERSION) {
    // A versioned V4 backup is current data. It must never reach the legacy
    // repair path, even when its malformed values resemble an older backup.
    return parseCurrentBrowserSnapshot(json);
  }

  return parseLegacyBackupRecord(raw);
}
