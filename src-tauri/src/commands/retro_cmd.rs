use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::retro_repo;
use crate::services::payroll_service::PayrollService;
use payroll_core::{RetroCalculationRequest, RetroCalculationResult, RetroEntitlementEngine};
use tauri::State;

#[tauri::command]
pub fn get_compensation_revisions(db: State<'_, DbState>) -> Result<Vec<CompensationRevision>> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    retro_repo::get_revisions(&conn)
}

#[tauri::command]
pub fn get_compensation_revision_overrides(
    db: State<'_, DbState>,
) -> Result<Vec<CompensationRevisionOverride>> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    retro_repo::get_overrides(&conn)
}

#[tauri::command]
pub fn get_retro_adjustment_batches(db: State<'_, DbState>) -> Result<Vec<RetroAdjustmentBatch>> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    retro_repo::get_batches(&conn)
}

#[tauri::command]
pub fn get_retro_adjustment_allocations(db: State<'_, DbState>) -> Result<Vec<RetroAllocation>> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    retro_repo::get_allocations(&conn)
}

#[tauri::command]
pub fn save_compensation_revision(
    db: State<'_, DbState>,
    revision: CompensationRevision,
    overrides: Vec<CompensationRevisionOverride>,
) -> Result<()> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    retro_repo::save_revision_with_overrides(&conn, &revision, &overrides)
}

#[tauri::command]
pub fn calculate_retro_preview(
    db: State<'_, DbState>,
    batch_id: String,
    revision: CompensationRevision,
    overrides: Vec<CompensationRevisionOverride>,
    personnel_id: String,
    payment_date: String,
    calculated_at: String,
    description: Option<String>,
) -> Result<RetroCalculationResult> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    let dataset = PayrollService::build_dataset_snapshot(&conn)?;
    RetroEntitlementEngine::calculate(&RetroCalculationRequest {
        batchId: batch_id,
        revision,
        overrides,
        personnelId: personnel_id,
        paymentDate: payment_date,
        calculatedAt: calculated_at,
        description,
        dataset,
    })
}

#[tauri::command]
pub fn save_retro_adjustment_batch(
    db: State<'_, DbState>,
    batch: RetroAdjustmentBatch,
    allocations: Vec<RetroAllocation>,
) -> Result<()> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    PayrollService::save_retro_adjustment_batch(&conn, &batch, &allocations)
}

#[tauri::command]
pub fn create_retro_payment(
    db: State<'_, DbState>,
    batch: RetroAdjustmentBatch,
    allocations: Vec<RetroAllocation>,
    payment_period_id: String,
    sequence: i32,
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    PayrollService::create_retro_payment(&conn, &batch, &allocations, &payment_period_id, sequence)
}
