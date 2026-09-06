use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
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
    let impact = if PersonnelRepository::get_by_id(&conn, &personel.id)?.is_some() {
        // Hizmet yılı, iş primi grubu, OKS/BES ve kişi bazlı kesintiler bordro
        // girdisidir. Mevcut personel kartına yapılan kayıt, hesaplanmış bütün
        // açık bordroları yeniden hesaplanması gereken duruma getirir.
        Some(PayrollInvalidationRepository::assert_mutation_allowed(
            &conn,
            &payroll_core::PayrollMutation::Person {
                personnelId: personel.id.clone(),
            },
        )?)
    } else {
        None
    };
    PersonnelRepository::save(&conn, &personel)?;
    if let Some(impact) = impact {
        PayrollInvalidationRepository::apply_impact(&conn, &impact)?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_personnel(db: State<'_, DbState>, id: String) -> Result<()> {
    let conn = db.lock().map_err(|e| {
        DomainError::DatabaseError(format!("SQLite bağlantı kilidi alınamadı: {e}"))
    })?;
    let impact = if PersonnelRepository::get_by_id(&conn, &id)?.is_some() {
        Some(PayrollInvalidationRepository::assert_mutation_allowed(
            &conn,
            &payroll_core::PayrollMutation::Person {
                personnelId: id.clone(),
            },
        )?)
    } else {
        None
    };
    if let Some(impact) = impact.as_ref() {
        if !impact.affectedRetroBatches.is_empty() {
            return Err(DomainError::ValidationError(
                "Retro batch tarihçesi bulunan personel silinemez; audit ledger korunmalıdır."
                    .into(),
            ));
        }
    }
    PersonnelRepository::delete(&conn, &id)?;
    if let Some(impact) = impact {
        PayrollInvalidationRepository::apply_impact(&conn, &impact)?;
    }
    Ok(())
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
