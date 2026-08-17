use crate::db::DbState;
use crate::domain::{DomainError, Result};
use crate::services::payroll_notice_service::{PayrollNotice, PayrollNoticeService};
use tauri::State;

#[tauri::command]
pub fn get_payroll_notices(
    db: State<'_, DbState>,
    period_id: String,
) -> Result<Vec<PayrollNotice>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollNoticeService::get_period_notices(&conn, &period_id)
}
