use crate::db::DbState;
use crate::domain::models::AnnualPayrollParameters;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use tauri::State;

#[tauri::command]
pub fn get_annual_payroll_parameters(
    db: State<'_, DbState>,
) -> Result<Vec<AnnualPayrollParameters>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    AnnualPayrollParametersRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_annual_payroll_parameters(
    db: State<'_, DbState>,
    parameters: AnnualPayrollParameters,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    AnnualPayrollParametersRepository::save(&conn, &parameters)
}
