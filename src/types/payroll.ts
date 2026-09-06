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

export interface GvIndirimGirdileri {
  dogumAskerlikGvIndirimTutar?: number;
  hayatSigortasiPrimiTutar?: number;
  saglikSigortasiPrimiTutar?: number;
}

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
  gvIndirimleri?: GvIndirimGirdileri;
}

export interface IsPrimiGrupItem {
  id: string;
  ad: string;
  oran: number;
  /** Grup aktif/pasif. Pasif gruplar bordro motorunda iş primi oran kaynağı olarak kullanılamaz. */
  aktif?: boolean;
}

export interface IsPrimiHesapDetayi {
  grupId: string;
  grupAd: string;
  oran: number;
  hakGunu: number;
  /** Yalnız gösterim amaçlıdır; bordro toplamının authoritative girdisi değildir. */
  gunlukIsPrimi: number;
  tutar: number;
}

/**
 * Hesaplanan bordronun gelir vergisi bölümünün denetlenebilir snapshot'ı.
 * Çalışanın gerçek kümülatifi (`cariGvMatrahi`/`yeniKumulatifGvMatrahi`) ile
 * asgari ücret istisnasının kendi takvim referansı (`asgariUcretGvMatrahi`/
 * `asgariUcretReferansKumulatifMatrahi`) açıkça ayrılır; GİB uyumlu hesapta
 * `brutGelirVergisi - uygulananGvIstisnasi = kesilenGelirVergisi` sağlanır.
 */
export interface GvHesapDetayi {
  oncekiKumulatifGvMatrahi?: number;
  /** Cari dönem GV matrahı (brüt gelir - işçi SGK - işçi işsizlik). */
  cariGvMatrahi: number;
  /** Cari sonrası gerçek kümülatif matrah (önceki + cari). */
  yeniKumulatifGvMatrahi: number;
  /** İstisna ÖNCESİ hesaplanan gelir vergisi (gerçek kümülatif üzerinden). */
  brutGelirVergisi: number;
  /** Asgari ücretin aylık GV matrahı (takvim referansı). */
  asgariUcretGvMatrahi: number;
  /** Asgari ücret istisnasının takvim referans kümülatif matrahı. */
  asgariUcretReferansKumulatifMatrahi: number;
  /** Asgari ücretin ilgili ay için hesaplanan vergi istisnası hakkı. */
  asgariUcretGvIstisnasi: number;
  ayniAyOncekiKullanilanGvIstisnasi?: number;
  tahakkukOncesiKalanGvIstisnasi?: number;
  /** Gerçekte uygulanan istisna: min(brüt GV, aylık istisna hakkı). */
  uygulananGvIstisnasi: number;
  tahakkukSonrasiKalanGvIstisnasi?: number;
  /** Kesilecek gelir vergisi (negatif olamaz). */
  kesilenGelirVergisi: number;
  dogumAskerlikGvIndirimi?: number;
  sigortaGvIndirimAdayi?: number;
  sigortaGvAylikLimiti?: number;
  sigortaGvYillikKalanLimiti?: number;
  uygulanabilirSigortaGvIndirimi?: number;
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

/** Yıllık kümülatif gelir vergisi tarifesindeki tek dilim. */
export interface TaxBracket {
  limit: number;
  /** 0-1 arası oran (örn. %15 için 0.15). */
  oran: number;
}

/** Vergi yılına bağlı, değiştirilebilir bordro parametreleri. */
export interface AnnualPayrollParameters {
  year: number;
  gelirVergisiDilimleri: TaxBracket[];
  sigortaGvYillikBrutAsgariUcretTavani?: number;
  updatedAt?: string;
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
  ay: number; // 1-12 DÖNEM BAŞLANGIÇ AYI (15'in bulunduğu ay). Dönem adı/id bundan türetilir, anlamı değişmez.
  baslangicTarihi: string; // "YYYY-MM-DD"
  bitisTarihi: string; // "YYYY-MM-DD"
  donemAdi: string;
  /** Vergi yılı (ödeme/tahakkuk yılı). Asgari ücret GV referans kümülatifi ve vergi hesabı bu alan üzerinden yürür. */
  taxYear: number;
  /** Vergi ayı (ödeme/tahakkuk ayı), 1-12. Varsayılan öneri = bitiş ayı (ay + 1). */
  taxMonth: number;
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

export interface StatutoryParameterSegment {
  effectiveFrom: string; // YYYY-MM-DD, active period inclusive
  gunlukAsgariUcret?: number;
  pekTavanKatsayisi?: number;
  gunlukYemekIstisnasiSGK?: number;
  gunlukYemekIstisnasiGV?: number;
}

export interface ResolvedStatutorySegmentSnapshot {
  effectiveFrom: string;
  effectiveTo: string;
  sgkPrimGunSayisi: number;
  fiiliYemekGunu: number;
  gunlukAsgariUcret: number;
  pekTavanKatsayisi: number;
  gunlukYemekIstisnasiSGK: number;
  gunlukYemekIstisnasiGV: number;
}

export type StatutorySnapshotSource =
  | 'ATTENDANCE_BACKED'
  | 'PROVISIONAL_PAYMENT_MONTH'
  | 'LEGACY_UNKNOWN';

export interface ResolvedStatutorySnapshot {
  /** Optional for legacy persisted snapshots created before provenance existed. */
  source?: StatutorySnapshotSource;
  segments: ResolvedStatutorySegmentSnapshot[];
  sgkPrimGunSayisi: number;
  pekAltSinir: number;
  pekUstSinir: number;
  sgkYemekIstisnasiToplam: number;
  gvYemekIstisnasiToplam: number;
  gvReferansGunlukAsgariUcret: number;
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
  gunlukYemekIstisnasiGV?: number;
  statutoryParameterSegments?: StatutoryParameterSegment[];
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

export const BACKUP_FORMAT_VERSION = 4;
/** Native app_settings anahtarı: kurum genelinde zam yürürlük ayları (1-12). */
export const ZAM_AYLARI_SETTING_KEY = 'zam_aylari';

/** JSON yedek sözleşmesi. V4, SQLite'daki tüm kullanıcı verisini kapsar. */
export interface BackupPayload {
  backupVersion: number;
  exportedAt: string;
  donemler: BordroDonemi[];
  aktifDonemId: string;
  personeller: Personel[];
  kurumDegerleriMap: Record<string, DönemselKurumDegerleri>;
  puantajlar: PersonelPuantaj[];
  bordrolar: BordroKaydi[];
  taxOpenings: PersonelTaxOpening[];
  sickLeaveRecords: SickLeaveRecord[];
  annualPayrollParameters: AnnualPayrollParameters[];
  /** Kurum genelinde zam yürürlük ayları; seçilen ayın 1'i esas alınır. */
  zamAylari: number[];
  /** V4 retro graph; V3 is adapted through the legacy compatibility path. */
  compensationRevisions: CompensationRevision[];
  compensationRevisionOverrides: CompensationRevisionOverride[];
  retroBatches: RetroAdjustmentBatch[];
  retroAllocations: RetroAllocation[];
}

export type PuantajOzeti = Record<PuantajKodu, number>;

export interface PersonelPuantaj {
  id: string; // `${personelId}_${donemId}`
  personelId: string;
  donemId: string;
  gunler: Record<string, PuantajKodu>;
}

export interface ManualPayrollIncomeInput {
  /** Kişi+dönem bazında kullanıcı tarafından girilen brüt Tediye tutarı. */
  tediye?: number | null;
  /** Kişi+dönem bazında kullanıcı tarafından girilen brüt TİS ikramiyesi tutarı. */
  tisIkramiyesi?: number | null;
}

export type AccrualType =
  | 'NORMAL'
  | 'TEDIYE'
  | 'TIS_IKRAMIYE'
  | 'SUPPLEMENTAL'
  | 'RETRO_ADJUSTMENT';

export type CompensationRevisionReason =
  | 'COLLECTIVE_AGREEMENT'
  | 'ADMINISTRATIVE_DECISION'
  | 'COURT_DECISION'
  | 'PAY_CORRECTION'
  | 'MISSING_ACCRUAL'
  | 'OTHER';

export type CompensationRevisionStatus = 'DRAFT' | 'CALCULATED' | 'STALE' | 'FINALIZED';
export type CompensationRevisionScope =
  | 'ALL_PERSONNEL'
  | 'SELECTED_PERSONNEL'
  | 'PERSONNEL_GROUP';

export type RetroParameterKey =
  | 'GUNLUK_TABAN_UCRET'
  | 'GUNLUK_YEMEK'
  | 'BIRLESTIRILMIS_SOSYAL_YARDIM'
  | 'GUNLUK_VASITA_YOL'
  | 'GIYIM_YARDIMI'
  | 'HIZMET_ZAMMI_BIRIMI'
  | 'IS_PRIMI_YUZDE'
  | 'GECE_CALISMA_PRIMI_YUZDE'
  | 'GECE_CALISMA_TATILI_PRIMI_YUZDE'
  | 'EK_ODEME'
  | 'DIGER_GELIR'
  | 'TEDIYE'
  | 'TIS_BONUS';

export type RetroEarningCode =
  | 'BASE_WAGE'
  | 'NIGHT_WORK'
  | 'NIGHT_HOLIDAY'
  | 'WORK_PREMIUM'
  | 'SOCIAL_AID'
  | 'MEAL'
  | 'TRANSPORT'
  | 'CLOTHING'
  | 'SERVICE_INCREMENT'
  | 'TIS_BONUS'
  | 'TEDIYE'
  | 'SUPPLEMENTAL'
  | 'OTHER';

export type RetroTaxTreatment = 'TAXABLE' | 'EXEMPT';
export type RetroSettlementStatus = 'UNSETTLED' | 'PAID' | 'OVERPAYMENT';
export type RetroSgkTreatment =
  | 'WAGE_SOURCE_MONTH'
  | 'NON_WAGE_PAYMENT_MONTH'
  | 'NON_WAGE_CARRY'
  | 'EXEMPT';

export interface CompensationRevision {
  id: string;
  reason: CompensationRevisionReason;
  title: string;
  effectiveFrom: string;
  effectiveTo?: string | null;
  decisionDate?: string | null;
  signedAt?: string | null;
  description?: string | null;
  status?: CompensationRevisionStatus;
  scope?: CompensationRevisionScope;
  personnelIds?: string[];
  personnelGroup?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface CompensationRevisionOverride {
  id: string;
  revisionId: string;
  parameter: RetroParameterKey;
  value: number;
  personnelId?: string | null;
}

export interface RetroAdjustmentBatch {
  id: string;
  revisionId: string;
  personnelId: string;
  paymentDate: string;
  status?: CompensationRevisionStatus;
  /** Additive V4 field; missing legacy values mean UNSETTLED. */
  settlementStatus?: RetroSettlementStatus;
  totalGrossDelta: number;
  description?: string | null;
  createdAt?: string | null;
  calculatedAt?: string | null;
  finalizedAt?: string | null;
}

export interface RetroAllocation {
  id: string;
  batchId: string;
  personnelId: string;
  sourcePeriodId: string;
  earningCode: RetroEarningCode;
  originalRecognizedAmount: number;
  previousAuthoritativeRetroAmount?: number;
  targetAmount: number;
  deltaAmount: number;
  sgkTreatment: RetroSgkTreatment;
  incomeTaxTreatment: RetroTaxTreatment;
  stampTaxTreatment: RetroTaxTreatment;
  originalPek?: number;
  retroPekDelta?: number;
  adjustedPek?: number;
  workerSgkDelta?: number;
  workerUnemploymentDelta?: number;
  employerSgkDelta?: number;
  employerUnemploymentDelta?: number;
  metadata?: string | null;
}

export interface PayrollAccrualInput {
  accrualId: string;
  accrualType: AccrualType;
  paymentDate: string;
  sequence: number;
  grossAmount?: number | null;
  description?: string | null;
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
  /** Rust core's current-period PEK before devreden PEK. */
  hamPek?: number;
  /** Devreden PEK actually fitted into the current period ceiling. */
  devredenPekKullanilan?: number;
  /** Authoritative worker-prime base before artificial lower-bound completion. */
  primMatrahi?: number;
  finalPek: number;
  devredenPekAşanTutar: number;
  pekAltSinir: number;
  pekUstSinir: number;
  altSinirTamamlamaFarki?: number;
  fiiliYemekGunu: number;
  yemekIstisnasiTutar: number;
  isverenSgkPrimi?: number;
  isverenIssizlikPrimi?: number;
  pekAltSinirTamamlamaIsverenPrimi?: number;
  isverenPrimToplami?: number;
  sgkIsverenOraniYuzde?: number;
  isverenIssizlikOraniYuzde?: number;
}

export const BORDRO_STATUS_VALUES = ['DRAFT', 'CALCULATED', 'STALE', 'FINALIZED'] as const;
export type BordroStatus = (typeof BORDRO_STATUS_VALUES)[number];

export interface BordroKaydi {
  id: string;
  personelId: string;
  donemId: string;
  accrualId: string;
  accrualType: AccrualType;
  paymentDate: string;
  sequence: number;
  accrualDescription?: string | null;
  puantajOzeti: PuantajOzeti;
  gelirler: GelirKalemleri;
  gelirToplam: number;
  kesintiler: KesintiKalemleri;
  kesintiToplam: number;
  netOdeme: number;
  status: BordroStatus;
  olusturulmaTarihi: string;
  sonGuncellemeTarihi: string;
  notlar?: string;
  oncekiKumulatifGvMatrahi?: number;
  oncekiKumulatifAsgariGvMatrahi?: number;
  manuelKumulatifGvMatrahi?: number;
  devredenPekGelen?: DevredenPekKaydi[];
  sonrakiDevredenPek?: DevredenPekKaydi[];
  pekDetay?: PekDetayi;
  isPrimiDetay?: IsPrimiHesapDetayi;
  gvDetay?: GvHesapDetayi;
  damgaDetay?: DamgaVergisiHesapDetayi;
  statutorySnapshot?: ResolvedStatutorySnapshot;
  odenenRaporluGun?: number;
  raporluGun?: number;
}

export interface DamgaVergisiHesapDetayi {
  brutDamgaVergisi: number;
  aylikDamgaIstisnaHakki: number;
  ayniAyOncekiKullanilanDamgaIstisnasi: number;
  uygulananDamgaIstisnasi: number;
  kalanDamgaIstisnasi: number;
  kesilenDamgaVergisi: number;
}

export interface ZamHesaplama {
  eskiTutar: number;
  zamOrani: number;
  yeniTutar: number;
}
