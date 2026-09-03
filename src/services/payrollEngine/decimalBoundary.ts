import type { BordroKaydi } from '../../types/payroll';
import type { PayrollCalculationRequest } from './types';

// These are the Decimal-backed model fields. Integer day/month/year fields are
// intentionally absent so the JSON boundary cannot turn tax chronology or
// attendance counts into decimal strings by accident.
const DECIMAL_KEYS = new Set([
  'dogumAskerlikGvIndirimTutar',
  'hayatSigortasiPrimiTutar',
  'saglikSigortasiPrimiTutar',
  'sabitSendikaAidati',
  'oksOraniYuzde',
  'sabitBesTutar',
  'icraTutar',
  'kisiBorcuTutar',
  'dogumAskerlikBorclanmasiTutar',
  'hayatSaglikSigortasiTutar',
  'digerKesintiTutar',
  'devirKumulatifGvMatrahi',
  'devirKumulatifAsgariGvMatrahi',
  'gvCumulativeOpening',
  'limit',
  'oran',
  'sigortaGvYillikBrutAsgariUcretTavani',
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
  'isciSgkPrimi',
  'isciIssizlikPrimi',
  'gelirVergisi',
  'damgaVergisi',
  'sendikaAidati',
  'bes',
  'kisiBorcu',
  'dogumAskerlikBorclanmasi',
  'hayatSaglikSigortasi',
  'digerKesinti',
  'tutar',
  'hesaplananPek',
  'hamPek',
  'devredenPekKullanilan',
  'primMatrahi',
  'finalPek',
  'devredenPekAşanTutar',
  'pekAltSinir',
  'pekUstSinir',
  'altSinirTamamlamaFarki',
  'yemekIstisnasiTutar',
  'isverenSgkPrimi',
  'isverenIssizlikPrimi',
  'pekAltSinirTamamlamaIsverenPrimi',
  'isverenPrimToplami',
  'sgkIsverenOraniYuzde',
  'isverenIssizlikOraniYuzde',
  'cariGvMatrahi',
  'yeniKumulatifGvMatrahi',
  'brutGelirVergisi',
  'asgariUcretGvMatrahi',
  'asgariUcretReferansKumulatifMatrahi',
  'asgariUcretGvIstisnasi',
  'uygulananGvIstisnasi',
  'kesilenGelirVergisi',
  'dogumAskerlikGvIndirimi',
  'sigortaGvIndirimAdayi',
  'sigortaGvAylikLimiti',
  'sigortaGvYillikKalanLimiti',
  'uygulanabilirSigortaGvIndirimi',
  'gunlukAsgariUcret',
  'pekTavanKatsayisi',
  'gunlukYemekIstisnasiSGK',
  'gunlukYemekIstisnasiGV',
  'gunlukTabanUcret',
  'gunlukYemek',
  'gunlukVasitaYol',
  'giyimYardimi',
  'hizmetZammiBirimi',
  'isPrimiYuzde',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
  'digerGelirVarsayilan',
  'sgkIsciOraniYuzde',
  'issizlikIsciOraniYuzde',
  'gelirVergisiOraniYuzde',
  'damgaVergisiOraniBinde',
  'sendikaAidatiYuzde',
  'besOraniYuzde',
  'sgkIsverenOraniYuzde',
  'issizlikIsverenOraniYuzde',
  'sabitTutar',
  'amount',
  'gunlukIsPrimi',
  'gelirToplam',
  'kesintiToplam',
  'netOdeme',
  'oncekiKumulatifGvMatrahi',
  'oncekiKumulatifAsgariGvMatrahi',
  'manuelKumulatifGvMatrahi',
]);

function exactDecimal(value: number): string {
  if (!Number.isFinite(value)) {
    throw new Error('WASM bordro isteğinde sonlu olmayan parasal değer var.');
  }
  return Object.is(value, -0) ? '0' : value.toString();
}

function encodeDecimals(value: unknown, key?: string): unknown {
  if (typeof value === 'number' && key && DECIMAL_KEYS.has(key)) {
    return exactDecimal(value);
  }
  if (Array.isArray(value)) return value.map((item) => encodeDecimals(item));
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([entryKey, entryValue]) => [
        entryKey,
        encodeDecimals(entryValue, entryKey),
      ])
    );
  }
  return value;
}

/**
 * Converts the UI representation to the explicit string-Decimal boundary.
 * Rust's shared models serialize every Decimal as a string; this function is
 * the compatibility adapter for the existing number-based presentation model.
 */
export function encodeDecimalValues<T>(value: T): T {
  return encodeDecimals(value) as T;
}

function decodeDecimals(value: unknown, key?: string): unknown {
  if (typeof value === 'string' && key && DECIMAL_KEYS.has(key)) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : value;
  }
  if (Array.isArray(value)) return value.map((item) => decodeDecimals(item));
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([entryKey, entryValue]) => [
        entryKey,
        decodeDecimals(entryValue, entryKey),
      ])
    );
  }
  return value;
}

/** Decodes strings for the existing UI model; it is never used for authority. */
export function decodeDecimalValues<T>(value: T): T {
  return decodeDecimals(value) as T;
}

/** Serializes Decimal-backed values for WASM or browser/native persistence. */
export function serializePayrollRequestForWasm(request: PayrollCalculationRequest): string {
  return JSON.stringify(encodeDecimals(request));
}

/** Parses a raw WASM result while preserving its exact string Decimal values. */
export function parsePayrollBoundaryJson(json: string): unknown {
  const value: unknown = JSON.parse(json);
  if (!value || typeof value !== 'object') {
    throw new Error('WASM bordro sonucu geçersiz.');
  }
  return value;
}

/** Parses a Rust result into the existing number-based presentation model. */
export function parseWasmPayrollResult(json: string): BordroKaydi {
  return decodeDecimalValues(parsePayrollBoundaryJson(json)) as BordroKaydi;
}

/** Keeps browser backup/IndexedDB JSON Decimal-safe without changing its schema. */
export function serializePayrollStorage(value: unknown, space?: number): string {
  return JSON.stringify(encodeDecimals(value), null, space);
}

export function parsePayrollStorage<T>(json: string): T {
  return decodeDecimalValues(JSON.parse(json)) as T;
}

export function isDecimalBoundaryKey(key: string): boolean {
  return DECIMAL_KEYS.has(key);
}
