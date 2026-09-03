import { BordroKaydi } from '../../types/payroll';
import { PayrollCalculationRequest } from './types';

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

function stringifyDecimals(value: unknown, key?: string): unknown {
  if (typeof value === 'number' && key && DECIMAL_KEYS.has(key)) {
    return exactDecimal(value);
  }
  if (Array.isArray(value)) return value.map((item) => stringifyDecimals(item));
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([entryKey, entryValue]) => [
        entryKey,
        stringifyDecimals(entryValue, entryKey),
      ])
    );
  }
  return value;
}

/** Serializes Decimal-backed fields as strings before serde_json reaches Rust. */
export function serializePayrollRequestForWasm(request: PayrollCalculationRequest): string {
  return JSON.stringify(stringifyDecimals(request));
}

/** Parses a Rust result and keeps the existing frontend number-based view model. */
export function parseWasmPayrollResult(json: string): BordroKaydi {
  const value: unknown = JSON.parse(json);
  if (!value || typeof value !== 'object') {
    throw new Error('WASM bordro sonucu geçersiz.');
  }
  return value as BordroKaydi;
}

export function isDecimalBoundaryKey(key: string): boolean {
  return DECIMAL_KEYS.has(key);
}

