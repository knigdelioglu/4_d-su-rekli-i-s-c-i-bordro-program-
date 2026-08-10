use crate::db::DbState;
use crate::domain::{DomainError, Result};
use crate::services::migration_service::MigrationService;
use tauri::State;

#[tauri::command]
pub fn check_legacy_migrated(db: State<'_, DbState>) -> Result<bool> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    MigrationService::is_migrated(&conn)
}

#[tauri::command]
pub fn migrate_legacy_payload(db: State<'_, DbState>, payload_json: String) -> Result<()> {
    let mut conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    MigrationService::migrate_legacy_data(&mut conn, &payload_json)
}

#[tauri::command]
pub fn replace_backup_payload(db: State<'_, DbState>, payload_json: String) -> Result<()> {
    let mut conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    MigrationService::replace_backup_data(&mut conn, &payload_json)
}
