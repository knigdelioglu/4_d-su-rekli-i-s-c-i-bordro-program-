use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::settings_repo::SettingsRepository;
use rusqlite::{params, Connection, OptionalExtension};

pub struct PeriodService;

impl PeriodService {
    /// Explicit destructive action; finalized history may be discarded here.
    /// Foreign-key cascades remove period-owned data; unrelated tax-opening components survive.
    pub fn delete_period(conn: &mut Connection, period_id: &str) -> Result<()> {
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let period = PeriodRepository::get_by_id(&tx, period_id)?
            .ok_or_else(|| DomainError::ValidationError("Silinecek dönem bulunamadı.".into()))?;
        let impact = PayrollInvalidationRepository::evaluate_mutation(
            &tx,
            &payroll_core::PayrollMutation::PeriodFromPosition {
                startDate: period.baslangicTarihi,
                taxYear: period.taxYear,
                taxMonth: period.taxMonth,
            },
        )?;
        let db_error = |e: rusqlite::Error| DomainError::DatabaseError(e.to_string());
        let mut batches = impact.affectedRetroBatches;
        batches.extend(impact.blockedByFinalizedRetroBatches);
        {
            let mut stmt = tx.prepare("SELECT batch_id FROM retro_adjustment_allocations WHERE source_period_id = ?1
                UNION SELECT accrual_id FROM payroll_records WHERE period_id = ?1 AND accrual_type = 'RETRO_ADJUSTMENT'").map_err(db_error)?;
            let rows = stmt
                .query_map([period_id], |row| row.get::<_, String>(0))
                .map_err(db_error)?;
            for row in rows {
                batches.push(row.map_err(db_error)?);
            }
        }
        for batch in batches {
            tx.execute(
                "DELETE FROM payroll_records WHERE accrual_id = ?1",
                [&batch],
            )
            .map_err(db_error)?;
            tx.execute(
                "DELETE FROM retro_adjustment_batches WHERE id = ?1",
                [&batch],
            )
            .map_err(db_error)?;
        }
        for key in impact
            .affectedPayrolls
            .iter()
            .chain(impact.blockedByFinalized.iter())
        {
            tx.execute(
                "UPDATE payroll_records SET status = 'STALE', updated_at = ?1
                WHERE personnel_id = ?2 AND period_id = ?3 AND accrual_id = ?4",
                params![
                    chrono::Utc::now().to_rfc3339(),
                    key.personnelId,
                    key.periodId,
                    key.accrualId
                ],
            )
            .map_err(db_error)?;
        }
        let tax_opening_ids = {
            let mut stmt = tx
                .prepare(
                    "SELECT id FROM personnel_tax_opening
                     WHERE effective_from_period_id = ?1
                        OR asgari_gv_effective_from_period_id = ?1",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([period_id], |row| row.get::<_, String>(0))
                .map_err(db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(db_error)?
        };
        let tax_opening_updated_at = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE personnel_tax_opening
             SET gv_cumulative_opening = NULL, effective_from_period_id = NULL,
                 updated_at = ?2
             WHERE effective_from_period_id = ?1",
            params![period_id, tax_opening_updated_at],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE personnel_tax_opening
             SET asgari_gv_cumulative_opening = NULL,
                 asgari_gv_effective_from_period_id = NULL, updated_at = ?2
             WHERE asgari_gv_effective_from_period_id = ?1",
            params![period_id, tax_opening_updated_at],
        )
        .map_err(db_error)?;
        for id in tax_opening_ids {
            tx.execute(
                "DELETE FROM personnel_tax_opening
                 WHERE id = ?1 AND gv_cumulative_opening IS NULL
                   AND asgari_gv_cumulative_opening IS NULL",
                [id],
            )
            .map_err(db_error)?;
        }
        tx.execute("DELETE FROM payroll_periods WHERE id = ?1", [period_id])
            .map_err(db_error)?;
        let active: Option<String> = tx
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'active_period_id'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if active.as_deref() == Some(period_id) {
            let next: Option<String> = tx
                .query_row(
                    "SELECT id FROM payroll_periods ORDER BY yil, ay, id LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_error)?;
            tx.execute("UPDATE app_settings SET value = ?1, updated_at = ?2 WHERE key = 'active_period_id'",
                params![next.unwrap_or_default(), chrono::Utc::now().to_rfc3339()]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    }

    /// Dönem kaydetme: vergi yılı/ayı (taxYear/taxMonth) kritik bordro metadata'sıdır;
    /// değişmesi geçmiş/sıralama/kümülatif GV sonuçlarını etkiler. Bir dönem için
    /// HERHANGİ bir bordro kaydı (DRAFT / CALCULATED / FINALIZED ayrımı yapılmaz)
    /// oluşturulduktan sonra bu iki alan immutable kabul edilir. Henüz bordro kaydı
    /// yoksa kullanıcı değiştirebilir; diğer izin verilen dönem alanlarının update
    /// davranışı bozulmaz. Reddedilen değişiklik sessizce yok sayılmaz.
    pub fn save_period(conn: &Connection, period: &BordroDonemi) -> Result<()> {
        let existing = PeriodRepository::get_by_id(conn, &period.id)?;

        if let Some(old) = existing {
            if old.taxYear != period.taxYear || old.taxMonth != period.taxMonth {
                let has_payroll = PayrollRepository::exists_for_period(conn, &period.id)?;

                if has_payroll {
                    return Err(DomainError::ValidationError(
                        "Bu dönem için bordro kaydı bulunduğundan Vergi Yılı/Ayı değiştirilemez."
                            .into(),
                    ));
                }
            }
        }

        PeriodRepository::save(conn, period)
    }

    /// Yeni dönem ve onun kurum ayarı tek SQLite transaction'ında yazılır.
    /// Kurum ayarı doğrulanamazsa dönem insert/update'i de geri alınır.
    pub fn save_period_with_settings(
        conn: &mut Connection,
        period: &BordroDonemi,
        settings: &payroll_core::PayrollPeriodInstitutionSettings,
    ) -> Result<()> {
        if settings.donemId != period.id {
            return Err(DomainError::ValidationError(
                "Dönem ile kurum ayarı aynı dönem kimliğini taşımalıdır.".into(),
            ));
        }

        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let existing = PeriodRepository::get_by_id(&tx, &period.id)?;
        if let Some(old) = existing {
            if (old.taxYear != period.taxYear || old.taxMonth != period.taxMonth)
                && PayrollRepository::exists_for_period(&tx, &period.id)?
            {
                return Err(DomainError::ValidationError(
                    "Bu dönem için bordro kaydı bulunduğundan Vergi Yılı/Ayı değiştirilemez."
                        .into(),
                ));
            }
        }

        PeriodRepository::save_in_transaction(&tx, period)?;
        SettingsRepository::save_active_institution_settings_in_transaction(&tx, settings)?;
        // The Rust core owns the supported legal package. Bootstrap it when a
        // new period is created, but never reinterpret an unsupported year as
        // 2026.
        if period.taxYear == 2026
            && AnnualPayrollParametersRepository::get_by_year(&tx, period.taxYear)?.is_none()
        {
            AnnualPayrollParametersRepository::save_in_transaction(
                &tx,
                &AnnualPayrollParameters::default_for_2026(),
            )?;
        }
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }
}
