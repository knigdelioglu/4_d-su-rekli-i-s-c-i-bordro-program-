use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::Result;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::services::payroll_service::PayrollService;
use tauri::State;

#[tauri::command]
pub fn get_payroll_list(db: State<'_, DbState>) -> Result<Vec<BordroKaydi>> {
    let conn = db.lock().unwrap();
    PayrollRepository::get_all(&conn)
}

#[tauri::command]
pub fn calculate_payroll(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
) -> Result<BordroKaydi> {
    let conn = db.lock().unwrap();
    PayrollService::calculate_payroll_for_personnel(&conn, &personnel_id, &period_id)
}

#[tauri::command]
pub fn set_payroll_status(
    db: State<'_, DbState>,
    personnel_id: String,
    period_id: String,
    status: BordroStatus,
) -> Result<()> {
    let conn = db.lock().unwrap();
    PayrollService::set_payroll_status(&conn, &personnel_id, &period_id, status)
}
