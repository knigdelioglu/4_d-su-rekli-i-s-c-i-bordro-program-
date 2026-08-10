use super::{dec_to_kurus, kurus_to_dec};
use crate::domain::models::*;
use crate::domain::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

pub struct TaxOpeningRepository;

impl TaxOpeningRepository {
    pub fn get_all(conn: &Connection) -> Result<Vec<PersonelTaxOpening>> {
        let mut stmt = conn.prepare(
            "SELECT id, personnel_id, year, gv_cumulative_opening, effective_from_period_id, created_at, updated_at
             FROM personnel_tax_opening",
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let opening_kurus: i64 = row.get(3)?;
                Ok(PersonelTaxOpening {
                    id: row.get(0)?,
                    personnelId: row.get(1)?,
                    year: row.get(2)?,
                    gvCumulativeOpening: kurus_to_dec(opening_kurus),
                    effectiveFromPeriodId: row.get(4)?,
                    createdAt: row.get(5)?,
                    updatedAt: row.get(6)?,
                })
            })
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn get_by_personnel_and_year(
        conn: &Connection,
        personnel_id: &str,
        year: i32,
    ) -> Result<Option<PersonelTaxOpening>> {
        conn.query_row(
            "SELECT id, personnel_id, year, gv_cumulative_opening, effective_from_period_id, created_at, updated_at
             FROM personnel_tax_opening WHERE personnel_id = ?1 AND year = ?2",
            params![personnel_id, year],
            |row| {
                let opening_kurus: i64 = row.get(3)?;
                Ok(PersonelTaxOpening {
                    id: row.get(0)?,
                    personnelId: row.get(1)?,
                    year: row.get(2)?,
                    gvCumulativeOpening: kurus_to_dec(opening_kurus),
                    effectiveFromPeriodId: row.get(4)?,
                    createdAt: row.get(5)?,
                    updatedAt: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))
    }

    pub fn save(conn: &Connection, t: &PersonelTaxOpening) -> Result<()> {
        if t.year <= 0 || t.gvCumulativeOpening < rust_decimal::Decimal::ZERO {
            return Err(crate::domain::DomainError::ValidationError(
                "Vergi açılışı geçerli bir yıl ve negatif olmayan bir matrah içermelidir.".into(),
            ));
        }
        if t.effectiveFromPeriodId.trim().is_empty() {
            return Err(crate::domain::DomainError::ValidationError(
                "Vergi açılışı için başlangıç dönemi zorunludur.".into(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let opening_kurus = dec_to_kurus(Some(t.gvCumulativeOpening));

        conn.execute(
            "INSERT INTO personnel_tax_opening (id, personnel_id, year, gv_cumulative_opening, effective_from_period_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(personnel_id, year) DO UPDATE SET
                gv_cumulative_opening=?4, effective_from_period_id=?5, updated_at=?7",
            params![t.id, t.personnelId, t.year, opening_kurus, t.effectiveFromPeriodId, now, now],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    pub fn delete(conn: &Connection, id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM personnel_tax_opening WHERE id = ?1",
            params![id],
        )
        .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
