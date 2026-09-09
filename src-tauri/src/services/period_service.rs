use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::settings_repo::SettingsRepository;
use rusqlite::Connection;

pub struct PeriodService;

impl PeriodService {
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
        SettingsRepository::save_institution_settings_in_transaction(&tx, settings)?;
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
