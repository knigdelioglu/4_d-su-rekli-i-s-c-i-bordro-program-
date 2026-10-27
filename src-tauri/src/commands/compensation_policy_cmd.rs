use crate::db::DbState;
use crate::domain::{DomainError, Result};
use crate::services::compensation_policy_service::{
    CompensationPolicyLoadData, CompensationPolicyLoadRequest, CompensationPolicyOperationResult,
    CompensationPolicyPreviewData, CompensationPolicyPreviewRequest, CompensationPolicySaveReceipt,
    CompensationPolicySaveRequest, CompensationPolicyService,
};
use tauri::State;

fn lock_database(db: &DbState) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })
}

#[tauri::command]
pub fn load_compensation_policy(
    db: State<'_, DbState>,
    request: CompensationPolicyLoadRequest,
) -> Result<CompensationPolicyOperationResult<CompensationPolicyLoadData>> {
    let conn = lock_database(&db)?;
    Ok(CompensationPolicyService::load(&conn, &request))
}

#[tauri::command]
pub fn preview_compensation_policy(
    db: State<'_, DbState>,
    request: CompensationPolicyPreviewRequest,
) -> Result<CompensationPolicyOperationResult<CompensationPolicyPreviewData>> {
    let conn = lock_database(&db)?;
    Ok(CompensationPolicyService::preview(&conn, &request))
}

#[tauri::command]
pub fn save_compensation_policy(
    license: State<'_, crate::license::LicenseRuntime>,
    db: State<'_, DbState>,
    request: CompensationPolicySaveRequest,
) -> Result<CompensationPolicyOperationResult<CompensationPolicySaveReceipt>> {
    crate::license::require_allowed(license.data_dir(), "ücret politikası kaydetme")
        .map_err(DomainError::ValidationError)?;
    let conn = lock_database(&db)?;
    Ok(CompensationPolicyService::save(&conn, &request))
}

#[tauri::command]
pub fn preview_road_rounding_repair(
    db: State<'_, DbState>,
    scope: crate::services::compensation_policy_service::CompensationPolicyReadScope,
) -> Result<
    CompensationPolicyOperationResult<
        crate::services::compensation_policy_service::RoadRoundingRepairPreview,
    >,
> {
    let conn = lock_database(&db)?;
    Ok(CompensationPolicyService::preview_rounding_repair(
        &conn, &scope,
    ))
}

#[tauri::command]
pub fn save_road_rounding_repair(
    license: State<'_, crate::license::LicenseRuntime>,
    db: State<'_, DbState>,
    scope: crate::services::compensation_policy_service::CompensationPolicyReadScope,
    preview_tokens: Vec<
        crate::services::compensation_policy_service::CompensationPolicyPreviewToken,
    >,
) -> Result<CompensationPolicyOperationResult<Vec<CompensationPolicySaveReceipt>>> {
    crate::license::require_allowed(license.data_dir(), "yol yuvarlama onarımı")
        .map_err(DomainError::ValidationError)?;
    let conn = lock_database(&db)?;
    Ok(CompensationPolicyService::save_rounding_repair(
        &conn,
        &scope,
        &preview_tokens,
    ))
}
