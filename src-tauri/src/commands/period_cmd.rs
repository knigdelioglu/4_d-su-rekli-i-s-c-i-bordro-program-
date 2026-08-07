use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::Result;
use crate::repositories::period_repo::PeriodRepository;
use tauri::State;

#[tauri::command]
pub fn get_periods(db: State<'_, DbState>) -> Result<Vec<BordroDonemi>> {
    let conn = db.lock().unwrap();
    PeriodRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_period(db: State<'_, DbState>, period: BordroDonemi) -> Result<()> {
    let conn = db.lock().unwrap();
    PeriodRepository::save(&conn, &period)
}
