#![allow(non_snake_case)]

use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::{SettingsRepository, ZAM_AYLARI_SETTING_KEY};
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};

pub const CURRENT_BACKUP_VERSION: u32 = 5;

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
    pub kurumDegerleriMap: Option<HashMap<String, DonemselKurumDegerleri>>,
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
            if expected_status != Some(payroll.status)
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

pub struct MigrationService;

impl MigrationService {
    pub fn is_migrated(conn: &Connection) -> Result<bool> {
        let flag = SettingsRepository::get_app_setting(conn, "legacy_migrated")?;
        Ok(flag.as_deref() == Some("true"))
    }

    fn parse_payload(payload_json: &str) -> Result<LegacyPayload> {
        let payload: LegacyPayload = serde_json::from_str(payload_json)
            .map_err(|e| DomainError::InvalidData(format!("Geçersiz yedek payload: {}", e)))?;

        if let Some(version) = payload.backupVersion {
            if version == 0 || version > CURRENT_BACKUP_VERSION {
                return Err(DomainError::InvalidData(format!(
                    "Desteklenmeyen yedek sürümü: {} (desteklenen en yeni sürüm: {}).",
                    version, CURRENT_BACKUP_VERSION
                )));
            }

            if version >= 2
                && (payload.taxOpenings.is_none()
                    || payload.sickLeaveRecords.is_none()
                    || payload.annualPayrollParameters.is_none())
            {
                return Err(DomainError::InvalidData(
                    "V2-V5 yedek payload'ı vergi açılışları, rapor kayıtları ve yıllık bordro parametrelerini içermelidir."
                        .into(),
                ));
            }
            if version >= CURRENT_BACKUP_VERSION
                && (payload.compensationRevisions.is_none()
                    || payload.compensationRevisionOverrides.is_none()
                    || payload.retroBatches.is_none()
                    || payload.retroAllocations.is_none())
            {
                return Err(DomainError::InvalidData(
                    "V5 yedek payload'ı retro revision, override, batch ve allocation koleksiyonlarını içermelidir."
                        .into(),
                ));
            }
            if version >= CURRENT_BACKUP_VERSION {
                validate_v5_retro_lifecycle_fields(payload_json)?;
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
            puantajlar,
            mut bordrolar,
            taxOpenings,
            sickLeaveRecords,
            annualPayrollParameters,
            zamAylari,
            compensationRevisions,
            compensationRevisionOverrides,
            retroBatches,
            retroAllocations,
        } = payload;
        // V3 already introduced independent payment-event/accrual metadata.
        // Only V1/V2 backups may be normalized to the single legacy NORMAL
        // node; treating V3 as pre-accrual would erase retro/TEDIYE ordering.
        let is_pre_accrual_backup = backupVersion.unwrap_or(1) < 3;
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
        if backupVersion.unwrap_or(1) < CURRENT_BACKUP_VERSION {
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
                settings.donemId = period_id;
                SettingsRepository::save_institution_settings_in_transaction(conn, &settings)?;
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
                SickLeaveRepository::save_in_transaction(conn, &record)?;
            }
        }

        // V1 yedeklerinde yıllık tarife alanı bulunmayabilir; V2 sözleşmesiyle
        // aynı authoritative üretim yolunu korumak için içe aktarılan
        // dönemlerin vergi yıllarına JSON/SQLite-safe varsayılan tarife ekle.
        // Payload'da açıkça verilen yıllık parametreler her zaman önceliklidir.
        if let Some(parameters) = annualPayrollParameters {
            for parameter in parameters {
                AnnualPayrollParametersRepository::save_in_transaction(conn, &parameter)?;
            }
        }

        if let Some(months) = zamAylari {
            let normalized_months = SettingsRepository::normalize_zam_aylari(&months)?;
            let value = serde_json::to_string(&normalized_months)
                .map_err(|e| DomainError::InvalidData(e.to_string()))?;
            SettingsRepository::set_app_setting_in_transaction(
                conn,
                ZAM_AYLARI_SETTING_KEY,
                &value,
            )?;
        }

        // Fill missing yearly parameters before importing the retro graph. The
        // normal parameter repository correctly blocks a tax-year mutation
        // when a FINALIZED retro batch already exists; during restore that
        // batch is part of this same transaction, so ordering the compatibility
        // default after it would make a clean V3/V4 restore fail against its own
        // incoming data.
        let imported_tax_years: BTreeSet<i32> = PeriodRepository::get_all(conn)?
            .into_iter()
            .map(|period| period.taxYear)
            .collect();
        for year in imported_tax_years {
            if AnnualPayrollParametersRepository::get_by_year(conn, year)?.is_none() {
                let mut defaults = AnnualPayrollParameters::default_for_2026();
                defaults.year = year;
                AnnualPayrollParametersRepository::save_in_transaction(conn, &defaults)?;
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
                if backupVersion.unwrap_or(1) < CURRENT_BACKUP_VERSION
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
                PayrollRepository::save_in_transaction(conn, &payroll)?;
            }
        }

        // V5 is the first backup contract that declares the complete retro
        // graph. Its FINALIZED settlement must therefore have an actual,
        // matching RETRO_ADJUSTMENT payment event; otherwise a restore would
        // claim a paid batch while dropping the bank/tax event linkage.
        match backupVersion.unwrap_or(1) {
            4 => validate_retro_payment_links(conn, "V4")?,
            version if version >= CURRENT_BACKUP_VERSION => {
                validate_retro_payment_links(conn, "V5")?
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
