use crate::db::DbState;
use crate::domain::models::AnnualPayrollParameters;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use rusqlite::params;
use tauri::State;

fn same_parameter_payload(
    left: &AnnualPayrollParameters,
    right: &AnnualPayrollParameters,
) -> Result<bool> {
    let mut left = left.clone();
    let mut right = right.clone();
    left.updatedAt = None;
    right.updatedAt = None;
    let left_json = serde_json::to_string(&left)
        .map_err(|e| DomainError::InvalidData(e.to_string()))?;
    let right_json = serde_json::to_string(&right)
        .map_err(|e| DomainError::InvalidData(e.to_string()))?;
    Ok(left_json == right_json)
}

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

    let finalized_exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM payroll_records AS pr
                JOIN payroll_periods AS pp ON pp.id = pr.period_id
                WHERE pp.tax_year = ?1
                  AND pr.status = 'FINALIZED'
             )",
            params![parameters.year],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    if finalized_exists != 0 {
        if let Some(existing) =
            AnnualPayrollParametersRepository::get_by_year(&conn, parameters.year)?
        {
            if same_parameter_payload(&existing, &parameters)? {
                return Ok(());
            }
        }

        // Fail closed until a dedicated amendment/version workflow exists. A
        // normal upsert must never rewrite the tariff context of an already
        // finalized tax year.
        return Err(DomainError::PayrollFinalized(format!(
            "{} vergi yılında kesinleşmiş bordro bulunduğundan yıllık vergi parametreleri normal düzenleme yoluyla değiştirilemez.",
            parameters.year
        )));
    }

    AnnualPayrollParametersRepository::save(&conn, &parameters)
}
