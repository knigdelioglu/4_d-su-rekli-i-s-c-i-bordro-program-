use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::services::payroll_service::PayrollService;
use payroll_core::PayrollMutation;
use tauri::State;

#[tauri::command]
pub fn get_payroll_list(db: State<'_, DbState>) -> Result<Vec<BordroKaydi>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollRepository::get_all(&conn)
}

#[tauri::command]
pub fn calculate_payroll(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    manual_income: Option<ManualPayrollIncomeInput>,
    accrual: Option<PayrollAccrualInput>,
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollService::validate_payroll_request_for_accrual(
        &conn,
        &personnel_id,
        &period_id,
        accrual.as_ref(),
    )?;
    PayrollService::calculate_payroll_for_accrual(
        &conn,
        &personnel_id,
        &period_id,
        accrual.as_ref(),
        manual_income.as_ref(),
    )
}

#[tauri::command]
pub fn finalize_payroll(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    accrual_id: Option<String>,
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollService::finalize_payroll_for_accrual(
        &conn,
        &personnel_id,
        &period_id,
        accrual_id.as_deref(),
    )
}

#[tauri::command]
pub fn evaluate_mutation_policy(
    db: State<'_, DbState>,
    mutation: PayrollMutation,
) -> Result<payroll_core::MutationImpact> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollService::evaluate_mutation_policy(&conn, &mutation)
}

#[tauri::command]
pub fn set_payroll_status(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    status: BordroStatus,
    accrual_id: Option<String>,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;

    if status == BordroStatus::FINALIZED {
        return Err(DomainError::ValidationError(
            "FINALIZED geçişi için finalize_payroll komutu kullanılmalıdır.".into(),
        ));
    }

    PayrollService::set_payroll_status_for_accrual(
        &conn,
        &personnel_id,
        &period_id,
        accrual_id.as_deref(),
        status,
    )
}
