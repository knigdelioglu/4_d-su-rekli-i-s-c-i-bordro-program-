/**
 * 4/D Sürekli İşçi Bordro Programı - Types
 */

export type PuantajKodu = 'Ç' | 'T' | 'G' | 'İ' | 'GÇ' | 'GÇT' | 'R';

export interface PuantajKoduBilgi {
  kod: PuantajKodu;
  tanim: string;
  aciklama: string;
  renk: string; // Tailwind class or hex color code
  bgRenk: string;
}

export const PUANTAJ_KODLARI: Record<PuantajKodu, PuantajKoduBilgi> = {
  Ç: {
    kod: 'Ç',
    tanim: 'Çalışılan Gün',
    aciklama: 'Normal çalışma günü',
    renk: 'text-emerald-700 dark:text-emerald-300',
    bgRenk: 'bg-emerald-50 border-emerald-200 text-emerald-800',
  },
  T: {
    kod: 'T',
    tanim: 'Hafta Tatili',
    aciklama: 'Haftalık dinlenme günü',
    renk: 'text-blue-700 dark:text-blue-300',
    bgRenk: 'bg-blue-50 border-blue-200 text-blue-800',
  },
  G: {
    kod: 'G',
    tanim: 'Genel Tatil',
    aciklama: 'Resmi veya dini bayram tatili',
    renk: 'text-purple-700 dark:text-purple-300',
    bgRenk: 'bg-purple-50 border-purple-200 text-purple-800',
  },
  İ: {
    kod: 'İ',
    tanim: 'Ücretli İzin',
    aciklama: 'Yıllık veya mazeret izni',
    renk: 'text-amber-700 dark:text-amber-300',
    bgRenk: 'bg-amber-50 border-amber-200 text-amber-800',
  },
  GÇ: {
    kod: 'GÇ',
    tanim: 'Gece Çalışması',
    aciklama: 'Gece vardiyasında çalışılan saat/gün',
    renk: 'text-indigo-700 dark:text-indigo-300',
    bgRenk: 'bg-indigo-50 border-indigo-200 text-indigo-800',
  },
  GÇT: {
    kod: 'GÇT',
    tanim: 'Gece Çalışması Tatili',
    aciklama: 'Gece çalışması dinlenme günü',
    renk: 'text-teal-700 dark:text-teal-300',
    bgRenk: 'bg-teal-50 border-teal-200 text-teal-800',
  },
  R: {
    kod: 'R',
    tanim: 'Rapor',
    aciklama: 'Hastalık/sağlık raporlu gün',
    renk: 'text-rose-700 dark:text-rose-300',
    bgRenk: 'bg-rose-50 border-rose-200 text-rose-800',
  },
};

export interface PersonelKesintiBilgileri {
  sendikaUyesi?: boolean;
  sabitSendikaAidati?: number; // TL
  besUyesi?: boolean;
  oksOraniYuzde?: number;
  sabitBesTutar?: number;
  icraTutar?: number;
  kisiBorcuTutar?: number;
  dogumAskerlikBorclanmasiTutar?: number;
  hayatSaglikSigortasiTutar?: number;
  digerKesintiTutar?: number;
}

export interface IsPrimiGrupItem {
  id: string;
  ad: string;
  oran: number;
}

export type IsPrimiGrubu = string;

export interface Personel {
  id: string;
  tcNo: string;
  ad: string;
  soyad: string;
  grup: IsPrimiGrubu;
  unvan?: string;
  sgkSicilNo: string;
  iban: string;
  hizmetYili: number;
  aciklama?: string;
  devirKumulatifGvMatrahi?: number;
  devirKumulatifGvMatrahiYili?: number;
  devirKumulatifGvMatrahiBaslangicAyi?: number;
  devirKumulatifAsgariGvMatrahi?: number;
  devirKumulatifAsgariGvMatrahiYili?: number;
  kesintiler?: PersonelKesintiBilgileri;
}

export interface PersonelTaxOpening {
  id: string;
  personnelId: string;
  year: number;
  gvCumulativeOpening: number; // TL
  effectiveFromPeriodId: string; // e.g. "2026-05"
  createdAt?: string;
  updatedAt?: string;
}

export interface BordroDonemi {
  id: string; // e.g. "2026-01"
  yil: number; // e.g. 2026
  ay: number; // 1-12
  baslangicTarihi: string; // "YYYY-MM-DD"
  bitisTarihi: string; // "YYYY-MM-DD"
  donemAdi: string;
}

export interface TediyeKalemi {
  id: number;
  ad: string;
  odemeAyi: string;
  gunSayisi: number;
  aktifDonemdeOdensin: boolean;
  sabitTutar?: number;
}

export interface TisIkramiyeKalemi {
  id: number;
  ad: string;
  odemeAyi: string;
  gunSayisi: number;
  aktifDonemdeOdensin: boolean;
  sabitTutar?: number;
}

export interface DönemselKurumDegerleri {
  donemId: string;
  gunlukTabanUcret: number;
  gunlukYemek: number;
  birlestirilmisSosyalYardim: number;
  gunlukVasitaYol: number;
  giyimYardimi: number;
  hizmetZammiBirimi: number;
  isPrimiYuzde?: number;
  isPrimiGruplari?: IsPrimiGrupItem[];
  geceCalismaPrimiYuzde?: number;
  geceCalismaTatiliPrimiYuzde?: number;
  ekOdeme?: number;
  digerGelirVarsayilan?: number;
  tediyeListesi?: TediyeKalemi[];
  tisIkramiyeListesi?: TisIkramiyeKalemi[];
  tediyeTisNotu?: string;

  sgkIsciOraniYuzde?: number;
  issizlikIsciOraniYuzde?: number;
  gelirVergisiOraniYuzde?: number;
  damgaVergisiOraniBinde?: number;
  sendikaAidatiYuzde?: number;
  sabitSendikaAidati?: number;
  besOraniYuzde?: number;
  sabitBesTutar?: number;

  gunlukYemekIstisnasiSGK?: number;
  pekTavanKatsayisi?: number;
  gunlukAsgariUcret?: number;

  sgkIsverenOraniYuzde?: number;
  issizlikIsverenOraniYuzde?: number;
}

export interface SickLeaveRecord {
  id: string;
  personnelId: string;
  startDate: string; // "YYYY-MM-DD"
  endDate: string;   // "YYYY-MM-DD"
  createdAt?: string;
  updatedAt?: string;
}

export type PuantajOzeti = Record<PuantajKodu, number>;

export interface PersonelPuantaj {
  id: string; // `${personelId}_${donemId}`
  personelId: string;
  donemId: string;
  gunler: Record<string, PuantajKodu>;
}

export interface GelirKalemleri {
  tabanBrutAylik: number | null;
  tediye: number | null;
  tisIkramiyesi: number | null;
  ekOdeme: number | null;
  yemek: number | null;
  birlestirilmisSosyalYardim: number | null;
  vasitaYol: number | null;
  giyimYardimi: number | null;
  isPrimi: number | null;
  geceCalismasiUcreti?: number | null;
  geceCalismasiTatiliUcreti?: number | null;
  hizmetZammi: number | null;
  digerGelir: number | null;
}

export interface KesintiKalemleri {
  isciSgkPrimi: number | null;
  isciIssizlikPrimi: number | null;
  gelirVergisi: number | null;
  damgaVergisi: number | null;
  sendikaAidati: number | null;
  bes: number | null;
  icra: number | null;
  kisiBorcu: number | null;
  dogumAskerlikBorclanmasi: number | null;
  hayatSaglikSigortasi: number | null;
  digerKesinti: number | null;
}

export interface DevredenPekKaydi {
  tutar: number;
  kalanAySayisi: number;
  kaynakDonemId?: string;
}

export interface PekDetayi {
  hesaplananPek: number;
  finalPek: number;
  devredenPekAşanTutar: number;
  pekAltSinir: number;
  pekUstSinir: number;
  fiiliYemekGunu: number;
  yemekIstisnasiTutar: number;
  isverenSgkPrimi?: number;
  isverenIssizlikPrimi?: number;
  isverenPrimToplami?: number;
  sgkIsverenOraniYuzde?: number;
  isverenIssizlikOraniYuzde?: number;
}

export type BordroStatus = 'DRAFT' | 'CALCULATED' | 'FINALIZED';

export interface BordroKaydi {
  id: string; // `${personelId}_${donemId}`
  personelId: string;
  donemId: string;
  puantajOzeti: PuantajOzeti;
  gelirler: GelirKalemleri;
  gelirToplam: number;
  kesintiler: KesintiKalemleri;
  kesintiToplam: number;
  netOdeme: number;
  status?: BordroStatus;
  olusturulmaTarihi: string;
  sonGuncellemeTarihi: string;
  notlar?: string;
  oncekiKumulatifGvMatrahi?: number;
  oncekiKumulatifAsgariGvMatrahi?: number;
  manuelKumulatifGvMatrahi?: number;
  devredenPekGelen?: DevredenPekKaydi[];
  sonrakiDevredenPek?: DevredenPekKaydi[];
  pekDetay?: PekDetayi;
  odenenRaporluGun?: number;
  raporluGun?: number;
}

export interface ZamHesaplama {
  eskiTutar: number;
  zamOrani: number;
  yeniTutar: number;
}
