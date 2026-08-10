use crate::db::DbState;
use crate::domain::models::SickLeaveRecord;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use tauri::State;

#[tauri::command]
pub fn get_sick_leave_records(
    state: State<'_, DbState>,
    personnel_id: Option<String>,
) -> Result<Vec<SickLeaveRecord>, String> {
    let conn = state.lock().map_err(|e| e.to_string())?;
    if let Some(ref pid) = personnel_id {
        SickLeaveRepository::get_by_personnel(&conn, pid).map_err(|e| e.to_string())
    } else {
        SickLeaveRepository::get_all(&conn).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn save_sick_leave_record(
    state: State<'_, DbState>,
    record: SickLeaveRecord,
) -> Result<(), String> {
    let conn = state.lock().map_err(|e| e.to_string())?;
    SickLeaveRepository::save(&conn, &record).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_sick_leave_record(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = state.lock().map_err(|e| e.to_string())?;
    SickLeaveRepository::delete(&conn, &id).map_err(|e| e.to_string())
}
