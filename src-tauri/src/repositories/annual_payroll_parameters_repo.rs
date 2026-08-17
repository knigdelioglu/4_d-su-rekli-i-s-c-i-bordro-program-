use crate::domain::models::{AnnualPayrollParameters, TaxBracket, OPEN_ENDED_TAX_BRACKET_LIMIT};
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

pub struct AnnualPayrollParametersRepository;

impl AnnualPayrollParametersRepository {
    fn validate(parameters: &AnnualPayrollParameters) -> Result<()> {
        if parameters.year <= 0 || parameters.gelirVergisiDilimleri.is_empty() {
            return Err(DomainError::ValidationError(
                "Yıllık bordro parametresi geçerli bir yıl ve en az bir vergi dilimi içermelidir."
                    .into(),
            ));
        }

        if parameters
            .sigortaGvYillikBrutAsgariUcretTavani
            .is_some_and(|value| value <= Decimal::ZERO)
        {
            return Err(DomainError::ValidationError(
                "Sigorta GV yıllık tavanı sıfırdan büyük olmalıdır.".into(),
            ));
        }

        let mut previous_limit = rust_decimal_macros::dec!(0);
        let max_persisted_limit = Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT);
        for TaxBracket { limit, oran } in &parameters.gelirVergisiDilimleri {
            if *limit <= previous_limit
                || *limit > max_persisted_limit
                || *oran < rust_decimal_macros::dec!(0)
                || *oran > rust_decimal_macros::dec!(1)
            {
                return Err(DomainError::ValidationError(
                    "Yıllık gelir vergisi dilimleri artan, SQLite'a sığan limitlere ve 0-1 arası oranlara sahip olmalıdır."
                        .into(),
                ));
            }
            previous_limit = *limit;
        }

        // The last bracket is semantically open-ended in the calculation
        // engine. Its persistence boundary must remain finite and safe for the
        // serde-float/SQLite JSON contract; Decimal::MAX is not a valid value.
        let last_limit = parameters
            .gelirVergisiDilimleri
            .last()
            .map(|bracket| bracket.limit)
            .ok_or_else(|| {
                DomainError::ValidationError(
                    "Yıllık gelir vergisi parametresinin son dilimi zorunludur.".into(),
                )
            })?;
        if last_limit > max_persisted_limit {
            return Err(DomainError::ValidationError(
                "Yıllık gelir vergisi son dilim sınırı SQLite-safe üst sınırı aşamaz.".into(),
            ));
        }

        Ok(())
    }

    fn parse_row(
        year: i32,
        params_json: &str,
        updated_at: String,
    ) -> Result<AnnualPayrollParameters> {
        let mut parameters: AnnualPayrollParameters =
            serde_json::from_str(params_json).map_err(|e| {
                DomainError::InvalidData(format!(
                    "{} yılı yıllık bordro parametreleri bozuk JSON içeriyor: {}",
                    year, e
                ))
            })?;

        // 2026 eski kayıtları bu alan eklenmeden önce persist edilmiş olabilir.
        // Geçmiş mevzuat arşivi oluşturmadan yalnız mevcut 2026 sözleşmesini
        // geriye uyumlu biçimde tamamlarız; gelecek yıllar açıkça girilmelidir.
        if parameters.sigortaGvYillikBrutAsgariUcretTavani.is_none() && year == 2026 {
            parameters.sigortaGvYillikBrutAsgariUcretTavani = Some(Decimal::from(396360));
        }

        if parameters.year != year {
            return Err(DomainError::InvalidData(format!(
                "Yıllık bordro parametreleri anahtarı ile payload yılı eşleşmiyor: {} / {}.",
                year, parameters.year
            )));
        }

        parameters.updatedAt = Some(updated_at);
        Self::validate(&parameters)?;
        Ok(parameters)
    }

    pub fn get_all(conn: &Connection) -> Result<Vec<AnnualPayrollParameters>> {
        let mut stmt = conn
            .prepare(
                "SELECT year, params_json, updated_at
                 FROM annual_payroll_parameters ORDER BY year ASC",
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let mut result = Vec::new();
        for row in rows {
            let (year, params_json, updated_at) =
                row.map_err(|e| DomainError::DatabaseError(e.to_string()))?;
            result.push(Self::parse_row(year, &params_json, updated_at)?);
        }
        Ok(result)
    }

    pub fn get_by_year(conn: &Connection, year: i32) -> Result<Option<AnnualPayrollParameters>> {
        let row = conn
            .query_row(
                "SELECT year, params_json, updated_at
                 FROM annual_payroll_parameters WHERE year = ?1",
                params![year],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        row.map(|(row_year, params_json, updated_at)| {
            Self::parse_row(row_year, &params_json, updated_at)
        })
        .transpose()
    }

    pub fn save(conn: &Connection, parameters: &AnnualPayrollParameters) -> Result<()> {
        Self::validate(parameters)?;
        let params_json = serde_json::to_string(parameters).map_err(|e| {
            DomainError::InvalidData(format!(
                "Yıllık bordro parametreleri serileştirilemedi: {}",
                e
            ))
        })?;
        let previous_json = conn
            .query_row(
                "SELECT params_json FROM annual_payroll_parameters WHERE year = ?1",
                params![parameters.year],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        if previous_json.as_deref() != Some(params_json.as_str()) {
            PayrollInvalidationRepository::mark_tax_year_stale(conn, parameters.year)?;
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO annual_payroll_parameters (year, params_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(year) DO UPDATE SET params_json = ?2, updated_at = ?3",
            params![parameters.year, params_json, now],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
