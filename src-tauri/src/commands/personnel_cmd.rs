use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn get_personnel_list(db: State<'_, DbState>) -> Result<Vec<Personel>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PersonnelRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_personnel(db: State<'_, DbState>, personel: Personel) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PersonnelRepository::save(&conn, &personel)
}

#[tauri::command]
pub fn delete_personnel(db: State<'_, DbState>, id: String) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    PersonnelRepository::delete(&conn, &id)
}

#[tauri::command]
pub fn get_tax_openings(db: State<'_, DbState>) -> Result<Vec<PersonelTaxOpening>> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    TaxOpeningRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_tax_opening(db: State<'_, DbState>, tax_opening: PersonelTaxOpening) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;

    let finalized_exists: i64 = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM payroll_records AS pr
                JOIN payroll_periods AS pp ON pp.id = pr.period_id
                WHERE pr.personnel_id = ?1
                  AND pp.tax_year = ?2
                  AND pr.status = 'FINALIZED'
             )",
            params![tax_opening.personnelId, tax_opening.year],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

    if finalized_exists != 0 {
        // Treat an exact re-save as an idempotent no-op. Any creation or real
        // mutation after the tax chain has a FINALIZED payroll would rewrite the
        // opening basis used by later cumulative tax calculations.
        if let Some(existing) = TaxOpeningRepository::get_by_personnel_and_year(
            &conn,
            &tax_opening.personnelId,
            tax_opening.year,
        )? {
            if existing.gvCumulativeOpening == tax_opening.gvCumulativeOpening
                && existing.effectiveFromPeriodId == tax_opening.effectiveFromPeriodId
            {
                return Ok(());
            }
        }

        return Err(DomainError::PayrollFinalized(
            "Bu personelin ilgili vergi yılında kesinleşmiş bordrosu bulunduğundan kümülatif vergi açılışı değiştirilemez."
                .into(),
        ));
    }

    TaxOpeningRepository::save(&conn, &tax_opening)
}
