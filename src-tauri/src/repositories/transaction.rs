use crate::domain::{DomainError, Result};
use rusqlite::{Connection, Transaction};

/// Runs one source-data mutation and its payroll invalidation in one SQLite
/// transaction.  A repository must use an explicit `*_in_transaction` method
/// when it is already called from a caller-owned transaction (for example,
/// backup restore or a multi-repository use case).
pub fn with_transaction<T, F>(conn: &Connection, operation: F) -> Result<T>
where
    F: FnOnce(&Transaction<'_>) -> Result<T>,
{
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    let value = operation(&tx)?;
    tx.commit()
        .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
    Ok(value)
}
