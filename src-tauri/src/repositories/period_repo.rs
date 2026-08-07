use crate::domain::models::*;
use crate::domain::Result;
use rusqlite::{params, Connection};
use chrono::Utc;

pub struct PeriodRepository;

impl PeriodRepository {
    pub fn get_all(conn: &Connection) -> Result<Vec<BordroDonemi>> {
        let mut stmt = conn.prepare(
            "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi
             FROM payroll_periods ORDER BY yil ASC, ay ASC",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map([], |row| {
            Ok(BordroDonemi {
                id: row.get(0)?,
                yil: row.get(1)?,
                ay: row.get(2)?,
                baslangicTarihi: row.get(3)?,
                bitisTarihi: row.get(4)?,
                donemAdi: row.get(5)?,
            })
        }).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<BordroDonemi>> {
        let all = Self::get_all(conn)?;
        Ok(all.into_iter().find(|d| d.id == id))
    }

    pub fn save(conn: &Connection, d: &BordroDonemi) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO payroll_periods (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                yil=?2, ay=?3, baslangic_tarihi=?4, bitis_tarihi=?5, donem_adi=?6",
            params![d.id, d.yil, d.ay, d.baslangicTarihi, d.bitisTarihi, d.donemAdi, now],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
