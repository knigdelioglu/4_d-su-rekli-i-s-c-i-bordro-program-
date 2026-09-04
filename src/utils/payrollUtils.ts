/**
 * Test-only legacy TypeScript payroll fixture helpers.
 *
 * Production code must import `payrollPresentation` for display/input helpers
 * and must use PayrollEngine for payroll results. `verify:production-graph`
 * fails if this module or its calculation exports enter the application graph.
 */

import {
  BordroDonemi,
  BordroKaydi,
  DevredenPekKaydi,
  DönemselKurumDegerleri,
  GelirKalemleri,
  GvHesapDetayi,
  IsPrimiGrubu,
  IsPrimiGrupItem,
  IsPrimiHesapDetayi,
  KesintiKalemleri,
  PekDetayi,
  Personel,
  PuantajKodu,
  PuantajOzeti,
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

/**
 * Standard 4 Tediye per year for 4/D Public Workers (6772 sayılı Kanun)
 */
export const DEFAULT_TEDIYE_LISTESI: TediyeKalemi[] = [
  { id: 1, ad: '1. Tediye', odemeAyi: 'Ocak', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 2, ad: '2. Tediye', odemeAyi: 'Nisan', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 3, ad: '3. Tediye', odemeAyi: 'Temmuz', gunSayisi: 13, aktifDonemdeOdensin: false },
  { id: 4, ad: '4. Tediye', odemeAyi: 'Aralık', gunSayisi: 13, aktifDonemdeOdensin: false },
];

/**
 * Standard 2 TİS İkramiyesi per year for 4/D Public Workers (Toplu İş Sözleşmesi)
 */
export const DEFAULT_TIS_IKRAMIYE_LISTESI: TisIkramiyeKalemi[] = [
  { id: 1, ad: '1. TİS İkramiyesi', odemeAyi: '', gunSayisi: 0, aktifDonemdeOdensin: false },
  { id: 2, ad: '2. TİS İkramiyesi', odemeAyi: '', gunSayisi: 0, aktifDonemdeOdensin: false },
];

export const DEFAULT_IS_PRIMI_GRUPLARI: IsPrimiGrupItem[] = [
  { id: '1. Grup', ad: '1. Grup', oran: 9, aktif: true },
  { id: '2. Grup', ad: '2. Grup', oran: 8, aktif: true },
  { id: '3. Grup', ad: '3. Grup', oran: 7, aktif: true },
];

/**
 * Default institution values according to 4/D specifications
 */
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
  tediyeTisNotu: "Tediye ve TİS listeleri yalnız referans takvimidir. Ödeme ayı ve gün sayısı burada not edilebilir; bordroya aktarılacak gerçek brüt Tediye/TİS tutarı Bordro Hesaplama ekranında personel ve dönem bazında manuel girilir.",

  // Default Kesinti Kalemleri & Yasal Oranlar
  sgkIsciOraniYuzde: 14,
  issizlikIsciOraniYuzde: 1,
  gelirVergisiOraniYuzde: 15,
  damgaVergisiOraniBinde: 7.59,
  sendikaAidatiYuzde: 65, // Günlük Çıplak Ücretin %65'i
  sabitSendikaAidati: 0,
  besOraniYuzde: 3,
  sabitBesTutar: 0,

  // 2026 SGK PEK (Prime Esas Kazanç) Varsayılan Parametreleri
  gunlukYemekIstisnasiSGK: 300.00, // Dönem baseline değeri; mevzuat değişimi segment ile girilebilir.
  gunlukYemekIstisnasiGV: 300.00, // SGK'dan bağımsız GV yemek istisnası baseline değeri.
  statutoryParameterSegments: [],
  pekTavanKatsayisi: 9, // 2026 PEK Tavan Katsayısı = 9
  gunlukAsgariUcret: 1101.00, // 2026 Günlük Brüt Asgari Ücret (TL) - PEK Alt Sınır Birimi

  // İşveren Prim Oranları
  sgkIsverenOraniYuzde: 21.75, // SGK İşveren Prim Oranı = %21,75
  issizlikIsverenOraniYuzde: 2.00, // İşveren İşsizlik Sigortası Prim Oranı = %2,00
};

/**
 * Generates 15th to 14th payroll period
 * Example: year = 2026, month = 1 (Ocak) -> 15.01.2026 to 14.02.2026
 */
export function createBordroDonemi(
  yil: number,
  ay: number,
  taxYear?: number,
  taxMonth?: number
): BordroDonemi {
  const pad = (n: number) => n.toString().padStart(2, '0');
  
  // Baslangic: Yil-Ay-15
  const baslangicTarihi = `${yil}-${pad(ay)}-15`;

  // Bitis: Sonraki ayin 14'u
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

  // Vergi (ödeme/tahakkuk) ayı: varsayılan öneri = bitiş ayı (ay + 1; Aralık → Ocak, yıl +1).
  // Kullanıcı seçeneği varsa (taxMonth/taxYear) onu kullan; yoksa bitiş ayı varsayımına dön.
  let deflTaxAy = ay + 1;
  let deflTaxYil = yil;
  if (deflTaxAy > 12) {
    deflTaxAy = 1;
    deflTaxYil = yil + 1;
  }
  const resTaxYear = taxYear !== undefined ? taxYear : deflTaxYil;
  const resTaxMonth = taxMonth !== undefined ? taxMonth : deflTaxAy;

  return {
    id: donemId,
    yil,
    ay,
    baslangicTarihi,
    bitisTarihi,
    donemAdi,
    taxYear: resTaxYear,
    taxMonth: resTaxMonth,
  };
}

export interface GunDetay {
  dateStr: string; // YYYY-MM-DD
  dayNumber: number; // 1-31
  dayOfWeek: number; // 0-6 (0 = Sunday)
  dayNameShort: string;
  isWeekend: boolean;
  isSunday: boolean;
}

/**
 * Returns array of day details between baslangicTarihi and bitisTarihi
 */
export function getPeriodDaysList(baslangicTarihi: string, bitisTarihi: string): GunDetay[] {
  const days: GunDetay[] = [];
  const start = new Date(baslangicTarihi + 'T00:00:00');
  const end = new Date(bitisTarihi + 'T00:00:00');

  const curr = new Date(start);
  while (curr <= end) {
    const pad = (n: number) => n.toString().padStart(2, '0');
    const year = curr.getFullYear();
    const month = pad(curr.getMonth() + 1);
    const dayNum = pad(curr.getDate());
    const dateStr = `${year}-${month}-${dayNum}`;

    const dayOfWeek = curr.getDay(); // 0 is Sunday
    const isSunday = dayOfWeek === 0;
    const isWeekend = dayOfWeek === 0 || dayOfWeek === 6;

    days.push({
      dateStr,
      dayNumber: curr.getDate(),
      dayOfWeek,
      dayNameShort: GUN_ISIMLERI_KISA[dayOfWeek],
      isWeekend,
      isSunday,
    });

    curr.setDate(curr.getDate() + 1);
  }

  return days;
}

/**
 * Calculates Puantaj Summary totals
 */
export function calculatePuantajOzeti(gunler: Record<string, PuantajKodu>): PuantajOzeti {
  const summary: PuantajOzeti = {
    Ç: 0,
    T: 0,
    G: 0,
    İ: 0,
    GÇ: 0,
    GÇT: 0,
    R: 0,
  };

  Object.values(gunler).forEach((kod) => {
    if (summary[kod] !== undefined) {
      summary[kod]++;
    }
  });

  return summary;
}

/**
 * Generates default attendance codes for a period
 * 4/D public-worker default: Monday-Friday -> Ç, Saturday-Sunday -> T.
 */
export function generateDefaultPuantajGunler(
  baslangicTarihi: string,
  bitisTarihi: string
): Record<string, PuantajKodu> {
  const days = getPeriodDaysList(baslangicTarihi, bitisTarihi);
  const gunler: Record<string, PuantajKodu> = {};

  days.forEach((day) => {
    gunler[day.dateStr] = day.isWeekend ? 'T' : 'Ç'; // Hafta tatili / çalışılan gün
  });

  return gunler;
}

/**
 * Gets empty Income object with nulls
 */
export function getEmptyGelirler(): GelirKalemleri {
  return {
    tabanBrutAylik: null,
    tediye: null,
    tisIkramiyesi: null,
    ekOdeme: null,
    yemek: null,
    birlestirilmisSosyalYardim: null,
    vasitaYol: null,
    giyimYardimi: null,
    isPrimi: null,
    hizmetZammi: null,
    digerGelir: null,
  };
}

/**
 * Gets empty Deductions object with nulls
 */
export function getEmptyKesintiler(): KesintiKalemleri {
  return {
    isciSgkPrimi: null,
    isciIssizlikPrimi: null,
    gelirVergisi: null,
    damgaVergisi: null,
    sendikaAidati: null,
    bes: null,
    icra: null,
    kisiBorcu: null,
    dogumAskerlikBorclanmasi: null,
    hayatSaglikSigortasi: null,
    digerKesinti: null,
  };
}

/**
 * Calculate total Income (treats null as 0 for calculation without overwriting null)
 */
export function calculateGelirToplam(gelirler: GelirKalemleri): number {
  let sum = 0;
  (Object.keys(gelirler) as Array<keyof GelirKalemleri>).forEach((key) => {
    const val = gelirler[key];
    if (typeof val === 'number' && !isNaN(val)) {
      sum += val;
    }
  });
  return Math.round(sum * 100) / 100;
}

/**
 * Calculate total Deductions (treats null as 0 for calculation)
 */
export function calculateKesintiToplam(kesintiler: KesintiKalemleri): number {
  let sum = 0;
  (Object.keys(kesintiler) as Array<keyof KesintiKalemleri>).forEach((key) => {
    const val = kesintiler[key];
    if (typeof val === 'number' && !isNaN(val)) {
      sum += val;
    }
  });
  return Math.round(sum * 100) / 100;
}

/**
 * Calculate Net Payment = Total Income - Total Deductions
 */
export function calculateNetOdeme(gelirToplam: number, kesintiToplam: number): number {
  return Math.round((gelirToplam - kesintiToplam) * 100) / 100;
}

/**
 * Rate increase helper formula:
 * ROUND(Eski × (1 + Zam / 100), 2)
 */
export function calculateZam(eskiTutar: number, zamOrani: number): number {
  if (isNaN(eskiTutar) || isNaN(zamOrani)) return 0;
  const newAmount = eskiTutar * (1 + zamOrani / 100);
  return Math.round(newAmount * 100) / 100;
}

/**
 * Format currency to TL string (e.g. 2.443,28 TL)
 * Returns fallback if null or undefined
 */
export function formatTL(val: number | null | undefined, emptyLabel: string = '—'): string {
  if (val === null || val === undefined) {
    return emptyLabel;
  }
  return (
    val.toLocaleString('tr-TR', {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + ' TL'
  );
}

/**
 * Format Date to Turkish string
 */
export function formatDateTR(dateStr: string): string {
  if (!dateStr) return '';
  const d = new Date(dateStr + 'T00:00:00');
  if (isNaN(d.getTime())) return dateStr;
  const day = d.getDate();
  const month = AY_ISIMLERI[d.getMonth()];
  const year = d.getFullYear();
  return `${day} ${month} ${year}`;
}

/**
 * İş primi grubunu tanımlı (aktif) gruplar arasında çözer.
 * İş primi oranının tek authoritative kaynağı personelin grubudur; `isPrimiYuzde`
 * burada kullanılmaz. Sessiz fallback yok: grup boş/tanımsız, hiçbir aktif kayıtla
 * eşleşmiyor, grup pasif veya oran geçersizse açık hata (Error) fırlatılır.
 */
export function resolveIsPrimiGrubu(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): IsPrimiGrupItem {
  const grp = (grup ?? '').toString().trim();
  if (!grp) {
    throw new Error('Personelin iş primi grubu tanımlı değil.');
  }

  const list = isPrimiGruplari && isPrimiGruplari.length > 0 ? isPrimiGruplari : undefined;
  if (!list) {
    throw new Error(`İş primi grupları tanımlı değil. Personel grubu: '${grp}'. Tanımlı gruplardan birine atayın.`);
  }

  const found = list.find((g) => (g.ad === grp || g.id === grp) && g.aktif !== false);
  if (found) {
    if (found.oran < 0) {
      throw new Error(`İş primi grubu '${found.ad}' oranı geçersiz (negatif).`);
    }
    return found;
  }

  const pasif = list.find((g) => g.ad === grp || g.id === grp);
  if (pasif) {
    throw new Error(`İş primi grubu '${pasif.ad}' pasif durumda ve kullanılamaz.`);
  }

  throw new Error(`Personelin iş primi grubu geçersiz: '${grp}'. Tanımlı gruplardan birini seçin.`);
}

/**
 * İş primi oranını (aktif gruplar içinde) döndürür. Grup çözümlenemiyorsa hata fırlatır.
 */
export function getGrupIsPrimiOrani(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): number {
  return resolveIsPrimiGrubu(grup, isPrimiGruplari).oran;
}

/**
 * Görsel gösterim için güvenli varyant: grup çözümlenemezse `null` döner
 * (asla tahmini bir oran/fallback üretmez). Hesaplama için kullanılmaz.
 */
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

/**
 * İş primi hesap detayı: tek-final-rounding uygular.
 *   tutar = round2(günlük_taban × oran / 100 × hak_günü)
 * Günlük değer (günlük × oran/100) yalnız görsel gösterim içindir; bordro toplamının
 * authoritative girdisi değildir.
 */
export function calculateIsPrimiDetayi(
  gunlukTabanUcret: number,
  isPrimiHakGunu: number,
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): IsPrimiHesapDetayi {
  const item = resolveIsPrimiGrubu(grup, isPrimiGruplari);
  const oranKatsayi = item.oran / 100;
  const gunlukIsPrimi = Math.round(gunlukTabanUcret * oranKatsayi * 100) / 100;
  const tutar =
    Math.round(gunlukTabanUcret * oranKatsayi * isPrimiHakGunu * 100) / 100;

  return {
    grupId: item.id,
    grupAd: item.ad,
    oran: item.oran,
    hakGunu: isPrimiHakGunu,
    gunlukIsPrimi,
    tutar,
  };
}

/**
 * Auto fill Gelirler from Puantaj + Kurum Değerleri
 * Formula logic for 4/D public workers:
 * - Taban / Brüt Aylık = (Ç + T + G + İ) * Günlük Taban Ücret
 * - Yemek = Ç * Günlük Yemek Tutarı (or Ç + GÇ)
 * - Vasıta / Yol = Ç * Günlük Vasıta Tutarı
 * - Birleştirilmiş Sosyal Yardım = Aylık maktu (Kurum Değeri)
 * - Giyim Yardımı = (Kurum Değeri)
 * - Hizmet Zammı = Hizmet Yılı * Hizmet Zammı Birimi (24.67 TL * Yıl)
 * - İş Primi = ROUND(Günlük Çıplak Ücret * Oran, 2) * Hak Kazanılan Gün (Ç)
 */
export function autoFillGelirlerFromPuantaj(
  puantajOzeti: PuantajOzeti,
  kurumDegerleri: DönemselKurumDegerleri,
  hizmetYili: number,
  grup: IsPrimiGrubu = '1. Grup'
): GelirKalemleri {
  // Payed days count = Worked + Rest + Public Holiday + Paid Leave + Night Work + Night Rest
  const hakedisGunSayisi =
    puantajOzeti.Ç +
    puantajOzeti.T +
    puantajOzeti.G +
    puantajOzeti.İ +
    (puantajOzeti.GÇ || 0) +
    (puantajOzeti.GÇT || 0);

  const tabanBrutAylik = Math.round(hakedisGunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;

  // Actual worked days = Ç + GÇ
  const fiiliCalismaGunu = puantajOzeti.Ç + (puantajOzeti.GÇ || 0);

  // Meals applied for actual worked days (Ç + GÇ)
  const yemek = Math.round(fiiliCalismaGunu * kurumDegerleri.gunlukYemek * 100) / 100;

  // Transit applied for actual worked days (Ç + GÇ)
  const vasitaYol = Math.round(fiiliCalismaGunu * kurumDegerleri.gunlukVasitaYol * 100) / 100;

  // Social assistance flat monthly
  const birlestirilmisSosyalYardim = kurumDegerleri.birlestirilmisSosyalYardim || 0;

  // Clothing allowance flat or 0
  const giyimYardimi = kurumDegerleri.giyimYardimi || 0;

  // Service seniority bonus = Hizmet yılı * birim (24.67 TL/yıl)
  const hizmetZammi = Math.round((hizmetYili || 0) * (kurumDegerleri.hizmetZammiBirimi || 24.67) * 100) / 100;

  // Work premium based on worker Group (strict, single final rounding).
  // Yetkili kaynak personelin grubudur; kurumsal `isPrimiYuzde` hesapta rol oynamaz.
  // Hak günü = Ç + GÇ.  Grup çözümlenemezse açık hata fırlatılır (sessiz fallback yok).
  //   tutar = round2(günlük_taban × oran / 100 × hak_günü)
  const isPrimiHakGunu = fiiliCalismaGunu;
  const isPrimi = calculateIsPrimiDetayi(
    kurumDegerleri.gunlukTabanUcret,
    isPrimiHakGunu,
    grup,
    kurumDegerleri.isPrimiGruplari
  ).tutar;

  // Night Work premium (GÇ): Daily base * rate * GÇ days
  const gcOrani = kurumDegerleri.geceCalismaPrimiYuzde || 0;
  const geceCalismasiUcreti =
    gcOrani > 0 && (puantajOzeti.GÇ || 0) > 0
      ? Math.round(kurumDegerleri.gunlukTabanUcret * (gcOrani / 100) * (puantajOzeti.GÇ || 0) * 100) / 100
      : 0;

  // Night Rest premium (GÇT): Daily base * rate * GÇT days
  const gctOrani = kurumDegerleri.geceCalismaTatiliPrimiYuzde || 0;
  const geceCalismasiTatiliUcreti =
    gctOrani > 0 && (puantajOzeti.GÇT || 0) > 0
      ? Math.round(kurumDegerleri.gunlukTabanUcret * (gctOrani / 100) * (puantajOzeti.GÇT || 0) * 100) / 100
      : 0;

  // Tediye ve TİS ikramiyesi manual-only ürün girdileridir.
  // Legacy browser helper da dönem listesinden otomatik tutar üretmez.
  const tediye: number | null = null;
  const tisIkramiyesi: number | null = null;

  return {
    tabanBrutAylik,
    tediye,
    tisIkramiyesi,
    ekOdeme: kurumDegerleri.ekOdeme || null,
    yemek,
    birlestirilmisSosyalYardim,
    vasitaYol,
    giyimYardimi,
    isPrimi,
    geceCalismasiUcreti,
    geceCalismasiTatiliUcreti,
    hizmetZammi,
    digerGelir: kurumDegerleri.digerGelirVarsayilan || null,
  };
}

/**
 * Legacy TypeScript payroll fixture engine.
 *
 * The production payroll path is Rust and reads annual parameters from SQLite.
 * These functions remain only for the existing TypeScript regression fixtures;
 * application components must not use them to create or persist payrolls.
 *
 * 2026 Gelir Vergisi Kümülatif Vergi Tarifesi
 * 1. Dilim: 190.000 TL'ye kadar %15
 * 2. Dilim: 190.000 - 400.000 TL arası %20
 * 3. Dilim: 400.000 - 1.500.000 TL arası %27
 * 4. Dilim: 1.500.000 - 5.300.000 TL arası %35
 * 5. Dilim: 5.300.000 TL üzeri %40
 */
export const GELIR_VERGISI_DILIMLERI_2026 = [
  { limit: 190000, oran: 0.15 },
  { limit: 400000, oran: 0.20 },
  { limit: 1500000, oran: 0.27 },
  { limit: 5300000, oran: 0.35 },
  { limit: Infinity, oran: 0.40 },
];

export function calculateTotalTaxForCumulativeMatrah(kumulatif: number): number {
  if (kumulatif <= 0) return 0;
  let remaining = kumulatif;
  let totalTax = 0;
  let previousLimit = 0;

  for (const bracket of GELIR_VERGISI_DILIMLERI_2026) {
    const bracketSize = bracket.limit - previousLimit;
    const taxableInBracket = Math.min(remaining, bracketSize);
    if (taxableInBracket > 0) {
      totalTax += taxableInBracket * bracket.oran;
      remaining -= taxableInBracket;
    }
    previousLimit = bracket.limit;
    if (remaining <= 0) break;
  }
  return totalTax;
}

/**
 * Gelir vergisi kalemlerinin parasal yuvarlama politikası (GİB uygulaması).
 * Yarım kuruşluk değerler sıfırdan uzağa yuvarlanır (banker's rounding değil):
 * Ocak asgari istisnası tax(28.075,50) = 4.211,325 → 4.211,33.
 * Yalnız GV kalemlerinde kullanılır; `Math.round` (genel 2 hane) ve SGK yuvarlamasına dokunulmaz.
 */
export function roundGvAmount(val: number): number {
  if (val === 0) return 0;
  const sign = val < 0 ? -1 : 1;
  const n = Math.abs(val);
  // Float hatasından kaçınmak için parseFloat üzerinden çalışan half-up yuvarlama.
  return sign * Math.round((n + Number.EPSILON) * 100) / 100;
}

export function calculateGelirVergisi2026(matrah: number, kumulatifOnceki: number = 0): number {
  if (matrah <= 0) return 0;
  const totalTaxCurrent = calculateTotalTaxForCumulativeMatrah(kumulatifOnceki + matrah);
  const totalTaxPrevious = calculateTotalTaxForCumulativeMatrah(kumulatifOnceki);
  return roundGvAmount(totalTaxCurrent - totalTaxPrevious);
}

/**
 * Asgari ücretin takvim referans aylık GV matrahı:
 * aylık brüt asgari - (işçi SGK + işsizlik). Oranlar 0-1 aralığındadır (ör. 0.14).
 */
export function calculateAylikAsgariUcretGvMatrahi(
  gunlukAsgari: number,
  sgkIsciOrani: number,
  issizlikIsciOrani: number
): number {
  const aylikBrutAsgari = Math.round(gunlukAsgari * 30 * 100) / 100;
  const aylikAsgariSgk = Math.round(aylikBrutAsgari * (sgkIsciOrani + issizlikIsciOrani) * 100) / 100;
  return Math.max(0, Math.round((aylikBrutAsgari - aylikAsgariSgk) * 100) / 100);
}

/**
 * GV matrahı = brüt gelir - işçi SGK - işçi işsizlik (negatif olamaz).
 */
export function calculateGvMatrah(brutGelir: number, isciSgk: number, isciIssizlik: number): number {
  return Math.max(0, brutGelir - isciSgk - isciIssizlik);
}

/**
 * Gelir vergisi bloğunun tam, denetlenebilir hesap detayını döndürür.
 * İstisna BİR kez toplam cari matraha uygulanır (matrah indirimi değil, vergi düşümü);
 * kesilecek GV negatif olamaz; GV kalemlerinde roundG2 kullanılır.
 */
export function calculateGvHesapDetayi(
  cariGvMatrahi: number,
  kumulatifGvMatrahiOnceki: number,
  asgariUcretAylikGvMatrahi: number,
  kumulatifAsgariGvOnceki: number
): GvHesapDetayi {
  const brutGelirVergisi = calculateGelirVergisi2026(cariGvMatrahi, kumulatifGvMatrahiOnceki);
  const asgariUcretGvIstisnasi = calculateGelirVergisi2026(
    asgariUcretAylikGvMatrahi,
    kumulatifAsgariGvOnceki
  );
  const uygulananGvIstisnasi = Math.min(brutGelirVergisi, asgariUcretGvIstisnasi);
  const kesilenGelirVergisi = Math.max(0, brutGelirVergisi - uygulananGvIstisnasi);

  return {
    cariGvMatrahi,
    yeniKumulatifGvMatrahi: Math.round((kumulatifGvMatrahiOnceki + cariGvMatrahi) * 100) / 100,
    brutGelirVergisi: roundGvAmount(brutGelirVergisi),
    asgariUcretGvMatrahi: asgariUcretAylikGvMatrahi,
    asgariUcretReferansKumulatifMatrahi: Math.round((kumulatifAsgariGvOnceki + asgariUcretAylikGvMatrahi) * 100) / 100,
    asgariUcretGvIstisnasi: roundGvAmount(asgariUcretGvIstisnasi),
    uygulananGvIstisnasi: roundGvAmount(uygulananGvIstisnasi),
    kesilenGelirVergisi: roundGvAmount(kesilenGelirVergisi),
  };
}

/**
 * Prime Esas Kazanç (PEK / SGK Matrahı) Hesaplama Motoru (GİB 4/D)
 * Doğru Mevzuat Kuralları:
 * 1. SGK'ya tabi bütün brüt kazançları topla (Taban, Kıdem, Tediye, İkramiye, Vasıta, Giyim vb.)
 * 2. Nakdi vasıta/yol yardımı PEK'e tamamen dahildir (0 istisna).
 * 3. Yemek yardımı: Tarihte/dönemde geçerli günlük SGK yemek istisnası (17.04.2026 sonrası 300 TL) yalnız fiili çalışılan yemek günü sayısı kadar düşülür.
 * 4. 2026 PEK Alt/Üst Sınırı:
 *    - Prim gün sayısına göre belirlenir (Ç + T + G + İ + GÇ + GÇT + R). Günlük alt sınır = 1.101,00 TL, günlük tavan = 9.909,00 TL (katsayı 9).
 *    - 30 prim günü için alt sınır = 33.030,00 TL, üst sınır = 297.270,00 TL.
 * 5. Ücret dışı ödemelerde PEK tavanını aşan kısım takip eden en fazla 2 ayın PEK tavanına devredilir (Devreden PEK).
 */
export function calculatePrimeEsasKazanc(
  gelirler: GelirKalemleri,
  puantajOzeti?: PuantajOzeti,
  kurumDegerleri?: DönemselKurumDegerleri,
  devredenPekGelen: number | DevredenPekKaydi[] = 0
): {
  hesaplananPek: number;
  finalPek: number;
  devredenPekAşanTutar: number;
  sonrakiDevredenList: DevredenPekKaydi[];
  fiiliYemekGunu: number;
  yemekIstisnasiTutar: number;
  pekAltSinir: number;
  pekUstSinir: number;
  altSinirTamamlamaFarki: number;
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  pekAltSinirTamamlamaIsverenPrimi: number;
  isverenPrimToplami: number;
  sgkIsverenOraniYuzde: number;
  isverenIssizlikOraniYuzde: number;
} {
  // Prim gün sayısı (Ç + T + G + İ + GÇ + GÇT + R toplamı, üst sınır 30)
  let rawPrimGun = 0;
  if (puantajOzeti) {
    rawPrimGun =
      (puantajOzeti.Ç || 0) +
      (puantajOzeti.T || 0) +
      (puantajOzeti.G || 0) +
      (puantajOzeti.İ || 0) +
      (puantajOzeti.GÇ || 0) +
      (puantajOzeti.GÇT || 0) +
      (puantajOzeti.R || 0);
  }
  const primGunSayisi = Math.min(30, Math.max(0, rawPrimGun));

  // Fiili yemek günü (Puantajdaki Ç + GÇ gün sayısı)
  const fiiliYemekGunu = (puantajOzeti?.Ç ?? 0) + (puantajOzeti?.GÇ ?? 0);

  // Günlük SGK Yemek İstisnası (17.04.2026 sonrası SGK 2026/12 Genelgesi uyarınca 300.00 TL)
  const gunlukYemekIstisnasi = kurumDegerleri?.gunlukYemekIstisnasiSGK ?? 300.00;

  const brutYemek = gelirler.yemek || 0;
  // Yemek yardımı istisnai düşüm
  const yemekIstisnasiTutar = Math.min(brutYemek, gunlukYemekIstisnasi * fiiliYemekGunu);
  const sgkTabiYemek = Math.max(0, brutYemek - yemekIstisnasiTutar);

  // Nakdi Vasıta / Yol yardımı PEK'e TAMAMEN dahildir (0 istisna)
  const vasitaYol = gelirler.vasitaYol || 0;

  // Diğer brüt gelir kalemleri
  const tabanBrut = gelirler.tabanBrutAylik || 0;
  const tediye = gelirler.tediye || 0;
  const tisIkramiyesi = gelirler.tisIkramiyesi || 0;
  const ekOdeme = gelirler.ekOdeme || 0;
  const birlestirilmisSosyalYardim = gelirler.birlestirilmisSosyalYardim || 0;
  const giyimYardimi = gelirler.giyimYardimi || 0;
  const isPrimi = gelirler.isPrimi || 0;
  const geceCalismasi = gelirler.geceCalismasiUcreti || 0;
  const geceCalismasiTatili = gelirler.geceCalismasiTatiliUcreti || 0;
  const hizmetZammi = gelirler.hizmetZammi || 0;
  const digerGelir = gelirler.digerGelir || 0;

  // Normal maaş ve sürekli haklar (Ücretler)
  const ucretler =
    tabanBrut +
    sgkTabiYemek +
    vasitaYol +
    birlestirilmisSosyalYardim +
    giyimYardimi +
    hizmetZammi +
    digerGelir +
    geceCalismasi +
    geceCalismasiTatili;

  // Ücret Dışı Ödemeler (5510 S.K. 80/d maddesi uyarınca tavan aşımı sonraki 2 aya devreden prim/ikramiye/tediye ödemeleri)
  const ucretDisiOdemeler = tediye + tisIkramiyesi + ekOdeme + isPrimi;

  // Toplam SGK tabi kazanç
  const hamPek = ucretler + ucretDisiOdemeler;

  // Günlük Brüt Asgari Ücret (2026 resmî PEK alt sınır birimi = 1.101,00 TL)
  const gunlukAsgariUcret = kurumDegerleri?.gunlukAsgariUcret ?? 1101.00;

  // Prim gün sayısına göre PEK Alt Sınırı (30 gün için 33.030,00 TL)
  const pekAltSinir = Math.round(gunlukAsgariUcret * primGunSayisi * 100) / 100;

  // PEK Tavan Katsayısı = 9 -> Günlük Üst Sınır = 1.101 * 9 = 9.909,00 TL
  const tavanKatsayisi = kurumDegerleri?.pekTavanKatsayisi ?? 9;
  // Prim gün sayısına göre PEK Üst Sınırı (30 gün için 297.270,00 TL)
  const pekUstSinir = Math.round(gunlukAsgariUcret * tavanKatsayisi * primGunSayisi * 100) / 100;

  // Devreden PEK kayıtlarını standardize et
  let devredenList: DevredenPekKaydi[] = [];
  if (typeof devredenPekGelen === 'number') {
    if (devredenPekGelen > 0) {
      devredenList = [{ tutar: devredenPekGelen, kalanAySayisi: 2 }];
    }
  } else if (Array.isArray(devredenPekGelen)) {
    devredenList = devredenPekGelen.map((item) => ({ ...item }));
  }

  let pekMatrahAdayi = hamPek;
  let eklenecekDevredenToplam = 0;
  const sonrakiDevredenList: DevredenPekKaydi[] = [];

  // Önceki aylardan gelen devreden bakiyeleri tavan boşluğu oranında kullan
  for (const item of devredenList) {
    if (item.tutar <= 0 || item.kalanAySayisi <= 0) continue;

    const tavanBoslugu = Math.max(0, pekUstSinir - pekMatrahAdayi);
    const eklenecek = Math.min(item.tutar, tavanBoslugu);

    if (eklenecek > 0) {
      pekMatrahAdayi += eklenecek;
      eklenecekDevredenToplam += eklenecek;
    }

    const kalanTutar = Math.round((item.tutar - eklenecek) * 100) / 100;
    // Sığmayan bakiye ve henüz 2. ayını tamamlamamış olanlar 1 ay düşürülerek sonraki döneme devreder
    if (kalanTutar > 0 && item.kalanAySayisi > 1) {
      sonrakiDevredenList.push({
        tutar: kalanTutar,
        kalanAySayisi: item.kalanAySayisi - 1,
        kaynakDonemId: item.kaynakDonemId,
      });
    }
  }

  let devredenPekAşanTutar = 0;
  // Tavan kontrolü & Ücret dışı ödemelerin tavanı aşan kısmının sonraki 2 aya devri (5510 S.K. 80/d)
  if (pekMatrahAdayi > pekUstSinir) {
    // 5510 S.K. 80-d Maddesi: Ücretlerin tavanı aşan kısmı devretmez.
    // Yalnızca ücret dışı ödemelerin (tediye, ikramiye vb.) tavan nedeniyle o ay PEK'e dahil edilemeyen kısmı devreder.
    const ucretTavanKapasitesi = Math.max(0, pekUstSinir - ucretler - eklenecekDevredenToplam);
    const ucretDisiKullanilan = Math.min(ucretDisiOdemeler, ucretTavanKapasitesi);
    devredenPekAşanTutar = Math.round(Math.max(0, ucretDisiOdemeler - ucretDisiKullanilan) * 100) / 100;

    pekMatrahAdayi = pekUstSinir;
  }

  // Bu ay tavanı aşan yeni ücret dışı ödeme varsa 2 ay ömürlü olarak devreden listesine ekle
  if (devredenPekAşanTutar > 0) {
    sonrakiDevredenList.push({
      tutar: devredenPekAşanTutar,
      kalanAySayisi: 2,
    });
  }

  // Alt sınır kontrolü
  let finalPek = pekMatrahAdayi;
  const altSinirTamamlamaFarki =
    hamPek > 0 && hamPek < pekAltSinir ? Math.round((pekAltSinir - hamPek) * 100) / 100 : 0;

  if (finalPek < pekAltSinir && hamPek > 0) {
    finalPek = pekAltSinir;
  }

  const isverenSgkRate = (kurumDegerleri?.sgkIsverenOraniYuzde ?? 21.75) / 100;
  const isverenIssizlikRate = (kurumDegerleri?.issizlikIsverenOraniYuzde ?? 2.00) / 100;
  const isciSgkRate = (kurumDegerleri?.sgkIsciOraniYuzde ?? 14) / 100;
  const isciIssizlikRate = (kurumDegerleri?.issizlikIsciOraniYuzde ?? 1) / 100;

  const isverenSgkPrimi = Math.round(finalPek * isverenSgkRate * 100) / 100;
  const isverenIssizlikPrimi = Math.round(finalPek * isverenIssizlikRate * 100) / 100;

  const isverenAltSinirSgkFarki = Math.round(altSinirTamamlamaFarki * isciSgkRate * 100) / 100;
  const isverenAltSinirIssizlikFarki = Math.round(altSinirTamamlamaFarki * isciIssizlikRate * 100) / 100;
  const pekAltSinirTamamlamaIsverenPrimi =
    Math.round((isverenAltSinirSgkFarki + isverenAltSinirIssizlikFarki) * 100) / 100;

  const isverenPrimToplami =
    Math.round((isverenSgkPrimi + isverenIssizlikPrimi + pekAltSinirTamamlamaIsverenPrimi) * 100) / 100;

  return {
    hesaplananPek: Math.round(hamPek * 100) / 100,
    finalPek: Math.round(finalPek * 100) / 100,
    devredenPekAşanTutar,
    sonrakiDevredenList,
    fiiliYemekGunu,
    yemekIstisnasiTutar: Math.round(yemekIstisnasiTutar * 100) / 100,
    pekAltSinir,
    pekUstSinir,
    altSinirTamamlamaFarki,
    isverenSgkPrimi,
    isverenIssizlikPrimi,
    pekAltSinirTamamlamaIsverenPrimi,
    isverenPrimToplami,
    sgkIsverenOraniYuzde: kurumDegerleri?.sgkIsverenOraniYuzde ?? 21.75,
    isverenIssizlikOraniYuzde: kurumDegerleri?.issizlikIsverenOraniYuzde ?? 2.00,
  };
}

export interface StatutoryDeductionsResult extends Partial<KesintiKalemleri> {
  pekResult?: ReturnType<typeof calculatePrimeEsasKazanc>;
  gvDetay?: GvHesapDetayi;
}

/**
 * Statutory & Personel Deductions Auto-Calculator
 * Calculates deductions using period parameters, employee profile and PEK motor:
 * - Worker SGK Pension (İşçi SGK): PEK x 14%
 * - Worker Unemployment (İşsizlik): PEK x 1%
 * - Gelir Vergisi: 2026 Kümülatif Vergi Tarifesi + Asgari Ücret Gelir Vergisi İstisnası (7349 S.K.)
 * - Damga Vergisi: Brüt Gelir x 7.59‰ - Asgari Ücret Damga Vergisi İstisnası (7349 S.K.)
 * - Sendika Aidatı: Günlük Çıplak Ücret (2.443,28 TL) x %65 (Yalnızca sendika üyesi ise)
 * - OKS (Otomatik Katılım Bireysel Emeklilik): PEK x OKS Oranı (%3) (Yalnızca OKS üyesi ise, kuruş atılır: Math.floor).
 * - İcra, Kişi Borcu, Doğum/Askerlik, Sağlık Sigortası, Diğer Kesinti: From personel profile
 */
export function calculateStatutoryDeductions(
  gelirler: GelirKalemleri,
  kurumDegerleri?: DönemselKurumDegerleri,
  personel?: Personel,
  puantajOzeti?: PuantajOzeti,
  kumulatifGvMatrahiOnceki: number = 0,
  devredenPekGelen: number | DevredenPekKaydi[] = 0,
  kumulatifAsgariUcretGvMatrahiOnceki: number = 0
): StatutoryDeductionsResult {
  const brutGelir = calculateGelirToplam(gelirler);
  if (brutGelir <= 0) {
    return {
      isciSgkPrimi: 0,
      isciIssizlikPrimi: 0,
      gelirVergisi: 0,
      damgaVergisi: 0,
      sendikaAidati: 0,
      bes: 0,
      icra: 0,
      kisiBorcu: 0,
      dogumAskerlikBorclanmasi: 0,
      hayatSaglikSigortasi: 0,
      digerKesinti: 0,
    };
  }

  // PEK Motoru üzerinden matrah tespiti
  const pekResult = calculatePrimeEsasKazanc(gelirler, puantajOzeti, kurumDegerleri, devredenPekGelen);
  // Worker deductions must be calculated over real earnings (hesaplananPek / ham_pek) capped at ceiling,
  // NOT on the artificially inflated floor finalPek (5510 m.82 & 4447 m.49).
  const workerPekMatrah = Math.min(pekResult.hesaplananPek, pekResult.pekUstSinir);

  // Configured or default rates
  const sgkRate = (kurumDegerleri?.sgkIsciOraniYuzde ?? 14) / 100;
  const issizlikRate = (kurumDegerleri?.issizlikIsciOraniYuzde ?? 1) / 100;
  const dvPerThousand = kurumDegerleri?.damgaVergisiOraniBinde ?? 7.59;
  const dvRate = dvPerThousand / 1000;

  // SGK Primi ve İşsizlik workerPekMatrah üzerinden
  const isciSgkPrimi = Math.round(workerPekMatrah * sgkRate * 100) / 100;
  const isciIssizlikPrimi = Math.round(workerPekMatrah * issizlikRate * 100) / 100;

  // Gelir vergisi matrahı = Brüt - İşçi SGK - İşçi İşsizlik
  const gelirVergisiMatrah = Math.max(0, brutGelir - isciSgkPrimi - isciIssizlikPrimi);

  // Asgari Ücret Gelir Vergisi ve Damga Vergisi İstisnası (7349 Sayılı Kanun)
  // İstisna tutarı; eksik gün veya çalışma gün sayısına bakılmaksızın, dönemin yasal aylık brüt asgari ücreti (30 gün) üzerinden hesaplanır.
  const gunlukAsgariUcret = kurumDegerleri?.gunlukAsgariUcret ?? 1101.00;
  const aylikBrutAsgariUcret = Math.round(gunlukAsgariUcret * 30 * 100) / 100;
  const aylikAsgariSgk = Math.round(aylikBrutAsgariUcret * (sgkRate + issizlikRate) * 100) / 100;
  const asgariUcretGvMatrah = Math.max(0, aylikBrutAsgariUcret - aylikAsgariSgk);

  // GV bloğu GİB uyumlu tam hesap detayı üzerinden (yalnız GV kalemlerinde roundGvAmount)
  const gvDetay = calculateGvHesapDetayi(
    gelirVergisiMatrah,
    kumulatifGvMatrahiOnceki,
    asgariUcretGvMatrah,
    kumulatifAsgariUcretGvMatrahiOnceki
  );
  const gelirVergisi = gvDetay.kesilenGelirVergisi;

  // Asgari Ücret Damga Vergisi İstisnası (7349 S.K. - Yasal aylık brüt asgari ücret üzerinden)
  const asgariUcretDvIstisnasi = Math.round(aylikBrutAsgariUcret * dvRate * 100) / 100;
  const hamDamgaVergisi = Math.round(brutGelir * dvRate * 100) / 100;
  const damgaVergisi = Math.max(0, Math.round((hamDamgaVergisi - asgariUcretDvIstisnasi) * 100) / 100);

  const pKesintiler = personel?.kesintiler;

  // Sendika Aidatı (Günlük Çıplak Ücret x %65) - Yalnızca sendika üyesi ise
  let sendikaAidati: number | null = null;
  const isSendika = pKesintiler ? pKesintiler.sendikaUyesi === true : false;
  if (isSendika) {
    if (pKesintiler?.sabitSendikaAidati && pKesintiler.sabitSendikaAidati > 0) {
      sendikaAidati = pKesintiler.sabitSendikaAidati;
    } else if (kurumDegerleri?.sabitSendikaAidati && kurumDegerleri.sabitSendikaAidati > 0) {
      sendikaAidati = kurumDegerleri.sabitSendikaAidati;
    } else {
      const gunlukCiplakUcret = kurumDegerleri?.gunlukTabanUcret ?? 2443.28;
      const sendikaOrani = (kurumDegerleri?.sendikaAidatiYuzde ?? 65) / 100;
      sendikaAidati = Math.round(gunlukCiplakUcret * sendikaOrani * 100) / 100;
    }
  } else {
    sendikaAidati = 0;
  }

  // OKS (Otomatik Katılım Bireysel Emeklilik Sistemi) - Yalnızca OKS üyesi ise
  // PEK x %3 (Kuruş kısmı atılır: Math.floor)
  let bes: number | null = null;
  const isOks = pKesintiler ? pKesintiler.besUyesi === true : false;

  if (isOks) {
    if (pKesintiler?.sabitBesTutar && pKesintiler.sabitBesTutar > 0) {
      bes = pKesintiler.sabitBesTutar;
    } else if (kurumDegerleri?.sabitBesTutar && kurumDegerleri.sabitBesTutar > 0) {
      bes = kurumDegerleri.sabitBesTutar;
    } else {
      const customOran = pKesintiler?.oksOraniYuzde;
      const oksOrani = (customOran && customOran > 0 ? customOran : (kurumDegerleri?.besOraniYuzde ?? 3)) / 100;
      const rawOks = workerPekMatrah * oksOrani;
      bes = Math.floor(rawOks);
    }
  } else {
    bes = 0;
  }

  // Personal recurring deduction amounts from employee profile
  const icra = (pKesintiler?.icraTutar && pKesintiler.icraTutar > 0) ? pKesintiler.icraTutar : null;
  const kisiBorcu = (pKesintiler?.kisiBorcuTutar && pKesintiler.kisiBorcuTutar > 0) ? pKesintiler.kisiBorcuTutar : null;
  const dogumAskerlikBorclanmasi = (pKesintiler?.dogumAskerlikBorclanmasiTutar && pKesintiler.dogumAskerlikBorclanmasiTutar > 0) ? pKesintiler.dogumAskerlikBorclanmasiTutar : null;
  const hayatSaglikSigortasi = (pKesintiler?.hayatSaglikSigortasiTutar && pKesintiler.hayatSaglikSigortasiTutar > 0) ? pKesintiler.hayatSaglikSigortasiTutar : null;
  const digerKesinti = (pKesintiler?.digerKesintiTutar && pKesintiler.digerKesintiTutar > 0) ? pKesintiler.digerKesintiTutar : null;

  return {
    isciSgkPrimi,
    isciIssizlikPrimi,
    gelirVergisi,
    damgaVergisi,
    sendikaAidati,
    bes,
    icra,
    kisiBorcu,
    dogumAskerlikBorclanmasi,
    hayatSaglikSigortasi,
    digerKesinti,
    pekResult,
    gvDetay,
  };
}

/**
 * Calculates incoming carryover PEK (Devreden PEK) for a person in the active period
 * from previously saved payroll records in the immediately preceding period.
 */
export function calculateIncomingDevredenPek(
  personelId: string,
  aktifDonem: BordroDonemi,
  bordrolar: BordroKaydi[],
  donemler: BordroDonemi[]
): DevredenPekKaydi[] {
  if (!bordrolar || !donemler || !aktifDonem) return [];

  // Filter periods prior to active period in the timeline, ordered descending by year and month
  const priorPeriods = donemler
    .filter((d) => d.yil < aktifDonem.yil || (d.yil === aktifDonem.yil && d.ay < aktifDonem.ay))
    .sort((a, b) => (b.yil * 12 + b.ay) - (a.yil * 12 + a.ay));

  if (priorPeriods.length === 0) return [];

  const immediatelyPriorPeriod = priorPeriods[0];
  const priorBordro = bordrolar.find(
    (b) => b.personelId === personelId && b.donemId === immediatelyPriorPeriod.id
  );

  if (!priorBordro || !priorBordro.sonrakiDevredenPek) return [];

  return priorBordro.sonrakiDevredenPek;
}

/**
 * Vergi (ödeme/tahakkuk) yılı/ayını döndürür. Normal yeni kayıtlar `taxYear`/
 * `taxMonth` alanını taşır (authoritative). Yalnız bu alanları hiç içermeyen
 * LEGACY kayıtlar için geriye dönük fallback uygulanır: bitiş ayı varsayımı
 * (ay + 1; Aralık → Ocak, yıl +1) — migration backfill ile aynı kural.
 */
export function effectiveTaxOf(d: Pick<BordroDonemi, 'yil' | 'ay' | 'taxYear' | 'taxMonth'>): {
  taxYear: number;
  taxMonth: number;
} {
  const taxYear = d.taxYear ?? (d.ay === 12 ? d.yil + 1 : d.yil);
  const taxMonth = d.taxMonth ?? (d.ay === 12 ? 1 : d.ay + 1);
  return { taxYear, taxMonth };
}

/**
 * Checks if there is a conflict between manual cumulative tax base carryover
 * and existing saved payroll records in the same tax year prior to the carryover start month.
 * Vergi yılı/ayı (taxYear/taxMonth) sıralaması authoritative'dir.
 */
export function checkDevredenGvMatrahConflict(
  personel: Personel,
  aktifDonem: BordroDonemi,
  bordrolar: BordroKaydi[],
  donemler: BordroDonemi[]
): { hasConflict: boolean; conflictingPeriodNames: string[] } {
  if (!personel || !personel.devirKumulatifGvMatrahi || personel.devirKumulatifGvMatrahi <= 0) {
    return { hasConflict: false, conflictingPeriodNames: [] };
  }

  const effTaxYear = effectiveTaxOf(aktifDonem).taxYear;

  // Devir yılı aktif dönemin VERGİ YILINDAN farklıysa aktif vergi yılında çakışma yok.
  if (personel.devirKumulatifGvMatrahiYili && personel.devirKumulatifGvMatrahiYili !== effTaxYear) {
    return { hasConflict: false, conflictingPeriodNames: [] };
  }

  const startMonth = personel.devirKumulatifGvMatrahiBaslangicAyi || 1;
  // Legacy başlangıç alanı çalışma ayıdır; vergi ayına çevrilir (bitiş ayı kuralı).
  const startTaxMonth = startMonth === 12 ? 1 : startMonth + 1;

  if (startTaxMonth <= 1) {
    return { hasConflict: false, conflictingPeriodNames: [] };
  }

  // Devirden önce aynı vergi yılında vergi ayı startTaxMonth'tan küçük olan kayıtlı bordrolar çakışır.
  const priorPeriodMap = new Map(
    donemler
      .filter((d) => {
        const t = effectiveTaxOf(d);
        return t.taxYear === effTaxYear && t.taxMonth < startTaxMonth;
      })
      .map((d) => [d.id, d.donemAdi])
  );

  const conflictingNames: string[] = [];
  for (const b of bordrolar) {
    if (b.personelId === personel.id && priorPeriodMap.has(b.donemId)) {
      conflictingNames.push(priorPeriodMap.get(b.donemId)!);
    }
  }

  return {
    hasConflict: conflictingNames.length > 0,
    conflictingPeriodNames: conflictingNames,
  };
}

/**
 * Calculates previous cumulative Income Tax Base (Kümülatif Gelir Vergisi Matrahı)
 * for a person from the tax-year opening (devir) + previous saved payroll records.
 * Sıralama ve yıl filtresi taxYear/taxMonth üzerinden çalışır (çalışma yılı/ayı
 * authoritative değildir); yıl geçişinde önceki takvim yılı kümülatifi taşınmaz.
 * Throws explicit conflict error if devir overlaps with prior saved bordros in the same tax year.
 */
export function calculatePreviousCumulativeGvMatrah(
  personelId: string,
  aktifDonem: BordroDonemi,
  bordrolar: BordroKaydi[],
  donemler: BordroDonemi[],
  personel?: Personel
): number {
  let cumulativeMatrah = 0;

  const eff = effectiveTaxOf(aktifDonem);
  const effTaxYear = eff.taxYear;
  const effTaxMonth = eff.taxMonth;

  // Devir matrahı yalnız aktif dönemin VERGİ YILI ile eşleştiğinde kullanılır.
  const hasOpeningForYear = Boolean(
    personel &&
      personel.devirKumulatifGvMatrahi &&
      personel.devirKumulatifGvMatrahi > 0 &&
      (!personel.devirKumulatifGvMatrahiYili || personel.devirKumulatifGvMatrahiYili === effTaxYear)
  );

  if (hasOpeningForYear && personel) {
    const conflict = checkDevredenGvMatrahConflict(personel, aktifDonem, bordrolar, donemler);
    if (conflict.hasConflict) {
      throw new Error(
        `ÇAKIŞMA UYARISI: Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır. Mükerrer vergi matrahını önlemek için devir tutarını veya devir başlangıç dönemini düzeltin.`
      );
    }
    cumulativeMatrah = personel.devirKumulatifGvMatrahi;
  }

  if (!bordrolar || !donemler || !aktifDonem) return cumulativeMatrah;

  // Açılış varsa başlangıç vergi ayı legacy çalışma ayından türetilir; yoksa yıl başından.
  const startTaxMonth =
    hasOpeningForYear && personel?.devirKumulatifGvMatrahiBaslangicAyi
      ? (personel.devirKumulatifGvMatrahiBaslangicAyi === 12
          ? 1
          : personel.devirKumulatifGvMatrahiBaslangicAyi + 1)
      : 1;

  // Aynı vergi yılında başlangıç vergi ayından aktif vergi ayına kadar olan gerçek bordrolar.
  const priorPeriodIds = new Set(
    donemler
      .filter((d) => {
        const t = effectiveTaxOf(d);
        return t.taxYear === effTaxYear && t.taxMonth >= startTaxMonth && t.taxMonth < effTaxMonth;
      })
      .map((d) => d.id)
  );

  for (const b of bordrolar) {
    if (b.personelId === personelId && priorPeriodIds.has(b.donemId)) {
      const isciSgk = b.kesintiler.isciSgkPrimi || 0;
      const isciIssizlik = b.kesintiler.isciIssizlikPrimi || 0;
      const gvMatrah = Math.max(0, b.gelirToplam - isciSgk - isciIssizlik);
      cumulativeMatrah += gvMatrah;
    }
  }

  return Math.round(cumulativeMatrah * 100) / 100;
}

/**
 * Calculates previous cumulative Minimum Wage Income Tax Base (Kümülatif Asgari Ücret Gelir Vergisi Matrahı)
 * for a person in the current period's year from previous saved payroll records (7349 S.K.).
 */
export function calculatePreviousCumulativeAsgariUcretGvMatrah(
  personelId: string,
  aktifDonem: BordroDonemi,
  bordrolar: BordroKaydi[],
  donemler: BordroDonemi[],
  kurumDegerleriMap?: Record<string, DönemselKurumDegerleri>,
  personel?: Personel
): number {
  let cumulativeAsgariMatrah = 0;

  if (personel && personel.devirKumulatifAsgariGvMatrahi) {
    const effTaxYear = effectiveTaxOf(aktifDonem).taxYear;
    if (personel.devirKumulatifAsgariGvMatrahiYili === effTaxYear || !personel.devirKumulatifAsgariGvMatrahiYili) {
      cumulativeAsgariMatrah = personel.devirKumulatifAsgariGvMatrahi;
    }
  }

  if (!aktifDonem || !donemler) return cumulativeAsgariMatrah;

  // Referans kümülatif, dönemin "başlangıç ayı" (ay) DEĞİL vergi (ödeme/tahakkuk)
  // yılı/ayı (taxYear/taxMonth) takvim konumuna dayanır (GİB 7349 S.K.). Eski
  // kayıtlarda alan yoksa bitiş ayı varsayımına geri dönülür (ay + 1; Aralık → Ocak).
  const effTaxYear = effectiveTaxOf(aktifDonem).taxYear;
  const effTaxMonth = effectiveTaxOf(aktifDonem).taxMonth;

  const priorPeriods = donemler.filter((d) => {
    const t = effectiveTaxOf(d);
    return t.taxYear === effTaxYear && t.taxMonth < effTaxMonth;
  });

  for (const period of priorPeriods) {
    // Kümülatif asgari ücret GV matrahı takvim konumuna dayanır (GİB 7349 S.K.):
    // çalışanın kendi bordrosu o ayda kayıtlı olmasa bile o ayın yasal asgari ücret
    // matrahı referansa eklenir. Böylece istisna, hiç kayıtlı bordrosu olmayan
    // çalışanlar için eksik hesaplanmaz.

    const kDegerleri = kurumDegerleriMap?.[period.id] || DEFAULT_KURUM_DEGERLERI;
    const gunlukAsgariUcret = kDegerleri.gunlukAsgariUcret ?? 1101.00;
    const sgkRate = (kDegerleri.sgkIsciOraniYuzde ?? 14) / 100;
    const issizlikRate = (kDegerleri.issizlikIsciOraniYuzde ?? 1) / 100;

    const aylikBrutAsgariUcret = Math.round(gunlukAsgariUcret * 30 * 100) / 100;
    const aylikAsgariSgk = Math.round(aylikBrutAsgariUcret * (sgkRate + issizlikRate) * 100) / 100;
    const aylikAsgariGvMatrah = Math.max(0, aylikBrutAsgariUcret - aylikAsgariSgk);
    cumulativeAsgariMatrah += aylikAsgariGvMatrah;
  }

  return Math.round(cumulativeAsgariMatrah * 100) / 100;
}
