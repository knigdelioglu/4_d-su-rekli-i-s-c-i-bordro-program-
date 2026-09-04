import { BACKUP_FORMAT_VERSION, BORDRO_STATUS_VALUES } from '../../types/payroll';
import {
  assertExactDecimalDto,
  type PayrollStorageDto,
} from '../payrollEngine/decimalBoundary';

type UnknownRecord = Record<string, unknown>;

const PUANTAJ_OZETI_KEYS = ['Ç', 'T', 'G', 'İ', 'GÇ', 'GÇT', 'R'] as const;
const ACCRUAL_TYPE_VALUES = ['NORMAL', 'TEDIYE', 'TIS_IKRAMIYE', 'SUPPLEMENTAL'] as const;

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
  ] as const;
  arrayFields.forEach((key) => {
    const propertyPath = fieldPath('$', key);
    assertArray(required(value, key, '$'), propertyPath);
  });
}

/** Full current V3 runtime schema. Unknown fields are intentionally ignored. */
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
  assertCrossRecordIntegrity(typedPayload);
}

export function parseAndValidatePayrollPayload(value: unknown): PayrollStorageDto {
  validateCurrentPayrollPayload(value);
  return value;
}
