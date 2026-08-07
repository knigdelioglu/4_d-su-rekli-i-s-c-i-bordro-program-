/**
 * 4/D Sürekli İşçi Bordro Programı - Utility Functions
 */

import {
  BordroDonemi,
  BordroKaydi,
  DevredenPekKaydi,
  DönemselKurumDegerleri,
  GelirKalemleri,
  IsPrimiGrubu,
  IsPrimiGrupItem,
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
  { id: '1. Grup', ad: '1. Grup', oran: 9 },
  { id: '2. Grup', ad: '2. Grup', oran: 8 },
  { id: '3. Grup', ad: '3. Grup', oran: 7 },
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
  ekOdeme: 0,
  digerGelirVarsayilan: 0,
  tediyeListesi: DEFAULT_TEDIYE_LISTESI,
  tisIkramiyeListesi: DEFAULT_TIS_IKRAMIYE_LISTESI,
  tediyeTisNotu: "6772 sayılı Kanun uyarınca 4/D kamu çalışanlarına yılda 4 defa ilave tediye (13'er günlük) ve Toplu İş Sözleşmesi (TİS) hükümlerine göre yılda 2 defa ikramiye ödenir. Aşağıdan ödeme aylarını, gün sayılarını (manuel) ve aktif dönemde ödenip ödenmeyeceğini belirleyebilirsiniz.",

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
  gunlukYemekIstisnasiSGK: 300.00, // 2026-08 (17.04.2026 sonrası) Günlük SGK yemek istisnası = 300 TL
  pekTavanKatsayisi: 9, // 2026 PEK Tavan Katsayısı = 9
  gunlukAsgariUcret: 1101.00, // 2026 Günlük Brüt Asgari Ücret (TL) - PEK Alt Sınır Birimi
};

/**
 * Generates 15th to 14th payroll period
 * Example: year = 2026, month = 1 (Ocak) -> 15.01.2026 to 14.02.2026
 */
export function createBordroDonemi(yil: number, ay: number): BordroDonemi {
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

  return {
    id: donemId,
    yil,
    ay,
    baslangicTarihi,
    bitisTarihi,
    donemAdi,
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
 * Weekdays -> Ç, Sunday -> T, Saturday -> Ç or T (Default: Sunday is T, Mon-Sat is Ç)
 */
export function generateDefaultPuantajGunler(
  baslangicTarihi: string,
  bitisTarihi: string
): Record<string, PuantajKodu> {
  const days = getPeriodDaysList(baslangicTarihi, bitisTarihi);
  const gunler: Record<string, PuantajKodu> = {};

  days.forEach((day) => {
    if (day.isSunday) {
      gunler[day.dateStr] = 'T'; // Hafta tatili
    } else {
      gunler[day.dateStr] = 'Ç'; // Çalışılan gün
    }
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
 * İş Primi Grubu Oranı (%9, %8, %7 veya özel tanımlı gruplar)
 */
export function getGrupIsPrimiOrani(
  grup?: IsPrimiGrubu | string,
  isPrimiGruplari?: IsPrimiGrupItem[]
): number {
  const list = isPrimiGruplari && isPrimiGruplari.length > 0
    ? isPrimiGruplari
    : DEFAULT_IS_PRIMI_GRUPLARI;

  if (!grup) return list[0]?.oran ?? 9;

  // Exact match by name or id
  const exact = list.find((g) => g.ad === grup || g.id === grup);
  if (exact) return Number(exact.oran) || 0;

  // Partial match
  const partial = list.find((g) => grup.includes(g.ad) || g.ad.includes(grup));
  if (partial) return Number(partial.oran) || 0;

  // Fallback defaults for legacy strings "1. Grup", "2. Grup", "3. Grup"
  if (grup.includes('1. Grup')) return 9;
  if (grup.includes('2. Grup')) return 8;
  if (grup.includes('3. Grup')) return 7;

  return list[0]?.oran ?? 9;
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
  // Payed days count = Worked + Rest + Public Holiday + Paid Leave
  const hakedisGunSayisi = puantajOzeti.Ç + puantajOzeti.T + puantajOzeti.G + puantajOzeti.İ;
  
  const tabanBrutAylik = Math.round(hakedisGunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;
  
  // Meals applied for actual worked days (Ç)
  const yemek = Math.round(puantajOzeti.Ç * kurumDegerleri.gunlukYemek * 100) / 100;
  
  // Transit applied for actual worked days (Ç)
  const vasitaYol = Math.round(puantajOzeti.Ç * kurumDegerleri.gunlukVasitaYol * 100) / 100;
  
  // Social assistance flat monthly
  const birlestirilmisSosyalYardim = kurumDegerleri.birlestirilmisSosyalYardim || 0;
  
  // Clothing allowance flat or 0
  const giyimYardimi = kurumDegerleri.giyimYardimi || 0;
  
  // Service seniority bonus = Hizmet yılı * birim (24.67 TL/yıl)
  const hizmetZammi = Math.round((hizmetYili || 0) * (kurumDegerleri.hizmetZammiBirimi || 24.67) * 100) / 100;

  // Work premium based on worker Group: 1. Grup = %9, 2. Grup = %8, 3. Grup = %7
  // Formula: Daily İş Primi = ROUND(gunlukTabanUcret * isPrimiOrani / 100, 2)
  // Monthly İş Primi = Daily İş Primi * worked days (Ç)
  const isPrimiOrani = kurumDegerleri.isPrimiYuzde && kurumDegerleri.isPrimiYuzde > 0
    ? kurumDegerleri.isPrimiYuzde
    : getGrupIsPrimiOrani(grup, kurumDegerleri.isPrimiGruplari);
  
  const gunlukIsPrimi = Math.round(kurumDegerleri.gunlukTabanUcret * (isPrimiOrani / 100) * 100) / 100;
  const isPrimiHakGunu = puantajOzeti.Ç || 0;
  const isPrimi = Math.round(gunlukIsPrimi * isPrimiHakGunu * 100) / 100;

  // Active Tediye calculation
  let tediye: number | null = null;
  const activeTediye = kurumDegerleri.tediyeListesi?.find((t) => t.aktifDonemdeOdensin);
  if (activeTediye) {
    tediye = activeTediye.sabitTutar && activeTediye.sabitTutar > 0
      ? activeTediye.sabitTutar
      : Math.round(activeTediye.gunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;
  }

  // Active TİS İkramiyesi calculation
  let tisIkramiyesi: number | null = null;
  const activeTis = kurumDegerleri.tisIkramiyeListesi?.find((t) => t.aktifDonemdeOdensin);
  if (activeTis) {
    tisIkramiyesi = activeTis.sabitTutar && activeTis.sabitTutar > 0
      ? activeTis.sabitTutar
      : Math.round(activeTis.gunSayisi * kurumDegerleri.gunlukTabanUcret * 100) / 100;
  }

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
    hizmetZammi,
    digerGelir: kurumDegerleri.digerGelirVarsayilan || null,
  };
}

/**
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

export function calculateGelirVergisi2026(matrah: number, kumulatifOnceki: number = 0): number {
  if (matrah <= 0) return 0;
  const totalTaxCurrent = calculateTotalTaxForCumulativeMatrah(kumulatifOnceki + matrah);
  const totalTaxPrevious = calculateTotalTaxForCumulativeMatrah(kumulatifOnceki);
  return Math.round((totalTaxCurrent - totalTaxPrevious) * 100) / 100;
}

/**
 * Prime Esas Kazanç (PEK / SGK Matrahı) Hesaplama Motoru (GİB 4/D)
 * Doğru Mevzuat Kuralları:
 * 1. SGK'ya tabi bütün brüt kazançları topla (Taban, Kıdem, Tediye, İkramiye, Vasıta, Giyim vb.)
 * 2. Nakdi vasıta/yol yardımı PEK'e tamamen dahildir (0 istisna).
 * 3. Yemek yardımı: Tarihte/dönemde geçerli günlük SGK yemek istisnası (17.04.2026 sonrası 300 TL) yalnız fiili çalışılan yemek günü sayısı kadar düşülür.
 * 4. 2026 PEK Alt/Üst Sınırı:
 *    - Prim gün sayısına göre belirlenir (Ç + T + G + İ + R). Günlük alt sınır = 1.101,00 TL, günlük tavan = 9.909,00 TL (katsayı 9).
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
} {
  // Prim gün sayısı (Ç + T + G + İ + R toplamı, üst sınır 30)
  let rawPrimGun = 0;
  if (puantajOzeti) {
    rawPrimGun =
      (puantajOzeti.Ç || 0) +
      (puantajOzeti.T || 0) +
      (puantajOzeti.G || 0) +
      (puantajOzeti.İ || 0) +
      (puantajOzeti.R || 0);
  }
  const primGunSayisi = Math.min(30, Math.max(0, rawPrimGun));

  // Fiili yemek günü (Puantajdaki 'Ç' gün sayısı)
  const fiiliYemekGunu = puantajOzeti?.Ç ?? 0;

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
    digerGelir;

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
  if (finalPek < pekAltSinir && hamPek > 0) {
    finalPek = pekAltSinir;
  }

  return {
    hesaplananPek: Math.round(hamPek * 100) / 100,
    finalPek: Math.round(finalPek * 100) / 100,
    devredenPekAşanTutar,
    sonrakiDevredenList,
    fiiliYemekGunu,
    yemekIstisnasiTutar: Math.round(yemekIstisnasiTutar * 100) / 100,
    pekAltSinir,
    pekUstSinir,
  };
}

export interface StatutoryDeductionsResult extends Partial<KesintiKalemleri> {
  pekResult?: ReturnType<typeof calculatePrimeEsasKazanc>;
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
  const pekMatrah = pekResult.finalPek;

  // Configured or default rates
  const sgkRate = (kurumDegerleri?.sgkIsciOraniYuzde ?? 14) / 100;
  const issizlikRate = (kurumDegerleri?.issizlikIsciOraniYuzde ?? 1) / 100;
  const dvPerThousand = kurumDegerleri?.damgaVergisiOraniBinde ?? 7.59;
  const dvRate = dvPerThousand / 1000;

  // SGK Primi ve İşsizlik PEK üzerinden
  const isciSgkPrimi = Math.round(pekMatrah * sgkRate * 100) / 100;
  const isciIssizlikPrimi = Math.round(pekMatrah * issizlikRate * 100) / 100;

  // Gelir vergisi matrahı = Brüt - İşçi SGK - İşçi İşsizlik
  const gelirVergisiMatrah = Math.max(0, brutGelir - isciSgkPrimi - isciIssizlikPrimi);

  // Asgari Ücret Gelir Vergisi ve Damga Vergisi İstisnası (7349 Sayılı Kanun)
  // İstisna tutarı; eksik gün veya çalışma gün sayısına bakılmaksızın, dönemin yasal aylık brüt asgari ücreti (30 gün) üzerinden hesaplanır.
  const gunlukAsgariUcret = kurumDegerleri?.gunlukAsgariUcret ?? 1101.00;
  const aylikBrutAsgariUcret = Math.round(gunlukAsgariUcret * 30 * 100) / 100;
  const aylikAsgariSgk = Math.round(aylikBrutAsgariUcret * (sgkRate + issizlikRate) * 100) / 100;
  const asgariUcretGvMatrah = Math.max(0, aylikBrutAsgariUcret - aylikAsgariSgk);

  // Asgari Ücret GV İstisnası (7349 S.K. - Yasal aylık asgari ücret matrahı ve kümülatif asgari ücret matrahı üzerinden hesaplama)
  const asgariUcretGvIstisnasi = calculateGelirVergisi2026(asgariUcretGvMatrah, kumulatifAsgariUcretGvMatrahiOnceki);
  const hamGelirVergisi = calculateGelirVergisi2026(gelirVergisiMatrah, kumulatifGvMatrahiOnceki);
  const gelirVergisi = Math.max(0, Math.round((hamGelirVergisi - asgariUcretGvIstisnasi) * 100) / 100);

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
      const rawOks = pekMatrah * oksOrani;
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
 * Checks if there is a conflict between manual cumulative tax base carryover
 * and existing saved payroll records in the same year prior to the carryover start month.
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

  // If devir year is different from active period year, no conflict in active period year
  if (personel.devirKumulatifGvMatrahiYili && personel.devirKumulatifGvMatrahiYili !== aktifDonem.yil) {
    return { hasConflict: false, conflictingPeriodNames: [] };
  }

  const startMonth = personel.devirKumulatifGvMatrahiBaslangicAyi || 1;

  if (startMonth <= 1) {
    return { hasConflict: false, conflictingPeriodNames: [] };
  }

  // Find saved bordros for this person in the same year that belong to months PRIOR to startMonth
  const priorPeriodMap = new Map(
    donemler
      .filter((d) => d.yil === aktifDonem.yil && d.ay < startMonth)
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
 * for a person in the current period's year from initial devir value + previous saved payroll records.
 * Throws explicit conflict error if devir overlaps with prior saved bordros in the same year.
 */
export function calculatePreviousCumulativeGvMatrah(
  personelId: string,
  aktifDonem: BordroDonemi,
  bordrolar: BordroKaydi[],
  donemler: BordroDonemi[],
  personel?: Personel
): number {
  let cumulativeMatrah = 0;

  // Use devir base ONLY if its year matches the active period's year
  if (personel && personel.devirKumulatifGvMatrahi && personel.devirKumulatifGvMatrahi > 0) {
    const isSameYear =
      !personel.devirKumulatifGvMatrahiYili ||
      personel.devirKumulatifGvMatrahiYili === aktifDonem.yil;

    if (isSameYear) {
      // Check for conflict with saved prior bordros before start month
      const conflict = checkDevredenGvMatrahConflict(personel, aktifDonem, bordrolar, donemler);
      if (conflict.hasConflict) {
        throw new Error(
          `ÇAKIŞMA UYARISI: Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır. Mükerrer vergi matrahını önlemek için devir tutarını veya devir başlangıç dönemini düzeltin.`
        );
      }
      cumulativeMatrah = personel.devirKumulatifGvMatrahi;
    }
  }

  if (!bordrolar || !donemler || !aktifDonem) return cumulativeMatrah;

  const startMonth =
    personel?.devirKumulatifGvMatrahiYili === aktifDonem.yil &&
    personel?.devirKumulatifGvMatrahiBaslangicAyi
      ? personel.devirKumulatifGvMatrahiBaslangicAyi
      : 1;

  // Filter periods in the same year that occur AT OR AFTER startMonth and PRIOR to active period
  const priorPeriodIds = new Set(
    donemler
      .filter((d) => d.yil === aktifDonem.yil && d.ay >= startMonth && d.ay < aktifDonem.ay)
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
    if (personel.devirKumulatifAsgariGvMatrahiYili === aktifDonem.yil || !personel.devirKumulatifAsgariGvMatrahiYili) {
      cumulativeAsgariMatrah = personel.devirKumulatifAsgariGvMatrahi;
    }
  }

  if (!aktifDonem || !donemler) return cumulativeAsgariMatrah;

  const priorPeriods = donemler.filter(
    (d) => d.yil === aktifDonem.yil && d.ay < aktifDonem.ay
  );

  for (const period of priorPeriods) {
    // Only accumulate minimum wage matrah for periods where employee actually has a saved payroll record
    const hasBordro = bordrolar?.some(
      (b) => b.personelId === personelId && b.donemId === period.id
    );
    if (!hasBordro) continue;

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
