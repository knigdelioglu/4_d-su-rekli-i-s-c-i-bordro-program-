use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::settings_repo::SettingsRepository;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub fn get_institution_settings(
    db: State<'_, DbState>,
) -> Result<HashMap<String, DonemselKurumDegerleri>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::get_all_institution_settings(&conn)
}

#[tauri::command]
pub fn save_institution_settings(
    db: State<'_, DbState>,
    settings: DonemselKurumDegerleri,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::save_institution_settings(&conn, &settings)
}

#[tauri::command]
pub fn get_app_setting(db: State<'_, DbState>, key: String) -> Result<Option<String>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::get_app_setting(&conn, &key)
}

#[tauri::command]
pub fn set_app_setting(db: State<'_, DbState>, key: String, value: String) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::set_app_setting(&conn, &key, &value)
}
