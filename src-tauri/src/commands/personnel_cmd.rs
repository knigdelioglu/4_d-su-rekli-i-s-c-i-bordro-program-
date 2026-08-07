use crate::db::DbState;
use crate::domain::models::*;
use crate::domain::Result;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use tauri::State;

#[tauri::command]
pub fn get_personnel_list(db: State<'_, DbState>) -> Result<Vec<Personel>> {
    let conn = db.lock().unwrap();
    PersonnelRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_personnel(db: State<'_, DbState>, personel: Personel) -> Result<()> {
    let conn = db.lock().unwrap();
    PersonnelRepository::save(&conn, &personel)
}

#[tauri::command]
pub fn delete_personnel(db: State<'_, DbState>, id: String) -> Result<()> {
    let conn = db.lock().unwrap();
    PersonnelRepository::delete(&conn, &id)
}

#[tauri::command]
pub fn get_tax_openings(db: State<'_, DbState>) -> Result<Vec<PersonelTaxOpening>> {
    let conn = db.lock().unwrap();
    TaxOpeningRepository::get_all(&conn)
}

#[tauri::command]
pub fn save_tax_opening(db: State<'_, DbState>, tax_opening: PersonelTaxOpening) -> Result<()> {
    let conn = db.lock().unwrap();
    TaxOpeningRepository::save(&conn, &tax_opening)
}
