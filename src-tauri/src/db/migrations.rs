use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::collections::HashSet;

// rusqlite_migration addresses the seventh entry in this list as
// user_version = 6 -> 7.  Keep the marker at 7 when that entry is repaired
// manually so the following additive migrations can continue normally.
const DEVIR_MIGRATION_VERSION: u32 = 7;
const NOTLAR_MIGRATION_VERSION: u32 = 8;
const DEVIR_MIGRATION_PREVIOUS_VERSION: u32 = 5;

const DEVIR_COLUMNS: [(&str, &str); 5] = [
    ("devir_kumulatif_gv_matrahi", "INTEGER DEFAULT 0"),
    ("devir_kumulatif_gv_matrahi_yili", "INTEGER"),
    ("devir_kumulatif_gv_matrahi_baslangic_ayi", "INTEGER"),
    ("devir_kumulatif_asgari_gv_matrahi", "INTEGER DEFAULT 0"),
    ("devir_kumulatif_asgari_gv_matrahi_yili", "INTEGER"),
];

pub fn get_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            r#"
            CREATE TABLE IF NOT EXISTS personnel (
                id TEXT PRIMARY KEY,
                tc_no TEXT UNIQUE NOT NULL,
                ad TEXT NOT NULL,
                soyad TEXT NOT NULL,
                grup TEXT NOT NULL,
                unvan TEXT,
                sgk_sicil_no TEXT NOT NULL DEFAULT '',
                iban TEXT NOT NULL DEFAULT '',
                hizmet_yili INTEGER NOT NULL DEFAULT 0,
                aciklama TEXT,
                sendika_uyesi INTEGER NOT NULL DEFAULT 0,
                sabit_sendika_aidati INTEGER DEFAULT 0,
                bes_uyesi INTEGER NOT NULL DEFAULT 0,
                oks_orani_yuzde INTEGER DEFAULT 0,
                sabit_bes_tutar INTEGER DEFAULT 0,
                icra_tutar INTEGER DEFAULT 0,
                kisi_borcu_tutar INTEGER DEFAULT 0,
                dogum_askerlik_borclanmasi_tutar INTEGER DEFAULT 0,
                hayat_saglik_sigortasi_tutar INTEGER DEFAULT 0,
                diger_kesinti_tutar INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS payroll_periods (
                id TEXT PRIMARY KEY,
                yil INTEGER NOT NULL,
                ay INTEGER NOT NULL,
                baslangic_tarihi TEXT NOT NULL,
                bitis_tarihi TEXT NOT NULL,
                donem_adi TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS attendance_records (
                id TEXT PRIMARY KEY,
                personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                attendance_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CONSTRAINT unique_personnel_period_attendance UNIQUE(personnel_id, period_id)
            );

            CREATE TABLE IF NOT EXISTS personnel_tax_opening (
                id TEXT PRIMARY KEY,
                personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                year INTEGER NOT NULL,
                gv_cumulative_opening INTEGER NOT NULL,
                effective_from_period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CONSTRAINT unique_personnel_tax_opening_year UNIQUE(personnel_id, year)
            );

            CREATE TABLE IF NOT EXISTS payroll_records (
                id TEXT PRIMARY KEY,
                personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                gross_total INTEGER NOT NULL,
                sgk_base INTEGER NOT NULL,
                gv_base INTEGER NOT NULL,
                previous_cumulative_gv INTEGER NOT NULL,
                new_cumulative_gv INTEGER NOT NULL,
                income_tax INTEGER NOT NULL,
                stamp_tax INTEGER NOT NULL,
                total_deductions INTEGER NOT NULL,
                net_payment INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'CALCULATED',
                puantaj_summary_json TEXT NOT NULL,
                pek_detail_json TEXT,
                devreden_pek_gelen_json TEXT,
                sonraki_devreden_pek_json TEXT,
                calculated_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CONSTRAINT unique_personnel_period_payroll UNIQUE(personnel_id, period_id)
            );

            CREATE TABLE IF NOT EXISTS payroll_income_items (
                id TEXT PRIMARY KEY,
                payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
                item_type TEXT NOT NULL,
                description TEXT NOT NULL,
                amount INTEGER NOT NULL,
                source TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS payroll_deduction_items (
                id TEXT PRIMARY KEY,
                payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
                item_type TEXT NOT NULL,
                description TEXT NOT NULL,
                amount INTEGER NOT NULL,
                source TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS institution_settings (
                period_id TEXT PRIMARY KEY REFERENCES payroll_periods(id) ON DELETE CASCADE,
                settings_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS annual_payroll_parameters (
                year INTEGER PRIMARY KEY,
                params_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sick_leave_records (
                id TEXT PRIMARY KEY,
                personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_payroll_records_personnel ON payroll_records(personnel_id);
            CREATE INDEX IF NOT EXISTS idx_payroll_records_period ON payroll_records(period_id);
            CREATE INDEX IF NOT EXISTS idx_attendance_records_period ON attendance_records(period_id);
            CREATE INDEX IF NOT EXISTS idx_tax_opening_personnel_year ON personnel_tax_opening(personnel_id, year);
            CREATE INDEX IF NOT EXISTS idx_sick_leave_personnel ON sick_leave_records(personnel_id);
            CREATE INDEX IF NOT EXISTS idx_sick_leave_start_date ON sick_leave_records(start_date);
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN raporlu_gun INTEGER;
            ALTER TABLE payroll_records ADD COLUMN odenen_raporlu_gun INTEGER;
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN is_primi_snapshot_json TEXT;
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN gv_snapshot_json TEXT;
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_periods ADD COLUMN tax_year INTEGER;
            ALTER TABLE payroll_periods ADD COLUMN tax_month INTEGER;

            UPDATE payroll_periods
               SET tax_month = CASE
                                    WHEN ay = 12 THEN 1
                                    ELSE ay + 1
                                END,
                   tax_year  = yil + CASE
                                         WHEN ay = 12 THEN 1
                                         ELSE 0
                                     END
             WHERE tax_month IS NULL OR tax_year IS NULL;

            UPDATE payroll_periods SET tax_month = COALESCE(tax_month, 1);
            UPDATE payroll_periods SET tax_year = COALESCE(tax_year, yil);
            "#,
        ),
        // Migration 1 originally declared this table, but some databases were
        // already at version 5 before that declaration was added to the
        // initial schema. Keep the repair idempotent for those installations.
        M::up(
            r#"
            CREATE TABLE IF NOT EXISTS sick_leave_records (
                id TEXT PRIMARY KEY,
                personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sick_leave_personnel ON sick_leave_records(personnel_id);
            CREATE INDEX IF NOT EXISTS idx_sick_leave_start_date ON sick_leave_records(start_date);
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE personnel ADD COLUMN devir_kumulatif_gv_matrahi INTEGER DEFAULT 0;
            ALTER TABLE personnel ADD COLUMN devir_kumulatif_gv_matrahi_yili INTEGER;
            ALTER TABLE personnel ADD COLUMN devir_kumulatif_gv_matrahi_baslangic_ayi INTEGER;
            ALTER TABLE personnel ADD COLUMN devir_kumulatif_asgari_gv_matrahi INTEGER DEFAULT 0;
            ALTER TABLE personnel ADD COLUMN devir_kumulatif_asgari_gv_matrahi_yili INTEGER;
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN notlar TEXT;
            "#,
        ),
        M::up(
            r#"
            INSERT OR IGNORE INTO annual_payroll_parameters (year, params_json, updated_at)
            VALUES (
                2026,
                '{"year":2026,"gelirVergisiDilimleri":[{"limit":190000,"oran":0.15},{"limit":400000,"oran":0.20},{"limit":1500000,"oran":0.27},{"limit":5300000,"oran":0.35},{"limit":1000000000000000,"oran":0.40}]}',
                CURRENT_TIMESTAMP
            );
            "#,
        ),
        M::up(
            r#"
            ALTER TABLE payroll_records ADD COLUMN statutory_snapshot_json TEXT;
            "#,
        ),
    ])
}

fn table_columns(
    conn: &Connection,
    table: &str,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let pragma = match table {
        "personnel" => "PRAGMA table_info(personnel)",
        "payroll_periods" => "PRAGMA table_info(payroll_periods)",
        "payroll_records" => "PRAGMA table_info(payroll_records)",
        _ => return Err(format!("Desteklenmeyen migration tablosu: {table}").into()),
    };
    let mut stmt = conn.prepare(pragma)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row?);
    }
    Ok(columns)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    columns: &mut HashSet<String>,
    column: &str,
    definition: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if columns.insert(column.to_string()) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

/// Some pre-release databases were marked as migration 5 while already
/// containing one or more devir columns. Migration 6's plain ALTER statements
/// are not safe for that shape. Complete the set and advance only that
/// migration marker; all other migration steps retain their normal order.
fn prepare_devir_migration(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let user_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if !(DEVIR_MIGRATION_PREVIOUS_VERSION..DEVIR_MIGRATION_VERSION).contains(&user_version) {
        return Ok(());
    }

    let existing_columns = table_columns(conn, "personnel")?;
    if !DEVIR_COLUMNS
        .iter()
        .any(|(column, _)| existing_columns.contains(*column))
    {
        return Ok(());
    }

    let payroll_columns = table_columns(conn, "payroll_records")?;
    let repaired_version = if payroll_columns.contains("notlar") {
        NOTLAR_MIGRATION_VERSION
    } else {
        DEVIR_MIGRATION_VERSION
    };

    let tx = conn.transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS sick_leave_records (
            id TEXT PRIMARY KEY,
            personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
            start_date TEXT NOT NULL,
            end_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sick_leave_personnel ON sick_leave_records(personnel_id);
        CREATE INDEX IF NOT EXISTS idx_sick_leave_start_date ON sick_leave_records(start_date);",
    )?;
    let mut columns = existing_columns;
    for (column, definition) in DEVIR_COLUMNS {
        add_column_if_missing(&tx, "personnel", &mut columns, column, definition)?;
    }
    tx.pragma_update(None, "user_version", repaired_version)?;
    tx.commit()?;
    Ok(())
}

/// Repairs additive columns for databases whose user_version was advanced by
/// an interrupted/alternate migration path. Existing columns and data are
/// preserved; only missing columns are added.
fn ensure_optional_columns(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let tx = conn.transaction()?;

    let mut personnel_columns = table_columns(&tx, "personnel")?;
    for (column, definition) in DEVIR_COLUMNS {
        add_column_if_missing(&tx, "personnel", &mut personnel_columns, column, definition)?;
    }

    let mut payroll_columns = table_columns(&tx, "payroll_records")?;
    for (column, definition) in [
        ("raporlu_gun", "INTEGER"),
        ("odenen_raporlu_gun", "INTEGER"),
        ("is_primi_snapshot_json", "TEXT"),
        ("gv_snapshot_json", "TEXT"),
        ("statutory_snapshot_json", "TEXT"),
        ("notlar", "TEXT"),
    ] {
        add_column_if_missing(
            &tx,
            "payroll_records",
            &mut payroll_columns,
            column,
            definition,
        )?;
    }

    let mut period_columns = table_columns(&tx, "payroll_periods")?;
    add_column_if_missing(
        &tx,
        "payroll_periods",
        &mut period_columns,
        "tax_year",
        "INTEGER",
    )?;
    add_column_if_missing(
        &tx,
        "payroll_periods",
        &mut period_columns,
        "tax_month",
        "INTEGER",
    )?;
    tx.execute_batch(
        "UPDATE payroll_periods
            SET tax_month = CASE WHEN ay = 12 THEN 1 ELSE ay + 1 END,
                tax_year = yil + CASE WHEN ay = 12 THEN 1 ELSE 0 END
          WHERE tax_month IS NULL OR tax_year IS NULL;
         UPDATE payroll_periods SET tax_month = COALESCE(tax_month, 1);
         UPDATE payroll_periods SET tax_year = COALESCE(tax_year, yil);",
    )?;

    tx.commit()?;
    Ok(())
}

pub fn initialize_db(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    prepare_devir_migration(conn)?;
    let migrations = get_migrations();
    migrations.to_latest(conn)?;
    ensure_optional_columns(conn)?;
    Ok(())
}
