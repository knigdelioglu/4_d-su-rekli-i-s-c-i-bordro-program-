use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::retro_repo;
use chrono::Utc;
use payroll_core::{MutationImpact, PayrollDatasetSnapshot, PayrollMutation};
use rusqlite::{params, Connection};

/// SQLite adapter for the core-owned payroll mutation policy.
///
/// This type does not decide which payrolls are affected. It snapshots the
/// relevant persisted records, asks `payroll-core` for the impact, rejects any
/// FINALIZED blocker, and applies the returned mutable keys as STALE.
pub struct PayrollInvalidationRepository;

impl PayrollInvalidationRepository {
    fn policy_snapshot(conn: &Connection) -> Result<PayrollDatasetSnapshot> {
        Ok(PayrollDatasetSnapshot {
            periods: PeriodRepository::get_all(conn)?,
            payrolls: PayrollRepository::get_all(conn)?,
            retroBatches: retro_repo::get_batches(conn)?,
            retroAllocations: retro_repo::get_allocations(conn)?,
            ..PayrollDatasetSnapshot::default()
        })
    }

    pub fn evaluate_mutation(
        conn: &Connection,
        mutation: &PayrollMutation,
    ) -> Result<MutationImpact> {
        let snapshot = Self::policy_snapshot(conn)?;
        payroll_core::evaluate_payroll_invalidation(&snapshot, mutation)
    }

    pub fn assert_mutation_allowed(
        conn: &Connection,
        mutation: &PayrollMutation,
    ) -> Result<MutationImpact> {
        let impact = Self::evaluate_mutation(conn, mutation)?;
        if !impact.blockedByFinalized.is_empty()
            || !impact.blockedByFinalizedRetroBatches.is_empty()
        {
            let keys = impact
                .blockedByFinalized
                .iter()
                .map(|key| format!("{} / {} / {}", key.personnelId, key.periodId, key.accrualId))
                .collect::<Vec<_>>()
                .join(", ");
            let batches = impact.blockedByFinalizedRetroBatches.join(", ");
            let detail = [
                (!keys.is_empty()).then_some(keys),
                (!batches.is_empty()).then_some(format!("retro batch: {batches}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            return Err(DomainError::PayrollFinalized(format!(
                "Kesinleştirilmiş bordro/retro tarihçesini etkileyen veri değiştirilemez: {}.",
                detail
            )));
        }
        Ok(impact)
    }

    pub fn apply_impact(conn: &Connection, impact: &MutationImpact) -> Result<usize> {
        if !impact.blockedByFinalized.is_empty()
            || !impact.blockedByFinalizedRetroBatches.is_empty()
        {
            return Err(DomainError::PayrollFinalized(
                "Kesinleştirilmiş bordro/retro tarihçesini etkileyen mutation uygulanamaz.".into(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let mut changed = 0;
        for key in &impact.affectedPayrolls {
            changed += conn
                .execute(
                    "UPDATE payroll_records
                     SET status = 'STALE', updated_at = ?1
                     WHERE personnel_id = ?2
                       AND period_id = ?3
                       AND accrual_id = ?4
                       AND status IN ('CALCULATED', 'DRAFT')",
                    params![now, key.personnelId, key.periodId, key.accrualId],
                )
                .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        }
        for batch_id in &impact.affectedRetroBatches {
            changed += conn
                .execute(
                    "UPDATE retro_adjustment_batches
                     SET status = 'STALE'
                     WHERE id = ?1
                       AND status IN ('DRAFT', 'CALCULATED')",
                    params![batch_id],
                )
                .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        }
        Ok(changed)
    }
}
