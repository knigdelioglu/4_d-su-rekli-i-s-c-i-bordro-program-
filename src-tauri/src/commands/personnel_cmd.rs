use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::params;
use rusqlite::OptionalExtension;
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

/// Saves the personnel compatibility fields and its canonical tax opening in
/// one SQLite transaction. A failure in either write rolls back both sides.
#[tauri::command]
pub fn save_personnel_and_tax_opening(
    db: State<'_, DbState>,
    personel: Personel,
    tax_opening: PersonelTaxOpening,
) -> Result<()> {
    if personel.id != tax_opening.personnelId {
        return Err(DomainError::ValidationError(
            "Personel ile vergi opening aynı personel kimliğini taşımalıdır.".into(),
        ));
    }
    let mut conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    let tx = conn
        .transaction()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let existing = tx
        .query_row(
            "SELECT 1 FROM personnel WHERE id = ?1",
            params![personel.id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?
        .is_some();
    let person_impact = existing
        .then(|| {
            PayrollInvalidationRepository::assert_mutation_allowed(
                &tx,
                &payroll_core::PayrollMutation::Person {
                    personnelId: personel.id.clone(),
                },
            )
        })
        .transpose()?;

    PersonnelRepository::save_in_transaction(&tx, &personel)?;
    // This call validates the canonical value/effective-period pairs and
    // applies its tax-year invalidation inside the caller-owned transaction.
    TaxOpeningRepository::save_in_transaction(&tx, &tax_opening)?;
    if let Some(impact) = person_impact {
        PayrollInvalidationRepository::apply_impact(&tx, &impact)?;
    }
    tx.commit()
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
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
    TaxOpeningRepository::save(&conn, &tax_opening)
}
