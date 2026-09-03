import type { BackupPayload } from '../../types/payroll';
import type { PayrollCalculationRequest } from './types';

/**
 * Decimal-backed JSON keys emitted by the shared Rust models.
 *
 * Keep this list explicit: it is the small, auditable schema contract between
 * Rust Decimal values and browser JSON. Integer chronology/attendance fields
 * are intentionally absent.
 */
const DECIMAL_KEY_LIST = [
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
  'icra',
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
  'sgkYemekIstisnasiToplam',
  'gvYemekIstisnasiToplam',
  'gvReferansGunlukAsgariUcret',
] as const;

export type DecimalKey = (typeof DECIMAL_KEY_LIST)[number];
export type DecimalString = string;

/** Type-level mirror of the runtime Decimal-key schema. */
export type Exactify<T> = T extends readonly (infer Item)[]
  ? Exactify<Item>[]
  : T extends object
    ? {
        [Key in keyof T]: Key extends DecimalKey
          ? T[Key] extends number
            ? DecimalString
            : T[Key] extends null
              ? null
              : T[Key] extends undefined
                ? undefined
                : T[Key] extends number | null | undefined
                  ? DecimalString | null | undefined
                  : T[Key]
          : Exactify<T[Key]>;
      }
    : T;

/** Exact browser snapshot used by IndexedDB and the calculation boundary. */
export type PayrollStorageDto = Exactify<BackupPayload>;
export type PayrollStorageFields = Omit<PayrollStorageDto, 'backupVersion' | 'exportedAt'>;

export const DECIMAL_KEYS = new Set<DecimalKey>(DECIMAL_KEY_LIST);

const DECIMAL_TEXT_PATTERN = /^-?(?:0|[1-9]\d*)(?:\.\d+)?$/;

export function isExactDecimalString(value: unknown): value is DecimalString {
  return typeof value === 'string' && DECIMAL_TEXT_PATTERN.test(value);
}

function exactDecimalFromNumber(value: number): DecimalString {
  if (!Number.isFinite(value)) {
    throw new Error('Decimal sınırında sonlu olmayan parasal değer var.');
  }
  if (Object.is(value, -0)) return '0';

  const text = value.toString();
  if (!/[eE]/.test(text)) return text;

  // Number#toString may choose exponent notation for a finite UI input. Rust
  // Decimal JSON is deliberately kept in plain decimal notation so this
  // compatibility adapter never emits a grammar the strict boundary rejects.
  const [mantissa, exponentText] = text.split(/[eE]/);
  const exponent = Number(exponentText);
  const sign = mantissa.startsWith('-') ? '-' : '';
  const unsignedMantissa = mantissa.replace(/^-/, '');
  const [whole, fraction = ''] = unsignedMantissa.split('.');
  const digits = `${whole}${fraction}`;
  const decimalIndex = whole.length + exponent;

  if (decimalIndex <= 0) return `${sign}0.${'0'.repeat(-decimalIndex)}${digits}`;
  if (decimalIndex >= digits.length) return `${sign}${digits}${'0'.repeat(decimalIndex - digits.length)}`;
  return `${sign}${digits.slice(0, decimalIndex)}.${digits.slice(decimalIndex)}`;
}

function encodeDecimals(value: unknown, key?: string): unknown {
  if (typeof value === 'number' && key && DECIMAL_KEYS.has(key as DecimalKey)) {
    return exactDecimalFromNumber(value);
  }
  if (typeof value === 'string' && key && DECIMAL_KEYS.has(key as DecimalKey)) {
    if (!isExactDecimalString(value)) {
      throw new Error(`Geçersiz Decimal metni: ${value}`);
    }
    return value;
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
 * Converts a UI/native compatibility object to the exact string DTO.
 * This is an explicit adapter at the presentation/input boundary; it is not
 * used to decode authoritative browser snapshots.
 */
export function toPayrollBoundaryDto<T>(value: T): Exactify<T> {
  return encodeDecimals(value) as Exactify<T>;
}

/** Backward-compatible name for callers that intentionally encode UI input. */
export function encodeDecimalValues<T>(value: T): Exactify<T> {
  return toPayrollBoundaryDto(value);
}

function assertExactDecimals(value: unknown, key?: string, path = '$'): void {
  if (key && DECIMAL_KEYS.has(key as DecimalKey)) {
    if (value === null || value === undefined) return;
    if (typeof value !== 'string') {
      throw new Error(`${path} Decimal değeri string olmalıdır; JS number kabul edilmez.`);
    }
    if (!isExactDecimalString(value)) {
      throw new Error(`${path} Decimal metni exact plain formatta olmalıdır.`);
    }
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertExactDecimals(item, undefined, `${path}[${index}]`));
    return;
  }
  if (value && typeof value === 'object') {
    Object.entries(value).forEach(([entryKey, entryValue]) => {
      assertExactDecimals(entryValue, entryKey, `${path}.${entryKey}`);
    });
  }
}

/** Fails closed when a request/result tries to cross the Rust boundary with a number. */
export function assertExactDecimalDto(value: unknown): void {
  assertExactDecimals(value);
}

function decodeDecimals(value: unknown, key?: string, path = '$'): unknown {
  if (typeof value === 'string' && key && DECIMAL_KEYS.has(key as DecimalKey)) {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) {
      throw new Error(`${path} Decimal değeri UI gösterimi için sayıya çevrilemedi.`);
    }
    return parsed;
  }
  if (Array.isArray(value)) {
    return value.map((item, index) => decodeDecimals(item, undefined, `${path}[${index}]`));
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([entryKey, entryValue]) => [
        entryKey,
        decodeDecimals(entryValue, entryKey, `${path}.${entryKey}`),
      ])
    );
  }
  return value;
}

/** Presentation-only adapter. It must never feed persistence or calculation. */
export function toPayrollUiModel<T>(value: T): T {
  return decodeDecimals(value) as T;
}

/** Legacy compatibility name; use toPayrollUiModel at explicit UI boundaries. */
export function decodeDecimalValues<T>(value: T): T {
  return toPayrollUiModel(value);
}

/** Parses a raw WASM result while preserving exact Decimal strings. */
export function parsePayrollBoundaryJson<T = unknown>(json: string): T {
  const value: unknown = JSON.parse(json);
  if (!value || typeof value !== 'object') {
    throw new Error('WASM bordro sonucu geçersiz.');
  }
  assertExactDecimalDto(value);
  return value as T;
}

/** Alias with a name that makes the authoritative result contract explicit. */
export function parseWasmPayrollBoundaryResult<T = unknown>(json: string): T {
  return parsePayrollBoundaryJson<T>(json);
}

/**
 * Production callers receive the exact boundary DTO. Rendering code should
 * call toPayrollUiModel explicitly when it needs formatted numbers.
 */
export function parseWasmPayrollResult<T = unknown>(json: string): T {
  return parseWasmPayrollBoundaryResult<T>(json);
}

/** Parses IndexedDB/backup JSON without converting Decimal strings to numbers. */
export function parsePayrollStorage<T>(json: string): T {
  return parsePayrollBoundaryJson<T>(json);
}

/**
 * One-time compatibility adapter for old numeric localStorage backups. The
 * current IndexedDB snapshot path remains strict; old values can only retain
 * the precision present in that legacy JSON number.
 */
export function parseLegacyPayrollStorage<T>(json: string): T {
  const value: unknown = JSON.parse(json);
  if (!value || typeof value !== 'object') {
    throw new Error('Yedek JSON nesne içermiyor.');
  }
  return toPayrollBoundaryDto(value) as unknown as T;
}

/** Serializes only exact/encoded Decimal strings for storage or backup. */
export function serializePayrollStorage(value: unknown, space?: number): string {
  const encoded = encodeDecimals(value);
  assertExactDecimalDto(encoded);
  return JSON.stringify(encoded, null, space);
}

/** Requests must already be exact; numeric Decimal fields fail closed here. */
export function serializePayrollRequestForWasm(request: PayrollCalculationRequest): string {
  assertExactDecimalDto(request);
  return JSON.stringify(request);
}

/**
 * Applies a UI edit to an exact object. Unchanged values retain their original
 * Decimal text, including values too large/precise for a JS number. Only the
 * field actually changed by the UI is encoded from its presentation value.
 */
export function mergePayrollUiIntoBoundary<T>(
  previousBoundary: unknown,
  nextUiValue: T
): Exactify<T> {
  const merge = (previous: unknown, next: unknown, key?: string): unknown => {
    if (key && DECIMAL_KEYS.has(key as DecimalKey)) {
      if (next === null || next === undefined) return next;
      if (typeof next === 'string') {
        if (!isExactDecimalString(next)) throw new Error(`Geçersiz Decimal metni: ${next}`);
        return next;
      }
      if (typeof next !== 'number') return next;
      if (typeof previous === 'string') {
        const previousAsUi = Number(previous);
        if (Number.isFinite(previousAsUi) && Object.is(previousAsUi, next)) return previous;
      }
      return exactDecimalFromNumber(next);
    }
    if (Array.isArray(next)) {
      const previousArray = Array.isArray(previous) ? previous : [];
      return next.map((item, index) => merge(previousArray[index], item));
    }
    if (next && typeof next === 'object') {
      const previousObject = previous && typeof previous === 'object' ? previous : {};
      return Object.fromEntries(
        Object.entries(next).map(([entryKey, entryValue]) => [
          entryKey,
          merge((previousObject as Record<string, unknown>)[entryKey], entryValue, entryKey),
        ])
      );
    }
    return next;
  };

  return merge(previousBoundary, nextUiValue) as Exactify<T>;
}

export function isDecimalBoundaryKey(key: string): boolean {
  return DECIMAL_KEYS.has(key as DecimalKey);
}
