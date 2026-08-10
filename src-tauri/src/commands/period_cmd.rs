use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::period_repo::PeriodRepository;
use crate::services::period_service::PeriodService;
use tauri::State;

#[tauri::command]
pub fn get_periods(db: State<'_, DbState>) -> Result<Vec<BordroDonemi>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PeriodRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_period(db: State<'_, DbState>, period: BordroDonemi) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PeriodService::save_period(&conn, &period)
}

#[tauri::command]
pub fn save_period_with_settings(
    db: State<'_, DbState>,
    period: BordroDonemi,
    settings: DonemselKurumDegerleri,
) -> Result<()> {
    let mut conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PeriodService::save_period_with_settings(&mut conn, &period, &settings)
}
