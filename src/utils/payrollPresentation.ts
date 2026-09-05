/**
 * Browser presentation and input helpers.
 *
 * Payroll calculations are intentionally absent from this module. Production
 * payroll results come from PayrollEngine (Tauri or WASM); this file only
 * owns dates, display formatting, attendance presentation, and form defaults.
 */

import type {
  AnnualPayrollParameters,
  BordroDonemi,
  DönemselKurumDegerleri,
  IsPrimiGrubu,
  IsPrimiGrupItem,
  PuantajKodu,
  PuantajOzeti,
  StatutoryParameterSegment,
  TaxBracket,
  TediyeKalemi,
  TisIkramiyeKalemi,
} from '../types/payroll';

export const AY_ISIMLERI = [
  'Ocak',
  'Şubat',
  'Mart',
  'Nisan',
  'Mayıs',
  'Haziran',
  'Temmuz',
  'Ağustos',
  'Eylül',
  'Ekim',
  'Kasım',
  'Aralık',
];

export const GUN_ISIMLERI_KISA = ['Paz', 'Pzt', 'Sal', 'Çar', 'Per', 'Cum', 'Cmt'];
export const GUN_ISIMLERI_UZUN = [
  'Pazar',
  'Pazartesi',
  'Salı',
  'Çarşamba',
  'Perşembe',
  'Cuma',
  'Cumartesi',
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

export const DEFAULT_KURUM_DEGERLERI: Omit<DönemselKurumDegerleri, 'donemId'> = {
  gunlukTabanUcret: 2443.28,
  gunlukYemek: 300.75,
  birlestirilmisSosyalYardim: 5089.70,
  gunlukVasitaYol: 128.93,
  giyimYardimi: 269.70,
  hizmetZammiBirimi: 24.67,
  isPrimiYuzde: 0,
  isPrimiGruplari: DEFAULT_IS_PRIMI_GRUPLARI,
  geceCalismaPrimiYuzde: 0,
  geceCalismaTatiliPrimiYuzde: 0,
  ekOdeme: 0,
  digerGelirVarsayilan: 0,
  tediyeListesi: DEFAULT_TEDIYE_LISTESI,
  tisIkramiyeListesi: DEFAULT_TIS_IKRAMIYE_LISTESI,
  tediyeTisNotu:
    'Tediye ve TİS listeleri yalnız referans takvimidir. Ödeme ayı ve gün sayısı burada not edilebilir; bordroya aktarılacak gerçek brüt Tediye/TİS tutarı Bordro Hesaplama ekranında personel ve dönem bazında manuel girilir.',
  sgkIsciOraniYuzde: 14,
  issizlikIsciOraniYuzde: 1,
  gelirVergisiOraniYuzde: 15,
  damgaVergisiOraniBinde: 7.59,
  sendikaAidatiYuzde: 65,
  sabitSendikaAidati: 0,
  besOraniYuzde: 3,
  sabitBesTutar: 0,
  gunlukYemekIstisnasiSGK: 300.00,
  gunlukYemekIstisnasiGV: 300.00,
  statutoryParameterSegments: [],
  pekTavanKatsayisi: 9,
  gunlukAsgariUcret: 1101.00,
  sgkIsverenOraniYuzde: 21.75,
  issizlikIsverenOraniYuzde: 2.00,
};

const REQUIRED_PERIOD_INCOME_FIELDS = [
  'gunlukTabanUcret',
  'gunlukYemek',
  'birlestirilmisSosyalYardim',
  'gunlukVasitaYol',
  'giyimYardimi',
  'hizmetZammiBirimi',
] as const;

const REQUIRED_PERIOD_LEGAL_NON_NEGATIVE_FIELDS = [
  'sgkIsciOraniYuzde',
  'issizlikIsciOraniYuzde',
  'sendikaAidatiYuzde',
  'besOraniYuzde',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
  'gunlukYemekIstisnasiSGK',
  'gunlukYemekIstisnasiGV',
  'gunlukAsgariUcret',
  'pekTavanKatsayisi',
  'sgkIsverenOraniYuzde',
  'issizlikIsverenOraniYuzde',
] as const;

const PERIOD_PERCENTAGE_FIELDS = [
  'sgkIsciOraniYuzde',
  'issizlikIsciOraniYuzde',
  'sendikaAidatiYuzde',
  'besOraniYuzde',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
  'sgkIsverenOraniYuzde',
  'issizlikIsverenOraniYuzde',
] as const;

/**
 * Persistence-safe sentinel for the open-ended final tax bracket.
 * No browser/shared counterpart exists; keep this in parity with
 * `src-tauri/src/domain/models.rs:11` (`OPEN_ENDED_TAX_BRACKET_LIMIT`).
 */
export const OPEN_ENDED_TAX_BRACKET_LIMIT = 1_000_000_000_000_000;

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isOptionalConstrainedNumber(
  value: unknown,
  constraint: (value: number) => boolean
): boolean {
  return value === undefined || value === null || (isFiniteNumber(value) && constraint(value));
}

/** Strict Gregorian YYYY-MM-DD parsing without Date's rollover behavior. */
function parseCalendarDate(value: unknown): number | null {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;

  const year = Number(value.slice(0, 4));
  const month = Number(value.slice(5, 7));
  const day = Number(value.slice(8, 10));
  if (year <= 0 || month < 1 || month > 12) return null;

  const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const daysInMonth = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][
    month - 1
  ];
  if (!daysInMonth || day < 1 || day > daysInMonth) return null;

  return year * 10000 + month * 100 + day;
}

/**
 * Readiness check for period income and aid values. The payroll domain permits
 * zero for every required value except the daily base wage, which must be
 * positive.
 */
export function hasCompletePeriodIncomeParameters(
  kurumDegerleri: Partial<DönemselKurumDegerleri> | undefined,
  donemId: string
): boolean {
  if (
    !kurumDegerleri ||
    typeof kurumDegerleri.donemId !== 'string' ||
    typeof donemId !== 'string' ||
    kurumDegerleri.donemId !== donemId
  ) {
    return false;
  }

  return REQUIRED_PERIOD_INCOME_FIELDS.every((field) => {
    const value = kurumDegerleri[field];
    if (!isFiniteNumber(value)) return false;
    return field === 'gunlukTabanUcret' ? value > 0 : value >= 0;
  });
}

function hasCompleteIsPrimiGroups(groups: unknown): boolean {
  if (!Array.isArray(groups) || groups.length === 0) return false;

  const activeIds = new Set<string>();
  const activeNames = new Set<string>();

  for (const group of groups) {
    if (!group || typeof group !== 'object' || Array.isArray(group)) return false;
    const candidate = group as Partial<IsPrimiGrupItem>;
    const trimmedId = typeof candidate.id === 'string' ? candidate.id.trim() : '';
    const trimmedName = typeof candidate.ad === 'string' ? candidate.ad.trim() : '';
    if (
      trimmedId.length === 0 ||
      trimmedName.length === 0 ||
      !isFiniteNumber(candidate.oran) ||
      candidate.oran < 0 ||
      candidate.oran > 100 ||
      (candidate.aktif !== undefined && typeof candidate.aktif !== 'boolean')
    ) {
      return false;
    }

    // Rust serde defaults a missing `aktif` field to true; only explicit false
    // is inactive and excluded from the duplicate-active identity sets.
    if (candidate.aktif !== false) {
      if (activeIds.has(trimmedId) || activeNames.has(trimmedName)) return false;
      activeIds.add(trimmedId);
      activeNames.add(trimmedName);
    }
  }

  return true;
}

function hasValidStatutoryParameterSegments(
  kurumDegerleri: Partial<DönemselKurumDegerleri>,
  aktifDonem: BordroDonemi
): boolean {
  const periodStart = parseCalendarDate(aktifDonem.baslangicTarihi);
  const periodEnd = parseCalendarDate(aktifDonem.bitisTarihi);
  if (periodStart === null || periodEnd === null || periodStart > periodEnd) return false;

  const segments = kurumDegerleri.statutoryParameterSegments;
  if (
    segments === undefined ||
    segments === null ||
    (Array.isArray(segments) && segments.length === 0)
  ) {
    return true;
  }
  if (!Array.isArray(segments)) return false;

  let previousDate: number | undefined;
  for (const segment of segments) {
    if (!segment || typeof segment !== 'object' || Array.isArray(segment)) return false;
    const candidate = segment as Partial<StatutoryParameterSegment>;
    const effectiveDate = parseCalendarDate(candidate.effectiveFrom);
    if (
      effectiveDate === null ||
      effectiveDate < periodStart ||
      effectiveDate > periodEnd ||
      (previousDate !== undefined && effectiveDate <= previousDate)
    ) {
      return false;
    }
    previousDate = effectiveDate;

    const hasOverride =
      (candidate.gunlukAsgariUcret !== undefined && candidate.gunlukAsgariUcret !== null) ||
      (candidate.pekTavanKatsayisi !== undefined && candidate.pekTavanKatsayisi !== null) ||
      (candidate.gunlukYemekIstisnasiSGK !== undefined &&
        candidate.gunlukYemekIstisnasiSGK !== null) ||
      (candidate.gunlukYemekIstisnasiGV !== undefined &&
        candidate.gunlukYemekIstisnasiGV !== null);
    if (!hasOverride) return false;

    if (!isOptionalConstrainedNumber(candidate.gunlukAsgariUcret, (value) => value > 0)) {
      return false;
    }
    if (!isOptionalConstrainedNumber(candidate.pekTavanKatsayisi, (value) => value >= 1)) {
      return false;
    }
    if (
      !isOptionalConstrainedNumber(candidate.gunlukYemekIstisnasiSGK, (value) => value >= 0) ||
      !isOptionalConstrainedNumber(candidate.gunlukYemekIstisnasiGV, (value) => value >= 0)
    ) {
      return false;
    }
  }

  return true;
}

/**
 * This readiness helper is the presentation-side fail-closed mirror of the
 * authoritative Rust payroll validation boundary. It intentionally does not
 * apply form defaults: a fallback value is not a saved/current value.
 */
export function hasCompletePeriodLegalParameters(
  kurumDegerleri: Partial<DönemselKurumDegerleri> | undefined,
  aktifDonem: BordroDonemi | undefined
): boolean {
  if (
    !kurumDegerleri ||
    !aktifDonem ||
    typeof kurumDegerleri.donemId !== 'string' ||
    typeof aktifDonem.id !== 'string' ||
    kurumDegerleri.donemId !== aktifDonem.id
  ) {
    return false;
  }

  if (
    !REQUIRED_PERIOD_LEGAL_NON_NEGATIVE_FIELDS.every((field) => {
      const value = kurumDegerleri[field];
      return isFiniteNumber(value) && value >= 0;
    })
  ) {
    return false;
  }

  if (
    !PERIOD_PERCENTAGE_FIELDS.every((field) => {
      const value = kurumDegerleri[field];
      return isFiniteNumber(value) && value >= 0 && value <= 100;
    })
  ) {
    return false;
  }

  const damgaVergisiOraniBinde = kurumDegerleri.damgaVergisiOraniBinde;
  if (
    !isFiniteNumber(damgaVergisiOraniBinde) ||
    damgaVergisiOraniBinde < 0 ||
    damgaVergisiOraniBinde > 1000
  ) {
    return false;
  }

  if (
    !isFiniteNumber(kurumDegerleri.gunlukAsgariUcret) ||
    kurumDegerleri.gunlukAsgariUcret <= 0 ||
    !isFiniteNumber(kurumDegerleri.pekTavanKatsayisi) ||
    kurumDegerleri.pekTavanKatsayisi < 1
  ) {
    return false;
  }

  if (
    ![
      'sabitSendikaAidati',
      'sabitBesTutar',
      'ekOdeme',
      'digerGelirVarsayilan',
    ].every((field) =>
      isOptionalConstrainedNumber(kurumDegerleri[field], (value) => value >= 0)
    )
  ) {
    return false;
  }

  return (
    hasCompleteIsPrimiGroups(kurumDegerleri.isPrimiGruplari) &&
    hasValidStatutoryParameterSegments(kurumDegerleri, aktifDonem)
  );
}

function isValidTaxBracket(bracket: unknown): bracket is TaxBracket {
  if (!bracket || typeof bracket !== 'object') return false;
  const candidate = bracket as Partial<TaxBracket>;
  return (
    isFiniteNumber(candidate.limit) &&
    candidate.limit > 0 &&
    candidate.limit <= OPEN_ENDED_TAX_BRACKET_LIMIT &&
    isFiniteNumber(candidate.oran) &&
    candidate.oran >= 0 &&
    candidate.oran <= 1
  );
}

/**
 * Readiness check for the annual tariff used by the active tax year. The
 * tariff is only accepted when every stored bracket is usable by the payroll
 * engine; no tax value is inferred or rewritten here.
 */
export function hasCompleteAnnualPayrollParameters(
  parameters: Partial<AnnualPayrollParameters> | undefined,
  taxYear: number
): boolean {
  if (
    !parameters ||
    !isFiniteNumber(taxYear) ||
    !Number.isInteger(taxYear) ||
    taxYear <= 0 ||
    !isFiniteNumber(parameters.year) ||
    !Number.isInteger(parameters.year) ||
    parameters.year <= 0 ||
    parameters.year !== taxYear ||
    !Array.isArray(parameters.gelirVergisiDilimleri) ||
    parameters.gelirVergisiDilimleri.length === 0
  ) {
    return false;
  }

  let previousLimit = 0;
  for (const bracket of parameters.gelirVergisiDilimleri) {
    if (!isValidTaxBracket(bracket) || bracket.limit <= previousLimit) return false;
    previousLimit = bracket.limit;
  }

  return isOptionalConstrainedNumber(
    parameters.sigortaGvYillikBrutAsgariUcretTavani,
    (value) => value > 0
  );
}

export function createBordroDonemi(
  yil: number,
  ay: number,
  taxYear?: number,
  taxMonth?: number
): BordroDonemi {
  const pad = (n: number) => n.toString().padStart(2, '0');
  const baslangicTarihi = `${yil}-${pad(ay)}-15`;

  let sonrakiAy = ay + 1;
  let sonrakiYil = yil;
  if (sonrakiAy > 12) {
    sonrakiAy = 1;
    sonrakiYil = yil + 1;
  }
  const bitisTarihi = `${sonrakiYil}-${pad(sonrakiAy)}-14`;

  const ayAdi = AY_ISIMLERI[ay - 1];
  const sonrakiAyAdi = AY_ISIMLERI[sonrakiAy - 1];
  const donemId = `${yil}-${pad(ay)}`;
  const donemAdi = `${ayAdi} ${yil} Dönemi (15 ${ayAdi} - 14 ${sonrakiAyAdi})`;

  let defaultTaxMonth = ay + 1;
  let defaultTaxYear = yil;
  if (defaultTaxMonth > 12) {
    defaultTaxMonth = 1;
    defaultTaxYear = yil + 1;
  }

  return {
    id: donemId,
    yil,
    ay,
    baslangicTarihi,
    bitisTarihi,
    donemAdi,
    taxYear: taxYear ?? defaultTaxYear,
    taxMonth: taxMonth ?? defaultTaxMonth,
  };
}

/** Default accrual date that remains inside the period's authoritative tax month. */
export function getDefaultAccrualPaymentDate(period: BordroDonemi): string {
  const taxPrefix = `${String(period.taxYear).padStart(4, '0')}-${String(period.taxMonth).padStart(2, '0')}-`;
  if (period.bitisTarihi.startsWith(taxPrefix)) return period.bitisTarihi;
  const monthEnd = new Date(Date.UTC(period.taxYear, period.taxMonth, 0));
  return Number.isNaN(monthEnd.getTime())
    ? period.bitisTarihi
    : monthEnd.toISOString().slice(0, 10);
}

export interface GunDetay {
  dateStr: string;
  dayNumber: number;
  dayOfWeek: number;
  dayNameShort: string;
  isWeekend: boolean;
  isSunday: boolean;
}

export function getPeriodDaysList(baslangicTarihi: string, bitisTarihi: string): GunDetay[] {
  const days: GunDetay[] = [];
  const start = new Date(`${baslangicTarihi}T00:00:00`);
  const end = new Date(`${bitisTarihi}T00:00:00`);
  const curr = new Date(start);

  while (curr <= end) {
    const pad = (n: number) => n.toString().padStart(2, '0');
    const year = curr.getFullYear();
    const month = pad(curr.getMonth() + 1);
    const dayNum = pad(curr.getDate());
    const dateStr = `${year}-${month}-${dayNum}`;
    const dayOfWeek = curr.getDay();
    const isSunday = dayOfWeek === 0;

    days.push({
      dateStr,
      dayNumber: curr.getDate(),
      dayOfWeek,
      dayNameShort: GUN_ISIMLERI_KISA[dayOfWeek],
      isWeekend: isSunday || dayOfWeek === 6,
      isSunday,
    });
    curr.setDate(curr.getDate() + 1);
  }

  return days;
}

export function calculatePuantajOzeti(gunler: Record<string, PuantajKodu>): PuantajOzeti {
  const summary: PuantajOzeti = { Ç: 0, T: 0, G: 0, İ: 0, GÇ: 0, GÇT: 0, R: 0 };
  Object.values(gunler).forEach((kod) => {
    if (summary[kod] !== undefined) summary[kod]++;
  });
  return summary;
}

const COMPACT_PUANTAJ_CODES: readonly PuantajKodu[] = ['Ç', 'T', 'G', 'GÇ', 'GÇT', 'İ', 'R'];

export function formatCompactPuantaj(summary: PuantajOzeti): string {
  const entries = COMPACT_PUANTAJ_CODES
    .filter((code) => summary[code] > 0)
    .map((code) => `${summary[code]} ${code}`);
  return entries.length > 0 ? entries.join(' · ') : '0 gün';
}

export function generateDefaultPuantajGunler(
  baslangicTarihi: string,
  bitisTarihi: string
): Record<string, PuantajKodu> {
  const gunler: Record<string, PuantajKodu> = {};
  getPeriodDaysList(baslangicTarihi, bitisTarihi).forEach((day) => {
    // 4/D public-worker default: Monday-Friday work, Saturday-Sunday weekly rest.
    gunler[day.dateStr] = day.isWeekend ? 'T' : 'Ç';
  });
  return gunler;
}

export function formatTL(val: number | null | undefined, emptyLabel: string = '—'): string {
  if (val === null || val === undefined) return emptyLabel;
  return `${val.toLocaleString('tr-TR', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })} TL`;
}

export function formatDateTR(dateStr: string): string {
  if (!dateStr) return '';
  const d = new Date(`${dateStr}T00:00:00`);
  if (Number.isNaN(d.getTime())) return dateStr;
  return `${d.getDate()} ${AY_ISIMLERI[d.getMonth()]} ${d.getFullYear()}`;
}

export function resolveIsPrimiGrubu(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): IsPrimiGrupItem {
  const grp = (grup ?? '').toString().trim();
  if (!grp) throw new Error('Personelin iş primi grubu tanımlı değil.');

  const list = isPrimiGruplari && isPrimiGruplari.length > 0 ? isPrimiGruplari : undefined;
  if (!list) {
    throw new Error(
      `İş primi grupları tanımlı değil. Personel grubu: '${grp}'. Tanımlı gruplardan birine atayın.`
    );
  }

  const found = list.find((g) => (g.ad === grp || g.id === grp) && g.aktif !== false);
  if (found) {
    if (found.oran < 0) throw new Error(`İş primi grubu '${found.ad}' oranı geçersiz (negatif).`);
    return found;
  }

  const inactive = list.find((g) => g.ad === grp || g.id === grp);
  if (inactive) throw new Error(`İş primi grubu '${inactive.ad}' pasif durumda ve kullanılamaz.`);
  throw new Error(`Personelin iş primi grubu geçersiz: '${grp}'. Tanımlı gruplardan birini seçin.`);
}

export function getGrupIsPrimiOrani(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): number {
  return resolveIsPrimiGrubu(grup, isPrimiGruplari).oran;
}

export function getGrupIsPrimiOraniDisplay(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): number | null {
  try {
    return getGrupIsPrimiOrani(grup, isPrimiGruplari);
  } catch {
    return null;
  }
}
