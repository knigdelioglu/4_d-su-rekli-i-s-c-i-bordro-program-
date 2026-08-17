use rusqlite::{Connection, OptionalExtension};
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

/// Work-period identity is part of the accounting timeline. 15-14 geometry
/// makes `(yil, ay)` and `(baslangic_tarihi, bitis_tarihi)` authoritative
/// identities, not user-editable labels. Refuse to guess which row is correct
/// if an old database already contains duplicates; silently picking one would
/// make previous-period / deferred-PEK resolution non-deterministic.
fn ensure_temporal_constraints(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let duplicate_work_month: Option<(i32, i32, i64)> = conn
        .query_row(
            "SELECT yil, ay, COUNT(*)
             FROM payroll_periods
             GROUP BY yil, ay
             HAVING COUNT(*) > 1
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if let Some((year, month, count)) = duplicate_work_month {
        return Err(format!(
            "payroll_periods çalışma dönemi çakışması: {year}-{month:02} için {count} kayıt var; otomatik seçim yapılmayacak."
        )
        .into());
    }

    let duplicate_date_range: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT baslangic_tarihi, bitis_tarihi, COUNT(*)
             FROM payroll_periods
             GROUP BY baslangic_tarihi, bitis_tarihi
             HAVING COUNT(*) > 1
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if let Some((start, end, count)) = duplicate_date_range {
        return Err(format!(
            "payroll_periods tarih aralığı çakışması: {start}–{end} için {count} kayıt var; otomatik seçim yapılmayacak."
        )
        .into());
    }

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_periods_work_year_month
         ON payroll_periods(yil, ay)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_periods_work_date_range
         ON payroll_periods(baslangic_tarihi, bitis_tarihi)",
        [],
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
    ensure_temporal_constraints(&conn)?;
    Ok(conn)
}

pub fn create_in_memory_connection() -> Result<Connection, Box<dyn std::error::Error>> {
    let mut conn = Connection::open_in_memory()?;
    super::migrations::initialize_db(&mut conn)?;
    ensure_temporal_constraints(&conn)?;
    Ok(conn)
}
