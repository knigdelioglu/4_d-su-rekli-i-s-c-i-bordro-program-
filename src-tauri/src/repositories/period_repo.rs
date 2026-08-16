use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use chrono::{Datelike, NaiveDate};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};

pub struct PeriodRepository;

impl PeriodRepository {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<BordroDonemi> {
        Ok(BordroDonemi {
            id: row.get(0)?,
            yil: row.get(1)?,
            ay: row.get(2)?,
            baslangicTarihi: row.get(3)?,
            bitisTarihi: row.get(4)?,
            donemAdi: row.get(5)?,
            taxYear: row.get(6)?,
            taxMonth: row.get(7)?,
        })
    }

    pub fn validate_period(period: &BordroDonemi) -> Result<()> {
        if period.yil <= 0 || !(1..=12).contains(&period.ay) {
            return Err(crate::domain::DomainError::ValidationError(
                "Dönem yılı geçerli olmalı ve ayı 1-12 arasında olmalıdır.".into(),
            ));
        }
        if period.taxYear <= 0 || !(1..=12).contains(&period.taxMonth) {
            return Err(crate::domain::DomainError::ValidationError(
                "Vergi yılı geçerli olmalı ve vergi ayı 1-12 arasında olmalıdır.".into(),
            ));
        }

        let start =
            NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").map_err(|_| {
                crate::domain::DomainError::ValidationError(format!(
                    "Dönem başlangıç tarihi geçersiz: {}.",
                    period.baslangicTarihi
                ))
            })?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").map_err(|_| {
            crate::domain::DomainError::ValidationError(format!(
                "Dönem bitiş tarihi geçersiz: {}.",
                period.bitisTarihi
            ))
        })?;
        if start > end {
            return Err(crate::domain::DomainError::ValidationError(
                "Dönem başlangıç tarihi bitiş tarihinden sonra olamaz.".into(),
            ));
        }

        // Bu uygulamanın authoritative çalışma dönemi 15-14'tür. Serbest tarih
        // aralığı kabul etmek SGK gün hesabını ve dönem bazlı gelir üretimini
        // belirsiz hale getirir; invalid state hesap aşamasına kadar yaşayamaz.
        if start.day() != 15 || end.day() != 14 {
            return Err(DomainError::ValidationError(format!(
                "Bordro dönemi 15-14 olmalıdır: {} - {}.",
                period.baslangicTarihi, period.bitisTarihi
            )));
        }

        let (expected_end_year, expected_end_month) = if start.month() == 12 {
            (start.year() + 1, 1)
        } else {
            (start.year(), start.month() + 1)
        };
        if end.year() != expected_end_year || end.month() != expected_end_month {
            return Err(DomainError::ValidationError(format!(
                "Bordro dönemi başlangıç ayını izleyen ayın 14'ünde bitmelidir: {} - {}.",
                period.baslangicTarihi, period.bitisTarihi
            )));
        }

        // `ay` dönem başlangıç ayıdır; taxYear/taxMonth ise ayrı ödeme/tahakkuk
        // metadata'sıdır ve ürün sözleşmesi gereği kullanıcı tarafından seçilebilir.
        if period.yil != start.year() || period.ay != start.month() as i32 {
            return Err(DomainError::ValidationError(format!(
                "Dönem yıl/ay metadata'sı başlangıç tarihiyle uyuşmuyor: {}-{:02} / {}.",
                period.yil, period.ay, period.baslangicTarihi
            )));
        }

        Ok(())
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<BordroDonemi>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month
             FROM payroll_periods ORDER BY yil ASC, ay ASC",
            )
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], Self::from_row)
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for r in rows {
            let period = r.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            Self::validate_period(&period)?;
            result.push(period);
        }
        Ok(result)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<BordroDonemi>> {
        let period = conn
            .query_row(
                "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month
             FROM payroll_periods WHERE id = ?1",
                params![id],
                Self::from_row,
            )
            .optional()
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        period
            .map(|period| {
                Self::validate_period(&period)?;
                Ok(period)
            })
            .transpose()
    }

    pub fn get_by_tax_year_before_month(
        conn: &Connection,
        tax_year: i32,
        tax_month: i32,
    ) -> Result<Vec<BordroDonemi>> {
        let mut stmt = conn
            .prepare(
                "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month
                 FROM payroll_periods
                 WHERE tax_year = ?1 AND tax_month < ?2
                 ORDER BY tax_month ASC, id ASC",
            )
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![tax_year, tax_month], Self::from_row)
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        let mut periods = Vec::new();
        for row in rows {
            let period =
                row.map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
            Self::validate_period(&period)?;
            periods.push(period);
        }
        Ok(periods)
    }

    pub fn get_previous_by_work_period(
        conn: &Connection,
        active_period: &BordroDonemi,
    ) -> Result<Option<BordroDonemi>> {
        let period = conn
            .query_row(
                "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month
                 FROM payroll_periods
                 WHERE yil < ?1 OR (yil = ?1 AND ay < ?2)
                 ORDER BY yil DESC, ay DESC, id DESC
                 LIMIT 1",
                params![active_period.yil, active_period.ay],
                Self::from_row,
            )
            .optional()
            .map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;
        period
            .map(|period| {
                Self::validate_period(&period)?;
                Ok(period)
            })
            .transpose()
    }

    pub fn save(conn: &Connection, d: &BordroDonemi) -> Result<()> {
        Self::validate_period(d)?;
        let collision: Option<String> = conn
            .query_row(
                "SELECT id FROM payroll_periods
                 WHERE tax_year = ?1 AND tax_month = ?2 AND id <> ?3
                 LIMIT 1",
                params![d.taxYear, d.taxMonth, d.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if let Some(conflicting_id) = collision {
            return Err(DomainError::ValidationError(format!(
                "Vergi yılı/ayı çakışması: {}-{:02} zaten {} döneminde kullanılıyor.",
                d.taxYear, d.taxMonth, conflicting_id
            )));
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO payroll_periods (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                yil=?2, ay=?3, baslangic_tarihi=?4, bitis_tarihi=?5, donem_adi=?6, tax_year=?7, tax_month=?8",
            params![d.id, d.yil, d.ay, d.baslangicTarihi, d.bitisTarihi, d.donemAdi, d.taxYear, d.taxMonth, now],
        ).map_err(|e| crate::domain::DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
