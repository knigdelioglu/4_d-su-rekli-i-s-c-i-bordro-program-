use crate::domain::{DomainError, Result};
use chrono::Utc;
use rusqlite::{params, Connection};

/// Bordro girdileri değiştiğinde daha önce hesaplanmış sonuçların sessizce
/// authoritative kalmasını engelleyen merkezi STALE yayılımı.
///
/// Bu katman geçmiş bordroları yeniden üretmeye çalışmaz. Excel benzeri çalışma
/// sözleşmesi basittir: girdi değişirse ilgili CALCULATED bordro yeniden
/// hesaplanmadan yazdırılmaya/kesinleştirilmeye hazır değildir.
pub struct PayrollInvalidationRepository;

impl PayrollInvalidationRepository {
    pub fn mark_person_period_and_dependents_stale(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE personnel_id = ?2
               AND status = 'CALCULATED'
               AND period_id IN (
                    SELECT candidate.id
                    FROM payroll_periods AS candidate
                    JOIN payroll_periods AS current ON current.id = ?3
                    WHERE candidate.id = current.id
                       OR candidate.baslangic_tarihi > current.baslangic_tarihi
                       OR (
                            candidate.tax_year = current.tax_year
                            AND candidate.tax_month > current.tax_month
                       )
               )",
            params![now, personnel_id, period_id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    pub fn mark_period_and_dependents_stale(conn: &Connection, period_id: &str) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE status = 'CALCULATED'
               AND period_id IN (
                    SELECT candidate.id
                    FROM payroll_periods AS candidate
                    JOIN payroll_periods AS current ON current.id = ?2
                    WHERE candidate.id = current.id
                       OR candidate.baslangic_tarihi > current.baslangic_tarihi
                       OR (
                            candidate.tax_year = current.tax_year
                            AND candidate.tax_month > current.tax_month
                       )
               )",
            params![now, period_id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    /// Dönem henüz INSERT edilmemişken veya dönem konumu değişirken kullanılır.
    /// Hem çalışma kronolojisi hem aynı vergi yılındaki vergi kronolojisi korunur.
    pub fn mark_from_period_position_stale(
        conn: &Connection,
        start_date: &str,
        tax_year: i32,
        tax_month: i32,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE status = 'CALCULATED'
               AND period_id IN (
                    SELECT candidate.id
                    FROM payroll_periods AS candidate
                    WHERE candidate.baslangic_tarihi >= ?2
                       OR (
                            candidate.tax_year = ?3
                            AND candidate.tax_month >= ?4
                       )
               )",
            params![now, start_date, tax_year, tax_month],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    pub fn mark_personnel_stale(conn: &Connection, personnel_id: &str) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE personnel_id = ?2 AND status = 'CALCULATED'",
            params![now, personnel_id],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    pub fn mark_personnel_tax_year_stale(
        conn: &Connection,
        personnel_id: &str,
        tax_year: i32,
    ) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE personnel_id = ?2
               AND status = 'CALCULATED'
               AND period_id IN (
                    SELECT id FROM payroll_periods WHERE tax_year = ?3
               )",
            params![now, personnel_id, tax_year],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    pub fn mark_tax_year_stale(conn: &Connection, tax_year: i32) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE status = 'CALCULATED'
               AND period_id IN (
                    SELECT id FROM payroll_periods WHERE tax_year = ?2
               )",
            params![now, tax_year],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }

    pub fn mark_all_calculated_stale(conn: &Connection) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE payroll_records
             SET status = 'STALE', updated_at = ?1
             WHERE status = 'CALCULATED'",
            params![now],
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }
}
