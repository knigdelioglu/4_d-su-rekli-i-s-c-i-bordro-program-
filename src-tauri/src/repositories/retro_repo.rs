use super::{dec_to_kurus, kurus_to_dec};
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use payroll_core::PayrollMutation;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;

fn round2(value: rust_decimal::Decimal) -> rust_decimal::Decimal {
    value.round_dp(2)
}

fn validate_settlement_status(
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
) -> Result<()> {
    let has_negative_delta = batch.totalGrossDelta < rust_decimal::Decimal::ZERO
        || allocations
            .iter()
            .any(|allocation| allocation.deltaAmount < rust_decimal::Decimal::ZERO);
    if has_negative_delta && batch.status == CompensationRevisionStatus::FINALIZED {
        return Err(DomainError::ValidationError(
            "Negatif retro fark FINALIZED ödeme batch'i olamaz; OVERPAYMENT olarak açık settlement kaydı tutulmalıdır."
                .into(),
        ));
    }
    let expected = if has_negative_delta {
        RetroSettlementStatus::OVERPAYMENT
    } else if batch.status == CompensationRevisionStatus::FINALIZED {
        RetroSettlementStatus::PAID
    } else {
        RetroSettlementStatus::UNSETTLED
    };
    if batch.settlementStatus != expected {
        return Err(DomainError::InvalidData(format!(
            "Retro batch settlement statusı tutarsız: {:?} durumu için {:?} bekleniyordu, {:?} geldi.",
            batch.status, expected, batch.settlementStatus
        )));
    }
    Ok(())
}

fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|error| DomainError::InvalidData(error.to_string()))
}

fn decode<T: DeserializeOwned>(value: &str, field: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|error| {
        DomainError::InvalidData(format!("Retro {field} snapshot'ı bozuk: {error}"))
    })
}

fn enum_json<T: Serialize>(value: &T) -> Result<String> {
    encode(value).map(|value| value.trim_matches('"').to_string())
}

fn parse_enum<T: DeserializeOwned>(value: &str, field: &str) -> Result<T> {
    decode(&format!("\"{}\"", value), field)
}

pub fn get_revisions(conn: &Connection) -> Result<Vec<CompensationRevision>> {
    let mut statement = conn
        .prepare(
            "SELECT id, reason, title, effective_from, effective_to, decision_date,
                    signed_at, description, status, scope, personnel_ids_json,
                    personnel_group, created_at, updated_at
             FROM compensation_revisions
             ORDER BY effective_from, id",
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
            ))
        })
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let mut result = Vec::new();
    for row in rows {
        let (
            id,
            reason,
            title,
            effective_from,
            effective_to,
            decision_date,
            signed_at,
            description,
            status,
            scope,
            personnel_ids_json,
            personnel_group,
            created_at,
            updated_at,
        ) = row.map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        result.push(CompensationRevision {
            id,
            reason: parse_enum(&reason, "revision nedeni")?,
            title,
            effectiveFrom: effective_from,
            effectiveTo: effective_to,
            decisionDate: decision_date,
            signedAt: signed_at,
            description,
            status: parse_enum(&status, "revision durumu")?,
            scope: parse_enum(&scope, "revision kapsamı")?,
            personnelIds: decode(&personnel_ids_json, "personel kapsamı")?,
            personnelGroup: personnel_group,
            createdAt: created_at,
            updatedAt: updated_at,
        });
    }
    Ok(result)
}

pub fn get_overrides(conn: &Connection) -> Result<Vec<CompensationRevisionOverride>> {
    let mut statement = conn
        .prepare(
            "SELECT id, revision_id, parameter, value, personnel_id
             FROM compensation_revision_overrides
             ORDER BY revision_id, parameter, personnel_id, id",
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let mut result = Vec::new();
    for row in rows {
        let (id, revision_id, parameter, value, personnel_id) =
            row.map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        result.push(CompensationRevisionOverride {
            id,
            revisionId: revision_id,
            parameter: parse_enum(&parameter, "revision parametresi")?,
            value: kurus_to_dec(value),
            personnelId: personnel_id,
        });
    }
    Ok(result)
}

pub fn get_batches(conn: &Connection) -> Result<Vec<RetroAdjustmentBatch>> {
    let mut statement = conn
        .prepare(
            "SELECT id, revision_id, personnel_id, payment_date, status,
                    settlement_status, total_gross_delta, description, created_at,
                    calculated_at, finalized_at
             FROM retro_adjustment_batches
             ORDER BY payment_date, id",
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let mut result = Vec::new();
    for row in rows {
        let (
            id,
            revision_id,
            personnel_id,
            payment_date,
            status,
            settlement_status,
            total_gross_delta,
            description,
            created_at,
            calculated_at,
            finalized_at,
        ) = row.map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        result.push(RetroAdjustmentBatch {
            id,
            revisionId: revision_id,
            personnelId: personnel_id,
            paymentDate: payment_date,
            status: parse_enum(&status, "retro batch durumu")?,
            settlementStatus: parse_enum(&settlement_status, "retro settlement durumu")?,
            totalGrossDelta: kurus_to_dec(total_gross_delta),
            description,
            createdAt: created_at,
            calculatedAt: calculated_at,
            finalizedAt: finalized_at,
        });
    }
    Ok(result)
}

pub fn get_allocations(conn: &Connection) -> Result<Vec<RetroAllocation>> {
    let mut statement = conn
        .prepare(
            "SELECT id, batch_id, personnel_id, source_period_id, earning_code,
                    original_recognized_amount, previous_retro_amount, target_amount,
                    delta_amount, sgk_treatment, income_tax_treatment, stamp_tax_treatment,
                    original_pek, retro_pek_delta, adjusted_pek, worker_sgk_delta,
                    worker_unemployment_delta, employer_sgk_delta,
                    employer_unemployment_delta, metadata
             FROM retro_adjustment_allocations
             ORDER BY source_period_id, earning_code, id",
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)?,
                row.get::<_, i64>(16)?,
                row.get::<_, i64>(17)?,
                row.get::<_, i64>(18)?,
                row.get::<_, Option<String>>(19)?,
            ))
        })
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let mut result = Vec::new();
    for row in rows {
        let (
            id,
            batch_id,
            personnel_id,
            source_period_id,
            earning_code,
            original_recognized_amount,
            previous_retro_amount,
            target_amount,
            delta_amount,
            sgk_treatment,
            income_tax_treatment,
            stamp_tax_treatment,
            original_pek,
            retro_pek_delta,
            adjusted_pek,
            worker_sgk_delta,
            worker_unemployment_delta,
            employer_sgk_delta,
            employer_unemployment_delta,
            metadata,
        ) = row.map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        result.push(RetroAllocation {
            id,
            batchId: batch_id,
            personnelId: personnel_id,
            sourcePeriodId: source_period_id,
            earningCode: parse_enum(&earning_code, "earning code")?,
            originalRecognizedAmount: kurus_to_dec(original_recognized_amount),
            previousAuthoritativeRetroAmount: kurus_to_dec(previous_retro_amount),
            targetAmount: kurus_to_dec(target_amount),
            deltaAmount: kurus_to_dec(delta_amount),
            sgkTreatment: parse_enum(&sgk_treatment, "SGK treatment")?,
            incomeTaxTreatment: parse_enum(&income_tax_treatment, "GV treatment")?,
            stampTaxTreatment: parse_enum(&stamp_tax_treatment, "DV treatment")?,
            originalPek: kurus_to_dec(original_pek),
            retroPekDelta: kurus_to_dec(retro_pek_delta),
            adjustedPek: kurus_to_dec(adjusted_pek),
            workerSgkDelta: kurus_to_dec(worker_sgk_delta),
            workerUnemploymentDelta: kurus_to_dec(worker_unemployment_delta),
            employerSgkDelta: kurus_to_dec(employer_sgk_delta),
            employerUnemploymentDelta: kurus_to_dec(employer_unemployment_delta),
            metadata,
        });
    }
    Ok(result)
}

fn existing_revision_status(
    conn: &Connection,
    id: &str,
) -> Result<Option<CompensationRevisionStatus>> {
    conn.query_row(
        "SELECT status FROM compensation_revisions WHERE id = ?1",
        params![id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| DomainError::DatabaseError(error.to_string()))?
    .map(|value| parse_enum(&value, "revision durumu"))
    .transpose()
}

fn same_revision_definition(
    left: &CompensationRevision,
    right: &CompensationRevision,
) -> bool {
    left.id == right.id
        && left.reason == right.reason
        && left.title == right.title
        && left.effectiveFrom == right.effectiveFrom
        && left.effectiveTo == right.effectiveTo
        && left.decisionDate == right.decisionDate
        && left.signedAt == right.signedAt
        && left.description == right.description
        && left.scope == right.scope
        && left.personnelIds == right.personnelIds
        && left.personnelGroup == right.personnelGroup
}

fn same_revision_overrides(
    left: &[CompensationRevisionOverride],
    right: &[CompensationRevisionOverride],
) -> bool {
    left.len() == right.len()
        && left.iter().all(|candidate| {
            right.iter().any(|other| {
                candidate.id == other.id
                    && candidate.revisionId == other.revisionId
                    && candidate.parameter == other.parameter
                    && candidate.value == other.value
                    && candidate.personnelId == other.personnelId
            })
        })
}

pub fn save_revision_with_overrides(
    conn: &Connection,
    revision: &CompensationRevision,
    overrides: &[CompensationRevisionOverride],
) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    save_revision_with_overrides_in_transaction_impl(&tx, revision, overrides, false)?;
    tx.commit()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))
}

/// Persists a revision while an outer caller-owned transaction is already open.
/// This is used by backup restore so a payload containing retro data remains one
/// atomic import instead of attempting a nested SQLite transaction.
pub fn save_revision_with_overrides_in_transaction(
    conn: &Connection,
    revision: &CompensationRevision,
    overrides: &[CompensationRevisionOverride],
) -> Result<()> {
    save_revision_with_overrides_in_transaction_impl(conn, revision, overrides, false)
}

/// Restores a revision from a trusted backup while an outer caller-owned
/// transaction is already open. Restore is the only path allowed to write a
/// FINALIZED revision; normal application mutations remain fail-closed.
pub fn restore_revision_with_overrides_in_transaction(
    conn: &Connection,
    revision: &CompensationRevision,
    overrides: &[CompensationRevisionOverride],
) -> Result<()> {
    save_revision_with_overrides_in_transaction_impl(conn, revision, overrides, true)
}

fn save_revision_with_overrides_in_transaction_impl(
    conn: &Connection,
    revision: &CompensationRevision,
    overrides: &[CompensationRevisionOverride],
    allow_finalized_restore: bool,
) -> Result<()> {
    if revision.id.trim().is_empty() || revision.title.trim().is_empty() {
        return Err(DomainError::ValidationError(
            "Revision kimliği ve başlığı boş olamaz.".into(),
        ));
    }
    if !allow_finalized_restore && revision.status != CompensationRevisionStatus::DRAFT {
        return Err(DomainError::ValidationError(
            "Yeni veya değiştirilen revision yalnız DRAFT durumda kaydedilebilir; CALCULATED/FINALIZED state'i domain service üretmelidir."
                .into(),
        ));
    }
    let mut override_keys = HashSet::new();
    for item in overrides {
        if item.revisionId != revision.id {
            return Err(DomainError::ValidationError(
                "Revision override başka bir revision kimliğine ait olamaz.".into(),
            ));
        }
        let key = format!(
            "{:?}\u{0}{}",
            item.parameter,
            item.personnelId.as_deref().unwrap_or("")
        );
        if !override_keys.insert(key) {
            return Err(DomainError::ValidationError(
                "Aynı revision parametresi ve personel kapsamı birden fazla kez tanımlanamaz."
                    .into(),
            ));
        }
    }
    let status = existing_revision_status(conn, &revision.id)?;
    if !allow_finalized_restore && status.is_some() {
        let existing_revision = get_revisions(conn)?
            .into_iter()
            .find(|existing| existing.id == revision.id);
        let existing_overrides = get_overrides(conn)?
            .into_iter()
            .filter(|item| item.revisionId == revision.id)
            .collect::<Vec<_>>();
        if existing_revision
            .as_ref()
            .is_some_and(|existing| same_revision_definition(existing, revision))
            && same_revision_overrides(&existing_overrides, overrides)
        {
            // Re-opening the same immutable definition is a no-op. This is
            // required when a second correction uses the same TİS revision
            // after its first payment was finalized.
            return Ok(());
        }
    }
    if !allow_finalized_restore && status == Some(CompensationRevisionStatus::FINALIZED) {
        return Err(DomainError::PayrollFinalized(
            "FINALIZED revision değiştirilemez.".into(),
        ));
    }
    if !allow_finalized_restore {
        let finalized_batch: Option<String> = conn
            .query_row(
                "SELECT id FROM retro_adjustment_batches
                 WHERE revision_id = ?1 AND status = 'FINALIZED' LIMIT 1",
                params![revision.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        if let Some(batch_id) = finalized_batch {
            return Err(DomainError::PayrollFinalized(format!(
                "{} revision'ına bağlı FINALIZED retro batch değiştirilemez.",
                batch_id
            )));
        }
    }
    let mut stale_impacts = Vec::new();
    if status.is_some() {
        let mut stale_batches = conn
            .prepare(
                "SELECT id FROM retro_adjustment_batches
                 WHERE revision_id = ?1 AND status IN ('DRAFT', 'CALCULATED')",
            )
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        let stale_ids = stale_batches
            .query_map(params![revision.id], |row| row.get::<_, String>(0))
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        drop(stale_batches);
        for batch_id in &stale_ids {
            let payroll_identity: Option<(String, String, String)> = conn
                .query_row(
                    "SELECT personnel_id, period_id, status
                     FROM payroll_records
                     WHERE accrual_type = 'RETRO_ADJUSTMENT' AND accrual_id = ?1
                     LIMIT 1",
                    params![batch_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
            let Some((personnel_id, period_id, status_text)) = payroll_identity else {
                continue;
            };
            if status_text == "FINALIZED" {
                return Err(DomainError::PayrollFinalized(format!(
                    "{} retro payment event'i FINALIZED olduğu için revision değiştirilemez.",
                    batch_id
                )));
            }
            stale_impacts.push(PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &PayrollMutation::AccrualCalculation {
                    personnelId: personnel_id,
                    periodId: period_id,
                    accrualId: batch_id.clone(),
                },
            )?);
        }
        conn.execute(
            "UPDATE retro_adjustment_batches SET status = 'STALE'
             WHERE revision_id = ?1 AND status IN ('DRAFT', 'CALCULATED')",
            params![revision.id],
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        for batch_id in stale_ids {
            conn.execute(
                "UPDATE payroll_records SET status = 'STALE'
                 WHERE accrual_type = 'RETRO_ADJUSTMENT'
                   AND accrual_id = ?1
                   AND status IN ('DRAFT', 'CALCULATED')",
                params![batch_id],
            )
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        }
        for impact in &stale_impacts {
            PayrollInvalidationRepository::apply_impact(conn, impact)?;
        }
    }
    let reason = enum_json(&revision.reason)?;
    let revision_status = enum_json(&revision.status)?;
    let scope = enum_json(&revision.scope)?;
    let personnel_ids_json = encode(&revision.personnelIds)?;
    conn.execute(
        "INSERT INTO compensation_revisions
            (id, reason, title, effective_from, effective_to, decision_date, signed_at,
             description, status, scope, personnel_ids_json, personnel_group, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            reason = excluded.reason, title = excluded.title,
            effective_from = excluded.effective_from, effective_to = excluded.effective_to,
            decision_date = excluded.decision_date, signed_at = excluded.signed_at,
            description = excluded.description, status = excluded.status,
            scope = excluded.scope, personnel_ids_json = excluded.personnel_ids_json,
            personnel_group = excluded.personnel_group, updated_at = excluded.updated_at",
        params![
            revision.id,
            reason,
            revision.title,
            revision.effectiveFrom,
            revision.effectiveTo,
            revision.decisionDate,
            revision.signedAt,
            revision.description,
            revision_status,
            scope,
            personnel_ids_json,
            revision.personnelGroup,
            revision.createdAt,
            revision.updatedAt,
        ],
    )
    .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    conn.execute(
        "DELETE FROM compensation_revision_overrides WHERE revision_id = ?1",
        params![revision.id],
    )
    .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    for item in overrides
        .iter()
        .filter(|item| item.revisionId == revision.id)
    {
        conn.execute(
            "INSERT INTO compensation_revision_overrides
                (id, revision_id, parameter, value, personnel_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item.id,
                item.revisionId,
                enum_json(&item.parameter)?,
                dec_to_kurus(Some(item.value))?,
                item.personnelId,
            ],
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    }
    Ok(())
}

pub fn save_batch(
    conn: &Connection,
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    save_batch_in_transaction_impl(&tx, batch, allocations, false)?;
    tx.commit()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))
}

/// Persists a batch while an outer caller-owned transaction is already open.
/// This keeps backup restore atomic without nesting SQLite transactions.
pub fn save_batch_in_transaction(
    conn: &Connection,
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
) -> Result<()> {
    save_batch_in_transaction_impl(conn, batch, allocations, false)
}

/// Restores a trusted backup batch, including FINALIZED state, inside the
/// caller-owned restore transaction. Normal saves never use this path.
pub fn restore_batch_in_transaction(
    conn: &Connection,
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
) -> Result<()> {
    save_batch_in_transaction_impl(conn, batch, allocations, true)
}

fn save_batch_in_transaction_impl(
    conn: &Connection,
    batch: &RetroAdjustmentBatch,
    allocations: &[RetroAllocation],
    allow_finalized_restore: bool,
) -> Result<()> {
    if batch.id.trim().is_empty()
        || batch.revisionId.trim().is_empty()
        || batch.personnelId.trim().is_empty()
    {
        return Err(DomainError::ValidationError(
            "Retro batch kimliği, revision ve personel kimliği zorunludur.".into(),
        ));
    }
    if !allow_finalized_restore && batch.status != CompensationRevisionStatus::CALCULATED {
        return Err(DomainError::ValidationError(
            "Retro batch yalnız CALCULATED state'iyle persistence katmanına ulaşabilir; FINALIZED/STALE state'i domain service üretmelidir."
                .into(),
        ));
    }
    let mut allocation_ids = HashSet::new();
    let mut allocation_keys = HashSet::new();
    let allocation_total =
        allocations
            .iter()
            .try_fold(rust_decimal::Decimal::ZERO, |total, allocation| {
                if allocation.batchId != batch.id || allocation.personnelId != batch.personnelId {
                    return Err(DomainError::ValidationError(
                        "Retro allocation batch/personel ilişkisi geçersiz.".into(),
                    ));
                }
                if !allocation_ids.insert(allocation.id.clone()) {
                    return Err(DomainError::ValidationError(
                        "Retro allocation kimlikleri tekrarlanamaz.".into(),
                    ));
                }
                let key = format!(
                    "{}\u{0}{:?}",
                    allocation.sourcePeriodId, allocation.earningCode
                );
                if !allocation_keys.insert(key) {
                    return Err(DomainError::ValidationError(
                        "Aynı batch içinde source period ve earning code çifti tekrarlanamaz."
                            .into(),
                    ));
                }
                let policy = payroll_core::retro_earning_policy(allocation.earningCode);
                if allocation.sgkTreatment != policy.sgkTreatment
                    || allocation.incomeTaxTreatment != policy.incomeTaxTreatment
                    || allocation.stampTaxTreatment != policy.stampTaxTreatment
                {
                    return Err(DomainError::InvalidData(
                        "Retro allocation earning policy snapshot'ı canonical registry ile eşleşmiyor."
                            .into(),
                    ));
                }
                if allocation.originalRecognizedAmount < rust_decimal::Decimal::ZERO
                    || allocation.targetAmount < rust_decimal::Decimal::ZERO
                    || allocation.originalPek < rust_decimal::Decimal::ZERO
                    || allocation.adjustedPek < rust_decimal::Decimal::ZERO
                {
                    return Err(DomainError::InvalidData(
                        "Retro allocation authoritative entitlement/PEK tabanı negatif olamaz."
                        .into(),
                    ));
                }
                if round2(
                    allocation.targetAmount
                        - allocation.originalRecognizedAmount
                        - allocation.previousAuthoritativeRetroAmount,
                ) != round2(allocation.deltaAmount)
                {
                    return Err(DomainError::ValidationError(
                        "Retro allocation target - recognized ledger hesabıyla eşleşmiyor."
                            .into(),
                    ));
                }
                Ok(total + allocation.deltaAmount)
            })?;
    if round2(allocation_total) != round2(batch.totalGrossDelta) {
        return Err(DomainError::ValidationError(
            "Retro allocation toplamı batch brüt farkıyla eşleşmiyor.".into(),
        ));
    }
    validate_settlement_status(batch, allocations)?;
    if !allow_finalized_restore && batch.status == CompensationRevisionStatus::FINALIZED {
        return Err(DomainError::PayrollFinalized(
            "FINALIZED retro batch yeniden yazılamaz.".into(),
        ));
    }
    let existing_batch: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT revision_id, personnel_id, payment_date, status
             FROM retro_adjustment_batches
             WHERE id = ?1",
            params![batch.id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    if let Some((existing_revision, existing_personnel, existing_payment_date, existing_status)) =
        existing_batch
    {
        if existing_revision != batch.revisionId
            || existing_personnel != batch.personnelId
            || existing_payment_date != batch.paymentDate
        {
            return Err(DomainError::InvalidData(
                "Retro batch primary id'si farklı revision/personel/ödeme olayına ait; kayıt üzerine yazılamaz."
                    .into(),
            ));
        }
        if !allow_finalized_restore && existing_status == "FINALIZED" {
            return Err(DomainError::PayrollFinalized(
                "FINALIZED retro batch yeniden yazılamaz.".into(),
            ));
        }
    }
    if !allow_finalized_restore {
        let finalized_event: Option<String> = conn
            .query_row(
                "SELECT id FROM payroll_records WHERE accrual_id = ?1 AND status = 'FINALIZED' LIMIT 1",
                params![batch.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        if let Some(event_id) = finalized_event {
            return Err(DomainError::PayrollFinalized(format!(
                "{} retro payment event'i FINALIZED olduğu için batch değiştirilemez.",
                event_id
            )));
        }
    }
    conn.execute(
        "INSERT INTO retro_adjustment_batches
            (id, revision_id, personnel_id, payment_date, status, settlement_status,
             total_gross_delta, description, created_at, calculated_at, finalized_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            revision_id = excluded.revision_id, personnel_id = excluded.personnel_id,
            payment_date = excluded.payment_date, status = excluded.status,
            settlement_status = excluded.settlement_status,
            total_gross_delta = excluded.total_gross_delta, description = excluded.description,
            calculated_at = excluded.calculated_at, finalized_at = excluded.finalized_at",
        params![
            batch.id,
            batch.revisionId,
            batch.personnelId,
            batch.paymentDate,
            enum_json(&batch.status)?,
            enum_json(&batch.settlementStatus)?,
            dec_to_kurus(Some(batch.totalGrossDelta))?,
            batch.description,
            batch.createdAt,
            batch.calculatedAt,
            batch.finalizedAt,
        ],
    )
    .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    conn.execute(
        "DELETE FROM retro_adjustment_allocations WHERE batch_id = ?1",
        params![batch.id],
    )
    .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    for item in allocations {
        conn.execute(
            "INSERT INTO retro_adjustment_allocations
                (id, batch_id, personnel_id, source_period_id, earning_code,
                 original_recognized_amount, previous_retro_amount, target_amount, delta_amount,
                 sgk_treatment, income_tax_treatment, stamp_tax_treatment, original_pek,
                 retro_pek_delta, adjusted_pek, worker_sgk_delta, worker_unemployment_delta,
                 employer_sgk_delta, employer_unemployment_delta, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                item.id,
                item.batchId,
                item.personnelId,
                item.sourcePeriodId,
                enum_json(&item.earningCode)?,
                dec_to_kurus(Some(item.originalRecognizedAmount))?,
                dec_to_kurus(Some(item.previousAuthoritativeRetroAmount))?,
                dec_to_kurus(Some(item.targetAmount))?,
                dec_to_kurus(Some(item.deltaAmount))?,
                enum_json(&item.sgkTreatment)?,
                enum_json(&item.incomeTaxTreatment)?,
                enum_json(&item.stampTaxTreatment)?,
                dec_to_kurus(Some(item.originalPek))?,
                dec_to_kurus(Some(item.retroPekDelta))?,
                dec_to_kurus(Some(item.adjustedPek))?,
                dec_to_kurus(Some(item.workerSgkDelta))?,
                dec_to_kurus(Some(item.workerUnemploymentDelta))?,
                dec_to_kurus(Some(item.employerSgkDelta))?,
                dec_to_kurus(Some(item.employerUnemploymentDelta))?,
                item.metadata,
            ],
        )
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    }
    Ok(())
}
