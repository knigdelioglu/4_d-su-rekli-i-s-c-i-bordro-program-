#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SQLite/JSON-safe upper boundary used to represent the open-ended final
/// income-tax bracket. The calculation layer treats the final bracket as
/// unbounded, so this is only a persistence-safe sentinel.
pub const OPEN_ENDED_TAX_BRACKET_LIMIT: i64 = 1_000_000_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersonelKesintileri {
    pub sendikaUyesi: Option<bool>,
    pub sabitSendikaAidati: Option<Decimal>,
    pub besUyesi: Option<bool>,
    pub oksOraniYuzde: Option<Decimal>,
    pub sabitBesTutar: Option<Decimal>,
    pub icraTutar: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nafakaTutar: Option<Decimal>,
    pub kisiBorcuTutar: Option<Decimal>,
    pub dogumAskerlikBorclanmasiTutar: Option<Decimal>,
    pub hayatSaglikSigortasiTutar: Option<Decimal>,
    pub digerKesintiTutar: Option<Decimal>,
    #[serde(default)]
    pub gvIndirimleri: Option<GvIndirimGirdileri>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Normal GV opening bileşeni. None, normal opening'in tanımlı olmadığını
    /// ifade eder; Some(0) ise explicit sıfır opening'dir.
    #[serde(default)]
    pub gvCumulativeOpening: Option<Decimal>,
    /// Normal GV opening'in authoritative başlangıç dönemi.
    #[serde(default)]
    pub effectiveFromPeriodId: Option<String>,
    /// Asgari ücret GV referans takviminin opening değeri. Bu alan gerçek
    /// çalışanın GV kümülatifinden bağımsızdır ve eski kayıtlar için None'dır.
    #[serde(default)]
    pub asgariGvCumulativeOpening: Option<Decimal>,
    /// Asgari GV opening'i normal GV opening'inden farklı bir vergi döneminde
    /// başlayabilir. Bu bileşen normal GV opening ile ortak effective dönem
    /// kullanmaz; değeri Some ise kendi effective period ID'si de Some olmalıdır.
    #[serde(default)]
    pub asgariGvEffectiveFromPeriodId: Option<String>,
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
    /// Unpaid leave days. Kept separate from paid leave and reports so
    /// benefit policies can make an explicit entitlement decision.
    #[serde(rename = "Üİ", skip_serializing_if = "is_zero_i32")]
    pub ui: i32,
    /// Days outside the employment interval, entered explicitly in attendance
    /// until hire/termination dates are modeled on personnel records.
    #[serde(rename = "D", skip_serializing_if = "is_zero_i32")]
    pub d: i32,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
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
    /// The current positive entitlement was consumed by an outstanding
    /// overpayment receivable.  No cash payment event is created for this
    /// state, but the entitlement and receivable movements remain auditable.
    #[serde(rename = "SETTLED_BY_OFFSET")]
    SETTLED_BY_OFFSET,
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
    /// Positive gross amount that may become a payment event after the
    /// outstanding receivable has been offset.
    #[serde(default)]
    pub payableSettlementAmount: Decimal,
    /// Positive portion of the current entitlement consumed against an open
    /// overpayment receivable.
    #[serde(default)]
    pub offsetSettlementAmount: Decimal,
    /// Explicit external recovery applied to this batch. The current UI does
    /// not create recovery events, so newly calculated batches set this to 0.
    #[serde(default)]
    pub recoveredAmount: Decimal,
    /// Positive receivable generated by a negative entitlement correction.
    #[serde(default)]
    pub recoverableAmount: Decimal,
    /// Cumulative open receivable after this batch's offset/recovery flows.
    #[serde(default)]
    pub outstandingReceivable: Decimal,
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
    /// Source-month employer lower-bound completion snapshot. This is a
    /// source PEK base, not a payment-month entitlement amount.
    #[serde(default)]
    pub originalEmployerLowerBound: Decimal,
    #[serde(default)]
    pub targetEmployerLowerBound: Decimal,
    #[serde(default)]
    pub employerLowerBoundDelta: Decimal,
    /// Employer premium attributable specifically to lower-bound completion.
    #[serde(default)]
    pub employerLowerBoundPremiumDelta: Decimal,
    /// Outgoing source-period carry produced by the original canonical event
    /// chain. These snapshots are audit data; the historical payroll event is
    /// never overwritten.
    #[serde(default)]
    pub originalSourceCarry: Option<Vec<DevredenPekKaydi>>,
    /// Outgoing source-period carry produced by the revised canonical event
    /// chain. A later payment event may consume this state only through the
    /// shared core's carry reconciliation.
    #[serde(default)]
    pub targetSourceCarry: Option<Vec<DevredenPekKaydi>>,
    /// Settlement allocation snapshot. These values are positive flows and
    /// are deliberately separate from signed entitlement deltaAmount.
    #[serde(default)]
    pub payableSettlementAmount: Decimal,
    #[serde(default)]
    pub offsetSettlementAmount: Decimal,
    #[serde(default)]
    pub recoverableAmount: Decimal,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nafaka: Option<Decimal>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Same-tax-month PEK consumption after the canonical payment-event
    /// reconciliation.  Older snapshots omit these fields and fall back to
    /// their persisted `primMatrahi` chain until they are replayed.
    #[serde(default)]
    pub aylikOncekiPekTuketimi: Option<Decimal>,
    #[serde(default)]
    pub aylikSonrasiPekTuketimi: Option<Decimal>,
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
    /// Authoritative GV base loaded from/persisted to SQLite.  It is optional
    /// only for sparse legacy payloads; current calculated records populate it
    /// and strict boundaries reconcile it with `gvDetay.cariGvMatrahi`.
    #[serde(default)]
    pub persistedGvBase: Option<Decimal>,
    #[serde(default)]
    pub damgaDetay: Option<DamgaVergisiHesapDetayi>,
    /// Bordro hesaplanırken çözümlenen period-local yasal parametrelerin snapshot'ı.
    pub statutorySnapshot: Option<ResolvedStatutorySnapshot>,
    pub odenenRaporluGun: Option<i32>,
    pub raporluGun: Option<i32>,
    /// Absent on legacy records and on the legacy calculation path. New-engine
    /// provenance and its immutable input snapshot are introduced additively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calculationMetadata: Option<PayrollCalculationMetadata>,
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

/// Full, institution-wide normal compensation state for one policy version.
/// Decimal values use strings on JSON boundaries so persistence and WASM do
/// not pass through binary floating point or round to currency cents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyState {
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukTabanUcret: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukYemek: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub birlestirilmisSosyalYardim: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukVasitaYol: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub giyimYardimi: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub hizmetZammiBirimi: Decimal,
    /// Required nullable legacy/global rate. The new engine does not fall
    /// back to it when a personnel group is missing.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub isPrimiYuzde: Option<Decimal>,
    pub isPrimiGruplari: Vec<CompensationPolicyGroup>,
    #[serde(with = "rust_decimal::serde::str")]
    pub geceCalismaPrimiYuzde: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub geceCalismaTatiliPrimiYuzde: Decimal,
    /// Required nullable values: null means absent; "0" means explicit zero.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub ekOdeme: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub digerGelirVarsayilan: Option<Decimal>,
    /// None preserves the historical monthly-income behavior for older
    /// policy versions and newly migrated backups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socialBenefitAccrualPolicies: Option<SocialBenefitAccrualPolicies>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BenefitPaymentMethod {
    Full,
    FullIfEligibleDay,
    Proportional,
    NoPayment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BenefitDayType {
    Worked,
    PaidRestHolidayOrLeave,
    PaidSickLeave,
    UnpaidSickLeave,
    UnpaidLeave,
    OutsideEmployment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BenefitProrationDenominator {
    CalendarDays,
    ThirtyDayMonth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BenefitAccrualPolicy {
    pub method: BenefitPaymentMethod,
    #[serde(default)]
    pub eligibleDayTypes: Vec<BenefitDayType>,
    pub denominator: BenefitProrationDenominator,
    /// `None` is the legacy, uncapped 30-day behavior. New form-created
    /// proportional rules write `Some(false)` unless the contract explicitly
    /// permits the monthly amount to be exceeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowMonthlyAmountExcess: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SocialBenefitAccrualPolicies {
    #[serde(default)]
    pub birlestirilmisSosyalYardim: Option<BenefitAccrualPolicy>,
    #[serde(default)]
    pub giyimYardimi: Option<BenefitAccrualPolicy>,
    #[serde(default)]
    pub hizmetZammi: Option<BenefitAccrualPolicy>,
    #[serde(default)]
    pub ekOdeme: Option<BenefitAccrualPolicy>,
    #[serde(default)]
    pub digerGelir: Option<BenefitAccrualPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyGroup {
    pub id: String,
    pub ad: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub oran: Decimal,
    pub aktif: bool,
}

/// Declares whether an effective policy date starts a new earning rounding
/// slice or changes only the road amount. Tokens are part of the frozen JSON
/// contract and intentionally have no default.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompensationBoundaryScope {
    FullEarning,
    RoadOnly,
}

impl CompensationBoundaryScope {
    pub const fn as_wire_token(self) -> &'static str {
        match self {
            Self::FullEarning => "FULL_EARNING",
            Self::RoadOnly => "ROAD_ONLY",
        }
    }

    pub fn parse_wire_token(value: &str) -> Option<Self> {
        match value {
            "FULL_EARNING" => Some(Self::FullEarning),
            "ROAD_ONLY" => Some(Self::RoadOnly),
            _ => None,
        }
    }
}

/// Declares the persisted road rounding boundary for one policy version.
/// Every resolved active version in one payroll service period must agree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoadRoundingMode {
    PerCompensationSlice,
    PeriodTotal,
}

impl RoadRoundingMode {
    pub const fn as_wire_token(self) -> &'static str {
        match self {
            Self::PerCompensationSlice => "PER_COMPENSATION_SLICE",
            Self::PeriodTotal => "PERIOD_TOTAL",
        }
    }

    pub fn parse_wire_token(value: &str) -> Option<Self> {
        match value {
            "PER_COMPENSATION_SLICE" => Some(Self::PerCompensationSlice),
            "PERIOD_TOTAL" => Some(Self::PeriodTotal),
            _ => None,
        }
    }
}

/// Immutable full-state policy record. Corrections append a new row at the
/// same effective date and point to the previously active leaf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyVersion {
    pub id: String,
    pub effectiveFrom: String,
    pub state: CompensationPolicyState,
    pub boundaryScope: CompensationBoundaryScope,
    pub roadRoundingMode: RoadRoundingMode,
    pub createdAt: String,
    pub source: String,
    pub reason: String,
    pub supersedesVersionId: Option<String>,
}

/// Inclusive persisted range resolved from one immutable policy version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCompensationSegment {
    pub versionId: String,
    pub startDate: String,
    pub endDate: String,
    pub state: CompensationPolicyState,
    pub boundaryScope: CompensationBoundaryScope,
    pub roadRoundingMode: RoadRoundingMode,
}

/// Identifies records produced by the new compensation-policy calculation
/// path. Missing metadata is deliberately interpreted as legacy provenance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayrollCalculationProvenance {
    COMPENSATION_POLICY_ENGINE,
}

/// Optional envelope keeps legacy payroll JSON unchanged while allowing a
/// current-engine record to be explicitly marked even before its snapshot is
/// present (for example, while it is DRAFT).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollCalculationMetadata {
    pub provenance: PayrollCalculationProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PayrollCalculationSnapshot>,
}

/// Persisted calculation inputs and the resolved compensation timeline used
/// by one BordroKaydi. Its person, service-period, tax-period, and accrual
/// identity is owned by the parent BordroKaydi plus BordroDonemi context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollCalculationSnapshot {
    pub snapshotSchemaVersion: u16,
    /// Semantic formula/replay version; this is independent of a build hash.
    pub calculationVersion: String,
    pub compensation: ResolvedCompensationSnapshot,
    pub usedInputs: PayrollUsedInputSnapshot,
}

/// The exact full-state compensation segments resolved for the payroll period.
/// Reuses the domain segment model instead of maintaining a second state shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCompensationSnapshot {
    pub segments: Vec<ResolvedCompensationSegment>,
}

/// Exact scoped inputs consumed for one calculation. Optional source objects
/// remain explicit; financial Option values use required-nullable string JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollUsedInputSnapshot {
    pub personnelGroup: String,
    pub serviceYear: i32,
    pub personalDeductions: Option<PayrollPersonalDeductionInputsSnapshot>,
    pub attendance: Option<PayrollAttendanceInputSnapshot>,
    pub paidSickLeave: Option<PayrollPaidSickLeaveInputSnapshot>,
    pub manualIncome: PayrollManualIncomeInputSnapshot,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub accrualGrossAmount: Option<Decimal>,
    pub institutionDeductionSettings: PayrollInstitutionDeductionInputsSnapshot,
    pub statutoryParameters: PayrollStatutoryInputsSnapshot,
    pub annualParameters: PayrollAnnualParametersSnapshot,
    pub taxHistory: PayrollTaxHistoryInputsSnapshot,
    pub incomingPekCarry: PayrollIncomingPekCarrySnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollPersonalDeductionInputsSnapshot {
    pub sendikaUyesi: Option<bool>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sabitSendikaAidati: Option<Decimal>,
    pub besUyesi: Option<bool>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub oksOraniYuzde: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sabitBesTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub icraTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub nafakaTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub kisiBorcuTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub dogumAskerlikBorclanmasiTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub hayatSaglikSigortasiTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub digerKesintiTutar: Option<Decimal>,
    pub gvIndirimleri: Option<PayrollGvDeductionInputsSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollGvDeductionInputsSnapshot {
    #[serde(with = "rust_decimal::serde::str_option")]
    pub dogumAskerlikGvIndirimTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub hayatSigortasiPrimiTutar: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub saglikSigortasiPrimiTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayrollAttendanceCoverage {
    COMPLETE,
    PARTIAL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollAttendanceInputSnapshot {
    /// Coverage is relative to the parent BordroDonemi inclusive date range.
    pub coverage: PayrollAttendanceCoverage,
    pub days: Vec<PayrollAttendanceDaySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollAttendanceDaySnapshot {
    pub date: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollPaidSickLeaveInputSnapshot {
    /// Immutable policy id for the episode-to-paid-date rule.
    pub ruleVersion: String,
    pub sourceEpisodes: Vec<PayrollSickLeaveEpisodeSnapshot>,
    pub paidDates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollSickLeaveEpisodeSnapshot {
    pub id: String,
    pub startDate: String,
    pub endDate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollManualIncomeInputSnapshot {
    /// Distinguishes no request object from an explicitly supplied empty input.
    pub supplied: bool,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub tediye: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub tisIkramiyesi: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollInstitutionDeductionInputsSnapshot {
    /// Resolved institution choice used for income-tax and stamp-tax treatment.
    pub yemekTamGvIstisnaDamgaTabi: bool,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub damgaVergisiOraniBinde: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sendikaAidatiYuzde: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sabitSendikaAidati: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub besOraniYuzde: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sabitBesTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollStatutoryInputsSnapshot {
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukAsgariUcret: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub sgkIsciOraniYuzde: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub issizlikIsciOraniYuzde: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub pekTavanKatsayisi: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukYemekIstisnasiSGK: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gunlukYemekIstisnasiGV: Decimal,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sgkIsverenOraniYuzde: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub issizlikIsverenOraniYuzde: Option<Decimal>,
    pub statutoryParameterSegments: Vec<PayrollStatutoryInputSegmentSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollStatutoryInputSegmentSnapshot {
    pub effectiveFrom: String,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub gunlukAsgariUcret: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub pekTavanKatsayisi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollAnnualParametersSnapshot {
    pub year: i32,
    pub gelirVergisiDilimleri: Vec<PayrollTaxBracketSnapshot>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sigortaGvYillikBrutAsgariUcretTavani: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollTaxBracketSnapshot {
    #[serde(with = "rust_decimal::serde::str")]
    pub limit: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub oran: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayrollTaxOpeningSource {
    EXPLICIT_OPENING,
    LEGACY_PERSONNEL_VALUE,
    IMPLICIT_ZERO,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollTaxOpeningSnapshot {
    pub source: PayrollTaxOpeningSource,
    pub openingId: Option<String>,
    pub effectiveFromTaxYear: i32,
    pub effectiveFromTaxMonth: i32,
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollTaxHistoryInputsSnapshot {
    pub normalGvOpening: PayrollTaxOpeningSnapshot,
    pub minimumWageGvOpening: PayrollTaxOpeningSnapshot,
    #[serde(with = "rust_decimal::serde::str")]
    pub previousCumulativeGv: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub previousCumulativeMinimumWageGv: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub previousAnnualInsuranceGvDeduction: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub samePaymentMonthPekUsed: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub samePaymentMonthGvExemptionUsed: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub samePaymentMonthStampExemptionUsed: Decimal,
    pub previousPaymentEvents: Vec<PayrollPriorPaymentEventSnapshot>,
    pub minimumWageMonthlyReferences: Vec<PayrollMinimumWageMonthInputSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollPriorPaymentEventSnapshot {
    pub periodId: String,
    pub accrualId: String,
    pub accrualType: AccrualType,
    pub taxYear: i32,
    pub taxMonth: i32,
    pub paymentDate: String,
    pub sequence: i32,
    #[serde(with = "rust_decimal::serde::str")]
    pub gvTaxableBase: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub insuranceGvDeductionUsed: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gvExemptionUsed: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub stampExemptionUsed: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub pekPrimeBase: Decimal,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub monthPekConsumptionAfter: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str")]
    pub pekCeiling: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub mealSgkExemption: Decimal,
    pub income: PayrollIncomeInputsSnapshot,
    pub outgoingPekCarry: Vec<PayrollPekCarrySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollIncomeInputsSnapshot {
    #[serde(with = "rust_decimal::serde::str_option")]
    pub tabanBrutAylik: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub tediye: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub tisIkramiyesi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub ekOdeme: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub yemek: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub birlestirilmisSosyalYardim: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub vasitaYol: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub giyimYardimi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub isPrimi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub geceCalismasiUcreti: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub geceCalismasiTatiliUcreti: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub hizmetZammi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub digerGelir: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollMinimumWageMonthInputSnapshot {
    pub periodId: String,
    pub taxYear: i32,
    pub taxMonth: i32,
    #[serde(with = "rust_decimal::serde::str")]
    pub dailyMinimumWage: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub sgkWorkerRatePercent: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unemploymentWorkerRatePercent: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub monthlyGvBase: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayrollPekCarrySource {
    NONE,
    PAYMENT_EVENT,
    RETRO_OVERRIDE,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollIncomingPekCarrySnapshot {
    pub source: PayrollPekCarrySource,
    pub sourceId: Option<String>,
    pub sourcePeriodId: Option<String>,
    pub sourceTaxYear: Option<i32>,
    pub sourceTaxMonth: Option<i32>,
    pub taxMonthsElapsed: i32,
    pub records: Vec<PayrollPekCarrySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollPekCarrySnapshot {
    #[serde(with = "rust_decimal::serde::str")]
    pub tutar: Decimal,
    pub kalanAySayisi: i32,
    pub kaynakDonemId: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatutoryParameterSegment {
    /// Inclusive effective date inside this payroll period (YYYY-MM-DD).
    pub effectiveFrom: String,
    pub gunlukAsgariUcret: Option<Decimal>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatutorySnapshotSource {
    AttendanceBacked,
    ProvisionalPaymentMonth,
    /// Old/imported snapshots did not record provenance. Keep them readable,
    /// but let domain policy treat supplementary records conservatively.
    #[default]
    LegacyUnknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Legacy payroll snapshots may not contain these fields. When present,
    /// they make the historical SGK rate source immutable as well.
    #[serde(default)]
    pub sgkIsciOraniYuzde: Option<Decimal>,
    #[serde(default)]
    pub issizlikIsciOraniYuzde: Option<Decimal>,
}

/// Dönem için ilk authoritative hesaplamada kilitlenen yasal parametreler.
/// Attendance/PEK sonuçlarını içermez; bu nedenle kişi veya tahakkuk olayına
/// bağlı olmadan tarihsel asgari GV referansı üretilebilir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatutoryParameterSnapshot {
    pub gunlukAsgariUcret: Decimal,
    pub sgkIsciOraniYuzde: Decimal,
    pub issizlikIsciOraniYuzde: Decimal,
    pub pekTavanKatsayisi: Decimal,
    pub gunlukYemekIstisnasiSGK: Decimal,
    pub gunlukYemekIstisnasiGV: Decimal,
    #[serde(default)]
    pub statutoryParameterSegments: Vec<StatutoryParameterSegment>,
}

/// Dönem içindeki bağımsız yol yardımı değişikliği; tarih dahil uygulanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportRateChange {
    pub effectiveFrom: String,
    pub gunlukVasitaYol: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DonemselKurumDegerleri {
    pub donemId: String,
    pub gunlukTabanUcret: Decimal,
    pub gunlukYemek: Decimal,
    /// Kurum bordrosu tercihi: yemek tamamen GV istisna, tamamen DV'ye tabi.
    /// Alanı içermeyen eski ayarlar da varsayılan olarak bu tercihi kullanır.
    #[serde(default = "default_meal_payroll_tax_policy")]
    pub yemekTamGvIstisnaDamgaTabi: bool,
    pub birlestirilmisSosyalYardim: Decimal,
    pub gunlukVasitaYol: Decimal,
    /// None yalnız eski kayıtların zam takvimi hesabını korur. Some([]),
    /// dönem boyunca bağımsız başlangıç tutarı; tarihler bu tutarı değiştirir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transportRateChanges: Option<Vec<TransportRateChange>>,
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
    /// İlk authoritative payroll hesabında oluşturulan immutable yasal
    /// kaynak. Mevcut ayarların sonradan değiştirilmesi bu snapshot'ı bozmaz.
    #[serde(default)]
    pub statutoryParameterSnapshot: Option<StatutoryParameterSnapshot>,
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

/// Raw period-settings DTO retained for database and legacy-backup
/// compatibility. It is not an input model for active payroll calculation.
pub type LegacyInstitutionSettings = DonemselKurumDegerleri;

/// Inputs that are allowed to cross the active payroll calculation boundary.
/// Compensation rates live in `CompensationPolicyVersion`; the legacy
/// `DonemselKurumDegerleri` DTO remains available only for persisted/imported
/// period settings and compatibility adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PayrollPeriodInstitutionSettings {
    pub donemId: String,
    #[serde(default = "default_meal_payroll_tax_policy")]
    pub yemekTamGvIstisnaDamgaTabi: bool,
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
    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub gunlukYemekIstisnasiGV: Option<Decimal>,
    pub statutoryParameterSegments: Option<Vec<StatutoryParameterSegment>>,
    pub statutoryParameterSnapshot: Option<StatutoryParameterSnapshot>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukAsgariUcret: Option<Decimal>,
    pub sgkIsverenOraniYuzde: Option<Decimal>,
    pub issizlikIsverenOraniYuzde: Option<Decimal>,
}

impl Default for PayrollPeriodInstitutionSettings {
    fn default() -> Self {
        Self {
            donemId: String::new(),
            yemekTamGvIstisnaDamgaTabi: true,
            tediyeListesi: None,
            tisIkramiyeListesi: None,
            tediyeTisNotu: None,
            sgkIsciOraniYuzde: None,
            issizlikIsciOraniYuzde: None,
            gelirVergisiOraniYuzde: None,
            damgaVergisiOraniBinde: None,
            sendikaAidatiYuzde: None,
            sabitSendikaAidati: None,
            besOraniYuzde: None,
            sabitBesTutar: None,
            gunlukYemekIstisnasiSGK: None,
            gunlukYemekIstisnasiGV: None,
            statutoryParameterSegments: None,
            statutoryParameterSnapshot: None,
            pekTavanKatsayisi: None,
            gunlukAsgariUcret: None,
            sgkIsverenOraniYuzde: None,
            issizlikIsverenOraniYuzde: None,
        }
    }
}

impl From<&DonemselKurumDegerleri> for PayrollPeriodInstitutionSettings {
    fn from(value: &DonemselKurumDegerleri) -> Self {
        Self {
            donemId: value.donemId.clone(),
            yemekTamGvIstisnaDamgaTabi: value.yemekTamGvIstisnaDamgaTabi,
            tediyeListesi: value.tediyeListesi.clone(),
            tisIkramiyeListesi: value.tisIkramiyeListesi.clone(),
            tediyeTisNotu: value.tediyeTisNotu.clone(),
            sgkIsciOraniYuzde: value.sgkIsciOraniYuzde,
            issizlikIsciOraniYuzde: value.issizlikIsciOraniYuzde,
            gelirVergisiOraniYuzde: value.gelirVergisiOraniYuzde,
            damgaVergisiOraniBinde: value.damgaVergisiOraniBinde,
            sendikaAidatiYuzde: value.sendikaAidatiYuzde,
            sabitSendikaAidati: value.sabitSendikaAidati,
            besOraniYuzde: value.besOraniYuzde,
            sabitBesTutar: value.sabitBesTutar,
            gunlukYemekIstisnasiSGK: value.gunlukYemekIstisnasiSGK,
            gunlukYemekIstisnasiGV: value.gunlukYemekIstisnasiGV,
            statutoryParameterSegments: value.statutoryParameterSegments.clone(),
            statutoryParameterSnapshot: value.statutoryParameterSnapshot.clone(),
            pekTavanKatsayisi: value.pekTavanKatsayisi,
            gunlukAsgariUcret: value.gunlukAsgariUcret,
            sgkIsverenOraniYuzde: value.sgkIsverenOraniYuzde,
            issizlikIsverenOraniYuzde: value.issizlikIsverenOraniYuzde,
        }
    }
}

impl From<DonemselKurumDegerleri> for PayrollPeriodInstitutionSettings {
    fn from(value: DonemselKurumDegerleri) -> Self {
        Self::from(&value)
    }
}

fn default_meal_payroll_tax_policy() -> bool {
    true
}

impl Default for DonemselKurumDegerleri {
    fn default() -> Self {
        use rust_decimal_macros::dec;
        Self {
            donemId: "".to_string(),
            gunlukTabanUcret: dec!(2443.28),
            gunlukYemek: dec!(300.75),
            yemekTamGvIstisnaDamgaTabi: true,
            birlestirilmisSosyalYardim: dec!(5089.70),
            gunlukVasitaYol: dec!(128.93),
            transportRateChanges: None,
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
            statutoryParameterSnapshot: None,
            pekTavanKatsayisi: Some(dec!(9)),
            gunlukAsgariUcret: Some(dec!(1101.00)),
            sgkIsverenOraniYuzde: Some(dec!(21.75)),
            issizlikIsverenOraniYuzde: Some(dec!(2.00)),
        }
    }
}
