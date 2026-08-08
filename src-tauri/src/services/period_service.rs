use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
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
                let has_payroll = PayrollRepository::get_all(conn)?
                    .into_iter()
                    .any(|b| b.donemId == period.id);

                if has_payroll {
                    return Err(DomainError::ValidationError(
                        "Bu dönem için bordro kaydı bulunduğundan Vergi Yılı/Ayı değiştirilemez.".into(),
                    ));
                }
            }
        }

        PeriodRepository::save(conn, period)
    }
}
