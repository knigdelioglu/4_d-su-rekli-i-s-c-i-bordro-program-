#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQLite/JSON-safe upper boundary used to represent the open-ended final
/// income-tax bracket. The calculation layer treats the final bracket as
/// unbounded, so this is only a persistence-safe sentinel.
pub const OPEN_ENDED_TAX_BRACKET_LIMIT: i64 = 1_000_000_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GvIndirimGirdileri {
    /// Doğum/askerlik borçlanmasının cari bordroda GV matrahından indirime
    /// uygun ve belgeye dayalı kısmı. Net kesinti alanından bağımsızdır.
    pub dogumAskerlikGvIndirimTutar: Option<Decimal>,
    /// Çalışanın ödediği hayat sigortası priminin cari ay brüt tutarı.
    pub hayatSigortasiPrimiTutar: Option<Decimal>,
    /// Çalışanın ödediği şahıs/sağlık sigortası priminin cari ay brüt tutarı.
    pub saglikSigortasiPrimiTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersonelKesintileri {
    pub sendikaUyesi: Option<bool>,
    pub sabitSendikaAidati: Option<Decimal>,
    pub besUyesi: Option<bool>,
    pub oksOraniYuzde: Option<Decimal>,
    pub sabitBesTutar: Option<Decimal>,
    pub icraTutar: Option<Decimal>,
    pub kisiBorcuTutar: Option<Decimal>,
    pub dogumAskerlikBorclanmasiTutar: Option<Decimal>,
    pub hayatSaglikSigortasiTutar: Option<Decimal>,
    pub digerKesintiTutar: Option<Decimal>,
    #[serde(default)]
    pub gvIndirimleri: Option<GvIndirimGirdileri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Personel {
    pub id: String,
    pub tcNo: String,
    pub ad: String,
    pub soyad: String,
    pub grup: String,
    pub unvan: Option<String>,
    pub sgkSicilNo: String,
    pub iban: String,
    pub hizmetYili: i32,
    pub aciklama: Option<String>,
    pub devirKumulatifGvMatrahi: Option<Decimal>,
    pub devirKumulatifGvMatrahiYili: Option<i32>,
    pub devirKumulatifGvMatrahiBaslangicAyi: Option<i32>,
    pub devirKumulatifAsgariGvMatrahi: Option<Decimal>,
    pub devirKumulatifAsgariGvMatrahiYili: Option<i32>,
    pub kesintiler: Option<PersonelKesintileri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BordroDonemi {
    pub id: String, // e.g. "2026-05"
    pub yil: i32,   // e.g. 2026
    pub ay: i32, // 1-12 DÖNEM BAŞLANGIÇ AYI (15'in bulunduğu ay). Dönem adı/id bundan türetilir, anlamı DEĞİŞMEZ.
    pub baslangicTarihi: String,
    pub bitisTarihi: String,
    pub donemAdi: String,
    /// Vergi yılı (tahakkuk/ödeme yılı). Asgari ücret GV referans kümülatifi ve
    /// vergi hesabı bu alan üzerinden yürütülür. `yil`'den bağımsız olabilir
    /// (ör. Aralık dönemi → Ocak ödemesi için aynı takvim yılı +1).
    pub taxYear: i32,
    /// Vergi ayı (ödeme/tahakkuk ayı), 1-12. Varsayılan öneri = bitiş ayı (ay + 1).
    pub taxMonth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonelTaxOpening {
    pub id: String,
    pub personnelId: String,
    pub year: i32,
    pub gvCumulativeOpening: Decimal,
    pub effectiveFromPeriodId: String, // e.g. "2026-05"
    pub createdAt: Option<String>,
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxBracket {
    pub limit: Decimal,
    /// 0-1 arası oran (örn. %15 için 0.15).
    pub oran: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnualPayrollParameters {
    pub year: i32,
    pub gelirVergisiDilimleri: Vec<TaxBracket>,
    /// GVK 63/3 sigorta primi indiriminin yıllık brüt asgari ücret tavanı.
    #[serde(default)]
    pub sigortaGvYillikBrutAsgariUcretTavani: Option<Decimal>,
    pub updatedAt: Option<String>,
}

impl AnnualPayrollParameters {
    pub fn default_for_2026() -> Self {
        Self {
            year: 2026,
            gelirVergisiDilimleri: vec![
                TaxBracket {
                    limit: Decimal::from(190000),
                    oran: Decimal::new(15, 2),
                },
                TaxBracket {
                    limit: Decimal::from(400000),
                    oran: Decimal::new(20, 2),
                },
                TaxBracket {
                    limit: Decimal::from(1500000),
                    oran: Decimal::new(27, 2),
                },
                TaxBracket {
                    limit: Decimal::from(5300000),
                    oran: Decimal::new(35, 2),
                },
                // Keep the open-ended final bracket JSON/SQLite-safe. The
                // calculation layer gives the last bracket unlimited semantics.
                TaxBracket {
                    limit: Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT),
                    oran: Decimal::new(40, 2),
                },
            ],
            sigortaGvYillikBrutAsgariUcretTavani: Some(Decimal::from(396360)),
            updatedAt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonelPuantaj {
    pub id: String, // `${personelId}_${donemId}`
    pub personelId: String,
    pub donemId: String,
    pub gunler: HashMap<String, String>, // key: "YYYY-MM-DD", val: "Ç", "T", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PuantajOzeti {
    #[serde(rename = "Ç")]
    pub c: i32,
    #[serde(rename = "T")]
    pub t: i32,
    #[serde(rename = "G")]
    pub g: i32,
    #[serde(rename = "İ")]
    pub i: i32,
    #[serde(rename = "GÇ")]
    pub gc: i32,
    #[serde(rename = "GÇT")]
    pub gct: i32,
    #[serde(rename = "R")]
    pub r: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GelirKalemleri {
    pub tabanBrutAylik: Option<Decimal>,
    pub tediye: Option<Decimal>,
    pub tisIkramiyesi: Option<Decimal>,
    pub ekOdeme: Option<Decimal>,
    pub yemek: Option<Decimal>,
    pub birlestirilmisSosyalYardim: Option<Decimal>,
    pub vasitaYol: Option<Decimal>,
    pub giyimYardimi: Option<Decimal>,
    pub isPrimi: Option<Decimal>,
    pub geceCalismasiUcreti: Option<Decimal>,
    pub geceCalismasiTatiliUcreti: Option<Decimal>,
    pub hizmetZammi: Option<Decimal>,
    pub digerGelir: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManualPayrollIncomeInput {
    /// Legacy NORMAL records may still carry a manually stored Tediye amount.
    /// New Tediye entries are represented by a separate accrual node.
    pub tediye: Option<Decimal>,
    /// Legacy NORMAL records may still carry a manually stored TİS amount.
    /// New TİS entries are represented by a separate accrual node.
    pub tisIkramiyesi: Option<Decimal>,
}

/// A payroll period is a work-period container. An accrual is the immutable
/// payment/calculation node inside that period.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum AccrualType {
    #[default]
    NORMAL,
    TEDIYE,
    TIS_IKRAMIYE,
    SUPPLEMENTAL,
    RETRO_ADJUSTMENT,
}

/// A revision changes the entitlement rules for an already served period. It
/// is deliberately separate from an accrual/payment event: signing a
/// collective agreement does not itself create a payment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompensationRevisionReason {
    #[serde(rename = "COLLECTIVE_AGREEMENT")]
    COLLECTIVE_AGREEMENT,
    #[serde(rename = "ADMINISTRATIVE_DECISION")]
    ADMINISTRATIVE_DECISION,
    #[serde(rename = "COURT_DECISION")]
    COURT_DECISION,
    #[serde(rename = "PAY_CORRECTION")]
    PAY_CORRECTION,
    #[serde(rename = "MISSING_ACCRUAL")]
    MISSING_ACCRUAL,
    #[serde(rename = "OTHER")]
    OTHER,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CompensationRevisionStatus {
    #[default]
    #[serde(rename = "DRAFT")]
    DRAFT,
    #[serde(rename = "CALCULATED")]
    CALCULATED,
    #[serde(rename = "STALE")]
    STALE,
    #[serde(rename = "FINALIZED")]
    FINALIZED,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CompensationRevisionScope {
    #[default]
    #[serde(rename = "ALL_PERSONNEL")]
    ALL_PERSONNEL,
    #[serde(rename = "SELECTED_PERSONNEL")]
    SELECTED_PERSONNEL,
    #[serde(rename = "PERSONNEL_GROUP")]
    PERSONNEL_GROUP,
}

/// Settlement state is separate from the batch lifecycle. A calculated batch
/// can legitimately be an overpayment correction and therefore must remain
/// auditable without being mistaken for a payable event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RetroSettlementStatus {
    #[default]
    #[serde(rename = "UNSETTLED")]
    UNSETTLED,
    #[serde(rename = "PAID")]
    PAID,
    #[serde(rename = "OVERPAYMENT")]
    OVERPAYMENT,
}

/// Only contractual/operational compensation inputs are representable here.
/// Statutory inputs (minimum wage, SGK/GV/DV rates, PEK ceiling, tax brackets)
/// intentionally have no variants and therefore cannot be overridden by a
/// revision payload.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RetroParameterKey {
    #[serde(rename = "GUNLUK_TABAN_UCRET")]
    GUNLUK_TABAN_UCRET,
    #[serde(rename = "GUNLUK_YEMEK")]
    GUNLUK_YEMEK,
    #[serde(rename = "BIRLESTIRILMIS_SOSYAL_YARDIM")]
    BIRLESTIRILMIS_SOSYAL_YARDIM,
    #[serde(rename = "GUNLUK_VASITA_YOL")]
    GUNLUK_VASITA_YOL,
    #[serde(rename = "GIYIM_YARDIMI")]
    GIYIM_YARDIMI,
    #[serde(rename = "HIZMET_ZAMMI_BIRIMI")]
    HIZMET_ZAMMI_BIRIMI,
    #[serde(rename = "IS_PRIMI_YUZDE")]
    IS_PRIMI_YUZDE,
    #[serde(rename = "GECE_CALISMA_PRIMI_YUZDE")]
    GECE_CALISMA_PRIMI_YUZDE,
    #[serde(rename = "GECE_CALISMA_TATILI_PRIMI_YUZDE")]
    GECE_CALISMA_TATILI_PRIMI_YUZDE,
    #[serde(rename = "EK_ODEME")]
    EK_ODEME,
    #[serde(rename = "DIGER_GELIR")]
    DIGER_GELIR,
    #[serde(rename = "TEDIYE")]
    TEDIYE,
    #[serde(rename = "TIS_BONUS")]
    TIS_BONUS,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RetroEarningCode {
    #[serde(rename = "BASE_WAGE")]
    BASE_WAGE,
    #[serde(rename = "NIGHT_WORK")]
    NIGHT_WORK,
    #[serde(rename = "NIGHT_HOLIDAY")]
    NIGHT_HOLIDAY,
    #[serde(rename = "WORK_PREMIUM")]
    WORK_PREMIUM,
    #[serde(rename = "SOCIAL_AID")]
    SOCIAL_AID,
    #[serde(rename = "MEAL")]
    MEAL,
    #[serde(rename = "TRANSPORT")]
    TRANSPORT,
    #[serde(rename = "CLOTHING")]
    CLOTHING,
    #[serde(rename = "SERVICE_INCREMENT")]
    SERVICE_INCREMENT,
    #[serde(rename = "TIS_BONUS")]
    TIS_BONUS,
    #[serde(rename = "TEDIYE")]
    TEDIYE,
    #[serde(rename = "SUPPLEMENTAL")]
    SUPPLEMENTAL,
    #[serde(rename = "OTHER")]
    OTHER,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RetroTaxTreatment {
    #[serde(rename = "TAXABLE")]
    TAXABLE,
    #[serde(rename = "EXEMPT")]
    EXEMPT,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RetroSgkTreatment {
    #[serde(rename = "WAGE_SOURCE_MONTH")]
    WAGE_SOURCE_MONTH,
    #[serde(rename = "NON_WAGE_PAYMENT_MONTH")]
    NON_WAGE_PAYMENT_MONTH,
    #[serde(rename = "NON_WAGE_CARRY")]
    NON_WAGE_CARRY,
    #[serde(rename = "EXEMPT")]
    EXEMPT,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationRevision {
    pub id: String,
    pub reason: CompensationRevisionReason,
    pub title: String,
    pub effectiveFrom: String,
    #[serde(default)]
    pub effectiveTo: Option<String>,
    #[serde(default)]
    pub decisionDate: Option<String>,
    #[serde(default)]
    pub signedAt: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: CompensationRevisionStatus,
    #[serde(default)]
    pub scope: CompensationRevisionScope,
    #[serde(default)]
    pub personnelIds: Vec<String>,
    #[serde(default)]
    pub personnelGroup: Option<String>,
    #[serde(default)]
    pub createdAt: Option<String>,
    #[serde(default)]
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationRevisionOverride {
    pub id: String,
    pub revisionId: String,
    pub parameter: RetroParameterKey,
    pub value: Decimal,
    #[serde(default)]
    pub personnelId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetroAdjustmentBatch {
    pub id: String,
    pub revisionId: String,
    pub personnelId: String,
    pub paymentDate: String,
    #[serde(default)]
    pub status: CompensationRevisionStatus,
    #[serde(default)]
    pub settlementStatus: RetroSettlementStatus,
    pub totalGrossDelta: Decimal,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub createdAt: Option<String>,
    #[serde(default)]
    pub calculatedAt: Option<String>,
    #[serde(default)]
    pub finalizedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetroAllocation {
    pub id: String,
    pub batchId: String,
    pub personnelId: String,
    pub sourcePeriodId: String,
    pub earningCode: RetroEarningCode,
    /// Amount already recognized by the original/legacy payroll events.
    pub originalRecognizedAmount: Decimal,
    /// Amount recognized by earlier authoritative retro batches.
    #[serde(default)]
    pub previousAuthoritativeRetroAmount: Decimal,
    pub targetAmount: Decimal,
    pub deltaAmount: Decimal,
    pub sgkTreatment: RetroSgkTreatment,
    pub incomeTaxTreatment: RetroTaxTreatment,
    pub stampTaxTreatment: RetroTaxTreatment,
    /// Source-month PEK ledger snapshot. These are never written back to the
    /// original payroll record.
    #[serde(default)]
    pub originalPek: Decimal,
    #[serde(default)]
    pub retroPekDelta: Decimal,
    #[serde(default)]
    pub adjustedPek: Decimal,
    #[serde(default)]
    pub workerSgkDelta: Decimal,
    #[serde(default)]
    pub workerUnemploymentDelta: Decimal,
    #[serde(default)]
    pub employerSgkDelta: Decimal,
    #[serde(default)]
    pub employerUnemploymentDelta: Decimal,
    #[serde(default)]
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollAccrualInput {
    pub accrualId: String,
    pub accrualType: AccrualType,
    pub paymentDate: String,
    pub sequence: i32,
    /// Required for supplementary accruals; NORMAL uses the legacy income
    /// source and may leave this empty.
    #[serde(default)]
    pub grossAmount: Option<Decimal>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KesintiKalemleri {
    pub isciSgkPrimi: Option<Decimal>,
    pub isciIssizlikPrimi: Option<Decimal>,
    pub gelirVergisi: Option<Decimal>,
    pub damgaVergisi: Option<Decimal>,
    pub sendikaAidati: Option<Decimal>,
    pub bes: Option<Decimal>,
    pub icra: Option<Decimal>,
    pub kisiBorcu: Option<Decimal>,
    pub dogumAskerlikBorclanmasi: Option<Decimal>,
    pub hayatSaglikSigortasi: Option<Decimal>,
    pub digerKesinti: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SickLeaveRecord {
    pub id: String,
    pub personnelId: String,
    pub startDate: String, // YYYY-MM-DD
    pub endDate: String,   // YYYY-MM-DD
    pub createdAt: Option<String>,
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevredenPekKaydi {
    pub tutar: Decimal,
    pub kalanAySayisi: i32,
    pub kaynakDonemId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PekDetayi {
    /// Geriye dönük uyumluluk alanı; yeni hesaplarda `hamPek` ile aynıdır.
    pub hesaplananPek: Decimal,
    /// Yalnız cari dönem kazançlarından oluşan PEK, devreden PEK hariç.
    #[serde(default)]
    pub hamPek: Decimal,
    /// Önceki dönemlerden gelip cari ay tavanına fiilen sığdırılan PEK.
    #[serde(default)]
    pub devredenPekKullanilan: Decimal,
    /// İşçi SGK ve işsizlik primlerinin authoritative matrahı: ham PEK + bu ay kullanılan devreden PEK, tavanla sınırlı; yapay alt sınır tamamlama hariç.
    #[serde(default)]
    pub primMatrahi: Decimal,
    pub finalPek: Decimal,
    pub devredenPekAşanTutar: Decimal,
    pub pekAltSinir: Decimal,
    pub pekUstSinir: Decimal,
    #[serde(default)]
    pub altSinirTamamlamaFarki: Decimal,
    pub fiiliYemekGunu: i32,
    pub yemekIstisnasiTutar: Decimal,
    pub isverenSgkPrimi: Option<Decimal>,
    pub isverenIssizlikPrimi: Option<Decimal>,
    pub pekAltSinirTamamlamaIsverenPrimi: Option<Decimal>,
    pub isverenPrimToplami: Option<Decimal>,
    pub sgkIsverenOraniYuzde: Option<Decimal>,
    pub isverenIssizlikOraniYuzde: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum BordroStatus {
    DRAFT,
    #[default]
    CALCULATED,
    STALE,
    FINALIZED,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollIncomeItem {
    pub id: String,
    pub payrollId: String,
    pub itemType: String,
    pub description: String,
    pub amount: Decimal,
    pub source: String, // "MANUAL" or "CALCULATED"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollDeductionItem {
    pub id: String,
    pub payrollId: String,
    pub itemType: String,
    pub description: String,
    pub amount: Decimal,
    pub source: String, // "MANUAL" or "CALCULATED"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BordroKaydi {
    pub id: String,
    pub personelId: String,
    pub donemId: String,
    #[serde(default)]
    pub accrualId: String,
    #[serde(default)]
    pub accrualType: AccrualType,
    #[serde(default)]
    pub paymentDate: String,
    #[serde(default)]
    pub sequence: i32,
    #[serde(default)]
    pub accrualDescription: Option<String>,
    pub puantajOzeti: PuantajOzeti,
    pub gelirler: GelirKalemleri,
    pub gelirToplam: Decimal,
    pub kesintiler: KesintiKalemleri,
    pub kesintiToplam: Decimal,
    pub netOdeme: Decimal,
    #[serde(default)]
    pub status: BordroStatus,
    pub olusturulmaTarihi: String,
    pub sonGuncellemeTarihi: String,
    pub notlar: Option<String>,
    pub oncekiKumulatifGvMatrahi: Option<Decimal>,
    pub oncekiKumulatifAsgariGvMatrahi: Option<Decimal>,
    pub manuelKumulatifGvMatrahi: Option<Decimal>,
    pub devredenPekGelen: Option<Vec<DevredenPekKaydi>>,
    pub sonrakiDevredenPek: Option<Vec<DevredenPekKaydi>>,
    pub pekDetay: Option<PekDetayi>,
    pub isPrimiDetay: Option<IsPrimiHesapDetayi>,
    pub gvDetay: Option<GvHesapDetayi>,
    #[serde(default)]
    pub damgaDetay: Option<DamgaVergisiHesapDetayi>,
    /// Bordro hesaplanırken çözümlenen period-local yasal parametrelerin snapshot'ı.
    pub statutorySnapshot: Option<ResolvedStatutorySnapshot>,
    pub odenenRaporluGun: Option<i32>,
    pub raporluGun: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TediyeKalemi {
    pub id: i32,
    pub ad: String,
    pub odemeAyi: String,
    pub gunSayisi: i32,
    pub aktifDonemdeOdensin: bool,
    pub sabitTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TisIkramiyeKalemi {
    pub id: i32,
    pub ad: String,
    pub odemeAyi: String,
    pub gunSayisi: i32,
    pub aktifDonemdeOdensin: bool,
    pub sabitTutar: Option<Decimal>,
}

fn default_is_primi_aktif() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsPrimiGrupItem {
    pub id: String,
    pub ad: String,
    pub oran: Decimal,
    /// Grup aktif/pasif. Salt serileştirilmemiş/pasif gruplar bordro motorunda
    /// iş primi oran kaynağı olarak kullanılamaz.
    #[serde(default = "default_is_primi_aktif")]
    pub aktif: bool,
}

/// Hesaplanan bordronun iş primi bölümünün denetlenebilir snapshot'ı.
/// Grup tanımı/oranı daha sonra değiştirilse bile FINALIZED bordroda
/// uygulanan grup/oran/hak günü/tutar kayıt altında kalır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsPrimiHesapDetayi {
    pub grupId: String,
    pub grupAd: String,
    pub oran: Decimal,
    pub hakGunu: i32,
    /// Yalnız gösterim amaçlıdır; bordro toplamının authoritative girdisi değildir.
    pub gunlukIsPrimi: Decimal,
    pub tutar: Decimal,
}

/// Hesaplanan bordronun gelir vergisi bölümünün denetlenebilir snapshot'ı.
/// Çalışanın gerçek kümülatifi (`cariGvMatrahi`/`yeniKumulatifGvMatrahi`) ile
/// asgari ücret istisnasının kendi takvim referansı (`asgariUcretGvMatrahi`/
/// `asgariUcretReferansKumulatifMatrahi`) açıkça ayrılır; GİB'e uygun hesaplama
/// `brutGelirVergisi - uygulananGvIstisnasi = kesilenGelirVergisi` şeklindedir.
/// FINALIZED bordroda asgari ücret/tarife/açılış değişse bile kayıt altında kalır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GvHesapDetayi {
    #[serde(default)]
    pub oncekiKumulatifGvMatrahi: Decimal,
    /// Cari dönem GV matrahı (brüt gelir - işçi SGK - işçi işsizlik).
    pub cariGvMatrahi: Decimal,
    /// Cari sonrası gerçek kümülatif matrah (önceki + cari).
    pub yeniKumulatifGvMatrahi: Decimal,
    /// İstisna ÖNCESİ hesaplanan gelir vergisi (gerçek kümülatif üzerinden).
    pub brutGelirVergisi: Decimal,
    /// Asgari ücretin aylık GV matrahı (takvim referansı).
    pub asgariUcretGvMatrahi: Decimal,
    /// Asgari ücret istisnasının takvim referans kümülatif matrahı.
    pub asgariUcretReferansKumulatifMatrahi: Decimal,
    /// Asgari ücretin ilgili ay için hesaplanan vergi istisnası hakkı.
    pub asgariUcretGvIstisnasi: Decimal,
    #[serde(default)]
    pub ayniAyOncekiKullanilanGvIstisnasi: Decimal,
    #[serde(default)]
    pub tahakkukOncesiKalanGvIstisnasi: Decimal,
    /// Gerçekte uygulanan istisna: min(brüt GV, aylık istisna hakkı).
    pub uygulananGvIstisnasi: Decimal,
    #[serde(default)]
    pub tahakkukSonrasiKalanGvIstisnasi: Decimal,
    /// Kesilecek gelir vergisi (negatif olamaz).
    pub kesilenGelirVergisi: Decimal,
    /// Cari ayda gerçekten uygulanan doğum/askerlik borçlanması GV indirimi.
    #[serde(default)]
    pub dogumAskerlikGvIndirimi: Decimal,
    /// Hayat (%50) + sağlık/şahıs (%100) primlerinden oluşan brüt GV indirim adayı.
    #[serde(default)]
    pub sigortaGvIndirimAdayi: Decimal,
    /// Cari ay ücretinin %15'i üzerinden hesaplanan sigorta GV indirimi üst sınırı.
    #[serde(default)]
    pub sigortaGvAylikLimiti: Decimal,
    /// Yıllık brüt asgari ücret tavanından cari ay öncesi kullanım düşüldükten sonra kalan limit.
    #[serde(default)]
    pub sigortaGvYillikKalanLimiti: Decimal,
    /// Aday, aylık limit ve yıllık kalan limitin en küçüğü.
    #[serde(default)]
    pub uygulanabilirSigortaGvIndirimi: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DamgaVergisiHesapDetayi {
    pub brutDamgaVergisi: Decimal,
    pub aylikDamgaIstisnaHakki: Decimal,
    pub ayniAyOncekiKullanilanDamgaIstisnasi: Decimal,
    pub uygulananDamgaIstisnasi: Decimal,
    pub kalanDamgaIstisnasi: Decimal,
    pub kesilenDamgaVergisi: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatutoryParameterSegment {
    /// Inclusive effective date inside this payroll period (YYYY-MM-DD).
    pub effectiveFrom: String,
    pub gunlukAsgariUcret: Option<Decimal>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStatutorySegmentSnapshot {
    pub effectiveFrom: String,
    pub effectiveTo: String,
    pub sgkPrimGunSayisi: i32,
    pub fiiliYemekGunu: i32,
    pub gunlukAsgariUcret: Decimal,
    pub pekTavanKatsayisi: Decimal,
    pub gunlukYemekIstisnasiSGK: Decimal,
    pub gunlukYemekIstisnasiGV: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatutorySnapshotSource {
    AttendanceBacked,
    ProvisionalPaymentMonth,
    /// Old/imported snapshots did not record provenance. Keep them readable,
    /// but let domain policy treat supplementary records conservatively.
    LegacyUnknown,
}

impl Default for StatutorySnapshotSource {
    fn default() -> Self {
        Self::LegacyUnknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStatutorySnapshot {
    /// Provenance of the statutory/PEK capacity used by the calculation.
    /// Missing in legacy JSON and therefore defaults to `LegacyUnknown`.
    #[serde(default)]
    pub source: StatutorySnapshotSource,
    pub segments: Vec<ResolvedStatutorySegmentSnapshot>,
    pub sgkPrimGunSayisi: i32,
    pub pekAltSinir: Decimal,
    pub pekUstSinir: Decimal,
    pub sgkYemekIstisnasiToplam: Decimal,
    pub gvYemekIstisnasiToplam: Decimal,
    /// Gelir vergisi asgari ücret istisnası için vergi ayına taşınan son
    /// yürürlükteki günlük asgari ücret değeri.
    pub gvReferansGunlukAsgariUcret: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonemselKurumDegerleri {
    pub donemId: String,
    pub gunlukTabanUcret: Decimal,
    pub gunlukYemek: Decimal,
    pub birlestirilmisSosyalYardim: Decimal,
    pub gunlukVasitaYol: Decimal,
    pub giyimYardimi: Decimal,
    pub hizmetZammiBirimi: Decimal,
    pub isPrimiYuzde: Option<Decimal>,
    pub isPrimiGruplari: Option<Vec<IsPrimiGrupItem>>,
    pub geceCalismaPrimiYuzde: Option<Decimal>,
    pub geceCalismaTatiliPrimiYuzde: Option<Decimal>,
    pub ekOdeme: Option<Decimal>,
    pub digerGelirVarsayilan: Option<Decimal>,
    pub tediyeListesi: Option<Vec<TediyeKalemi>>,
    pub tisIkramiyeListesi: Option<Vec<TisIkramiyeKalemi>>,
    pub tediyeTisNotu: Option<String>,

    pub sgkIsciOraniYuzde: Option<Decimal>,
    pub issizlikIsciOraniYuzde: Option<Decimal>,
    pub gelirVergisiOraniYuzde: Option<Decimal>,
    pub damgaVergisiOraniBinde: Option<Decimal>,
    pub sendikaAidatiYuzde: Option<Decimal>,
    pub sabitSendikaAidati: Option<Decimal>,
    pub besOraniYuzde: Option<Decimal>,
    pub sabitBesTutar: Option<Decimal>,

    #[serde(
        alias = "sgk_yemek_istisnasi_gunluk",
        alias = "sgkYemekIstisnasiGunluk"
    )]
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    /// Gelir vergisi yemek istisnası SGK istisnasından bağımsız tutulur.
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
    /// Yalnız bu açık/gelecek 15-14 dönemi içinde geçerli değişiklikler.
    /// Genel tarihsel mevzuat arşivi değildir.
    pub statutoryParameterSegments: Option<Vec<StatutoryParameterSegment>>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukAsgariUcret: Option<Decimal>,

    #[serde(alias = "sgk_isveren_prim_orani", alias = "sgkIsverenPrimOrani")]
    pub sgkIsverenOraniYuzde: Option<Decimal>,
    #[serde(
        alias = "isveren_issizlik_prim_orani",
        alias = "isverenIssizlikPrimOrani"
    )]
    pub issizlikIsverenOraniYuzde: Option<Decimal>,
}

impl Default for DonemselKurumDegerleri {
    fn default() -> Self {
        use rust_decimal_macros::dec;
        Self {
            donemId: "".to_string(),
            gunlukTabanUcret: dec!(2443.28),
            gunlukYemek: dec!(300.75),
            birlestirilmisSosyalYardim: dec!(5089.70),
            gunlukVasitaYol: dec!(128.93),
            giyimYardimi: dec!(269.70),
            hizmetZammiBirimi: dec!(24.67),
            isPrimiYuzde: Some(dec!(0)),
            isPrimiGruplari: Some(vec![
                IsPrimiGrupItem {
                    id: "1. Grup".into(),
                    ad: "1. Grup".into(),
                    oran: dec!(9),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "2. Grup".into(),
                    ad: "2. Grup".into(),
                    oran: dec!(8),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "3. Grup".into(),
                    ad: "3. Grup".into(),
                    oran: dec!(7),
                    aktif: true,
                },
            ]),
            geceCalismaPrimiYuzde: Some(dec!(0)),
            geceCalismaTatiliPrimiYuzde: Some(dec!(0)),
            ekOdeme: Some(dec!(0)),
            digerGelirVarsayilan: Some(dec!(0)),
            tediyeListesi: None,
            tisIkramiyeListesi: None,
            tediyeTisNotu: None,
            sgkIsciOraniYuzde: Some(dec!(14)),
            issizlikIsciOraniYuzde: Some(dec!(1)),
            gelirVergisiOraniYuzde: Some(dec!(15)),
            damgaVergisiOraniBinde: Some(dec!(7.59)),
            sendikaAidatiYuzde: Some(dec!(65)),
            sabitSendikaAidati: Some(dec!(0)),
            besOraniYuzde: Some(dec!(3)),
            sabitBesTutar: Some(dec!(0)),
            gunlukYemekIstisnasiSGK: Some(dec!(300.00)),
            gunlukYemekIstisnasiGV: Some(dec!(300.00)),
            statutoryParameterSegments: None,
            pekTavanKatsayisi: Some(dec!(9)),
            gunlukAsgariUcret: Some(dec!(1101.00)),
            sgkIsverenOraniYuzde: Some(dec!(21.75)),
            issizlikIsverenOraniYuzde: Some(dec!(2.00)),
        }
    }
}
