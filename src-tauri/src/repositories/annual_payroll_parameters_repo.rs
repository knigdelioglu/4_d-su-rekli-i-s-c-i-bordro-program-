use crate::domain::models::AnnualPayrollParameters;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::transaction::with_transaction;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

pub struct AnnualPayrollParametersRepository;

impl AnnualPayrollParametersRepository {
    fn validate(parameters: &AnnualPayrollParameters) -> Result<()> {
        payroll_core::validate_annual_payroll_parameters(parameters)
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
        with_transaction(conn, |tx| Self::save_in_transaction(tx, parameters))
    }

    /// Caller-owned transaction variant used by backup restore.
    pub fn save_in_transaction(
        conn: &Connection,
        parameters: &AnnualPayrollParameters,
    ) -> Result<()> {
        Self::validate(parameters)?;

        // `updatedAt` DB metadata'sıdır; domain parametresi değildir. JSON içinde
        // timestamp değişmesi no-op bir kaydı gerçek mevzuat değişikliği gibi
        // gösterip bordroları gereksiz STALE yapmamalıdır.
        let mut persisted = parameters.clone();
        persisted.updatedAt = None;
        let params_json = serde_json::to_string(&persisted).map_err(|e| {
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
        let changed = previous_json.as_deref() != Some(params_json.as_str());
        let impact = changed
            .then(|| {
                PayrollInvalidationRepository::assert_mutation_allowed(
                    conn,
                    &payroll_core::PayrollMutation::TaxYear {
                        taxYear: parameters.year,
                    },
                )
            })
            .transpose()?;

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO annual_payroll_parameters (year, params_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(year) DO UPDATE SET params_json = ?2, updated_at = ?3",
            params![parameters.year, params_json, now],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        if let Some(impact) = impact {
            PayrollInvalidationRepository::apply_impact(conn, &impact)?;
        }

        Ok(())
    }
}
