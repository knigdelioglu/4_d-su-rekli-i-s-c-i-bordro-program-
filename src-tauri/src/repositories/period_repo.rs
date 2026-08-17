use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
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

    fn tax_ordinal(year: i32, month: i32) -> i64 {
        i64::from(year) * 12 + i64::from(month)
    }

    fn latest_finalized_period_start(conn: &Connection) -> Result<Option<String>> {
        conn.query_row(
            "SELECT MAX(pp.baslangic_tarihi)
             FROM payroll_records AS pr
             JOIN payroll_periods AS pp ON pp.id = pr.period_id
             WHERE pr.status = 'FINALIZED'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
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

    /// Production bordro hesabında vergi ayı 15-14 çalışma döneminin başlangıç
    /// veya bitiş takvim ayıyla örtüşmelidir. Bu kontrol hesaplama preflight'ında
    /// çağrılır; repository save ise eski fixture/legacy kayıtlarını salt bu nedenle
    /// kullanılamaz hale getirmez.
    pub fn validate_tax_month_overlap(period: &BordroDonemi) -> Result<()> {
        Self::validate_period(period)?;
        let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .map_err(|e| DomainError::ValidationError(e.to_string()))?;
        let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .map_err(|e| DomainError::ValidationError(e.to_string()))?;
        let tax_matches_start = period.taxYear == start.year()
            && period.taxMonth == start.month() as i32;
        let tax_matches_end =
            period.taxYear == end.year() && period.taxMonth == end.month() as i32;
        if !tax_matches_start && !tax_matches_end {
            return Err(DomainError::ValidationError(format!(
                "Vergi yılı/ayı {}-{:02}, {}–{} çalışma dönemiyle örtüşmüyor. Vergi ayı dönemin başlangıç veya bitiş ayı olmalıdır.",
                period.taxYear, period.taxMonth, period.baslangicTarihi, period.bitisTarihi
            )));
        }
        Ok(())
    }

    /// PEK çalışma kronolojisine, kümülatif vergi ise taxYear/taxMonth
    /// kronolojisine bağlıdır. Bu iki sıranın ters düşmesine izin vermek dairesel
    /// bağımlılık doğurur. Çalışma dönemi ilerledikçe vergi sırası da strictly
    /// ileri gitmek zorundadır.
    pub fn validate_tax_chronology(conn: &Connection, period: &BordroDonemi) -> Result<()> {
        Self::validate_period(period)?;
        let current_key = Self::tax_ordinal(period.taxYear, period.taxMonth);

        let previous = conn
            .query_row(
                "SELECT id, tax_year, tax_month
                 FROM payroll_periods
                 WHERE id <> ?1 AND baslangic_tarihi < ?2
                 ORDER BY baslangic_tarihi DESC, id DESC
                 LIMIT 1",
                params![period.id, period.baslangicTarihi],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if let Some((previous_id, year, month)) = previous {
            if Self::tax_ordinal(year, month) >= current_key {
                return Err(DomainError::ValidationError(format!(
                    "Vergi kronolojisi çalışma dönemiyle ters düşüyor: önceki {} dönemi {}-{:02}, {} dönemi ise {}-{:02}. Vergi ayları çalışma sırasıyla ileri gitmelidir.",
                    previous_id, year, month, period.id, period.taxYear, period.taxMonth
                )));
            }
        }

        let next = conn
            .query_row(
                "SELECT id, tax_year, tax_month
                 FROM payroll_periods
                 WHERE id <> ?1 AND baslangic_tarihi > ?2
                 ORDER BY baslangic_tarihi ASC, id ASC
                 LIMIT 1",
                params![period.id, period.baslangicTarihi],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, i32>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if let Some((next_id, year, month)) = next {
            if current_key >= Self::tax_ordinal(year, month) {
                return Err(DomainError::ValidationError(format!(
                    "Vergi kronolojisi çalışma dönemiyle ters düşüyor: {} dönemi {}-{:02}, sonraki {} dönemi ise {}-{:02}. Vergi ayları çalışma sırasıyla ileri gitmelidir.",
                    period.id, period.taxYear, period.taxMonth, next_id, year, month
                )));
            }
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
        Self::validate_period(active_period)?;

        let mut stmt = conn
            .prepare(
                "SELECT id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month
                 FROM payroll_periods
                 WHERE baslangic_tarihi < ?1
                 ORDER BY baslangic_tarihi DESC, id DESC
                 LIMIT 2",
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        let rows = stmt
            .query_map(params![active_period.baslangicTarihi], Self::from_row)
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut candidates = Vec::new();
        for row in rows {
            let period = row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            Self::validate_period(&period)?;
            candidates.push(period);
        }

        if candidates.len() >= 2
            && candidates[0].baslangicTarihi == candidates[1].baslangicTarihi
        {
            return Err(DomainError::InvalidData(format!(
                "Önceki çalışma dönemi belirsiz: {} başlangıç tarihine sahip birden fazla dönem var.",
                candidates[0].baslangicTarihi
            )));
        }

        Ok(candidates.into_iter().next())
    }

    pub fn save(conn: &Connection, d: &BordroDonemi) -> Result<()> {
        Self::validate_period(d)?;
        Self::validate_tax_chronology(conn, d)?;

        let latest_finalized_start = Self::latest_finalized_period_start(conn)?;
        let existing = Self::get_by_id(conn, &d.id)?;
        let causal_fields_changed = existing.as_ref().is_some_and(|old| {
            old.yil != d.yil
                || old.ay != d.ay
                || old.baslangicTarihi != d.baslangicTarihi
                || old.bitisTarihi != d.bitisTarihi
                || old.taxYear != d.taxYear
                || old.taxMonth != d.taxMonth
        });

        match existing.as_ref() {
            Some(old) => {
                if causal_fields_changed {
                    if let Some(latest) = latest_finalized_start.as_deref() {
                        let earliest_affected = if old.baslangicTarihi <= d.baslangicTarihi {
                            old.baslangicTarihi.as_str()
                        } else {
                            d.baslangicTarihi.as_str()
                        };
                        if latest >= earliest_affected {
                            return Err(DomainError::PayrollFinalized(
                                "Kesinleştirilmiş bordro tarihçesini etkileyen dönem yıl/ay, tarih veya vergi metadata'sı değiştirilemez."
                                    .into(),
                            ));
                        }
                    }
                }
            }
            None => {
                if latest_finalized_start
                    .as_deref()
                    .is_some_and(|latest| latest >= d.baslangicTarihi.as_str())
                {
                    return Err(DomainError::PayrollFinalized(
                        "Kesinleştirilmiş bordro tarihçesine geriye dönük yeni çalışma dönemi eklenemez."
                            .into(),
                    ));
                }
            }
        }

        let work_period_collision: Option<String> = conn
            .query_row(
                "SELECT id FROM payroll_periods
                 WHERE yil = ?1 AND ay = ?2 AND id <> ?3
                 LIMIT 1",
                params![d.yil, d.ay, d.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if let Some(conflicting_id) = work_period_collision {
            return Err(DomainError::ValidationError(format!(
                "Çalışma dönemi çakışması: {}-{:02} zaten {} döneminde tanımlı.",
                d.yil, d.ay, conflicting_id
            )));
        }

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

        if let Some(old) = existing.as_ref() {
            if causal_fields_changed {
                PayrollInvalidationRepository::mark_from_period_position_stale(
                    conn,
                    &old.baslangicTarihi,
                    old.taxYear,
                    old.taxMonth,
                )?;
                PayrollInvalidationRepository::mark_from_period_position_stale(
                    conn,
                    &d.baslangicTarihi,
                    d.taxYear,
                    d.taxMonth,
                )?;
            }
        } else {
            // Yeni bir geçmiş/ara dönem, önceden hesaplanmış sonraki bordroların
            // vergi veya PEK zincirine yeni bir düğüm ekleyebilir.
            PayrollInvalidationRepository::mark_from_period_position_stale(
                conn,
                &d.baslangicTarihi,
                d.taxYear,
                d.taxMonth,
            )?;
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
