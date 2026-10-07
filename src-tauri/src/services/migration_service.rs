#![allow(non_snake_case)]

use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::SettingsRepository;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use crate::services::payroll_service::PayrollService;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};

pub const CURRENT_BACKUP_VERSION: u32 = 6;
const MULTI_ACCRUAL_BACKUP_VERSION: u32 = 3;
const RETRO_AUTHORITY_BACKUP_VERSION: u32 = 5;
const COMPENSATION_SNAPSHOT_BACKUP_VERSION: u32 = 6;

/// Legacy person records stored only a month integer. Resolve it only when
/// the imported period set proves one unique period; ambiguous work-month vs
/// tax-month interpretations stay in the legacy person fields and are not
/// silently converted into a wrong opening row.
fn resolve_legacy_opening_period_id(
    conn: &Connection,
    year: i32,
    raw_month: Option<i32>,
) -> Result<Option<String>> {
    let Some(raw_month) = raw_month else {
        return Ok(None);
    };
    if !(1..=12).contains(&raw_month) {
        return Ok(None);
    }
    let mut candidates = PeriodRepository::get_all(conn)?
        .into_iter()
        .filter(|period| {
            period.taxYear == year && (period.ay == raw_month || period.taxMonth == raw_month)
        })
        .map(|period| period.id)
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    Ok((candidates.len() == 1).then(|| candidates.remove(0)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPayload {
    #[serde(default)]
    pub backupVersion: Option<u32>,
    #[serde(default)]
    pub exportedAt: Option<String>,
    #[serde(default)]
    pub donemler: Option<Vec<BordroDonemi>>,
    #[serde(default)]
    pub aktifDonemId: Option<String>,
    #[serde(default)]
    pub personeller: Option<Vec<LegacyPersonel>>,
    #[serde(default)]
    pub kurumDegerleriMap: Option<HashMap<String, payroll_core::PayrollPeriodInstitutionSettings>>,
    #[serde(default)]
    pub legacyInstitutionSettings: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub puantajlar: Option<Vec<PersonelPuantaj>>,
    #[serde(default)]
    pub bordrolar: Option<Vec<BordroKaydi>>,
    #[serde(default)]
    pub taxOpenings: Option<Vec<PersonelTaxOpening>>,
    #[serde(default)]
    pub sickLeaveRecords: Option<Vec<SickLeaveRecord>>,
    #[serde(default)]
    pub annualPayrollParameters: Option<Vec<AnnualPayrollParameters>>,
    #[serde(default)]
    pub zamAylari: Option<Vec<i32>>,
    #[serde(default)]
    pub compensationRevisions: Option<Vec<CompensationRevision>>,
    #[serde(default)]
    pub compensationRevisionOverrides: Option<Vec<CompensationRevisionOverride>>,
    #[serde(default)]
    pub retroBatches: Option<Vec<RetroAdjustmentBatch>>,
    #[serde(default)]
    pub retroAllocations: Option<Vec<RetroAllocation>>,
    #[serde(default)]
    pub compensationPolicyVersions: Option<Vec<CompensationPolicyVersion>>,
    /// V6 additive field: institution-wide groups, independent from policy dates.
    #[serde(default)]
    pub isPrimiGruplari: Option<Vec<IsPrimiGrupItem>>,
    /// Periods whose original legacy money fields were all present in the
    /// incoming JSON. This is import-only provenance and is never persisted.
    #[serde(skip)]
    pub exactLegacyCompensationSourcePeriods: BTreeSet<String>,
}

fn exact_legacy_compensation_source_periods(raw: &serde_json::Value) -> BTreeSet<String> {
    const REQUIRED_FIELDS: &[&str] = &[
        "gunlukTabanUcret",
        "gunlukYemek",
        "birlestirilmisSosyalYardim",
        "gunlukVasitaYol",
        "giyimYardimi",
        "hizmetZammiBirimi",
        "isPrimiYuzde",
        "isPrimiGruplari",
        "geceCalismaPrimiYuzde",
        "geceCalismaTatiliPrimiYuzde",
        "ekOdeme",
        "digerGelirVarsayilan",
    ];
    raw.get("legacyInstitutionSettings")
        .or_else(|| raw.get("kurumDegerleriMap"))
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|settings| settings.iter())
        .filter_map(|(period_id, settings)| {
            let settings = settings.as_object()?;
            REQUIRED_FIELDS
                .iter()
                .all(|field| settings.contains_key(*field))
                .then(|| period_id.clone())
        })
        .collect()
}

const ACTIVE_PERIOD_SETTING_FIELDS: &[&str] = &[
    "donemId",
    "yemekTamGvIstisnaDamgaTabi",
    "tediyeListesi",
    "tisIkramiyeListesi",
    "tediyeTisNotu",
    "sgkIsciOraniYuzde",
    "issizlikIsciOraniYuzde",
    "gelirVergisiOraniYuzde",
    "damgaVergisiOraniBinde",
    "sendikaAidatiYuzde",
    "sabitSendikaAidati",
    "besOraniYuzde",
    "sabitBesTutar",
    "gunlukYemekIstisnasiSGK",
    "gunlukYemekIstisnasiGV",
    "statutoryParameterSegments",
    "statutoryParameterSnapshot",
    "pekTavanKatsayisi",
    "gunlukAsgariUcret",
    "sgkIsverenOraniYuzde",
    "issizlikIsverenOraniYuzde",
];

const LEGACY_COMPENSATION_SETTING_FIELDS: &[&str] = &[
    "gunlukTabanUcret",
    "gunlukYemek",
    "birlestirilmisSosyalYardim",
    "gunlukVasitaYol",
    "transportRateChanges",
    "giyimYardimi",
    "hizmetZammiBirimi",
    "isPrimiYuzde",
    "isPrimiGruplari",
    "geceCalismaPrimiYuzde",
    "geceCalismaTatiliPrimiYuzde",
    "ekOdeme",
    "digerGelirVarsayilan",
];

fn legacy_settings_to_active_map(raw: &Value) -> Option<Value> {
    let source = raw.as_object()?;
    let mut active = Map::with_capacity(source.len());
    for (period_id, raw_settings) in source {
        let Some(raw_settings) = raw_settings.as_object() else {
            active.insert(period_id.clone(), raw_settings.clone());
            continue;
        };
        let mut settings = Map::new();
        for field in ACTIVE_PERIOD_SETTING_FIELDS {
            if let Some(value) = raw_settings.get(*field) {
                settings.insert((*field).to_owned(), value.clone());
            }
        }
        settings.insert("donemId".into(), Value::String(period_id.clone()));
        active.insert(period_id.clone(), Value::Object(settings));
    }
    Some(Value::Object(active))
}

fn contains_legacy_compensation_settings(raw: &Value) -> bool {
    raw.as_object().is_some_and(|settings_map| {
        settings_map.values().any(|settings| {
            settings.as_object().is_some_and(|settings| {
                LEGACY_COMPENSATION_SETTING_FIELDS
                    .iter()
                    .any(|field| settings.contains_key(*field))
            })
        })
    })
}

fn validate_v6_payload(payload: &mut LegacyPayload) -> Result<()> {
    let versions = payload.compensationPolicyVersions.as_ref().ok_or_else(|| {
        DomainError::InvalidData(
            "V6 yedek payload'ı compensationPolicyVersions koleksiyonunu içermelidir.".into(),
        )
    })?;
    let ordered_versions =
        payroll_core::validate_compensation_policy_timeline(versions).map_err(|error| {
            DomainError::InvalidData(format!("V6 policy timeline geçersiz: {error}"))
        })?;
    if let Some(groups) = payload.isPrimiGruplari.as_ref() {
        validate_global_premium_groups(groups)?;
    }
    let periods = payload.donemler.as_ref().ok_or_else(|| {
        DomainError::InvalidData("V6 yedek payload'ı donemler koleksiyonunu içermelidir.".into())
    })?;
    let mut periods_by_id = HashMap::with_capacity(periods.len());
    for period in periods {
        if periods_by_id.insert(period.id.as_str(), period).is_some() {
            return Err(DomainError::InvalidData(format!(
                "V6 yedek payload'ında yinelenen dönem kimliği: {}.",
                period.id
            )));
        }
    }
    let payrolls = payload.bordrolar.as_ref().ok_or_else(|| {
        DomainError::InvalidData("V6 yedek payload'ı bordrolar koleksiyonunu içermelidir.".into())
    })?;
    let mut payroll_ids = BTreeSet::new();
    for payroll in payrolls {
        if !payroll_ids.insert(payroll.id.as_str()) {
            return Err(DomainError::InvalidData(format!(
                "V6 yedek payload'ında yinelenen bordro kimliği: {}.",
                payroll.id
            )));
        }
        let period = periods_by_id.get(payroll.donemId.as_str()).ok_or_else(|| {
            DomainError::InvalidData(format!(
                "V6 bordro {} için parent dönem bulunamadı: {}.",
                payroll.id, payroll.donemId
            ))
        })?;
        let is_policy_engine_row = payroll
            .calculationMetadata
            .as_ref()
            .is_some_and(|metadata| {
                metadata.provenance
                    == payroll_core::PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE
            });
        if is_policy_engine_row {
            payroll_core::validate_payroll_calculation_metadata_with_context(
                payroll, period, payrolls, periods,
            )
            .map_err(|error| {
                DomainError::InvalidData(format!(
                    "V6 bordro {} calculationMetadata doğrulaması başarısız: {error}",
                    payroll.id
                ))
            })?;
            payroll_core::validate_compensation_snapshot_policy_links(payroll, &ordered_versions)
                .map_err(|error| {
                DomainError::InvalidData(format!(
                    "V6 bordro {} snapshot policy bağlantısı geçersiz: {error}",
                    payroll.id
                ))
            })?;
        }
    }
    payload.compensationPolicyVersions = Some(ordered_versions);
    Ok(())
}

fn validate_global_premium_groups(groups: &[IsPrimiGrupItem]) -> Result<()> {
    if groups.is_empty() {
        return Err(DomainError::InvalidData(
            "V6 iş primi grupları listesi boş olamaz.".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for group in groups {
        let normalized_id = group.id.trim().to_lowercase();
        let normalized_name = group.ad.trim().to_lowercase();
        if normalized_id.is_empty()
            || normalized_id != group.id.to_lowercase()
            || normalized_name.is_empty()
            || normalized_name != group.ad.to_lowercase()
            || group.oran < rust_decimal::Decimal::ZERO
            || group.oran > rust_decimal::Decimal::from(100)
            || !ids.insert(normalized_id)
            || !names.insert(normalized_name)
        {
            return Err(DomainError::InvalidData(
                "V6 iş primi grupları kod, ad ve %0–%100 oran bakımından geçersiz veya yinelenen değer içeriyor.".into(),
            ));
        }
    }
    Ok(())
}

fn validate_policy_restore_compatibility(
    conn: &Connection,
    incoming: &[CompensationPolicyVersion],
) -> Result<Vec<CompensationPolicyVersion>> {
    let ordered =
        payroll_core::validate_compensation_policy_timeline(incoming).map_err(|error| {
            DomainError::InvalidData(format!("Yedek policy timeline geçersiz: {error}"))
        })?;
    let existing = CompensationPolicyRepository::get_all_versions(conn)?;
    let incoming_by_id: HashMap<&str, &CompensationPolicyVersion> = ordered
        .iter()
        .map(|version| (version.id.as_str(), version))
        .collect();
    let existing_ids: BTreeSet<&str> = existing.iter().map(|version| version.id.as_str()).collect();

    for version in &existing {
        let Some(restored) = incoming_by_id.get(version.id.as_str()) else {
            return Err(DomainError::InvalidData(format!(
                "Immutable policy restore çakışması: mevcut sürüm {} payload'da yok; policy geçmişi silinemez.",
                version.id
            )));
        };
        let existing_json = serde_json::to_string(version)
            .map_err(|error| DomainError::InvalidData(error.to_string()))?;
        let restored_json = serde_json::to_string(restored)
            .map_err(|error| DomainError::InvalidData(error.to_string()))?;
        if existing_json != restored_json {
            return Err(DomainError::InvalidData(format!(
                "Immutable policy restore çakışması: {} kimliği farklı full-state, boundary flag veya supersession içeriyor.",
                version.id
            )));
        }
    }

    Ok(ordered
        .into_iter()
        .filter(|version| !existing_ids.contains(version.id.as_str()))
        .collect())
}

fn restore_policy_timeline_in_transaction(
    conn: &Connection,
    incoming: &[CompensationPolicyVersion],
) -> Result<()> {
    let missing = validate_policy_restore_compatibility(conn, incoming)?;
    for version in missing {
        CompensationPolicyRepository::insert_version_in_transaction(conn, &version)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPersonel {
    pub id: String,
    pub tcNo: String,
    pub ad: String,
    pub soyad: String,
    pub grup: String,
    #[serde(default)]
    pub unvan: Option<String>,
    #[serde(default)]
    pub sgkSicilNo: Option<String>,
    #[serde(default)]
    pub iban: Option<String>,
    #[serde(default)]
    pub hizmetYili: Option<i32>,
    #[serde(default)]
    pub aciklama: Option<String>,
    #[serde(default)]
    pub devirKumulatifGvMatrahi: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub devirKumulatifGvMatrahiYili: Option<i32>,
    #[serde(default)]
    pub devirKumulatifGvMatrahiBaslangicAyi: Option<i32>,
    #[serde(default)]
    pub devirKumulatifAsgariGvMatrahi: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub devirKumulatifAsgariGvMatrahiYili: Option<i32>,
    #[serde(default)]
    pub kesintiler: Option<PersonelKesintileri>,
}

/// V3/V4 persisted the signed entitlement ledger but had no explicit settlement
/// flow.  Upgrade that ledger once, in deterministic order, without changing
/// any historical payroll snapshot.  A positive legacy delta remains payable;
/// a negative delta becomes an open receivable.  Mixed allocation signs are
/// allocated against the batch net amount, matching the core settlement
/// reconciliation rather than treating every negative allocation as a new
/// receivable.
fn normalize_legacy_retro_settlement(
    batches: &mut [RetroAdjustmentBatch],
    allocations: &mut [RetroAllocation],
) {
    let mut order = (0..batches.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        batches[*left]
            .personnelId
            .cmp(&batches[*right].personnelId)
            .then_with(|| batches[*left].paymentDate.cmp(&batches[*right].paymentDate))
            .then_with(|| batches[*left].id.cmp(&batches[*right].id))
    });

    let mut outstanding_by_personnel = HashMap::<String, rust_decimal::Decimal>::new();
    for batch_index in order {
        let batch = &mut batches[batch_index];
        let authoritative = matches!(
            batch.status,
            CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
        );
        let total = batch.totalGrossDelta.round_dp(2);
        let payable = total.max(rust_decimal::Decimal::ZERO);
        let recoverable = (-total).max(rust_decimal::Decimal::ZERO);
        let personnel_id = batch.personnelId.clone();

        batch.payableSettlementAmount = payable;
        batch.offsetSettlementAmount = rust_decimal::Decimal::ZERO;
        batch.recoveredAmount = rust_decimal::Decimal::ZERO;
        batch.recoverableAmount = recoverable;
        batch.outstandingReceivable = outstanding_by_personnel
            .get(&personnel_id)
            .copied()
            .unwrap_or_default()
            + recoverable;
        batch.settlementStatus = if recoverable > rust_decimal::Decimal::ZERO {
            RetroSettlementStatus::OVERPAYMENT
        } else if batch.status == CompensationRevisionStatus::FINALIZED
            && payable > rust_decimal::Decimal::ZERO
        {
            RetroSettlementStatus::PAID
        } else {
            RetroSettlementStatus::UNSETTLED
        };

        let mut allocation_order = allocations
            .iter()
            .enumerate()
            .filter(|(_, allocation)| allocation.batchId == batch.id)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        allocation_order.sort_by(|left, right| {
            allocations[*left]
                .sourcePeriodId
                .cmp(&allocations[*right].sourcePeriodId)
                .then_with(|| {
                    allocations[*left]
                        .earningCode
                        .cmp(&allocations[*right].earningCode)
                })
                .then_with(|| allocations[*left].id.cmp(&allocations[*right].id))
        });
        let mut remaining_payable = payable;
        let mut remaining_recoverable = recoverable;
        for allocation_index in allocation_order {
            let allocation = &mut allocations[allocation_index];
            let positive = allocation.deltaAmount.max(rust_decimal::Decimal::ZERO);
            let negative = (-allocation.deltaAmount).max(rust_decimal::Decimal::ZERO);
            allocation.payableSettlementAmount = positive.min(remaining_payable);
            allocation.offsetSettlementAmount = rust_decimal::Decimal::ZERO;
            allocation.recoverableAmount = negative.min(remaining_recoverable);
            remaining_payable =
                (remaining_payable - allocation.payableSettlementAmount).round_dp(2);
            remaining_recoverable =
                (remaining_recoverable - allocation.recoverableAmount).round_dp(2);
        }
        if authoritative {
            outstanding_by_personnel.insert(personnel_id, batch.outstandingReceivable);
        }
    }
}

fn validate_retro_payment_links(conn: &Connection, backup_label: &str) -> Result<()> {
    let batches = crate::repositories::retro_repo::get_batches(conn)?;
    let payrolls = PayrollRepository::get_all(conn)?;

    for batch in &batches {
        let linked = payrolls
            .iter()
            .filter(|payroll| payroll.accrualId == batch.id)
            .collect::<Vec<_>>();
        if linked.len() > 1 {
            return Err(DomainError::InvalidData(format!(
                "{backup_label} restore: {} retro batch'i birden fazla payment event ile eşleşiyor.",
                batch.id
            )));
        }
        if batch.status == CompensationRevisionStatus::FINALIZED && linked.len() != 1 {
            return Err(DomainError::InvalidData(format!(
                "{backup_label} restore: FINALIZED retro batch {} için payment event bulunamadı.",
                batch.id
            )));
        }
        let payable = payroll_core::retro_payable_settlement_amount(batch);
        if batch.status == CompensationRevisionStatus::FINALIZED
            && payable <= rust_decimal::Decimal::ZERO
        {
            return Err(DomainError::InvalidData(format!(
                "{backup_label} restore: payable settlement olmayan retro batch {} FINALIZED olamaz.",
                batch.id
            )));
        }
        if let Some(payroll) = linked.first().copied() {
            let expected_status = match batch.status {
                CompensationRevisionStatus::FINALIZED => Some(BordroStatus::FINALIZED),
                CompensationRevisionStatus::CALCULATED => Some(BordroStatus::CALCULATED),
                CompensationRevisionStatus::DRAFT | CompensationRevisionStatus::STALE => None,
            };
            let linked_status_is_valid = expected_status == Some(payroll.status)
                || (batch.status == CompensationRevisionStatus::CALCULATED
                    && payroll.status == BordroStatus::STALE);
            if !linked_status_is_valid
                || payroll.accrualType != AccrualType::RETRO_ADJUSTMENT
                || payroll.personelId != batch.personnelId
                || payroll.paymentDate != batch.paymentDate
                || payable <= rust_decimal::Decimal::ZERO
                || payroll.gelirToplam != payable
            {
                return Err(DomainError::InvalidData(format!(
                    "{backup_label} restore: retro batch {} lifecycle durumu ile bağlı payment event durumu/kimliği/finansal snapshotı eşleşmiyor.",
                    batch.id
                )));
            }
        }
    }

    for payroll in payrolls
        .iter()
        .filter(|payroll| payroll.accrualType == AccrualType::RETRO_ADJUSTMENT)
    {
        let Some(batch) = batches.iter().find(|batch| batch.id == payroll.accrualId) else {
            return Err(DomainError::InvalidData(format!(
                "{backup_label} restore: {} retro payment event'i için batch bulunamadı.",
                payroll.accrualId
            )));
        };
        if payroll.personelId != batch.personnelId || payroll.paymentDate != batch.paymentDate {
            return Err(DomainError::InvalidData(format!(
                "{backup_label} restore: {} retro payment event'i batch personel/ödeme tarihiyle eşleşmiyor.",
                payroll.accrualId
            )));
        }
    }
    Ok(())
}

fn validate_v5_retro_lifecycle_fields(payload_json: &str) -> Result<()> {
    let root: serde_json::Value = serde_json::from_str(payload_json)
        .map_err(|error| DomainError::InvalidData(format!("Geçersiz V5 yedek JSON'u: {error}")))?;
    let Some(batches) = root
        .get("retroBatches")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for (index, batch) in batches.iter().enumerate() {
        let Some(batch) = batch.as_object() else {
            return Err(DomainError::InvalidData(format!(
                "V5 retroBatches[{index}] nesne olmalıdır."
            )));
        };
        for field in [
            "status",
            "settlementStatus",
            "payableSettlementAmount",
            "offsetSettlementAmount",
            "recoveredAmount",
            "recoverableAmount",
            "outstandingReceivable",
        ] {
            if !batch.contains_key(field) {
                return Err(DomainError::InvalidData(format!(
                    "V5 retroBatches[{index}].{field} zorunlu alan eksik."
                )));
            }
        }
    }
    if let Some(allocations) = root
        .get("retroAllocations")
        .and_then(serde_json::Value::as_array)
    {
        for (index, allocation) in allocations.iter().enumerate() {
            let Some(allocation) = allocation.as_object() else {
                return Err(DomainError::InvalidData(format!(
                    "V5 retroAllocations[{index}] nesne olmalıdır."
                )));
            };
            for field in [
                "originalEmployerLowerBound",
                "targetEmployerLowerBound",
                "employerLowerBoundDelta",
                "employerLowerBoundPremiumDelta",
                "payableSettlementAmount",
                "offsetSettlementAmount",
                "recoverableAmount",
            ] {
                if !allocation.contains_key(field) {
                    return Err(DomainError::InvalidData(format!(
                        "V5 retroAllocations[{index}].{field} zorunlu alan eksik."
                    )));
                }
            }
        }
    }
    Ok(())
}

fn replay_payroll_request_for_current_backup(
    dataset: &payroll_core::PayrollDatasetSnapshot,
    target: &BordroKaydi,
) -> Result<payroll_core::PayrollCalculationRequest> {
    let target_accrual_id = if target.accrualId.trim().is_empty() {
        target.id.clone()
    } else {
        target.accrualId.clone()
    };
    let period = dataset
        .periods
        .iter()
        .find(|period| period.id == target.donemId)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "V5 backup replay: {} bordrosunun dönemi bulunamadı.",
                target.id
            ))
        })?;
    let payment_date = if target.paymentDate.trim().is_empty() {
        payroll_core::payroll_engine::default_payment_date(period)
    } else {
        target.paymentDate.clone()
    };

    let gross_amount = match target.accrualType {
        AccrualType::NORMAL => None,
        AccrualType::TEDIYE => target.gelirler.tediye,
        AccrualType::TIS_IKRAMIYE => target.gelirler.tisIkramiyesi,
        AccrualType::SUPPLEMENTAL => target.gelirler.ekOdeme,
        AccrualType::RETRO_ADJUSTMENT => {
            let batch = dataset
                .retroBatches
                .iter()
                .find(|batch| batch.id == target_accrual_id)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "V5 backup replay: {} retro payment event'i için batch bulunamadı.",
                        target_accrual_id
                    ))
                })?;
            Some(payroll_core::retro_payable_settlement_amount(batch))
        }
    };

    let mut replay_dataset = dataset.clone();
    replay_dataset.payrolls.retain(|payroll| {
        payroll.id != target.id
            && (if payroll.accrualId.trim().is_empty() {
                payroll.id.as_str()
            } else {
                payroll.accrualId.as_str()
            }) != target_accrual_id
    });

    Ok(payroll_core::PayrollCalculationRequest {
        personnelId: target.personelId.clone(),
        periodId: target.donemId.clone(),
        calculatedAt: if !target.sonGuncellemeTarihi.trim().is_empty() {
            target.sonGuncellemeTarihi.clone()
        } else if !target.olusturulmaTarihi.trim().is_empty() {
            target.olusturulmaTarihi.clone()
        } else {
            "backup-replay".into()
        },
        manualIncome: (target.accrualType == AccrualType::NORMAL).then_some(
            ManualPayrollIncomeInput {
                tediye: target.gelirler.tediye,
                tisIkramiyesi: target.gelirler.tisIkramiyesi,
            },
        ),
        accrual: Some(PayrollAccrualInput {
            accrualId: target_accrual_id,
            accrualType: target.accrualType,
            paymentDate: payment_date,
            sequence: target.sequence,
            grossAmount: gross_amount,
            description: target.accrualDescription.clone(),
        }),
        dataset: replay_dataset,
    })
}

fn compare_backup_decimal(
    payroll_id: &str,
    field: &str,
    imported: rust_decimal::Decimal,
    replayed: rust_decimal::Decimal,
) -> Result<()> {
    if imported != replayed {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {}.{} snapshotı canonical Rust replay ile eşleşmiyor (import {}, replay {}).",
            payroll_id, field, imported, replayed
        )));
    }
    Ok(())
}

fn compare_backup_optional_decimal(
    payroll_id: &str,
    field: &str,
    imported: Option<rust_decimal::Decimal>,
    replayed: Option<rust_decimal::Decimal>,
) -> Result<()> {
    if imported != replayed {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {}.{} snapshotı canonical Rust replay ile eşleşmiyor (import {:?}, replay {:?}).",
            payroll_id, field, imported, replayed
        )));
    }
    Ok(())
}

fn compare_backup_optional_decimal_if_imported(
    payroll_id: &str,
    field: &str,
    imported: Option<rust_decimal::Decimal>,
    replayed: Option<rust_decimal::Decimal>,
) -> Result<()> {
    if let Some(imported) = imported {
        compare_backup_optional_decimal(payroll_id, field, Some(imported), replayed)?;
    }
    Ok(())
}

fn compare_backup_is_primi(
    payroll_id: &str,
    imported: Option<&IsPrimiHesapDetayi>,
    replayed: Option<&IsPrimiHesapDetayi>,
) -> Result<()> {
    let matches = match (imported, replayed) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.grupId == right.grupId
                && left.grupAd == right.grupAd
                && left.oran == right.oran
                && left.hakGunu == right.hakGunu
                && left.gunlukIsPrimi == right.gunlukIsPrimi
                && left.tutar == right.tutar
        }
        _ => false,
    };
    if !matches {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {} iş primi snapshot'ı canonical Rust replay ile eşleşmiyor.",
            payroll_id
        )));
    }
    Ok(())
}

fn compare_backup_income_and_deductions(
    imported: &BordroKaydi,
    replayed: &BordroKaydi,
) -> Result<()> {
    let id = imported.id.as_str();
    for (field, left, right) in [
        (
            "gelirler.tabanBrutAylik",
            imported.gelirler.tabanBrutAylik,
            replayed.gelirler.tabanBrutAylik,
        ),
        (
            "gelirler.tediye",
            imported.gelirler.tediye,
            replayed.gelirler.tediye,
        ),
        (
            "gelirler.tisIkramiyesi",
            imported.gelirler.tisIkramiyesi,
            replayed.gelirler.tisIkramiyesi,
        ),
        (
            "gelirler.ekOdeme",
            imported.gelirler.ekOdeme,
            replayed.gelirler.ekOdeme,
        ),
        (
            "gelirler.yemek",
            imported.gelirler.yemek,
            replayed.gelirler.yemek,
        ),
        (
            "gelirler.birlestirilmisSosyalYardim",
            imported.gelirler.birlestirilmisSosyalYardim,
            replayed.gelirler.birlestirilmisSosyalYardim,
        ),
        (
            "gelirler.vasitaYol",
            imported.gelirler.vasitaYol,
            replayed.gelirler.vasitaYol,
        ),
        (
            "gelirler.giyimYardimi",
            imported.gelirler.giyimYardimi,
            replayed.gelirler.giyimYardimi,
        ),
        (
            "gelirler.isPrimi",
            imported.gelirler.isPrimi,
            replayed.gelirler.isPrimi,
        ),
        (
            "gelirler.geceCalismasiUcreti",
            imported.gelirler.geceCalismasiUcreti,
            replayed.gelirler.geceCalismasiUcreti,
        ),
        (
            "gelirler.geceCalismasiTatiliUcreti",
            imported.gelirler.geceCalismasiTatiliUcreti,
            replayed.gelirler.geceCalismasiTatiliUcreti,
        ),
        (
            "gelirler.hizmetZammi",
            imported.gelirler.hizmetZammi,
            replayed.gelirler.hizmetZammi,
        ),
        (
            "gelirler.digerGelir",
            imported.gelirler.digerGelir,
            replayed.gelirler.digerGelir,
        ),
        (
            "kesintiler.isciSgkPrimi",
            imported.kesintiler.isciSgkPrimi,
            replayed.kesintiler.isciSgkPrimi,
        ),
        (
            "kesintiler.isciIssizlikPrimi",
            imported.kesintiler.isciIssizlikPrimi,
            replayed.kesintiler.isciIssizlikPrimi,
        ),
        (
            "kesintiler.gelirVergisi",
            imported.kesintiler.gelirVergisi,
            replayed.kesintiler.gelirVergisi,
        ),
        (
            "kesintiler.damgaVergisi",
            imported.kesintiler.damgaVergisi,
            replayed.kesintiler.damgaVergisi,
        ),
        (
            "kesintiler.sendikaAidati",
            imported.kesintiler.sendikaAidati,
            replayed.kesintiler.sendikaAidati,
        ),
        (
            "kesintiler.bes",
            imported.kesintiler.bes,
            replayed.kesintiler.bes,
        ),
        (
            "kesintiler.icra",
            imported.kesintiler.icra,
            replayed.kesintiler.icra,
        ),
        (
            "kesintiler.kisiBorcu",
            imported.kesintiler.kisiBorcu,
            replayed.kesintiler.kisiBorcu,
        ),
        (
            "kesintiler.dogumAskerlikBorclanmasi",
            imported.kesintiler.dogumAskerlikBorclanmasi,
            replayed.kesintiler.dogumAskerlikBorclanmasi,
        ),
        (
            "kesintiler.hayatSaglikSigortasi",
            imported.kesintiler.hayatSaglikSigortasi,
            replayed.kesintiler.hayatSaglikSigortasi,
        ),
        (
            "kesintiler.digerKesinti",
            imported.kesintiler.digerKesinti,
            replayed.kesintiler.digerKesinti,
        ),
    ] {
        compare_backup_optional_decimal(id, field, left, right)?;
    }
    for (field, left, right) in [
        ("gelirToplam", imported.gelirToplam, replayed.gelirToplam),
        (
            "kesintiToplam",
            imported.kesintiToplam,
            replayed.kesintiToplam,
        ),
        ("netOdeme", imported.netOdeme, replayed.netOdeme),
    ] {
        compare_backup_decimal(id, field, left, right)?;
    }
    Ok(())
}

fn compare_backup_snapshots(imported: &BordroKaydi, replayed: &BordroKaydi) -> Result<()> {
    let id = imported.id.as_str();
    compare_backup_income_and_deductions(imported, replayed)?;
    compare_backup_optional_decimal(
        id,
        "oncekiKumulatifGvMatrahi",
        imported.oncekiKumulatifGvMatrahi,
        replayed.oncekiKumulatifGvMatrahi,
    )?;
    compare_backup_optional_decimal(
        id,
        "oncekiKumulatifAsgariGvMatrahi",
        imported.oncekiKumulatifAsgariGvMatrahi,
        replayed.oncekiKumulatifAsgariGvMatrahi,
    )?;
    compare_backup_optional_decimal(
        id,
        "manuelKumulatifGvMatrahi",
        imported.manuelKumulatifGvMatrahi,
        replayed.manuelKumulatifGvMatrahi,
    )?;
    if imported.puantajOzeti.c != replayed.puantajOzeti.c
        || imported.puantajOzeti.t != replayed.puantajOzeti.t
        || imported.puantajOzeti.g != replayed.puantajOzeti.g
        || imported.puantajOzeti.i != replayed.puantajOzeti.i
        || imported.puantajOzeti.gc != replayed.puantajOzeti.gc
        || imported.puantajOzeti.gct != replayed.puantajOzeti.gct
        || imported.puantajOzeti.r != replayed.puantajOzeti.r
    {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {} puantaj özeti canonical Rust replay ile eşleşmiyor.",
            id
        )));
    }
    if imported.devredenPekGelen != replayed.devredenPekGelen
        || imported.sonrakiDevredenPek != replayed.sonrakiDevredenPek
    {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {} devreden PEK giriş/çıkış state'i canonical Rust replay ile eşleşmiyor.",
            id
        )));
    }
    if imported.statutorySnapshot != replayed.statutorySnapshot {
        return Err(DomainError::InvalidData(format!(
            "V5 backup replay: {} statutory snapshot canonical Rust replay ile eşleşmiyor.",
            id
        )));
    }
    // Compare Decimal values as numbers: serde_json values preserve the
    // imported scale, so equivalent values such as 219.9 and 219.90 differ.
    compare_backup_is_primi(
        id,
        imported.isPrimiDetay.as_ref(),
        replayed.isPrimiDetay.as_ref(),
    )?;

    let imported_gv_base = imported
        .persistedGvBase
        .or_else(|| imported.gvDetay.as_ref().map(|detail| detail.cariGvMatrahi));
    let replayed_gv_base = replayed
        .persistedGvBase
        .or_else(|| replayed.gvDetay.as_ref().map(|detail| detail.cariGvMatrahi));
    compare_backup_optional_decimal(id, "gvBase", imported_gv_base, replayed_gv_base)?;

    match (&imported.pekDetay, &replayed.pekDetay) {
        (Some(left), Some(right)) => {
            for (field, imported_value, replayed_value) in [
                (
                    "pekDetay.hesaplananPek",
                    left.hesaplananPek,
                    right.hesaplananPek,
                ),
                ("pekDetay.hamPek", left.hamPek, right.hamPek),
                (
                    "pekDetay.devredenPekKullanilan",
                    left.devredenPekKullanilan,
                    right.devredenPekKullanilan,
                ),
                ("pekDetay.primMatrahi", left.primMatrahi, right.primMatrahi),
                ("pekDetay.finalPek", left.finalPek, right.finalPek),
                (
                    "pekDetay.devredenPekAşanTutar",
                    left.devredenPekAşanTutar,
                    right.devredenPekAşanTutar,
                ),
                ("pekDetay.pekAltSinir", left.pekAltSinir, right.pekAltSinir),
                ("pekDetay.pekUstSinir", left.pekUstSinir, right.pekUstSinir),
                (
                    "pekDetay.altSinirTamamlamaFarki",
                    left.altSinirTamamlamaFarki,
                    right.altSinirTamamlamaFarki,
                ),
                (
                    "pekDetay.yemekIstisnasiTutar",
                    left.yemekIstisnasiTutar,
                    right.yemekIstisnasiTutar,
                ),
            ] {
                compare_backup_decimal(id, field, imported_value, replayed_value)?;
            }
            // These fields were added after older V5 snapshots were written.
            // An omitted historical value remains authoritative as omitted;
            // a supplied value still has to match replay exactly.
            compare_backup_optional_decimal_if_imported(
                id,
                "pekDetay.aylikOncekiPekTuketimi",
                left.aylikOncekiPekTuketimi,
                right.aylikOncekiPekTuketimi,
            )?;
            compare_backup_optional_decimal_if_imported(
                id,
                "pekDetay.aylikSonrasiPekTuketimi",
                left.aylikSonrasiPekTuketimi,
                right.aylikSonrasiPekTuketimi,
            )?;
            if left.fiiliYemekGunu != right.fiiliYemekGunu {
                return Err(DomainError::InvalidData(format!(
                    "V5 backup replay: {} pekDetay.fiiliYemekGunu canonical Rust replay ile eşleşmiyor.",
                    id
                )));
            }
            for (field, imported_value, replayed_value) in [
                (
                    "pekDetay.isverenSgkPrimi",
                    left.isverenSgkPrimi,
                    right.isverenSgkPrimi,
                ),
                (
                    "pekDetay.isverenIssizlikPrimi",
                    left.isverenIssizlikPrimi,
                    right.isverenIssizlikPrimi,
                ),
                (
                    "pekDetay.pekAltSinirTamamlamaIsverenPrimi",
                    left.pekAltSinirTamamlamaIsverenPrimi,
                    right.pekAltSinirTamamlamaIsverenPrimi,
                ),
                (
                    "pekDetay.isverenPrimToplami",
                    left.isverenPrimToplami,
                    right.isverenPrimToplami,
                ),
                (
                    "pekDetay.sgkIsverenOraniYuzde",
                    left.sgkIsverenOraniYuzde,
                    right.sgkIsverenOraniYuzde,
                ),
                (
                    "pekDetay.isverenIssizlikOraniYuzde",
                    left.isverenIssizlikOraniYuzde,
                    right.isverenIssizlikOraniYuzde,
                ),
            ] {
                compare_backup_optional_decimal(id, field, imported_value, replayed_value)?;
            }
        }
        (None, None) => {}
        _ => {
            return Err(DomainError::InvalidData(format!(
                "V5 backup replay: {} PEK snapshot varlığı canonical Rust replay ile eşleşmiyor.",
                id
            )))
        }
    }

    match (&imported.gvDetay, &replayed.gvDetay) {
        (Some(left), Some(right)) => {
            for (field, imported_value, replayed_value) in [
                (
                    "gvDetay.oncekiKumulatifGvMatrahi",
                    left.oncekiKumulatifGvMatrahi,
                    right.oncekiKumulatifGvMatrahi,
                ),
                (
                    "gvDetay.cariGvMatrahi",
                    left.cariGvMatrahi,
                    right.cariGvMatrahi,
                ),
                (
                    "gvDetay.yeniKumulatifGvMatrahi",
                    left.yeniKumulatifGvMatrahi,
                    right.yeniKumulatifGvMatrahi,
                ),
                (
                    "gvDetay.brutGelirVergisi",
                    left.brutGelirVergisi,
                    right.brutGelirVergisi,
                ),
                (
                    "gvDetay.asgariUcretGvMatrahi",
                    left.asgariUcretGvMatrahi,
                    right.asgariUcretGvMatrahi,
                ),
                (
                    "gvDetay.asgariUcretReferansKumulatifMatrahi",
                    left.asgariUcretReferansKumulatifMatrahi,
                    right.asgariUcretReferansKumulatifMatrahi,
                ),
                (
                    "gvDetay.asgariUcretGvIstisnasi",
                    left.asgariUcretGvIstisnasi,
                    right.asgariUcretGvIstisnasi,
                ),
                (
                    "gvDetay.ayniAyOncekiKullanilanGvIstisnasi",
                    left.ayniAyOncekiKullanilanGvIstisnasi,
                    right.ayniAyOncekiKullanilanGvIstisnasi,
                ),
                (
                    "gvDetay.tahakkukOncesiKalanGvIstisnasi",
                    left.tahakkukOncesiKalanGvIstisnasi,
                    right.tahakkukOncesiKalanGvIstisnasi,
                ),
                (
                    "gvDetay.uygulananGvIstisnasi",
                    left.uygulananGvIstisnasi,
                    right.uygulananGvIstisnasi,
                ),
                (
                    "gvDetay.tahakkukSonrasiKalanGvIstisnasi",
                    left.tahakkukSonrasiKalanGvIstisnasi,
                    right.tahakkukSonrasiKalanGvIstisnasi,
                ),
                (
                    "gvDetay.kesilenGelirVergisi",
                    left.kesilenGelirVergisi,
                    right.kesilenGelirVergisi,
                ),
                (
                    "gvDetay.dogumAskerlikGvIndirimi",
                    left.dogumAskerlikGvIndirimi,
                    right.dogumAskerlikGvIndirimi,
                ),
                (
                    "gvDetay.sigortaGvIndirimAdayi",
                    left.sigortaGvIndirimAdayi,
                    right.sigortaGvIndirimAdayi,
                ),
                (
                    "gvDetay.sigortaGvAylikLimiti",
                    left.sigortaGvAylikLimiti,
                    right.sigortaGvAylikLimiti,
                ),
                (
                    "gvDetay.sigortaGvYillikKalanLimiti",
                    left.sigortaGvYillikKalanLimiti,
                    right.sigortaGvYillikKalanLimiti,
                ),
                (
                    "gvDetay.uygulanabilirSigortaGvIndirimi",
                    left.uygulanabilirSigortaGvIndirimi,
                    right.uygulanabilirSigortaGvIndirimi,
                ),
            ] {
                compare_backup_decimal(id, field, imported_value, replayed_value)?;
            }
        }
        (None, None) => {}
        _ => {
            return Err(DomainError::InvalidData(format!(
                "V5 backup replay: {} GV snapshot varlığı canonical Rust replay ile eşleşmiyor.",
                id
            )))
        }
    }

    match (&imported.damgaDetay, &replayed.damgaDetay) {
        (Some(left), Some(right)) => {
            for (field, imported_value, replayed_value) in [
                (
                    "damgaDetay.brutDamgaVergisi",
                    left.brutDamgaVergisi,
                    right.brutDamgaVergisi,
                ),
                (
                    "damgaDetay.aylikDamgaIstisnaHakki",
                    left.aylikDamgaIstisnaHakki,
                    right.aylikDamgaIstisnaHakki,
                ),
                (
                    "damgaDetay.ayniAyOncekiKullanilanDamgaIstisnasi",
                    left.ayniAyOncekiKullanilanDamgaIstisnasi,
                    right.ayniAyOncekiKullanilanDamgaIstisnasi,
                ),
                (
                    "damgaDetay.uygulananDamgaIstisnasi",
                    left.uygulananDamgaIstisnasi,
                    right.uygulananDamgaIstisnasi,
                ),
                (
                    "damgaDetay.kalanDamgaIstisnasi",
                    left.kalanDamgaIstisnasi,
                    right.kalanDamgaIstisnasi,
                ),
                (
                    "damgaDetay.kesilenDamgaVergisi",
                    left.kesilenDamgaVergisi,
                    right.kesilenDamgaVergisi,
                ),
            ] {
                compare_backup_decimal(id, field, imported_value, replayed_value)?;
            }
        }
        (None, None) => {}
        _ => {
            return Err(DomainError::InvalidData(format!(
                "V5 backup replay: {} DV snapshot varlığı canonical Rust replay ile eşleşmiyor.",
                id
            )))
        }
    }
    Ok(())
}

#[cfg(test)]
mod backup_replay_comparison_tests {
    use super::{compare_backup_is_primi, compare_backup_optional_decimal_if_imported};
    use crate::domain::models::IsPrimiHesapDetayi;
    use rust_decimal::Decimal;

    #[test]
    fn synthetic_decimal_scale_is_semantically_equal_but_forged_amount_is_rejected() {
        let imported = IsPrimiHesapDetayi {
            grupId: "1. Grup".into(),
            grupAd: "1. Grup".into(),
            oran: "9.00".parse().unwrap(),
            hakGunu: 1,
            gunlukIsPrimi: "219.9".parse().unwrap(),
            tutar: "219.90".parse().unwrap(),
        };
        let replayed = IsPrimiHesapDetayi {
            gunlukIsPrimi: "219.90".parse().unwrap(),
            tutar: "219.900".parse().unwrap(),
            ..imported.clone()
        };

        compare_backup_is_primi("synthetic-payroll", Some(&imported), Some(&replayed))
            .expect("Decimal scale alone must not mark a snapshot corrupt");

        let forged = IsPrimiHesapDetayi {
            tutar: "220.90".parse().unwrap(),
            ..replayed
        };
        assert!(
            compare_backup_is_primi("synthetic-payroll", Some(&imported), Some(&forged))
                .expect_err("a true amount change must fail closed")
                .to_string()
                .contains("iş primi snapshot")
        );

        assert!(compare_backup_optional_decimal_if_imported(
            "synthetic-payroll",
            "pekDetay.aylikOncekiPekTuketimi",
            None,
            Some(Decimal::ZERO),
        )
        .is_ok());
        assert!(compare_backup_optional_decimal_if_imported(
            "synthetic-payroll",
            "pekDetay.aylikOncekiPekTuketimi",
            Some(Decimal::ZERO),
            Some(Decimal::ONE),
        )
        .is_err());
    }
}

fn validate_current_backup_replay(
    conn: &Connection,
    legacy_raise_months: &[i32],
    exact_source_periods: &BTreeSet<String>,
) -> Result<()> {
    let dataset = PayrollService::build_dataset_snapshot(conn)?;
    let legacy_input = payroll_core::payroll_engine::LegacyBackupPayrollInput {
        institutionSettings: SettingsRepository::get_all_institution_settings(conn)?,
        raiseMonths: legacy_raise_months.to_vec(),
    };
    payroll_core::validate_v5_retro_outstanding_receivables(&dataset.retroBatches)?;
    let targets = dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            ) && payroll.calculationMetadata.is_none()
        })
        .cloned()
        .collect::<Vec<_>>();

    for imported in targets {
        let Some(target_period) = dataset
            .periods
            .iter()
            .find(|period| period.id == imported.donemId)
        else {
            continue;
        };
        let source_history_complete = legacy_raise_months.is_empty()
            || dataset
                .periods
                .iter()
                .filter(|period| period.baslangicTarihi <= target_period.baslangicTarihi)
                .all(|period| exact_source_periods.contains(&period.id));
        if !exact_source_periods.contains(&imported.donemId) || !source_history_complete {
            continue;
        }
        let request = replay_payroll_request_for_current_backup(&dataset, &imported)?;
        let Some(replayed) =
            payroll_core::calculate_legacy_backup_payroll_checked(&request, &legacy_input)?
        else {
            continue;
        };
        compare_backup_snapshots(&imported, &replayed)?;
    }
    Ok(())
}

pub struct MigrationService;

impl MigrationService {
    pub fn is_migrated(conn: &Connection) -> Result<bool> {
        let flag = SettingsRepository::get_app_setting(conn, "legacy_migrated")?;
        Ok(flag.as_deref() == Some("true"))
    }

    fn parse_payload(payload_json: &str) -> Result<LegacyPayload> {
        let mut raw: serde_json::Value = serde_json::from_str(payload_json)
            .map_err(|e| DomainError::InvalidData(format!("Geçersiz yedek payload: {}", e)))?;
        let compensation_policy_data_present =
            raw.get("compensationPolicyVersions")
                .is_some_and(|value| match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::Array(versions) => !versions.is_empty(),
                    _ => true,
                });
        let compensation_metadata_data_present = raw
            .get("bordrolar")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|payrolls| {
                payrolls.iter().any(|payroll| {
                    payroll
                        .get("calculationMetadata")
                        .is_some_and(|metadata| !metadata.is_null())
                })
            });
        // Explicit compatibility mapping for pre-policy historical backups.
        // Do not replay recorded financial history with a new default policy.
        let mut historical_periods = BTreeSet::new();
        for (collection, field) in [
            ("bordrolar", "donemId"),
            ("retroAllocations", "sourcePeriodId"),
        ] {
            if let Some(items) = raw.get(collection).and_then(serde_json::Value::as_array) {
                for item in items {
                    if let Some(id) = item.get(field).and_then(serde_json::Value::as_str) {
                        historical_periods.insert(id.to_owned());
                    }
                }
            }
        }
        if let Some(settings_map) = raw
            .get_mut("kurumDegerleriMap")
            .and_then(serde_json::Value::as_object_mut)
        {
            for (id, settings) in settings_map {
                if historical_periods.contains(id) {
                    if let Some(settings) = settings.as_object_mut() {
                        settings
                            .entry("yemekTamGvIstisnaDamgaTabi")
                            .or_insert(serde_json::Value::Bool(false));
                    }
                }
            }
        }

        let backup_version = raw
            .get("backupVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        if raw.get("legacyInstitutionSettings").is_none()
            && raw
                .get("kurumDegerleriMap")
                .is_some_and(contains_legacy_compensation_settings)
        {
            raw["legacyInstitutionSettings"] = raw["kurumDegerleriMap"].clone();
        } else if backup_version < u64::from(COMPENSATION_SNAPSHOT_BACKUP_VERSION)
            && raw.get("legacyInstitutionSettings").is_none()
        {
            if let Some(settings) = raw.get("kurumDegerleriMap") {
                raw["legacyInstitutionSettings"] = settings.clone();
            }
        }
        if let Some(settings) = raw.get("kurumDegerleriMap").cloned() {
            if let Some(active_settings) = legacy_settings_to_active_map(&settings) {
                raw["kurumDegerleriMap"] = active_settings;
            }
        }
        let exact_source_periods = exact_legacy_compensation_source_periods(&raw);
        let mut payload: LegacyPayload = serde_json::from_value(raw)
            .map_err(|e| DomainError::InvalidData(format!("Geçersiz yedek payload: {}", e)))?;
        payload.exactLegacyCompensationSourcePeriods = exact_source_periods;

        let version = payload.backupVersion.unwrap_or(1);
        if version == 0 || version > CURRENT_BACKUP_VERSION {
            return Err(DomainError::InvalidData(format!(
                "Desteklenmeyen yedek sürümü: {} (desteklenen en yeni sürüm: {}).",
                version, CURRENT_BACKUP_VERSION
            )));
        }
        if version < COMPENSATION_SNAPSHOT_BACKUP_VERSION
            && (compensation_policy_data_present || compensation_metadata_data_present)
        {
            return Err(DomainError::InvalidData(
                "V1-V5 veya sürümsüz yedekler compensationPolicyVersions ya da calculationMetadata taşıyamaz; V6 gerekir."
                    .into(),
            ));
        }

        if let Some(version) = payload.backupVersion {
            if version >= 2
                && (payload.taxOpenings.is_none()
                    || payload.sickLeaveRecords.is_none()
                    || payload.annualPayrollParameters.is_none())
            {
                return Err(DomainError::InvalidData(
                    "V2+ yedek payload'ı vergi açılışları, rapor kayıtları ve yıllık bordro parametrelerini içermelidir."
                        .into(),
                ));
            }
            if version >= RETRO_AUTHORITY_BACKUP_VERSION
                && (payload.compensationRevisions.is_none()
                    || payload.compensationRevisionOverrides.is_none()
                    || payload.retroBatches.is_none()
                    || payload.retroAllocations.is_none())
            {
                return Err(DomainError::InvalidData(
                    "V5+ yedek payload'ı retro revision, override, batch ve allocation koleksiyonlarını içermelidir."
                        .into(),
                ));
            }
            if version >= RETRO_AUTHORITY_BACKUP_VERSION {
                // HEAD V5 required its complete retro graph, but did not
                // require these common top-level export fields. V6 is the
                // first strict full-snapshot contract.
                if version >= COMPENSATION_SNAPSHOT_BACKUP_VERSION
                    && (payload.exportedAt.is_none()
                        || payload.donemler.is_none()
                        || payload.aktifDonemId.is_none()
                        || payload.personeller.is_none()
                        || payload.kurumDegerleriMap.is_none()
                        || payload.puantajlar.is_none()
                        || payload.bordrolar.is_none())
                {
                    return Err(DomainError::InvalidData(
                        "V6 yedek payload'ı dönem, personel, ayar, puantaj ve bordro alanlarını içermelidir."
                            .into(),
                    ));
                }
                validate_v5_retro_lifecycle_fields(payload_json)?;
            }
            if version >= COMPENSATION_SNAPSHOT_BACKUP_VERSION {
                validate_v6_payload(&mut payload)?;
            }
        }
        if let Some(ref parameters) = payload.annualPayrollParameters {
            let mut seen_years = BTreeSet::new();
            for param in parameters {
                if !seen_years.insert(param.year) {
                    return Err(DomainError::ValidationError(format!(
                        "Yedek payload'ı yinelenen yıllık bordro parametresi yılı içeriyor: {}",
                        param.year
                    )));
                }
            }
        }
        Ok(payload)
    }

    fn import_payload(conn: &Connection, payload: LegacyPayload) -> Result<()> {
        let LegacyPayload {
            backupVersion,
            exportedAt: _,
            donemler,
            aktifDonemId,
            personeller,
            kurumDegerleriMap,
            legacyInstitutionSettings,
            puantajlar,
            mut bordrolar,
            taxOpenings,
            sickLeaveRecords,
            annualPayrollParameters,
            zamAylari: legacy_raise_months,
            compensationRevisions,
            compensationRevisionOverrides,
            retroBatches,
            retroAllocations,
            compensationPolicyVersions,
            isPrimiGruplari,
            exactLegacyCompensationSourcePeriods: exact_source_periods,
        } = payload;
        let compensation_policy_versions = compensationPolicyVersions.unwrap_or_default();
        restore_policy_timeline_in_transaction(conn, &compensation_policy_versions)?;
        let global_premium_groups = isPrimiGruplari
            .or_else(|| {
                compensation_policy_versions
                    .iter()
                    .max_by(|left, right| {
                        left.effectiveFrom.cmp(&right.effectiveFrom)
                            .then_with(|| left.createdAt.cmp(&right.createdAt))
                    })
                    .map(|version| {
                        version.state.isPrimiGruplari.iter().map(|group| IsPrimiGrupItem {
                            id: group.id.clone(),
                            ad: group.ad.clone(),
                            oran: group.oran,
                            aktif: group.aktif,
                        }).collect::<Vec<_>>()
                    })
            })
            .or_else(|| {
                let legacy = legacyInstitutionSettings.as_ref()?;
                let active_id = aktifDonemId.as_deref();
                let mut entries = legacy.iter().collect::<Vec<_>>();
                entries.sort_by(|(left, _), (right, _)| right.cmp(left));
                if let Some(active_id) = active_id {
                    entries.sort_by_key(|(period_id, _)| if period_id.as_str() == active_id { 0 } else { 1 });
                }
                entries.into_iter().find_map(|(_, settings)| {
                    serde_json::from_value::<Vec<IsPrimiGrupItem>>(
                        settings.get("isPrimiGruplari")?.clone(),
                    ).ok().filter(|groups| !groups.is_empty())
                })
            })
            .unwrap_or_default();
        let global_groups_json = serde_json::to_string(&global_premium_groups)
            .map_err(|error| DomainError::InvalidData(format!("İş primi grupları JSON'a çevrilemedi: {error}")))?;
        SettingsRepository::set_app_setting_in_transaction(
            conn,
            "is_primi_gruplari",
            &global_groups_json,
        )?;
        // V3 already introduced independent payment-event/accrual metadata.
        // Only V1/V2 backups may be normalized to the single legacy NORMAL
        // node; treating V3 as pre-accrual would erase retro/TEDIYE ordering.
        let is_pre_accrual_backup = backupVersion.unwrap_or(1) < MULTI_ACCRUAL_BACKUP_VERSION;
        let explicit_opening_keys: BTreeSet<(String, i32)> = taxOpenings
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|opening| (opening.personnelId.clone(), opening.year))
                    .collect()
            })
            .unwrap_or_default();
        let had_retro_batches = retroBatches.is_some();
        let mut retro_batches = retroBatches.unwrap_or_default();
        let mut retro_allocations = retroAllocations.unwrap_or_default();
        if backupVersion.unwrap_or(1) >= RETRO_AUTHORITY_BACKUP_VERSION {
            if let Some(payrolls) = bordrolar.as_ref() {
                for payroll in payrolls {
                    payroll_core::validate_current_payroll_snapshot_authority(payroll)?;
                }
            }
        }
        if backupVersion.unwrap_or(1) < RETRO_AUTHORITY_BACKUP_VERSION {
            normalize_legacy_retro_settlement(&mut retro_batches, &mut retro_allocations);
        }

        let revision_ids: BTreeSet<String> = compensationRevisions
            .as_ref()
            .map(|items| items.iter().map(|item| item.id.clone()).collect())
            .unwrap_or_default();
        if let Some(overrides) = compensationRevisionOverrides.as_ref() {
            if let Some(orphan) = overrides
                .iter()
                .find(|item| !revision_ids.contains(&item.revisionId))
            {
                return Err(DomainError::InvalidData(format!(
                    "Retro revision override {} için revision bulunamadı: {}.",
                    orphan.id, orphan.revisionId
                )));
            }
        }
        let batch_ids: BTreeSet<String> =
            retro_batches.iter().map(|item| item.id.clone()).collect();
        if !retro_allocations.is_empty() {
            if let Some(orphan) = retro_allocations
                .iter()
                .find(|item| !batch_ids.contains(&item.batchId))
            {
                return Err(DomainError::InvalidData(format!(
                    "Retro allocation {} için batch bulunamadı: {}.",
                    orphan.id, orphan.batchId
                )));
            }
        }

        if let Some(periods) = donemler {
            for period in periods {
                PeriodRepository::save_in_transaction(conn, &period)?;
            }
        }

        if let Some(inst_map) = kurumDegerleriMap {
            for (period_id, mut settings) in inst_map {
                settings.donemId = period_id.clone();
                SettingsRepository::save_active_institution_settings_in_transaction(
                    conn, &settings,
                )?;
            }
        }
        if let Some(legacy_map) = legacyInstitutionSettings {
            for (period_id, settings) in legacy_map {
                SettingsRepository::merge_legacy_compensation_settings_in_transaction(
                    conn, &period_id, &settings,
                )?;
            }
        }

        if let Some(personnel_list) = personeller {
            for legacy_personel in personnel_list {
                let personel = Personel {
                    id: legacy_personel.id.clone(),
                    tcNo: legacy_personel.tcNo,
                    ad: legacy_personel.ad,
                    soyad: legacy_personel.soyad,
                    grup: legacy_personel.grup,
                    unvan: legacy_personel.unvan,
                    sgkSicilNo: legacy_personel.sgkSicilNo.unwrap_or_default(),
                    iban: legacy_personel.iban.unwrap_or_default(),
                    hizmetYili: legacy_personel.hizmetYili.unwrap_or(1),
                    aciklama: legacy_personel.aciklama,
                    devirKumulatifGvMatrahi: legacy_personel.devirKumulatifGvMatrahi,
                    devirKumulatifGvMatrahiYili: legacy_personel.devirKumulatifGvMatrahiYili,
                    devirKumulatifGvMatrahiBaslangicAyi: legacy_personel
                        .devirKumulatifGvMatrahiBaslangicAyi,
                    devirKumulatifAsgariGvMatrahi: legacy_personel.devirKumulatifAsgariGvMatrahi,
                    devirKumulatifAsgariGvMatrahiYili: legacy_personel
                        .devirKumulatifAsgariGvMatrahiYili,
                    kesintiler: legacy_personel.kesintiler,
                };
                PersonnelRepository::save_in_transaction(conn, &personel)?;

                // Eski localStorage sözleşmesindeki GV/asgari GV devir
                // alanlarını yeni, ayrı opening modeline yalnızca period ID
                // güvenilir biçimde çözülebiliyorsa backfill et. Mevcut
                // explicit opening ve aynı anahtardaki DB kaydı authoritative
                // kalır; legacy alanlar bunları ezemez.
                let normal_value = personel.devirKumulatifGvMatrahi.unwrap_or_default();
                let asgari_value = personel.devirKumulatifAsgariGvMatrahi.unwrap_or_default();
                let normal_year = personel.devirKumulatifGvMatrahiYili;
                let asgari_year = personel.devirKumulatifAsgariGvMatrahiYili;
                let opening_year = normal_year.or(asgari_year);
                let years_match = normal_year
                    .zip(asgari_year)
                    .is_none_or(|(normal_year, asgari_year)| normal_year == asgari_year);
                if (normal_value > rust_decimal_macros::dec!(0)
                    || asgari_value > rust_decimal_macros::dec!(0))
                    && normal_value >= rust_decimal_macros::dec!(0)
                    && asgari_value >= rust_decimal_macros::dec!(0)
                    && years_match
                {
                    if let Some(year) = opening_year {
                        let has_explicit_opening =
                            explicit_opening_keys.contains(&(personel.id.clone(), year));
                        let has_existing_opening = TaxOpeningRepository::get_by_personnel_and_year(
                            conn,
                            &personel.id,
                            year,
                        )?
                        .is_some();
                        if !has_explicit_opening && !has_existing_opening {
                            if let Some(effective_period_id) = resolve_legacy_opening_period_id(
                                conn,
                                year,
                                personel.devirKumulatifGvMatrahiBaslangicAyi,
                            )? {
                                let tax_opening = PersonelTaxOpening {
                                    id: format!("{}_{}", personel.id, year),
                                    personnelId: personel.id.clone(),
                                    year,
                                    gvCumulativeOpening: (normal_value
                                        > rust_decimal_macros::dec!(0))
                                    .then_some(normal_value),
                                    effectiveFromPeriodId: (normal_value
                                        > rust_decimal_macros::dec!(0))
                                    .then_some(effective_period_id.clone()),
                                    asgariGvCumulativeOpening: (asgari_value
                                        > rust_decimal_macros::dec!(0))
                                    .then_some(asgari_value),
                                    asgariGvEffectiveFromPeriodId: (asgari_value
                                        > rust_decimal_macros::dec!(0))
                                    .then_some(effective_period_id),
                                    createdAt: None,
                                    updatedAt: None,
                                };
                                // A compatibility backfill must never turn an
                                // existing FINALIZED chain into a migration
                                // failure or attempt to mutate it. Leave the
                                // legacy fields intact; the runtime resolver
                                // will require an explicit period if the
                                // legacy value remains ambiguous.
                                let impact = PayrollInvalidationRepository::evaluate_mutation(
                                    conn,
                                    &payroll_core::PayrollMutation::PersonTaxYear {
                                        personnelId: personel.id.clone(),
                                        taxYear: year,
                                    },
                                )?;
                                if impact.blockedByFinalized.is_empty()
                                    && impact.blockedByFinalizedRetroBatches.is_empty()
                                {
                                    TaxOpeningRepository::save_in_transaction(conn, &tax_opening)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(openings) = taxOpenings {
            for mut opening in openings {
                // Restore keeps legacy period/year pairings intact. Runtime
                // writes use the stricter canonical repository path.
                // Before the openings were independent, an asgari value with
                // no own period shared the normal effective period. Preserve
                // that known legacy meaning only when the normal period is
                // present; unresolved pairs remain fail-closed.
                if opening.asgariGvCumulativeOpening.is_some()
                    && opening.asgariGvEffectiveFromPeriodId.is_none()
                {
                    opening.asgariGvEffectiveFromPeriodId = opening.effectiveFromPeriodId.clone();
                }
                TaxOpeningRepository::save_legacy_in_transaction(conn, &opening)?;
            }
        }

        if let Some(attendance_list) = puantajlar {
            for attendance in attendance_list {
                AttendanceRepository::save_in_transaction(conn, &attendance)?;
            }
        }

        if let Some(sick_records) = sickLeaveRecords {
            for record in sick_records {
                if backupVersion.unwrap_or(1) >= RETRO_AUTHORITY_BACKUP_VERSION {
                    SickLeaveRepository::save_for_restore_in_transaction(conn, &record)?;
                } else {
                    SickLeaveRepository::save_in_transaction(conn, &record)?;
                }
            }
        }

        // V1 yedeklerinde yıllık tarife alanı bulunmayabilir; V2 sözleşmesiyle
        // aynı authoritative üretim yolunu korumak için içe aktarılan
        // dönemlerin vergi yıllarına JSON/SQLite-safe varsayılan tarife ekle.
        // Payload'da açıkça verilen yıllık parametreler her zaman önceliklidir.
        if let Some(parameters) = annualPayrollParameters {
            let mut seen_years = BTreeSet::new();
            for parameter in &parameters {
                if !seen_years.insert(parameter.year) {
                    return Err(DomainError::ValidationError(format!(
                        "Yıllık bordro parametrelerinde mükerrer yıl kaydı bulundu: {}",
                        parameter.year
                    )));
                }
            }
            for parameter in parameters {
                if backupVersion.unwrap_or(1) >= RETRO_AUTHORITY_BACKUP_VERSION {
                    AnnualPayrollParametersRepository::save_for_restore_in_transaction(
                        conn, &parameter,
                    )?;
                } else {
                    AnnualPayrollParametersRepository::save_in_transaction(conn, &parameter)?;
                }
            }
        }

        // Fill missing supported-year parameters before importing the retro
        // graph. Unsupported years remain explicitly unconfigured. The normal
        // parameter repository correctly blocks a tax-year mutation
        // when a FINALIZED retro batch already exists; during restore that
        // batch is part of this same transaction, so ordering the compatibility
        // default after it would make a clean V3/V4 restore fail against its own
        // incoming data.
        let imported_tax_years: BTreeSet<i32> = PeriodRepository::get_all(conn)?
            .into_iter()
            .map(|period| period.taxYear)
            .collect();
        for year in imported_tax_years {
            if year == 2026 && AnnualPayrollParametersRepository::get_by_year(conn, year)?.is_none()
            {
                AnnualPayrollParametersRepository::save_in_transaction(
                    conn,
                    &AnnualPayrollParameters::default_for_2026(),
                )?;
            }
        }

        if let Some(revisions) = compensationRevisions {
            for revision in revisions {
                let overrides = compensationRevisionOverrides
                    .as_ref()
                    .map(|items| {
                        items
                            .iter()
                            .filter(|item| item.revisionId == revision.id)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                crate::repositories::retro_repo::restore_revision_with_overrides_in_transaction(
                    conn, &revision, &overrides,
                )?;
            }
        }
        if had_retro_batches {
            let allocations = retro_allocations.clone();
            for mut batch in retro_batches {
                let batch_allocations = allocations
                    .iter()
                    .filter(|allocation| allocation.batchId == batch.id)
                    .cloned()
                    .collect::<Vec<_>>();
                // V3/V4 did not persist settlementStatus, and some V3/V4 payloads
                // contain the retro graph without the corresponding payment
                // event.  Do not upgrade such a dangling FINALIZED batch to
                // PAID: retain the graph for audit, but make it STALE so it
                // cannot be counted as an authoritative entitlement on the
                // next calculation.  Only a matching FINALIZED payment event
                // can establish settlement during the compatibility import.
                if backupVersion.unwrap_or(1) < RETRO_AUTHORITY_BACKUP_VERSION
                    && batch.status == CompensationRevisionStatus::FINALIZED
                {
                    let has_matching_finalized_payment =
                        bordrolar.as_ref().is_some_and(|payrolls| {
                            payrolls.iter().any(|payroll| {
                                payroll.accrualId == batch.id
                                    && payroll.accrualType == AccrualType::RETRO_ADJUSTMENT
                                    && payroll.personelId == batch.personnelId
                                    && payroll.paymentDate == batch.paymentDate
                                    && payroll.status == BordroStatus::FINALIZED
                            })
                        });
                    if !has_matching_finalized_payment {
                        let linked_finalized_mismatch =
                            bordrolar.as_ref().is_some_and(|payrolls| {
                                payrolls.iter().any(|payroll| {
                                    payroll.accrualId == batch.id
                                        && payroll.accrualType == AccrualType::RETRO_ADJUSTMENT
                                        && payroll.status == BordroStatus::FINALIZED
                                })
                            });
                        if linked_finalized_mismatch {
                            return Err(DomainError::InvalidData(format!(
                                "V3/V4 FINALIZED retro batch {} için bağlı FINALIZED payment event personel veya ödeme tarihiyle eşleşmiyor.",
                                batch.id
                            )));
                        }
                        if let Some(payrolls) = bordrolar.as_mut() {
                            for payroll in payrolls.iter_mut().filter(|payroll| {
                                payroll.accrualId == batch.id
                                    && payroll.accrualType == AccrualType::RETRO_ADJUSTMENT
                            }) {
                                // A non-final legacy event cannot remain in the
                                // authoritative tax chain after its FINALIZED
                                // batch is downgraded to an unsettled graph.
                                payroll.status = BordroStatus::STALE;
                            }
                        }
                        batch.status = CompensationRevisionStatus::STALE;
                        batch.settlementStatus =
                            if batch.totalGrossDelta < rust_decimal::Decimal::ZERO {
                                RetroSettlementStatus::OVERPAYMENT
                            } else {
                                RetroSettlementStatus::UNSETTLED
                            };
                    } else if batch.settlementStatus == RetroSettlementStatus::UNSETTLED {
                        batch.settlementStatus =
                            if batch.totalGrossDelta < rust_decimal::Decimal::ZERO {
                                RetroSettlementStatus::OVERPAYMENT
                            } else {
                                RetroSettlementStatus::PAID
                            };
                    }
                }
                crate::repositories::retro_repo::restore_batch_in_transaction(
                    conn,
                    &batch,
                    &batch_allocations,
                )?;
            }
        }

        // Source data is loaded before payroll snapshots so a backup containing
        // FINALIZED records can be restored without treating the restore itself
        // as a new business mutation. `save_in_transaction` is the explicit
        // bulk-persistence path; normal calculation/finalization still goes
        // through the core policy and transaction adapters.
        if let Some(payroll_list) = bordrolar {
            let mut payroll_list = payroll_list;
            // Schema-2 NORMAL replay can reference earlier authoritative
            // payment events in the same tax month. Restore them first so
            // each incremental repository write can prove that provenance.
            payroll_list.sort_by(|left, right| {
                left.paymentDate
                    .cmp(&right.paymentDate)
                    .then_with(|| left.sequence.cmp(&right.sequence))
                    .then_with(|| left.accrualId.cmp(&right.accrualId))
            });
            for mut payroll in payroll_list {
                // V1/V2 snapshots had one person+period row and no accrual
                // metadata. Preserve the old id and make it the sole NORMAL
                // node without rewriting any financial snapshot.
                if payroll.accrualId.trim().is_empty() {
                    payroll.accrualId = payroll.id.clone();
                }
                if is_pre_accrual_backup {
                    payroll.accrualType = AccrualType::NORMAL;
                    payroll.sequence = 0;
                }
                if payroll.paymentDate.trim().is_empty() {
                    let period =
                        PeriodRepository::get_by_id(conn, &payroll.donemId)?.ok_or_else(|| {
                            DomainError::InvalidData(format!(
                                "{} bordrosunun dönemi bulunamadı.",
                                payroll.id
                            ))
                        })?;
                    payroll.paymentDate =
                        payroll_core::payroll_engine::default_payment_date(&period);
                }
                if payroll.accrualDescription.is_none() {
                    payroll.accrualDescription = payroll.notlar.clone();
                }
                if backupVersion.unwrap_or(1) < RETRO_AUTHORITY_BACKUP_VERSION {
                    PayrollRepository::save_legacy_in_transaction(conn, &payroll)?;
                } else {
                    PayrollRepository::save_for_restore_in_transaction(conn, &payroll)?;
                }
            }
        }

        // V5 is the first backup contract that declares the complete retro
        // graph. V6 carries that same authority plus compensation provenance.
        match backupVersion.unwrap_or(1) {
            4 => validate_retro_payment_links(conn, "V4")?,
            version if version >= RETRO_AUTHORITY_BACKUP_VERSION => {
                validate_retro_payment_links(conn, "V5")?;
                // Rows without policy-engine provenance keep their own
                // historical authority. Exact legacy sources are replayed;
                // unsupported rows remain readable and are not reinterpreted.
                validate_current_backup_replay(
                    conn,
                    legacy_raise_months.as_deref().unwrap_or(&[]),
                    &exact_source_periods,
                )?;
            }
            _ => {}
        }

        if let Some(active_id) = aktifDonemId {
            SettingsRepository::set_app_setting_in_transaction(
                conn,
                "active_period_id",
                &active_id,
            )?;
        }

        Ok(())
    }

    pub fn migrate_legacy_data(conn: &mut Connection, payload_json: &str) -> Result<()> {
        if Self::is_migrated(conn)? {
            return Ok(());
        }

        let payload = Self::parse_payload(payload_json)?;
        // Legacy import can contain source data and payroll snapshots. Treat
        // it as a full dataset mutation before opening the transaction so an
        // existing FINALIZED ledger is never silently replaced.
        PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::All,
        )?;
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Self::import_payload(&tx, payload)?;
        SettingsRepository::set_app_setting_in_transaction(&tx, "legacy_migrated", "true")?;
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Kullanıcı tarafından çağrılan import/reset akışı: mevcut domain verisini
    /// aynı transaction içinde siler ve yedek içeriğini eksiksiz yükler.
    pub fn replace_backup_data(conn: &mut Connection, payload_json: &str) -> Result<()> {
        let payload = Self::parse_payload(payload_json)?;
        // Keep the native bulk-replace path aligned with the browser's
        // core-owned `ALL` mutation policy. A confirmed UI reset/import still
        // cannot delete or replace an existing FINALIZED ledger.
        PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::All,
        )?;
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        validate_policy_restore_compatibility(
            &tx,
            payload
                .compensationPolicyVersions
                .as_deref()
                .unwrap_or_default(),
        )?;

        tx.execute_batch(
            "DELETE FROM retro_adjustment_allocations;
             DELETE FROM retro_adjustment_batches;
             DELETE FROM compensation_revision_overrides;
             DELETE FROM compensation_revisions;
             DELETE FROM payroll_income_items;
             DELETE FROM payroll_deduction_items;
             DELETE FROM payroll_records;
             DELETE FROM attendance_records;
             DELETE FROM sick_leave_records;
             DELETE FROM personnel_tax_opening;
             DELETE FROM institution_settings;
             DELETE FROM personnel;
             DELETE FROM payroll_periods;
             DELETE FROM annual_payroll_parameters;
             DELETE FROM app_settings;",
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Self::import_payload(&tx, payload)?;
        SettingsRepository::set_app_setting_in_transaction(&tx, "legacy_migrated", "true")?;
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
