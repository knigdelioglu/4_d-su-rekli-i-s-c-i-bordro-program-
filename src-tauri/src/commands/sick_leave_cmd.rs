use crate::db::DbState;
use crate::domain::models::SickLeaveRecord;
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use tauri::State;

fn protected_by_finalized_history(
    conn: &Connection,
    record: &SickLeaveRecord,
) -> Result<bool, String> {
    SickLeaveRepository::validate_record(record).map_err(|e| e.to_string())?;
    let start = NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d")
        .map_err(|e| e.to_string())?;
    let year_end = format!("{:04}-12-31", start.year());

    // Sick-leave quota is episode-order dependent within the start-date year.
    // A backdated insert/update/delete can therefore rewrite not only the payroll
    // containing this record but any later finalized payroll in that calendar
    // year's episode chain.
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM payroll_records AS pr
                JOIN payroll_periods AS pp ON pp.id = pr.period_id
                WHERE pr.personnel_id = ?1
                  AND pr.status = 'FINALIZED'
                  AND pp.bitis_tarihi >= ?2
                  AND pp.baslangic_tarihi <= ?3
             )",
            params![record.personnelId, record.startDate, year_end],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(exists != 0)
}

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

    // Updates must protect both the old episode position and the proposed new
    // position. Otherwise an old protected record could be moved to the future
    // and silently disappear from the historical first-five episode ordering.
    if let Some(existing) = SickLeaveRepository::get_by_id(&conn, &record.id)
        .map_err(|e| e.to_string())?
    {
        let unchanged = existing.personnelId == record.personnelId
            && existing.startDate == record.startDate
            && existing.endDate == record.endDate;
        if unchanged {
            return Ok(());
        }
        if protected_by_finalized_history(&conn, &existing)? {
            return Err(
                "Bu rapor kaydı kesinleşmiş bordro zincirinin yıllık rapor sıralamasında kullanıldığından değiştirilemez."
                    .into(),
            );
        }
    }

    if protected_by_finalized_history(&conn, &record)? {
        return Err(
            "Bu tarihe geriye dönük rapor eklemek/değiştirmek kesinleşmiş bordro zincirinin yıllık rapor sıralamasını değiştirir."
                .into(),
        );
    }

    SickLeaveRepository::save(&conn, &record).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_sick_leave_record(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = state.lock().map_err(|e| e.to_string())?;
    let existing = SickLeaveRepository::get_by_id(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Rapor kaydı bulunamadı.".to_string())?;

    if protected_by_finalized_history(&conn, &existing)? {
        return Err(
            "Bu rapor kaydı kesinleşmiş bordro zincirinin yıllık rapor sıralamasında kullanıldığından silinemez."
                .into(),
        );
    }

    SickLeaveRepository::delete(&conn, &id).map_err(|e| e.to_string())
}
