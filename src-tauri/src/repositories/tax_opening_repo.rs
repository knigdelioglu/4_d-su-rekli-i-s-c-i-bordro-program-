use super::{dec_to_kurus, kurus_to_dec};
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::transaction::with_transaction;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub struct TaxOpeningRepository;

impl TaxOpeningRepository {
    pub fn get_all(conn: &Connection) -> Result<Vec<PersonelTaxOpening>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, personnel_id, year, gv_cumulative_opening, effective_from_period_id,
                    asgari_gv_cumulative_opening, asgari_gv_effective_from_period_id,
                    created_at, updated_at
             FROM personnel_tax_opening",
            )
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let opening_kurus: i64 = row.get(3)?;
                Ok(PersonelTaxOpening {
                    id: row.get(0)?,
                    personnelId: row.get(1)?,
                    year: row.get(2)?,
                    gvCumulativeOpening: kurus_to_dec(opening_kurus),
                    effectiveFromPeriodId: row.get(4)?,
                    asgariGvCumulativeOpening: row.get::<_, Option<i64>>(5)?.map(kurus_to_dec),
                    asgariGvEffectiveFromPeriodId: row.get(6)?,
                    createdAt: row.get(7)?,
                    updatedAt: row.get(8)?,
                })
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn get_by_personnel_and_year(
        conn: &Connection,
        personnel_id: &str,
        year: i32,
    ) -> Result<Option<PersonelTaxOpening>> {
        conn.query_row(
            "SELECT id, personnel_id, year, gv_cumulative_opening, effective_from_period_id,
                    asgari_gv_cumulative_opening, asgari_gv_effective_from_period_id,
                    created_at, updated_at
             FROM personnel_tax_opening WHERE personnel_id = ?1 AND year = ?2",
            params![personnel_id, year],
            |row| {
                let opening_kurus: i64 = row.get(3)?;
                Ok(PersonelTaxOpening {
                    id: row.get(0)?,
                    personnelId: row.get(1)?,
                    year: row.get(2)?,
                    gvCumulativeOpening: kurus_to_dec(opening_kurus),
                    effectiveFromPeriodId: row.get(4)?,
                    asgariGvCumulativeOpening: row.get::<_, Option<i64>>(5)?.map(kurus_to_dec),
                    asgariGvEffectiveFromPeriodId: row.get(6)?,
                    createdAt: row.get(7)?,
                    updatedAt: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))
    }

    pub fn save(conn: &Connection, t: &PersonelTaxOpening) -> Result<()> {
        with_transaction(conn, |tx| Self::save_in_transaction(tx, t))
    }

    /// Caller-owned transaction variant used by backup restore.
    pub fn save_in_transaction(conn: &Connection, t: &PersonelTaxOpening) -> Result<()> {
        if t.year <= 0
            || t.gvCumulativeOpening < rust_decimal::Decimal::ZERO
            || t.asgariGvCumulativeOpening
                .is_some_and(|value| value < rust_decimal::Decimal::ZERO)
        {
            return Err(crate::domain::DomainError::ValidationError(
                "Vergi açılışı geçerli bir yıl ve negatif olmayan bir matrah içermelidir.".into(),
            ));
        }
        if t.effectiveFromPeriodId.trim().is_empty() {
            return Err(crate::domain::DomainError::ValidationError(
                "Vergi açılışı için başlangıç dönemi zorunludur.".into(),
            ));
        }
        let _effective_period = PeriodRepository::get_by_id(conn, &t.effectiveFromPeriodId)?
            .ok_or_else(|| {
                crate::domain::DomainError::ValidationError(format!(
                    "GV açılış başlangıç dönemi bulunamadı: {}.",
                    t.effectiveFromPeriodId
                ))
            })?;
        // Keep legacy rows readable even when an old import paired a tax-year
        // opening with a work-period ID from another tax year. The canonical
        // resolver validates this relation at calculation time; persistence
        // must not rewrite or discard existing history during migration.
        if let Some(asgari_period_id) = t.asgariGvEffectiveFromPeriodId.as_deref() {
            if asgari_period_id.trim().is_empty() {
                return Err(crate::domain::DomainError::ValidationError(
                    "Asgari GV açılışı için başlangıç dönemi boş olamaz.".into(),
                ));
            }
            let _asgari_period =
                PeriodRepository::get_by_id(conn, asgari_period_id)?.ok_or_else(|| {
                    crate::domain::DomainError::ValidationError(format!(
                        "Asgari GV açılış başlangıç dönemi bulunamadı: {}.",
                        asgari_period_id
                    ))
                })?;
            // As above, calculation resolves the period's taxYear/taxMonth and
            // fails closed if this legacy row is inconsistent.
        }

        let impact = PayrollInvalidationRepository::assert_mutation_allowed(
            conn,
            &payroll_core::PayrollMutation::PersonTaxYear {
                personnelId: t.personnelId.clone(),
                taxYear: t.year,
            },
        )?;
        let existing = Self::get_by_personnel_and_year(conn, &t.personnelId, t.year)?;
        let changed = existing
            .as_ref()
            .map(|old| {
                old.gvCumulativeOpening != t.gvCumulativeOpening
                    || old.effectiveFromPeriodId != t.effectiveFromPeriodId
                    || old.asgariGvCumulativeOpening != t.asgariGvCumulativeOpening
                    || old.asgariGvEffectiveFromPeriodId != t.asgariGvEffectiveFromPeriodId
            })
            .unwrap_or(true);
        let now = Utc::now().to_rfc3339();
        let opening_kurus = dec_to_kurus(Some(t.gvCumulativeOpening))?;
        let asgari_opening_kurus = dec_to_kurus(t.asgariGvCumulativeOpening)?;

        conn.execute(
            "INSERT INTO personnel_tax_opening (
                id, personnel_id, year, gv_cumulative_opening, effective_from_period_id,
                asgari_gv_cumulative_opening, asgari_gv_effective_from_period_id,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(personnel_id, year) DO UPDATE SET
                gv_cumulative_opening=?4,
                effective_from_period_id=?5,
                asgari_gv_cumulative_opening=?6,
                asgari_gv_effective_from_period_id=?7,
                updated_at=?9",
            params![
                t.id,
                t.personnelId,
                t.year,
                opening_kurus,
                t.effectiveFromPeriodId,
                asgari_opening_kurus,
                t.asgariGvEffectiveFromPeriodId,
                now,
                now
            ],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        if changed {
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        }

        Ok(())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        with_transaction(conn, |tx| Self::delete_in_transaction(tx, id))
    }

    /// Caller-owned transaction variant used by composite use cases.
    pub fn delete_in_transaction(conn: &Connection, id: &str) -> Result<()> {
        let owner = conn
            .query_row(
                "SELECT personnel_id, year FROM personnel_tax_opening WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some((personnel_id, year)) = owner {
            let impact = PayrollInvalidationRepository::assert_mutation_allowed(
                conn,
                &payroll_core::PayrollMutation::PersonTaxYear {
                    personnelId: personnel_id,
                    taxYear: year,
                },
            )?;
            conn.execute(
                "DELETE FROM personnel_tax_opening WHERE id = ?1",
                params![id],
            )
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
            return Ok(());
        }

        conn.execute(
            "DELETE FROM personnel_tax_opening WHERE id = ?1",
            params![id],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
