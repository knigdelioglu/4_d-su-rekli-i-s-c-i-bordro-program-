import { BACKUP_FORMAT_VERSION, BORDRO_STATUS_VALUES } from '../../types/payroll';
import {
  assertExactDecimalDto,
  isExactDecimalString,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';

type UnknownRecord = Record<string, unknown>;

const PUANTAJ_OZETI_KEYS = ['Ç', 'T', 'G', 'İ', 'GÇ', 'GÇT', 'R'] as const;
const ACCRUAL_TYPE_VALUES = [
  'NORMAL',
  'TEDIYE',
  'TIS_IKRAMIYE',
  'SUPPLEMENTAL',
  'RETRO_ADJUSTMENT',
] as const;
const COMPENSATION_REVISION_REASON_VALUES = [
  'COLLECTIVE_AGREEMENT',
  'ADMINISTRATIVE_DECISION',
  'COURT_DECISION',
  'PAY_CORRECTION',
  'MISSING_ACCRUAL',
  'OTHER',
] as const;
const COMPENSATION_REVISION_STATUS_VALUES = BORDRO_STATUS_VALUES;
const COMPENSATION_REVISION_SCOPE_VALUES = [
  'ALL_PERSONNEL',
  'SELECTED_PERSONNEL',
  'PERSONNEL_GROUP',
] as const;
const RETRO_PARAMETER_VALUES = [
  'GUNLUK_TABAN_UCRET',
  'GUNLUK_YEMEK',
  'BIRLESTIRILMIS_SOSYAL_YARDIM',
  'GUNLUK_VASITA_YOL',
  'GIYIM_YARDIMI',
  'HIZMET_ZAMMI_BIRIMI',
  'IS_PRIMI_YUZDE',
  'GECE_CALISMA_PRIMI_YUZDE',
  'GECE_CALISMA_TATILI_PRIMI_YUZDE',
  'EK_ODEME',
  'DIGER_GELIR',
  'TEDIYE',
  'TIS_BONUS',
] as const;
const RETRO_EARNING_CODE_VALUES = [
  'BASE_WAGE',
  'NIGHT_WORK',
  'NIGHT_HOLIDAY',
  'WORK_PREMIUM',
  'SOCIAL_AID',
  'MEAL',
  'TRANSPORT',
  'CLOTHING',
  'SERVICE_INCREMENT',
  'TIS_BONUS',
  'TEDIYE',
  'SUPPLEMENTAL',
  'OTHER',
] as const;
const RETRO_TAX_TREATMENT_VALUES = ['TAXABLE', 'EXEMPT'] as const;
const RETRO_SETTLEMENT_STATUS_VALUES = ['UNSETTLED', 'PAID', 'OVERPAYMENT'] as const;
const RETRO_SGK_TREATMENT_VALUES = [
  'WAGE_SOURCE_MONTH',
  'NON_WAGE_PAYMENT_MONTH',
  'NON_WAGE_CARRY',
  'EXEMPT',
] as const;
const RETRO_POLICY_BY_EARNING_CODE: Record<
  (typeof RETRO_EARNING_CODE_VALUES)[number],
  { sgkTreatment: (typeof RETRO_SGK_TREATMENT_VALUES)[number]; taxTreatment: (typeof RETRO_TAX_TREATMENT_VALUES)[number] }
> = {
  BASE_WAGE: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  NIGHT_WORK: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  NIGHT_HOLIDAY: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  WORK_PREMIUM: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  SOCIAL_AID: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  MEAL: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  TRANSPORT: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  CLOTHING: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  SERVICE_INCREMENT: { sgkTreatment: 'WAGE_SOURCE_MONTH', taxTreatment: 'TAXABLE' },
  TIS_BONUS: { sgkTreatment: 'NON_WAGE_PAYMENT_MONTH', taxTreatment: 'TAXABLE' },
  TEDIYE: { sgkTreatment: 'NON_WAGE_PAYMENT_MONTH', taxTreatment: 'TAXABLE' },
  SUPPLEMENTAL: { sgkTreatment: 'NON_WAGE_PAYMENT_MONTH', taxTreatment: 'TAXABLE' },
  OTHER: { sgkTreatment: 'NON_WAGE_PAYMENT_MONTH', taxTreatment: 'TAXABLE' },
};
const STATUTORY_SNAPSHOT_SOURCE_VALUES = [
  'ATTENDANCE_BACKED',
  'PROVISIONAL_PAYMENT_MONTH',
  'LEGACY_UNKNOWN',
] as const;

function hasOwn(record: UnknownRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key);
}

function fieldPath(path: string, key: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key)
    ? `${path}.${key}`
    : `${path}[${JSON.stringify(key)}]`;
}

function fail(path: string, message: string): never {
  throw new Error(`${path} ${message}`);
}

function isPlainRecord(value: unknown): value is UnknownRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

export function assertRecord(value: unknown, path = '$'): asserts value is UnknownRecord {
  if (!isPlainRecord(value)) fail(path, 'plain object olmalıdır.');
}

export function assertString(value: unknown, path: string): asserts value is string {
  if (typeof value !== 'string') fail(path, 'string olmalıdır.');
}

export function assertInteger(value: unknown, path: string): asserts value is number {
  if (typeof value !== 'number' || !Number.isInteger(value)) {
    fail(path, 'tam sayı olmalıdır.');
  }
}

export function assertBoolean(value: unknown, path: string): asserts value is boolean {
  if (typeof value !== 'boolean') fail(path, 'boolean olmalıdır.');
}

export function assertArray(value: unknown, path: string): asserts value is unknown[] {
  if (!Array.isArray(value)) fail(path, 'array olmalıdır.');
}

function requiredEnum<T extends string>(
  record: UnknownRecord,
  key: string,
  values: readonly T[],
  path: string
): void {
  const value = required(record, key, path);
  assertString(value, fieldPath(path, key));
  if (!values.includes(value as T)) {
    fail(fieldPath(path, key), `desteklenmeyen enum değeri: ${value}.`);
  }
}

function optionalEnum<T extends string>(
  record: UnknownRecord,
  key: string,
  values: readonly T[],
  path: string
): void {
  const value = optional(record, key, path);
  if (value === undefined || value === null) return;
  assertString(value, fieldPath(path, key));
  if (!values.includes(value as T)) {
    fail(fieldPath(path, key), `desteklenmeyen enum değeri: ${value}.`);
  }
}

function required(record: UnknownRecord, key: string, path: string): unknown {
  const propertyPath = fieldPath(path, key);
  if (!hasOwn(record, key)) fail(propertyPath, 'zorunlu alan eksik.');
  return record[key];
}

function optional(record: UnknownRecord, key: string, path: string): unknown {
  return hasOwn(record, key) ? record[key] : undefined;
}

function requiredString(record: UnknownRecord, key: string, path: string): void {
  assertString(required(record, key, path), fieldPath(path, key));
}

function optionalNullableString(record: UnknownRecord, key: string, path: string): void {
  const value = optional(record, key, path);
  if (value !== undefined && value !== null) assertString(value, fieldPath(path, key));
}

function requiredInteger(record: UnknownRecord, key: string, path: string): void {
  assertInteger(required(record, key, path), fieldPath(path, key));
}

function requiredNonNegativeInteger(record: UnknownRecord, key: string, path: string): void {
  requiredInteger(record, key, path);
  if ((record[key] as number) < 0) fail(fieldPath(path, key), 'negatif olamaz.');
}

function requiredIsoDate(record: UnknownRecord, key: string, path: string): void {
  const value = required(record, key, path);
  assertString(value, fieldPath(path, key));
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    fail(fieldPath(path, key), 'YYYY-AA-GG biçiminde tarih olmalıdır.');
  }
  const date = new Date(`${value}T00:00:00.000Z`);
  if (Number.isNaN(date.getTime()) || date.toISOString().slice(0, 10) !== value) {
    fail(fieldPath(path, key), 'geçerli bir takvim tarihi olmalıdır.');
  }
}

/** Checks presence/nullability; the exact Decimal grammar is checked once at the payload root. */
function requiredDecimal(record: UnknownRecord, key: string, path: string): void {
  const value = required(record, key, path);
  if (value === null || value === undefined) {
    fail(fieldPath(path, key), 'zorunlu Decimal değeri null veya undefined olamaz.');
  }
}

/** A required nullable Decimal is still required to have an own JSON property. */
function requiredNullableDecimal(record: UnknownRecord, key: string, path: string): void {
  const value = required(record, key, path);
  if (value === undefined) fail(fieldPath(path, key), 'Decimal alanı undefined olamaz.');
}

/** Rust Option<Decimal> fields may be omitted by old snapshots or serialized as null. */
function optionalDecimal(record: UnknownRecord, key: string, path: string): void {
  optional(record, key, path);
}

/** Decimal fields with serde(default) are optional for old snapshots but non-null when present. */
function optionalNonNullableDecimal(record: UnknownRecord, key: string, path: string): void {
  const value = optional(record, key, path);
  if (value === null) fail(fieldPath(path, key), 'Decimal alanı null olamaz.');
}

function optionalBoolean(record: UnknownRecord, key: string, path: string): void {
  const value = optional(record, key, path);
  if (value !== undefined) assertBoolean(value, fieldPath(path, key));
}

function optionalNullableBoolean(record: UnknownRecord, key: string, path: string): void {
  const value = optional(record, key, path);
  if (value !== undefined && value !== null) assertBoolean(value, fieldPath(path, key));
}

function optionalNullableInteger(record: UnknownRecord, key: string, path: string): void {
  const value = optional(record, key, path);
  if (value !== undefined && value !== null) assertInteger(value, fieldPath(path, key));
}

function optionalNullableRecord(
  record: UnknownRecord,
  key: string,
  path: string,
  validator: (value: unknown, childPath: string) => void
): void {
  const value = optional(record, key, path);
  if (value === undefined || value === null) return;
  const childPath = fieldPath(path, key);
  assertRecord(value, childPath);
  validator(value, childPath);
}

function optionalNullableArray(
  record: UnknownRecord,
  key: string,
  path: string,
  validator: (value: unknown, childPath: string) => void
): void {
  const value = optional(record, key, path);
  if (value === undefined || value === null) return;
  const childPath = fieldPath(path, key);
  assertArray(value, childPath);
  value.forEach((item, index) => validator(item, `${childPath}[${index}]`));
}

function validateGvIndirimGirdileri(value: unknown, path: string): void {
  assertRecord(value, path);
  optionalDecimal(value, 'dogumAskerlikGvIndirimTutar', path);
  optionalDecimal(value, 'hayatSigortasiPrimiTutar', path);
  optionalDecimal(value, 'saglikSigortasiPrimiTutar', path);
}

function validatePersonelKesintileri(value: unknown, path: string): void {
  assertRecord(value, path);
  const optionalBooleanFields = ['sendikaUyesi', 'besUyesi'] as const;
  optionalBooleanFields.forEach((key) => optionalNullableBoolean(value, key, path));
  [
    'sabitSendikaAidati',
    'oksOraniYuzde',
    'sabitBesTutar',
    'icraTutar',
    'kisiBorcuTutar',
    'dogumAskerlikBorclanmasiTutar',
    'hayatSaglikSigortasiTutar',
    'digerKesintiTutar',
  ].forEach((key) => optionalDecimal(value, key, path));
  optionalNullableRecord(value, 'gvIndirimleri', path, validateGvIndirimGirdileri);
}

export function validatePersonel(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['personeller'][number] {
  assertRecord(value, path);
  ['id', 'tcNo', 'ad', 'soyad', 'grup', 'sgkSicilNo', 'iban'].forEach((key) =>
    requiredString(value, key, path)
  );
  requiredInteger(value, 'hizmetYili', path);
  optionalNullableString(value, 'unvan', path);
  optionalNullableString(value, 'aciklama', path);
  [
    'devirKumulatifGvMatrahi',
    'devirKumulatifAsgariGvMatrahi',
  ].forEach((key) => optionalDecimal(value, key, path));
  [
    'devirKumulatifGvMatrahiYili',
    'devirKumulatifGvMatrahiBaslangicAyi',
    'devirKumulatifAsgariGvMatrahiYili',
  ].forEach((key) => optionalNullableInteger(value, key, path));
  optionalNullableRecord(value, 'kesintiler', path, validatePersonelKesintileri);
}

export function validateBordroDonemi(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['donemler'][number] {
  assertRecord(value, path);
  requiredString(value, 'id', path);
  ['yil', 'ay', 'taxYear', 'taxMonth'].forEach((key) => requiredInteger(value, key, path));
  ['baslangicTarihi', 'bitisTarihi', 'donemAdi'].forEach((key) =>
    requiredString(value, key, path)
  );
}

export function validatePuantaj(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['puantajlar'][number] {
  assertRecord(value, path);
  ['id', 'personelId', 'donemId'].forEach((key) => requiredString(value, key, path));
  const gunlerPath = fieldPath(path, 'gunler');
  const gunler = required(value, 'gunler', path);
  assertRecord(gunler, gunlerPath);
  Object.entries(gunler).forEach(([key, item]) => assertString(item, fieldPath(gunlerPath, key)));
}

function validatePuantajOzeti(value: unknown, path: string): void {
  assertRecord(value, path);
  PUANTAJ_OZETI_KEYS.forEach((key) => requiredInteger(value, key, path));
}

function validateGelirKalemleri(value: unknown, path: string): void {
  assertRecord(value, path);
  [
    'tabanBrutAylik',
    'tediye',
    'tisIkramiyesi',
    'ekOdeme',
    'yemek',
    'birlestirilmisSosyalYardim',
    'vasitaYol',
    'giyimYardimi',
    'isPrimi',
    'hizmetZammi',
    'digerGelir',
  ].forEach((key) => requiredNullableDecimal(value, key, path));
  ['geceCalismasiUcreti', 'geceCalismasiTatiliUcreti'].forEach((key) =>
    optionalDecimal(value, key, path)
  );
}

function validateKesintiKalemleri(value: unknown, path: string): void {
  assertRecord(value, path);
  [
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
  ].forEach((key) => requiredNullableDecimal(value, key, path));
}

function validateDevredenPekKaydi(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredDecimal(value, 'tutar', path);
  requiredInteger(value, 'kalanAySayisi', path);
  optionalNullableString(value, 'kaynakDonemId', path);
}

function validatePekDetayi(value: unknown, path: string): void {
  assertRecord(value, path);
  [
    'hesaplananPek',
    'finalPek',
    'devredenPekAşanTutar',
    'pekAltSinir',
    'pekUstSinir',
    'yemekIstisnasiTutar',
  ].forEach((key) => requiredDecimal(value, key, path));
  ['hamPek', 'devredenPekKullanilan', 'primMatrahi', 'altSinirTamamlamaFarki'].forEach((key) =>
    optionalNonNullableDecimal(value, key, path)
  );
  requiredInteger(value, 'fiiliYemekGunu', path);
  [
    'isverenSgkPrimi',
    'isverenIssizlikPrimi',
    'pekAltSinirTamamlamaIsverenPrimi',
    'isverenPrimToplami',
    'sgkIsverenOraniYuzde',
    'isverenIssizlikOraniYuzde',
  ].forEach((key) => optionalDecimal(value, key, path));
}

function validateIsPrimiHesapDetayi(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredString(value, 'grupId', path);
  requiredString(value, 'grupAd', path);
  requiredDecimal(value, 'oran', path);
  requiredInteger(value, 'hakGunu', path);
  requiredDecimal(value, 'gunlukIsPrimi', path);
  requiredDecimal(value, 'tutar', path);
}

function validateGvHesapDetayi(value: unknown, path: string): void {
  assertRecord(value, path);
  [
    'cariGvMatrahi',
    'yeniKumulatifGvMatrahi',
    'brutGelirVergisi',
    'asgariUcretGvMatrahi',
    'asgariUcretReferansKumulatifMatrahi',
    'asgariUcretGvIstisnasi',
    'uygulananGvIstisnasi',
    'kesilenGelirVergisi',
  ].forEach((key) => requiredDecimal(value, key, path));
  [
    'oncekiKumulatifGvMatrahi',
    'ayniAyOncekiKullanilanGvIstisnasi',
    'tahakkukOncesiKalanGvIstisnasi',
    'tahakkukSonrasiKalanGvIstisnasi',
    'dogumAskerlikGvIndirimi',
    'sigortaGvIndirimAdayi',
    'sigortaGvAylikLimiti',
    'sigortaGvYillikKalanLimiti',
    'uygulanabilirSigortaGvIndirimi',
  ].forEach((key) => optionalNonNullableDecimal(value, key, path));
}

function validateDamgaVergisiHesapDetayi(value: unknown, path: string): void {
  assertRecord(value, path);
  [
    'brutDamgaVergisi',
    'aylikDamgaIstisnaHakki',
    'ayniAyOncekiKullanilanDamgaIstisnasi',
    'uygulananDamgaIstisnasi',
    'kalanDamgaIstisnasi',
    'kesilenDamgaVergisi',
  ].forEach((key) => requiredDecimal(value, key, path));
}

function validateStatutoryParameterSegment(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredString(value, 'effectiveFrom', path);
  [
    'gunlukAsgariUcret',
    'pekTavanKatsayisi',
    'gunlukYemekIstisnasiSGK',
    'gunlukYemekIstisnasiGV',
  ].forEach((key) => optionalDecimal(value, key, path));
}

function validateResolvedStatutorySegmentSnapshot(value: unknown, path: string): void {
  assertRecord(value, path);
  ['effectiveFrom', 'effectiveTo'].forEach((key) => requiredString(value, key, path));
  ['sgkPrimGunSayisi', 'fiiliYemekGunu'].forEach((key) => requiredInteger(value, key, path));
  [
    'gunlukAsgariUcret',
    'pekTavanKatsayisi',
    'gunlukYemekIstisnasiSGK',
    'gunlukYemekIstisnasiGV',
  ].forEach((key) => requiredDecimal(value, key, path));
}

function validateResolvedStatutorySnapshot(value: unknown, path: string): void {
  assertRecord(value, path);
  const source = optional(value, 'source', path);
  if (source !== undefined) {
    assertString(source, fieldPath(path, 'source'));
    if (!STATUTORY_SNAPSHOT_SOURCE_VALUES.includes(source as (typeof STATUTORY_SNAPSHOT_SOURCE_VALUES)[number])) {
      fail(fieldPath(path, 'source'), 'geçerli bir statutory snapshot kaynağı olmalıdır.');
    }
  }
  const segmentsPath = fieldPath(path, 'segments');
  const segments = required(value, 'segments', path);
  assertArray(segments, segmentsPath);
  segments.forEach((item, index) =>
    validateResolvedStatutorySegmentSnapshot(item, `${segmentsPath}[${index}]`)
  );
  requiredInteger(value, 'sgkPrimGunSayisi', path);
  [
    'pekAltSinir',
    'pekUstSinir',
    'sgkYemekIstisnasiToplam',
    'gvYemekIstisnasiToplam',
    'gvReferansGunlukAsgariUcret',
  ].forEach((key) => requiredDecimal(value, key, path));
}

function validateTediyeKalemi(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredInteger(value, 'id', path);
  ['ad', 'odemeAyi'].forEach((key) => requiredString(value, key, path));
  requiredInteger(value, 'gunSayisi', path);
  requiredBoolean(value, 'aktifDonemdeOdensin', path);
  optionalDecimal(value, 'sabitTutar', path);
}

function requiredBoolean(record: UnknownRecord, key: string, path: string): void {
  assertBoolean(required(record, key, path), fieldPath(path, key));
}

function validateTisIkramiyeKalemi(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredInteger(value, 'id', path);
  ['ad', 'odemeAyi'].forEach((key) => requiredString(value, key, path));
  requiredInteger(value, 'gunSayisi', path);
  requiredBoolean(value, 'aktifDonemdeOdensin', path);
  optionalDecimal(value, 'sabitTutar', path);
}

function validateIsPrimiGrupItem(value: unknown, path: string): void {
  assertRecord(value, path);
  ['id', 'ad'].forEach((key) => requiredString(value, key, path));
  requiredDecimal(value, 'oran', path);
  optionalBoolean(value, 'aktif', path);
}

export function validateInstitutionSettings(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['kurumDegerleriMap'][string] {
  assertRecord(value, path);
  requiredString(value, 'donemId', path);
  [
    'gunlukTabanUcret',
    'gunlukYemek',
    'birlestirilmisSosyalYardim',
    'gunlukVasitaYol',
    'giyimYardimi',
    'hizmetZammiBirimi',
  ].forEach((key) => requiredDecimal(value, key, path));
  [
    'isPrimiYuzde',
    'geceCalismaPrimiYuzde',
    'geceCalismaTatiliPrimiYuzde',
    'ekOdeme',
    'digerGelirVarsayilan',
    'sgkIsciOraniYuzde',
    'issizlikIsciOraniYuzde',
    'gelirVergisiOraniYuzde',
    'damgaVergisiOraniBinde',
    'sendikaAidatiYuzde',
    'sabitSendikaAidati',
    'besOraniYuzde',
    'sabitBesTutar',
    'gunlukYemekIstisnasiSGK',
    'gunlukYemekIstisnasiGV',
    'pekTavanKatsayisi',
    'gunlukAsgariUcret',
    'sgkIsverenOraniYuzde',
    'issizlikIsverenOraniYuzde',
  ].forEach((key) => optionalDecimal(value, key, path));
  optionalNullableArray(value, 'isPrimiGruplari', path, validateIsPrimiGrupItem);
  optionalNullableArray(value, 'tediyeListesi', path, validateTediyeKalemi);
  optionalNullableArray(value, 'tisIkramiyeListesi', path, validateTisIkramiyeKalemi);
  optionalNullableString(value, 'tediyeTisNotu', path);
  optionalNullableArray(value, 'statutoryParameterSegments', path, validateStatutoryParameterSegment);
}

function validateTaxBracket(value: unknown, path: string): void {
  assertRecord(value, path);
  requiredDecimal(value, 'limit', path);
  requiredDecimal(value, 'oran', path);
}

export function validateAnnualPayrollParameters(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['annualPayrollParameters'][number] {
  assertRecord(value, path);
  requiredInteger(value, 'year', path);
  const bracketsPath = fieldPath(path, 'gelirVergisiDilimleri');
  const brackets = required(value, 'gelirVergisiDilimleri', path);
  assertArray(brackets, bracketsPath);
  brackets.forEach((item, index) => validateTaxBracket(item, `${bracketsPath}[${index}]`));
  optionalDecimal(value, 'sigortaGvYillikBrutAsgariUcretTavani', path);
  optionalNullableString(value, 'updatedAt', path);
}

export function validateTaxOpening(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['taxOpenings'][number] {
  assertRecord(value, path);
  ['id', 'personnelId', 'effectiveFromPeriodId'].forEach((key) =>
    requiredString(value, key, path)
  );
  requiredInteger(value, 'year', path);
  requiredDecimal(value, 'gvCumulativeOpening', path);
  optionalNullableString(value, 'createdAt', path);
  optionalNullableString(value, 'updatedAt', path);
}

export function validateSickLeaveRecord(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['sickLeaveRecords'][number] {
  assertRecord(value, path);
  ['id', 'personnelId', 'startDate', 'endDate'].forEach((key) => requiredString(value, key, path));
  optionalNullableString(value, 'createdAt', path);
  optionalNullableString(value, 'updatedAt', path);
}

export function validateBordroKaydi(
  value: unknown,
  path = '$'
): asserts value is PayrollStorageDto['bordrolar'][number] {
  assertRecord(value, path);
  ['id', 'personelId', 'donemId', 'accrualId', 'olusturulmaTarihi', 'sonGuncellemeTarihi'].forEach((key) =>
    requiredString(value, key, path)
  );
  if ((value.accrualId as string).trim() === '') fail(fieldPath(path, 'accrualId'), 'boş olamaz.');
  const accrualType = required(value, 'accrualType', path);
  const accrualTypePath = fieldPath(path, 'accrualType');
  assertString(accrualType, accrualTypePath);
  if (!(ACCRUAL_TYPE_VALUES as readonly string[]).includes(accrualType)) {
    fail(accrualTypePath, `geçersiz enum değeri: ${accrualType}.`);
  }
  requiredIsoDate(value, 'paymentDate', path);
  requiredNonNegativeInteger(value, 'sequence', path);
  optionalNullableString(value, 'accrualDescription', path);

  const status = required(value, 'status', path);
  const statusPath = fieldPath(path, 'status');
  assertString(status, statusPath);
  if (!(BORDRO_STATUS_VALUES as readonly string[]).includes(status)) {
    fail(statusPath, `geçersiz enum değeri: ${status}.`);
  }

  const puantajOzetiPath = fieldPath(path, 'puantajOzeti');
  const puantajOzeti = required(value, 'puantajOzeti', path);
  validatePuantajOzeti(puantajOzeti, puantajOzetiPath);
  const gelirlerPath = fieldPath(path, 'gelirler');
  validateGelirKalemleri(required(value, 'gelirler', path), gelirlerPath);
  requiredDecimal(value, 'gelirToplam', path);
  const kesintilerPath = fieldPath(path, 'kesintiler');
  validateKesintiKalemleri(required(value, 'kesintiler', path), kesintilerPath);
  requiredDecimal(value, 'kesintiToplam', path);
  requiredDecimal(value, 'netOdeme', path);

  ['oncekiKumulatifGvMatrahi', 'oncekiKumulatifAsgariGvMatrahi', 'manuelKumulatifGvMatrahi'].forEach(
    (key) => optionalDecimal(value, key, path)
  );
  optionalNullableString(value, 'notlar', path);
  optionalNullableArray(value, 'devredenPekGelen', path, validateDevredenPekKaydi);
  optionalNullableArray(value, 'sonrakiDevredenPek', path, validateDevredenPekKaydi);
  optionalNullableRecord(value, 'pekDetay', path, validatePekDetayi);
  optionalNullableRecord(value, 'isPrimiDetay', path, validateIsPrimiHesapDetayi);
  optionalNullableRecord(value, 'gvDetay', path, validateGvHesapDetayi);
  optionalNullableRecord(value, 'damgaDetay', path, validateDamgaVergisiHesapDetayi);
  optionalNullableRecord(value, 'statutorySnapshot', path, validateResolvedStatutorySnapshot);
  optionalNullableInteger(value, 'odenenRaporluGun', path);
  optionalNullableInteger(value, 'raporluGun', path);
}

function validateCompensationRevision(value: unknown, path: string): void {
  assertRecord(value, path);
  ['id', 'title', 'effectiveFrom'].forEach((key) => requiredString(value, key, path));
  requiredEnum(value, 'reason', COMPENSATION_REVISION_REASON_VALUES, path);
  optionalNullableString(value, 'effectiveTo', path);
  optionalNullableString(value, 'decisionDate', path);
  optionalNullableString(value, 'signedAt', path);
  optionalNullableString(value, 'description', path);
  optionalEnum(value, 'status', COMPENSATION_REVISION_STATUS_VALUES, path);
  optionalEnum(value, 'scope', COMPENSATION_REVISION_SCOPE_VALUES, path);
  const personnelIds = optional(value, 'personnelIds', path);
  if (personnelIds !== undefined && personnelIds !== null) {
    assertArray(personnelIds, fieldPath(path, 'personnelIds'));
    personnelIds.forEach((item, index) => assertString(item, `${fieldPath(path, 'personnelIds')}[${index}]`));
  }
  optionalNullableString(value, 'personnelGroup', path);
  optionalNullableString(value, 'createdAt', path);
  optionalNullableString(value, 'updatedAt', path);
}

function validateCompensationRevisionOverride(value: unknown, path: string): void {
  assertRecord(value, path);
  ['id', 'revisionId'].forEach((key) => requiredString(value, key, path));
  requiredEnum(value, 'parameter', RETRO_PARAMETER_VALUES, path);
  requiredDecimal(value, 'value', path);
  optionalNullableString(value, 'personnelId', path);
}

function validateRetroBatch(value: unknown, path: string): void {
  assertRecord(value, path);
  ['id', 'revisionId', 'personnelId', 'paymentDate'].forEach((key) => requiredString(value, key, path));
  requiredIsoDate(value, 'paymentDate', path);
  requiredEnum(value, 'status', COMPENSATION_REVISION_STATUS_VALUES, path);
  requiredEnum(value, 'settlementStatus', RETRO_SETTLEMENT_STATUS_VALUES, path);
  requiredDecimal(value, 'totalGrossDelta', path);
  optionalNullableString(value, 'description', path);
  optionalNullableString(value, 'createdAt', path);
  optionalNullableString(value, 'calculatedAt', path);
  optionalNullableString(value, 'finalizedAt', path);
}

function validateRetroAllocation(value: unknown, path: string): void {
  assertRecord(value, path);
  ['id', 'batchId', 'personnelId', 'sourcePeriodId'].forEach((key) => requiredString(value, key, path));
  requiredEnum(value, 'earningCode', RETRO_EARNING_CODE_VALUES, path);
  ['originalRecognizedAmount', 'targetAmount', 'deltaAmount'].forEach((key) =>
    requiredDecimal(value, key, path)
  );
  ['previousAuthoritativeRetroAmount', 'originalPek', 'retroPekDelta', 'adjustedPek', 'workerSgkDelta',
    'workerUnemploymentDelta', 'employerSgkDelta', 'employerUnemploymentDelta'].forEach((key) =>
    optionalDecimal(value, key, path)
  );
  requiredEnum(value, 'sgkTreatment', RETRO_SGK_TREATMENT_VALUES, path);
  requiredEnum(value, 'incomeTaxTreatment', RETRO_TAX_TREATMENT_VALUES, path);
  requiredEnum(value, 'stampTaxTreatment', RETRO_TAX_TREATMENT_VALUES, path);
  optionalNullableString(value, 'metadata', path);
}

function assertUniqueBy<T>(
  records: ReadonlyArray<T>,
  keyOf: (record: T) => string,
  collectionPath: string,
  duplicateDescription: (record: T) => string,
  duplicateField?: string
): void {
  const seen = new Map<string, number>();
  records.forEach((record, index) => {
    const key = keyOf(record);
    const previousIndex = seen.get(key);
    if (previousIndex !== undefined) {
      fail(
        duplicateField ? `${collectionPath}[${index}].${duplicateField}` : `${collectionPath}[${index}]`,
        `duplicate ${duplicateDescription(record)}; ilk kayıt ${collectionPath}[${previousIndex}]${
          duplicateField ? `.${duplicateField}` : ''}.`
      );
    }
    seen.set(key, index);
  });
}

function assertUniqueCompositeKey<T>(
  records: ReadonlyArray<T>,
  keyOf: (record: T) => ReadonlyArray<unknown>,
  collectionPath: string,
  fieldNames: ReadonlyArray<string>,
  formatKey: (record: T) => string
): void {
  assertUniqueBy(
    records,
    (record) => JSON.stringify(keyOf(record)),
    collectionPath,
    (record) => `(${fieldNames.join(', ')}): ${formatKey(record)}`
  );
}

function assertUniqueIds(
  records: ReadonlyArray<{ id: string }>,
  collectionPath: string
): void {
  assertUniqueBy(records, (record) => record.id, collectionPath, (record) => `id: ${record.id}`);
}

function retroCents(value: string, path: string): bigint {
  if (!isExactDecimalString(value)) {
    fail(path, 'retro parasal değeri exact Decimal metni olmalıdır.');
  }
  const negative = value.startsWith('-');
  const unsigned = negative ? value.slice(1) : value;
  const [whole, fraction = ''] = unsigned.split('.');
  if (fraction.length > 2) {
    fail(path, 'retro parasal değer native kuruş saklama sınırı olan 2 ondalık basamağı aşamaz.');
  }
  const coefficient = BigInt(`${whole}${fraction.padEnd(2, '0')}`);
  return negative ? -coefficient : coefficient;
}

function assertRetroLedgerAmounts(payload: PayrollStorageDto): void {
  const allocationsByBatch = new Map<string, Array<{ index: number; allocation: PayrollStorageDto['retroAllocations'][number] }>>();
  payload.retroAllocations.forEach((allocation, index) => {
    const policy = RETRO_POLICY_BY_EARNING_CODE[allocation.earningCode];
    if (
      allocation.sgkTreatment !== policy.sgkTreatment ||
      allocation.incomeTaxTreatment !== policy.taxTreatment ||
      allocation.stampTaxTreatment !== policy.taxTreatment
    ) {
      fail(
        `$.retroAllocations[${index}]`,
        'earning code policy snapshotı canonical registry ile eşleşmiyor.'
      );
    }
    const amounts = [
      ['originalRecognizedAmount', allocation.originalRecognizedAmount],
      ['previousAuthoritativeRetroAmount', allocation.previousAuthoritativeRetroAmount],
      ['targetAmount', allocation.targetAmount],
      ['deltaAmount', allocation.deltaAmount],
      ['originalPek', allocation.originalPek],
      ['retroPekDelta', allocation.retroPekDelta],
      ['adjustedPek', allocation.adjustedPek],
      ['workerSgkDelta', allocation.workerSgkDelta],
      ['workerUnemploymentDelta', allocation.workerUnemploymentDelta],
      ['employerSgkDelta', allocation.employerSgkDelta],
      ['employerUnemploymentDelta', allocation.employerUnemploymentDelta],
    ] as const;
    amounts.forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        const cents = retroCents(value, `$.retroAllocations[${index}].${key}`);
        if (
          ['originalRecognizedAmount', 'targetAmount', 'originalPek', 'adjustedPek'].includes(key)
          && cents < 0n
        ) {
          fail(
            `$.retroAllocations[${index}].${key}`,
            'authoritative entitlement/PEK tabanı negatif olamaz.'
          );
        }
      }
    });
    const list = allocationsByBatch.get(allocation.batchId) ?? [];
    list.push({ index, allocation });
    allocationsByBatch.set(allocation.batchId, list);

    const original = retroCents(
      allocation.originalRecognizedAmount,
      `$.retroAllocations[${index}].originalRecognizedAmount`
    );
    const previous = retroCents(
      allocation.previousAuthoritativeRetroAmount ?? '0.00',
      `$.retroAllocations[${index}].previousAuthoritativeRetroAmount`
    );
    const target = retroCents(
      allocation.targetAmount,
      `$.retroAllocations[${index}].targetAmount`
    );
    const delta = retroCents(
      allocation.deltaAmount,
      `$.retroAllocations[${index}].deltaAmount`
    );
    if (target - original - previous !== delta) {
      fail(
        `$.retroAllocations[${index}].deltaAmount`,
        'target - original recognized - previous authoritative retro hesabıyla eşleşmiyor.'
      );
    }
  });

  payload.retroBatches.forEach((batch, index) => {
    const total = retroCents(batch.totalGrossDelta, `$.retroBatches[${index}].totalGrossDelta`);
    const allocations = allocationsByBatch.get(batch.id) ?? [];
    const allocationTotal = allocations.reduce(
      (sum, item) => sum + retroCents(item.allocation.deltaAmount, `$.retroAllocations[${item.index}].deltaAmount`),
      0n
    );
    if (allocationTotal !== total) {
      fail(
        `$.retroBatches[${index}].totalGrossDelta`,
        `allocation delta toplamı batch toplamıyla eşleşmiyor: ${allocationTotal.toString()} kuruş.`
      );
    }
    const status = batch.status ?? 'DRAFT';
    const hasNegativeDelta = total < 0n || allocations.some((item) =>
      retroCents(item.allocation.deltaAmount, `$.retroAllocations[${item.index}].deltaAmount`) < 0n
    );
    if (hasNegativeDelta && status === 'FINALIZED') {
      fail(
        `$.retroBatches[${index}]`,
        'Negatif retro fark FINALIZED ödeme batch’i olamaz; OVERPAYMENT olarak açık settlement kaydı tutulmalıdır.'
      );
    }
    const expectedSettlement = hasNegativeDelta
      ? 'OVERPAYMENT'
      : status === 'FINALIZED'
        ? 'PAID'
        : 'UNSETTLED';
    if (batch.settlementStatus !== undefined && batch.settlementStatus !== expectedSettlement) {
      fail(
        `$.retroBatches[${index}].settlementStatus`,
        `batch durumu/tutarı ile settlement statusı tutarsız; beklenen ${expectedSettlement}.`
      );
    }
  });
}

function assertCrossRecordIntegrity(payload: PayrollStorageDto): void {
  const personnelIds = new Set(payload.personeller.map((person) => person.id));
  const periodIds = new Set(payload.donemler.map((period) => period.id));

  if (payload.aktifDonemId !== '' && !periodIds.has(payload.aktifDonemId)) {
    fail(
      '$.aktifDonemId',
      `mevcut olmayan dönem kimliği: ${payload.aktifDonemId}.`
    );
  }

  payload.puantajlar.forEach((attendance, index) => {
    if (!personnelIds.has(attendance.personelId)) {
      fail(`$.puantajlar[${index}].personelId`, `mevcut olmayan personel kimliği: ${attendance.personelId}.`);
    }
    if (!periodIds.has(attendance.donemId)) {
      fail(`$.puantajlar[${index}].donemId`, `mevcut olmayan dönem kimliği: ${attendance.donemId}.`);
    }
  });

  payload.bordrolar.forEach((payroll, index) => {
    if (!personnelIds.has(payroll.personelId)) {
      fail(`$.bordrolar[${index}].personelId`, `mevcut olmayan personel kimliği: ${payroll.personelId}.`);
    }
    if (!periodIds.has(payroll.donemId)) {
      fail(`$.bordrolar[${index}].donemId`, `mevcut olmayan dönem kimliği: ${payroll.donemId}.`);
    }
    const period = payload.donemler.find((candidate) => candidate.id === payroll.donemId);
    if (period) {
      const expectedTaxPrefix = `${String(period.taxYear).padStart(4, '0')}-${String(period.taxMonth).padStart(2, '0')}-`;
      if (!payroll.paymentDate.startsWith(expectedTaxPrefix)) {
        fail(
          `$.bordrolar[${index}].paymentDate`,
          `tahakkuk vergi yılı/ayı ile eşleşmiyor; beklenen ${period.taxYear}-${String(period.taxMonth).padStart(2, '0')}.`
        );
      }
    }
  });

  payload.taxOpenings.forEach((opening, index) => {
    if (!personnelIds.has(opening.personnelId)) {
      fail(
        `$.taxOpenings[${index}].personnelId`,
        `mevcut olmayan personel kimliği: ${opening.personnelId}.`
      );
    }
    if (!periodIds.has(opening.effectiveFromPeriodId)) {
      fail(
        `$.taxOpenings[${index}].effectiveFromPeriodId`,
        `mevcut olmayan dönem kimliği: ${opening.effectiveFromPeriodId}.`
      );
    }
  });

  payload.sickLeaveRecords.forEach((record, index) => {
    if (!personnelIds.has(record.personnelId)) {
      fail(
        `$.sickLeaveRecords[${index}].personnelId`,
        `mevcut olmayan personel kimliği: ${record.personnelId}.`
      );
    }
  });

  const revisions = payload.compensationRevisions ?? [];
  const revisionIds = new Set(revisions.map((revision) => revision.id));
  const batches = payload.retroBatches ?? [];
  const batchIds = new Set(batches.map((batch) => batch.id));
  const batchPersonnel = new Map(batches.map((batch) => [batch.id, batch.personnelId]));
  (payload.compensationRevisionOverrides ?? []).forEach((override, index) => {
    if (!revisionIds.has(override.revisionId)) {
      fail(
        `$.compensationRevisionOverrides[${index}].revisionId`,
        `mevcut olmayan revision kimliği: ${override.revisionId}.`
      );
    }
    if (override.personnelId && !personnelIds.has(override.personnelId)) {
      fail(
        `$.compensationRevisionOverrides[${index}].personnelId`,
        `mevcut olmayan personel kimliği: ${override.personnelId}.`
      );
    }
  });
  batches.forEach((batch, index) => {
    if (!revisionIds.has(batch.revisionId)) {
      fail(`$.retroBatches[${index}].revisionId`, `mevcut olmayan revision kimliği: ${batch.revisionId}.`);
    }
    if (!personnelIds.has(batch.personnelId)) {
      fail(`$.retroBatches[${index}].personnelId`, `mevcut olmayan personel kimliği: ${batch.personnelId}.`);
    }
    const status = batch.status ?? 'DRAFT';
    const linked = payload.bordrolar.filter((payroll) => payroll.accrualId === batch.id);
    if (linked.length > 1) {
      fail(
        `$.retroBatches[${index}]`,
        'Bir retro batch birden fazla payment event ile eşleşemez.'
      );
    }
    if (status === 'FINALIZED' && linked.length !== 1) {
      fail(
        `$.retroBatches[${index}]`,
        'FINALIZED retro batch tam olarak bir payment event ile eşleşmelidir.'
      );
    }
    if (linked.length === 1) {
      const payment = linked[0];
      const expectedPaymentStatus = status === 'FINALIZED' ? 'FINALIZED' : status === 'CALCULATED' ? 'CALCULATED' : null;
      const linkedStateIsValid = expectedPaymentStatus !== null &&
        payment.accrualType === 'RETRO_ADJUSTMENT' &&
        payment.personelId === batch.personnelId &&
        payment.paymentDate === batch.paymentDate &&
        payment.status === expectedPaymentStatus &&
        retroCents(payment.gelirToplam, `$.bordrolar[${payload.bordrolar.indexOf(payment)}].gelirToplam`) ===
          retroCents(batch.totalGrossDelta, `$.retroBatches[${index}].totalGrossDelta`);
      if (!linkedStateIsValid) {
        fail(
          `$.retroBatches[${index}]`,
          'Retro batch lifecycle durumu ile bağlı payment event durumu/kimliği/finansal snapshotı eşleşmiyor.'
        );
      }
    }
  });
  (payload.retroAllocations ?? []).forEach((allocation, index) => {
    if (!batchIds.has(allocation.batchId)) {
      fail(`$.retroAllocations[${index}].batchId`, `mevcut olmayan batch kimliği: ${allocation.batchId}.`);
    }
    if (!personnelIds.has(allocation.personnelId)) {
      fail(`$.retroAllocations[${index}].personnelId`, `mevcut olmayan personel kimliği: ${allocation.personnelId}.`);
    }
    if (!periodIds.has(allocation.sourcePeriodId)) {
      fail(`$.retroAllocations[${index}].sourcePeriodId`, `mevcut olmayan dönem kimliği: ${allocation.sourcePeriodId}.`);
    }
    if (batchPersonnel.get(allocation.batchId) !== allocation.personnelId) {
      fail(`$.retroAllocations[${index}].personnelId`, 'batch personeliyle eşleşmiyor.');
    }
  });

  Object.entries(payload.kurumDegerleriMap).forEach(([key, settings]) => {
    const settingsPath = fieldPath('$.kurumDegerleriMap', key);
    if (!periodIds.has(key)) {
      fail(settingsPath, `mevcut olmayan dönem kimliği: ${key}.`);
    }
    if (settings.donemId !== key) {
      fail(
        `${settingsPath}.donemId`,
        `map anahtarıyla eşleşmiyor: ${settings.donemId}.`
      );
    }
  });

  payload.bordrolar
    .filter((payroll) => payroll.accrualType === 'RETRO_ADJUSTMENT')
    .forEach((payroll) => {
      const batch = batches.find((candidate) => candidate.id === payroll.accrualId);
      if (!batch) {
        fail(
          `$.bordrolar[${payload.bordrolar.indexOf(payroll)}]`,
          `retro payment event'i için batch bulunamadı: ${payroll.accrualId}.`
        );
      }
      if (batch && (
        batch.personnelId !== payroll.personelId ||
        batch.paymentDate !== payroll.paymentDate
      )) {
        fail(
          `$.bordrolar[${payload.bordrolar.indexOf(payroll)}]`,
          'retro payment event batch personel/ödeme tarihiyle eşleşmiyor.'
        );
      }
    });
}

function validateTopLevelArrays(value: UnknownRecord): void {
  const arrayFields = [
    'donemler',
    'personeller',
    'puantajlar',
    'bordrolar',
    'taxOpenings',
    'sickLeaveRecords',
    'annualPayrollParameters',
    'zamAylari',
    'compensationRevisions',
    'compensationRevisionOverrides',
    'retroBatches',
    'retroAllocations',
  ] as const;
  arrayFields.forEach((key) => {
    const propertyPath = fieldPath('$', key);
    assertArray(required(value, key, '$'), propertyPath);
  });
}

/** Full current V4 runtime schema. Unknown fields are intentionally ignored. */
export function validateCurrentPayrollPayload(
  value: unknown
): asserts value is PayrollStorageDto {
  assertRecord(value, '$');
  const backupVersion = required(value, 'backupVersion', '$');
  assertInteger(backupVersion, '$.backupVersion');
  if (backupVersion !== BACKUP_FORMAT_VERSION) {
    fail(
      '$.backupVersion',
      `desteklenmeyen sürüm: ${backupVersion} (beklenen: ${BACKUP_FORMAT_VERSION}).`
    );
  }
  requiredString(value, 'exportedAt', '$');
  requiredString(value, 'aktifDonemId', '$');
  assertRecord(required(value, 'kurumDegerleriMap', '$'), '$.kurumDegerleriMap');
  validateTopLevelArrays(value);

  const periods = value.donemler as unknown[];
  periods.forEach((item, index) => validateBordroDonemi(item, `$.donemler[${index}]`));
  const personnel = value.personeller as unknown[];
  personnel.forEach((item, index) => validatePersonel(item, `$.personeller[${index}]`));
  const attendances = value.puantajlar as unknown[];
  attendances.forEach((item, index) => validatePuantaj(item, `$.puantajlar[${index}]`));
  const payrolls = value.bordrolar as unknown[];
  payrolls.forEach((item, index) => validateBordroKaydi(item, `$.bordrolar[${index}]`));
  const openings = value.taxOpenings as unknown[];
  openings.forEach((item, index) => validateTaxOpening(item, `$.taxOpenings[${index}]`));
  const sickLeaveRecords = value.sickLeaveRecords as unknown[];
  sickLeaveRecords.forEach((item, index) => validateSickLeaveRecord(item, `$.sickLeaveRecords[${index}]`));
  const annualParameters = value.annualPayrollParameters as unknown[];
  annualParameters.forEach((item, index) =>
    validateAnnualPayrollParameters(item, `$.annualPayrollParameters[${index}]`)
  );

  const zamAylari = value.zamAylari as unknown[];
  zamAylari.forEach((item, index) => assertInteger(item, `$.zamAylari[${index}]`));

  const revisions = required(value, 'compensationRevisions', '$');
  assertArray(revisions, '$.compensationRevisions');
  revisions.forEach((item, index) => validateCompensationRevision(item, `$.compensationRevisions[${index}]`));
  const revisionOverrides = required(value, 'compensationRevisionOverrides', '$');
  assertArray(revisionOverrides, '$.compensationRevisionOverrides');
  revisionOverrides.forEach((item, index) =>
    validateCompensationRevisionOverride(item, `$.compensationRevisionOverrides[${index}]`)
  );
  const retroBatches = required(value, 'retroBatches', '$');
  assertArray(retroBatches, '$.retroBatches');
  retroBatches.forEach((item, index) => validateRetroBatch(item, `$.retroBatches[${index}]`));
  const retroAllocations = required(value, 'retroAllocations', '$');
  assertArray(retroAllocations, '$.retroAllocations');
  retroAllocations.forEach((item, index) =>
    validateRetroAllocation(item, `$.retroAllocations[${index}]`)
  );

  const institutionSettings = value.kurumDegerleriMap;
  Object.entries(institutionSettings).forEach(([key, settings]) =>
    validateInstitutionSettings(settings, fieldPath('$.kurumDegerleriMap', key))
  );

  // The structural walk above intentionally runs first. This is the single
  // Decimal grammar/number rejection implementation for the whole payload.
  assertExactDecimalDto(value);

  const typedPayload = value as PayrollStorageDto;
  assertUniqueIds(typedPayload.donemler, '$.donemler');
  assertUniqueIds(typedPayload.personeller, '$.personeller');
  assertUniqueIds(typedPayload.puantajlar, '$.puantajlar');
  assertUniqueIds(typedPayload.bordrolar, '$.bordrolar');
  assertUniqueBy(
    typedPayload.bordrolar,
    (payroll) => payroll.accrualId,
    '$.bordrolar',
    (payroll) => `accrualId: ${payroll.accrualId}`,
    'accrualId'
  );
  assertUniqueIds(typedPayload.taxOpenings, '$.taxOpenings');
  assertUniqueIds(typedPayload.sickLeaveRecords, '$.sickLeaveRecords');
  const typedRevisions = typedPayload.compensationRevisions;
  const typedRevisionOverrides = typedPayload.compensationRevisionOverrides;
  const typedRetroBatches = typedPayload.retroBatches;
  const typedRetroAllocations = typedPayload.retroAllocations;
  assertUniqueIds(typedRevisions, '$.compensationRevisions');
  assertUniqueIds(typedRevisionOverrides, '$.compensationRevisionOverrides');
  assertUniqueIds(typedRetroBatches, '$.retroBatches');
  assertUniqueIds(typedRetroAllocations, '$.retroAllocations');
  assertUniqueCompositeKey(
    typedRevisionOverrides,
    (override) => [override.revisionId, override.parameter, override.personnelId ?? ''],
    '$.compensationRevisionOverrides',
    ['revisionId', 'parameter', 'personnelId'],
    (override) => `${override.revisionId} / ${override.parameter} / ${override.personnelId ?? ''}`
  );

  // These integrity checks mirror persistence-level uniqueness/FK invariants
  // enforced by native SQLite. They are not payroll business rules.
  assertUniqueBy(
    typedPayload.personeller,
    (person) => person.tcNo,
    '$.personeller',
    (person) => `tcNo: ${person.tcNo}`,
    'tcNo'
  );
  assertUniqueCompositeKey(
    typedPayload.puantajlar,
    (attendance) => [attendance.personelId, attendance.donemId],
    '$.puantajlar',
    ['personelId', 'donemId'],
    (attendance) => `${attendance.personelId} / ${attendance.donemId}`
  );
  const normalPayrolls = typedPayload.bordrolar.filter((payroll) => payroll.accrualType === 'NORMAL');
  assertUniqueCompositeKey(
    normalPayrolls,
    (payroll) => [payroll.personelId, payroll.donemId],
    '$.bordrolar',
    ['personelId', 'donemId'],
    (payroll) => `${payroll.personelId} / ${payroll.donemId}`
  );
  assertUniqueCompositeKey(
    typedPayload.bordrolar,
    (payroll) => [payroll.personelId, payroll.paymentDate, payroll.sequence],
    '$.bordrolar',
    ['personelId', 'paymentDate', 'sequence'],
    (payroll) => `${payroll.personelId} / ${payroll.paymentDate} / ${payroll.sequence}`
  );
  assertUniqueCompositeKey(
    typedPayload.taxOpenings,
    (opening) => [opening.personnelId, opening.year],
    '$.taxOpenings',
    ['personnelId', 'year'],
    (opening) => `${opening.personnelId} / ${opening.year}`
  );
  assertUniqueBy(
    typedPayload.annualPayrollParameters,
    (parameters) => String(parameters.year),
    '$.annualPayrollParameters',
    (parameters) => `year: ${parameters.year}`,
    'year'
  );
  assertUniqueCompositeKey(
    typedPayload.donemler,
    (period) => [period.taxYear, period.taxMonth],
    '$.donemler',
    ['taxYear', 'taxMonth'],
    (period) => `${period.taxYear} / ${period.taxMonth}`
  );
  assertUniqueCompositeKey(
    typedRetroAllocations,
    (allocation) => [allocation.batchId, allocation.sourcePeriodId, allocation.earningCode],
    '$.retroAllocations',
    ['batchId', 'sourcePeriodId', 'earningCode'],
    (allocation) => `${allocation.batchId} / ${allocation.sourcePeriodId} / ${allocation.earningCode}`
  );
  assertCrossRecordIntegrity(typedPayload);
  assertRetroLedgerAmounts(typedPayload);
}

export function parseAndValidatePayrollPayload(value: unknown): PayrollStorageDto {
  validateCurrentPayrollPayload(value);
  return value;
}
