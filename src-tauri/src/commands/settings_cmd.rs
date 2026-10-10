use crate::db::DbState;
use crate::domain::{DomainError, Result};
use crate::repositories::settings_repo::SettingsRepository;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub fn get_institution_settings(
    db: State<'_, DbState>,
) -> Result<HashMap<String, payroll_core::PayrollPeriodInstitutionSettings>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::get_all_active_institution_settings(&conn)
}

#[tauri::command]
pub fn save_institution_settings(
    db: State<'_, DbState>,
    settings: payroll_core::PayrollPeriodInstitutionSettings,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    SettingsRepository::save_active_institution_settings(&conn, &settings)
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

    if key == "is_primi_gruplari" {
        let current = SettingsRepository::get_app_setting(&conn, &key)?;
        if current.as_deref() == Some(&value) {
            return Ok(());
        }
        let groups: Vec<payroll_core::IsPrimiGrupItem> = serde_json::from_str(&value).map_err(|error| {
            DomainError::InvalidData(format!("İş primi grupları ayarı geçersiz JSON içeriyor: {error}"))
        })?;
        let versions = crate::repositories::compensation_policy_repo::CompensationPolicyRepository::get_all_versions(&conn)?;
        if let Some(head) = versions.iter().max_by(|a, b| a.effectiveFrom.cmp(&b.effectiveFrom).then_with(|| a.createdAt.cmp(&b.createdAt))) {
            let impact = crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::assert_mutation_allowed(
                &conn,
                &payroll_core::PayrollMutation::CompensationPolicy {
                    effectiveFrom: head.effectiveFrom.clone(),
                    throughInclusive: None,
                },
            )?;
            crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::apply_impact(&conn, &impact)?;
            let mut updated_head = head.clone();
            updated_head.state.isPrimiGruplari = groups
                .into_iter()
                .map(|g| payroll_core::CompensationPolicyGroup {
                    id: g.id,
                    ad: g.ad,
                    oran: g.oran,
                    aktif: g.aktif,
                })
                .collect();
            let state_json = serde_json::to_string(&updated_head.state).map_err(|e| DomainError::InvalidData(e.to_string()))?;
            conn.execute(
                "UPDATE compensation_policy_versions SET state_json = ?1 WHERE id = ?2",
                rusqlite::params![state_json, head.id],
            ).map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        }
    }

    SettingsRepository::set_app_setting(&conn, &key, &value)
}
