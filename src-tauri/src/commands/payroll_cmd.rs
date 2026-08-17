use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::kurus_to_dec;
use crate::services::payroll_preflight_service::PayrollPreflightService;
use crate::services::payroll_service::PayrollService;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

fn saved_manual_income(
    conn: &Connection,
    personnel_id: &str,
    period_id: &str,
) -> Result<ManualPayrollIncomeInput> {
    let payroll_id = conn
        .query_row(
            "SELECT id FROM payroll_records WHERE personnel_id = ?1 AND period_id = ?2",
            params![personnel_id, period_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?
        .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?;

    let mut result = ManualPayrollIncomeInput::default();
    let mut stmt = conn
        .prepare(
            "SELECT item_type, amount
             FROM payroll_income_items
             WHERE payroll_id = ?1
               AND item_type IN ('tediye', 'tisIkramiyesi')",
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let rows = stmt
        .query_map(params![payroll_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    for row in rows {
        let (item_type, amount) = row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        match item_type.as_str() {
            "tediye" => result.tediye = Some(kurus_to_dec(amount)),
            "tisIkramiyesi" => result.tisIkramiyesi = Some(kurus_to_dec(amount)),
            _ => {}
        }
    }
    Ok(result)
}

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
) -> Result<BordroKaydi> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PayrollPreflightService::validate_for_calculation(&conn, &personnel_id, &period_id)?;
    PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        &personnel_id,
        &period_id,
        manual_income.as_ref(),
    )
}

#[tauri::command]
pub fn set_payroll_status(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    status: BordroStatus,
) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;

    let current = PayrollRepository::get_status_and_created_at(&conn, &personnel_id, &period_id)?;
    let current_status = current
        .as_ref()
        .map(|(status, _)| *status)
        .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?;

    if status == BordroStatus::FINALIZED && current_status == BordroStatus::CALCULATED {
        // Mutation invalidation birincil korumadır. Bu yeniden hesaplama ise ikinci
        // emniyet kemeridir: herhangi bir kaynak mutation yolu STALE işaretini
        // atlasa bile eski sonuç FINALIZED yapılamaz.
        PayrollPreflightService::validate_for_calculation(&conn, &personnel_id, &period_id)?;
        let manual_income = saved_manual_income(&conn, &personnel_id, &period_id)?;
        PayrollService::calculate_payroll_for_personnel_with_manual_income(
            &conn,
            &personnel_id,
            &period_id,
            Some(&manual_income),
        )?;
    }

    PayrollService::set_payroll_status(&conn, &personnel_id, &period_id, status)
}
