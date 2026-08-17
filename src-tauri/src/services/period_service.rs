use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::settings_repo::SettingsRepository;
use rusqlite::{params, Connection};

pub struct PeriodService;

impl PeriodService {
    fn identity_changed(old: &BordroDonemi, new: &BordroDonemi) -> bool {
        old.yil != new.yil
            || old.ay != new.ay
            || old.baslangicTarihi != new.baslangicTarihi
            || old.bitisTarihi != new.bitisTarihi
            || old.taxYear != new.taxYear
            || old.taxMonth != new.taxMonth
    }

    fn has_attendance(conn: &Connection, period_id: &str) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM attendance_records WHERE period_id = ?1)",
                params![period_id],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    fn ensure_period_identity_mutation_allowed(
        conn: &Connection,
        old: &BordroDonemi,
        new: &BordroDonemi,
    ) -> Result<()> {
        if !Self::identity_changed(old, new) {
            return Ok(());
        }

        let has_payroll = PayrollRepository::exists_for_period(conn, &old.id)?;
        let has_attendance = Self::has_attendance(conn, &old.id)?;
        if has_payroll || has_attendance {
            return Err(DomainError::ValidationError(
                "Bu döneme puantaj veya bordro bağlandığından dönem kimliği (yıl/ay, 15-14 tarihleri ve vergi yılı/ayı) değiştirilemez. Yeni çalışma dönemi ayrı bir dönem olarak oluşturulmalıdır."
                    .into(),
            ));
        }

        Ok(())
    }

    /// Asgari ücret GV referans zinciri önceki vergi aylarının kurum ayarlarını
    /// kullanır. Bu yüzden yalnız aynı dönemin FINALIZED olması değil, aynı vergi
    /// yılında daha ileri bir FINALIZED bordronun bu döneme bağımlı olması da
    /// tarihsel ayarı immutable yapar.
    fn has_finalized_settings_dependency(
        conn: &Connection,
        period: &BordroDonemi,
    ) -> Result<bool> {
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM payroll_records AS pr
                    JOIN payroll_periods AS finalized_period ON finalized_period.id = pr.period_id
                    WHERE pr.status = 'FINALIZED'
                      AND finalized_period.tax_year = ?1
                      AND finalized_period.tax_month >= ?2
                 )",
                params![period.taxYear, period.taxMonth],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(exists != 0)
    }

    pub fn save_period(conn: &Connection, period: &BordroDonemi) -> Result<()> {
        if let Some(old) = PeriodRepository::get_by_id(conn, &period.id)? {
            Self::ensure_period_identity_mutation_allowed(conn, &old, period)?;
        }

        PeriodRepository::save(conn, period)
    }

    /// Yeni dönem ve onun kurum ayarı tek SQLite transaction'ında yazılır.
    /// Kurum ayarı doğrulanamazsa dönem insert/update'i de geri alınır.
    pub fn save_period_with_settings(
        conn: &mut Connection,
        period: &BordroDonemi,
        settings: &DonemselKurumDegerleri,
    ) -> Result<()> {
        if settings.donemId != period.id {
            return Err(DomainError::ValidationError(
                "Dönem ile kurum ayarı aynı dönem kimliğini taşımalıdır.".into(),
            ));
        }

        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some(old) = PeriodRepository::get_by_id(&tx, &period.id)? {
            Self::ensure_period_identity_mutation_allowed(&tx, &old, period)?;

            if Self::has_finalized_settings_dependency(&tx, &old)? {
                return Err(DomainError::PayrollFinalized(
                    "Bu dönemin kurum ayarları kesinleşmiş bordro zincirinde kullanıldığından değiştirilemez."
                        .into(),
                ));
            }
        }

        PeriodRepository::save(&tx, period)?;
        SettingsRepository::save_institution_settings(&tx, settings)?;
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }
}
