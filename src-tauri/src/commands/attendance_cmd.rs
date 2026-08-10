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
