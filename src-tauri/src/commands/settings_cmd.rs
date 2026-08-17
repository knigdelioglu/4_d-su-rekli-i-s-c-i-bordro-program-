use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::settings_repo::SettingsRepository;
use rusqlite::params;
use std::collections::HashMap;
use tauri::State;

fn normalized_settings_json(settings: &DonemselKurumDegerleri) -> Result<String> {
    let mut normalized = settings.clone();
    if normalized.gunlukYemekIstisnasiGV.is_none() {
        normalized.gunlukYemekIstisnasiGV = normalized.gunlukYemekIstisnasiSGK;
    }
    serde_json::to_string(&normalized).map_err(|e| DomainError::InvalidData(e.to_string()))
}

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

    let period = PeriodRepository::get_by_id(&conn, &settings.donemId)?.ok_or_else(|| {
        DomainError::ValidationError(format!(
            "Kurum ayarı için bordro dönemi bulunamadı: {}.",
            settings.donemId
        ))
    })?;

    // Minimum-wage GV reference cumulative is rebuilt from historical period
    // settings. Therefore a finalized payroll at this tax month or any later tax
    // month in the same year depends on this row even if this exact period has no
    // payroll record of its own.
    let finalized_dependency: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM payroll_records AS pr
                JOIN payroll_periods AS pp ON pp.id = pr.period_id
                WHERE pr.status = 'FINALIZED'
                  AND pp.tax_year = ?1
                  AND pp.tax_month >= ?2
             )",
            params![period.taxYear, period.taxMonth],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    if finalized_dependency != 0 {
        // Saving the exact same effective values is harmless and keeps UI save
        // actions idempotent. Only a real historical mutation is prohibited.
        if let Some(existing) = SettingsRepository::get_institution_settings(&conn, &settings.donemId)? {
            if normalized_settings_json(&existing)? == normalized_settings_json(&settings)? {
                return Ok(());
            }
        }

        return Err(DomainError::PayrollFinalized(
            "Bu dönemin kurum ayarları kesinleşmiş bordro zincirinde kullanıldığından değiştirilemez."
                .into(),
        ));
    }

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
