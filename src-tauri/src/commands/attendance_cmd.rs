use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use tauri::State;

#[tauri::command]
pub fn get_attendance_list(db: State<'_, DbState>) -> Result<Vec<PersonelPuantaj>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    AttendanceRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_attendance(db: State<'_, DbState>, attendance: PersonelPuantaj) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    AttendanceRepository::save(&conn, &attendance)
}

#[tauri::command]
pub fn save_attendances(db: State<'_, DbState>, attendances: Vec<PersonelPuantaj>) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    AttendanceRepository::save_many(&conn, &attendances)
}

#[tauri::command]
pub fn apply_attendance_bulk_intent(
    db: State<'_, DbState>,
    period_id: String,
    personnel_ids: Vec<String>,
    dates: Vec<String>,
    code: String,
) -> Result<()> {
    let conn = db.lock().map_err(|error| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {error}"))
    })?;
    AttendanceRepository::apply_bulk_intent(&conn, &period_id, &personnel_ids, &dates, &code)
}
