use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type DbState = Arc<Mutex<Connection>>;

pub fn get_app_dir() -> PathBuf {
    let mut dir = dirs_next().unwrap_or_else(|| PathBuf::from("."));
    dir.push("4d_bordro_data");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn dirs_next() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir().map(|h| h.join(".4d_bordro"))
}

fn ensure_integrity_indexes(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let duplicate_work_periods: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT yil, ay FROM payroll_periods GROUP BY yil, ay HAVING COUNT(*) > 1
         )",
        [],
        |row| row.get(0),
    )?;
    if duplicate_work_periods > 0 {
        return Err("Veritabanında aynı yıl/ay için birden fazla bordro dönemi var; güvenli başlatma için dönemler tekilleştirilmelidir.".into());
    }

    let duplicate_tax_periods: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT tax_year, tax_month
            FROM payroll_periods
            GROUP BY tax_year, tax_month
            HAVING COUNT(*) > 1
         )",
        [],
        |row| row.get(0),
    )?;
    if duplicate_tax_periods > 0 {
        return Err("Veritabanında aynı vergi yılı/ayı için birden fazla bordro dönemi var; kümülatif vergi hesabı güvenli değil.".into());
    }

    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_payroll_periods_work_period
             ON payroll_periods(yil, ay);
         CREATE UNIQUE INDEX IF NOT EXISTS ux_payroll_periods_tax_period
             ON payroll_periods(tax_year, tax_month);",
    )?;
    Ok(())
}

pub fn create_connection(
    db_path: Option<PathBuf>,
) -> Result<Connection, Box<dyn std::error::Error>> {
    let path = match db_path {
        Some(p) => p,
        None => {
            let app_dir = get_app_dir();
            app_dir.join("bordro.sqlite")
        }
    };

    let mut conn = Connection::open(path)?;
    super::migrations::initialize_db(&mut conn)?;
    ensure_integrity_indexes(&conn)?;
    Ok(conn)
}

pub fn create_in_memory_connection() -> Result<Connection, Box<dyn std::error::Error>> {
    let mut conn = Connection::open_in_memory()?;
    super::migrations::initialize_db(&mut conn)?;
    ensure_integrity_indexes(&conn)?;
    Ok(conn)
}
