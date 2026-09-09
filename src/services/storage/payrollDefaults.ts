import type {
  AnnualPayrollParameters,
  DönemselKurumDegerleri,
  IsPrimiGrupItem,
  TaxBracket,
  TediyeKalemi,
  TisIkramiyeKalemi,
} from '../../types/payroll';

/**
 * Yasal parametrelerin persistence/bootstrap sözleşmesi.
 * Bu listeye eklenmeyen bir vergi yılı için eski yıl tarifesi üretilmez.
 */
export const SUPPORTED_PAYROLL_DEFAULT_YEARS = [2026] as const;

const DEFAULT_2026_TAX_BRACKETS: TaxBracket[] = [
  { limit: 190000, oran: 0.15 },
  { limit: 400000, oran: 0.20 },
  { limit: 1500000, oran: 0.27 },
  { limit: 5300000, oran: 0.35 },
  { limit: 1_000_000_000_000_000, oran: 0.40 },
];

export const DEFAULT_TEDIYE_LISTESI: TediyeKalemi[] = [
  { id: 1, ad: '1. Tediye', odemeAyi: 'Ocak', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 2, ad: '2. Tediye', odemeAyi: 'Nisan', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 3, ad: '3. Tediye', odemeAyi: 'Temmuz', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 4, ad: '4. Tediye', odemeAyi: 'Aralık', gunSayisi: 13, aktifDonemdeOdensin: false },
];

export const DEFAULT_TIS_IKRAMIYE_LISTESI: TisIkramiyeKalemi[] = [
  { id: 1, ad: '1. TİS İkramiyesi', odemeAyi: '', gunSayisi: 0, aktifDonemdeOdensin: false },
  { id: 2, ad: '2. TİS İkramiyesi', odemeAyi: '', gunSayisi: 0, aktifDonemdeOdensin: false },
];

export const DEFAULT_IS_PRIMI_GRUPLARI: IsPrimiGrupItem[] = [
  { id: '1. Grup', ad: '1. Grup', oran: 9, aktif: true },
  { id: '2. Grup', ad: '2. Grup', oran: 8, aktif: true },
  { id: '3. Grup', ad: '3. Grup', oran: 7, aktif: true },
];

/** Dönem snapshot'ına ait mevzuat değerleri; kurumun ücret politikası değildir. */
export const DEFAULT_STATUTORY_PERIOD_PARAMETERS = {
  sgkIsciOraniYuzde: 14,
  issizlikIsciOraniYuzde: 1,
  gelirVergisiOraniYuzde: 15,
  damgaVergisiOraniBinde: 7.59,
  gunlukYemekIstisnasiSGK: 300,
  gunlukYemekIstisnasiGV: 300,
  pekTavanKatsayisi: 9,
  gunlukAsgariUcret: 1101,
  sgkIsverenOraniYuzde: 21.75,
  issizlikIsverenOraniYuzde: 2,
  statutoryParameterSegments: [],
} satisfies Omit<
  DönemselKurumDegerleri,
  | 'donemId'
  | 'gunlukTabanUcret'
  | 'gunlukYemek'
  | 'birlestirilmisSosyalYardim'
  | 'gunlukVasitaYol'
  | 'giyimYardimi'
  | 'hizmetZammiBirimi'
  | 'isPrimiYuzde'
  | 'isPrimiGruplari'
  | 'geceCalismaPrimiYuzde'
  | 'geceCalismaTatiliPrimiYuzde'
  | 'ekOdeme'
  | 'digerGelirVarsayilan'
  | 'tediyeListesi'
  | 'tisIkramiyeListesi'
  | 'tediyeTisNotu'
  | 'sendikaAidatiYuzde'
  | 'sabitSendikaAidati'
  | 'besOraniYuzde'
  | 'sabitBesTutar'
  | 'statutoryParameterSnapshot'
>;

export function getDefaultAnnualPayrollParameters(
  year: number
): AnnualPayrollParameters | undefined {
  if (year !== 2026) return undefined;
  return {
    year,
    gelirVergisiDilimleri: DEFAULT_2026_TAX_BRACKETS.map((bracket) => ({ ...bracket })),
    sigortaGvYillikBrutAsgariUcretTavani: 396360,
  };
}

export function ensureAnnualPayrollParameters(
  parameters: AnnualPayrollParameters[],
  year: number
): AnnualPayrollParameters[] {
  if (parameters.some((parameter) => parameter.year === year)) return parameters;
  const defaults = getDefaultAnnualPayrollParameters(year);
  return defaults
    ? [...parameters, defaults].sort((left, right) => left.year - right.year)
    : parameters;
}

/**
 * Boş kurulumun dönem başlangıç snapshot'ı. Kurumun ücretleri bilerek sıfırdır;
 * yasal değerler ise desteklenen yıl için deterministik olarak hazırdır.
 */
export const DEFAULT_PRODUCTION_KURUM_DEGERLERI: Omit<DönemselKurumDegerleri, 'donemId'> = {
  gunlukTabanUcret: 0,
  gunlukYemek: 0,
  birlestirilmisSosyalYardim: 0,
  gunlukVasitaYol: 0,
  giyimYardimi: 0,
  hizmetZammiBirimi: 0,
  isPrimiYuzde: 0,
  isPrimiGruplari: [],
  geceCalismaPrimiYuzde: 0,
  geceCalismaTatiliPrimiYuzde: 0,
  ekOdeme: 0,
  digerGelirVarsayilan: 0,
  tediyeListesi: [],
  tisIkramiyeListesi: [],
  tediyeTisNotu:
    'Kurum ücretleri henüz tanımlanmadı. Yasal değerler sistem tarafından hazırlandı; kurumunuza özgü ücretleri bu bölümden girin.',
  ...DEFAULT_STATUTORY_PERIOD_PARAMETERS,
  sendikaAidatiYuzde: 0,
  sabitSendikaAidati: 0,
  besOraniYuzde: 0,
  sabitBesTutar: 0,
};
