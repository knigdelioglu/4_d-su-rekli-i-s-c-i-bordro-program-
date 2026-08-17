use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use rusqlite::params;
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

    let has_finalized: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM payroll_records
                WHERE period_id = ?1 AND status = 'FINALIZED'
             )",
            params![attendance.donemId],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    if has_finalized != 0 {
        return Err(DomainError::PayrollFinalized(
            "Bu dönemin puantajı kesinleşmiş bordro bulunduğundan değiştirilemez.".into(),
        ));
    }

    AttendanceRepository::save(&conn, &attendance)
}
