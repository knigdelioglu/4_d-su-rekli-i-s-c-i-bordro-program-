use crate::domain::Result;
use crate::services::payroll_service::PayrollService;
use rusqlite::Connection;

/// Compatibility façade for existing native callers. The preflight rules live
/// exclusively in `payroll-core`; this type no longer contains a second DB
/// implementation of the validation algorithm.
pub struct PayrollPreflightService;

impl PayrollPreflightService {
    pub fn validate_for_calculation(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<()> {
        PayrollService::validate_payroll_request(conn, personnel_id, period_id)
    }
}
