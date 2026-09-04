use rusqlite::{Connection, OptionalExtension};
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

fn ensure_unique_tax_year_month(conn: &Connection) -> rusqlite::Result<()> {
    let duplicate: Option<(i32, i32, i64)> = conn
        .query_row(
            "SELECT tax_year, tax_month, COUNT(*)
             FROM payroll_periods
             WHERE tax_year IS NOT NULL AND tax_month IS NOT NULL
             GROUP BY tax_year, tax_month
             HAVING COUNT(*) > 1
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((year, month, count)) = duplicate {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "payroll_periods vergi yılı/ayı çakışması: {}-{:02} için {} kayıt var; migration otomatik seçim yapmayacak.",
                    year, month, count
                ),
            ),
        )));
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_periods_tax_year_month
         ON payroll_periods(tax_year, tax_month)",
        [],
    )?;
    Ok(())
}

fn ensure_multi_accrual_schema(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let columns = table_columns(conn, "payroll_records")?;
    let required = [
        "accrual_id",
        "accrual_type",
        "payment_date",
        "sequence",
        "accrual_description",
        "damga_snapshot_json",
    ];
    let table_sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'payroll_records'",
        [],
        |row| row.get(0),
    )?;
    let normalized_sql = table_sql
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let default_payment_date_expr = "CASE
                WHEN strftime('%Y-%m', pp.bitis_tarihi) = printf('%04d-%02d', pp.tax_year, pp.tax_month)
                THEN pp.bitis_tarihi
                ELSE date(pp.tax_year || '-' || printf('%02d', pp.tax_month) || '-01', '+1 month', '-1 day')
             END";
    let has_legacy_unique = normalized_sql.contains("unique(personnel_id, period_id)")
        || normalized_sql.contains("unique (personnel_id, period_id)");

    if required.iter().all(|column| columns.contains(*column)) && !has_legacy_unique {
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_accrual_id
                 ON payroll_records(accrual_id);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_one_normal
                 ON payroll_records(personnel_id, period_id)
                 WHERE accrual_type = 'NORMAL';
             CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_payment_sequence
                 ON payroll_records(personnel_id, payment_date, sequence);
             CREATE INDEX IF NOT EXISTS idx_payroll_records_accrual_order
                 ON payroll_records(personnel_id, payment_date, sequence, accrual_id);",
        )?;
        return Ok(());
    }

    // Rebuild the parent and child tables atomically. SQLite cannot remove a
    // table-level UNIQUE constraint with ALTER TABLE; keeping the copy in the
    // same migration transaction means an error rolls back the whole repair.
    let accrual_id_expr = if columns.contains("accrual_id") {
        "COALESCE(NULLIF(pr.accrual_id, ''), pr.id)"
    } else {
        "pr.id"
    };
    let accrual_type_expr = if columns.contains("accrual_type") {
        "COALESCE(NULLIF(pr.accrual_type, ''), 'NORMAL')"
    } else {
        "'NORMAL'"
    };
    let payment_date_expr = if columns.contains("payment_date") {
        format!("COALESCE(NULLIF(pr.payment_date, ''), {default_payment_date_expr})")
    } else {
        default_payment_date_expr.to_owned()
    };
    let sequence_expr = if columns.contains("sequence") {
        "COALESCE(pr.sequence, 0)"
    } else {
        "0"
    };
    let description_expr = if columns.contains("accrual_description") {
        "pr.accrual_description"
    } else {
        "pr.notlar"
    };
    let damga_snapshot_expr = if columns.contains("damga_snapshot_json") {
        "pr.damga_snapshot_json"
    } else {
        "NULL"
    };

    conn.execute_batch(
        "CREATE TABLE payroll_income_items__multi_backup AS
             SELECT id, payroll_id, item_type, description, amount, source
             FROM payroll_income_items;
         CREATE TABLE payroll_deduction_items__multi_backup AS
             SELECT id, payroll_id, item_type, description, amount, source
             FROM payroll_deduction_items;
         DROP TABLE payroll_income_items;
         DROP TABLE payroll_deduction_items;
         CREATE TABLE payroll_records__multi (
             id TEXT PRIMARY KEY,
             personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
             period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
             accrual_id TEXT NOT NULL,
             accrual_type TEXT NOT NULL DEFAULT 'NORMAL',
             payment_date TEXT NOT NULL,
             sequence INTEGER NOT NULL DEFAULT 0,
             accrual_description TEXT,
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
             raporlu_gun INTEGER,
             odenen_raporlu_gun INTEGER,
             is_primi_snapshot_json TEXT,
             gv_snapshot_json TEXT,
             statutory_snapshot_json TEXT,
             damga_snapshot_json TEXT,
             notlar TEXT
         );",
    )?;
    let insert_sql = format!(
        "INSERT INTO payroll_records__multi (
             id, personnel_id, period_id, accrual_id, accrual_type, payment_date, sequence,
             accrual_description, gross_total, sgk_base, gv_base, previous_cumulative_gv,
             new_cumulative_gv, income_tax, stamp_tax, total_deductions, net_payment, status,
             puantaj_summary_json, pek_detail_json, devreden_pek_gelen_json,
             sonraki_devreden_pek_json, calculated_at, updated_at, raporlu_gun,
             odenen_raporlu_gun, is_primi_snapshot_json, gv_snapshot_json,
             statutory_snapshot_json, damga_snapshot_json, notlar
         )
         SELECT pr.id, pr.personnel_id, pr.period_id, {accrual_id_expr}, {accrual_type_expr},
                {payment_date_expr}, {sequence_expr}, {description_expr}, pr.gross_total,
                pr.sgk_base, pr.gv_base, pr.previous_cumulative_gv, pr.new_cumulative_gv,
                pr.income_tax, pr.stamp_tax, pr.total_deductions, pr.net_payment, pr.status,
                pr.puantaj_summary_json, pr.pek_detail_json, pr.devreden_pek_gelen_json,
                pr.sonraki_devreden_pek_json, pr.calculated_at, pr.updated_at, pr.raporlu_gun,
                pr.odenen_raporlu_gun, pr.is_primi_snapshot_json, pr.gv_snapshot_json,
                pr.statutory_snapshot_json, {damga_snapshot_expr}, pr.notlar
           FROM payroll_records pr
           JOIN payroll_periods pp ON pp.id = pr.period_id;"
    );
    conn.execute_batch(&insert_sql)?;
    conn.execute_batch(
        "DROP TABLE payroll_records;
         ALTER TABLE payroll_records__multi RENAME TO payroll_records;
         CREATE TABLE payroll_income_items (
             id TEXT PRIMARY KEY,
             payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
             item_type TEXT NOT NULL,
             description TEXT NOT NULL,
             amount INTEGER NOT NULL,
             source TEXT NOT NULL
         );
         INSERT INTO payroll_income_items
             SELECT id, payroll_id, item_type, description, amount, source
             FROM payroll_income_items__multi_backup;
         DROP TABLE payroll_income_items__multi_backup;
         CREATE TABLE payroll_deduction_items (
             id TEXT PRIMARY KEY,
             payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
             item_type TEXT NOT NULL,
             description TEXT NOT NULL,
             amount INTEGER NOT NULL,
             source TEXT NOT NULL
         );
         INSERT INTO payroll_deduction_items
             SELECT id, payroll_id, item_type, description, amount, source
             FROM payroll_deduction_items__multi_backup;
         DROP TABLE payroll_deduction_items__multi_backup;
         CREATE INDEX IF NOT EXISTS idx_payroll_records_personnel
             ON payroll_records(personnel_id);
         CREATE INDEX IF NOT EXISTS idx_payroll_records_period
             ON payroll_records(period_id);
         CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_accrual_id
             ON payroll_records(accrual_id);
         CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_one_normal
             ON payroll_records(personnel_id, period_id)
             WHERE accrual_type = 'NORMAL';
         CREATE UNIQUE INDEX IF NOT EXISTS idx_payroll_records_payment_sequence
             ON payroll_records(personnel_id, payment_date, sequence);
         CREATE INDEX IF NOT EXISTS idx_payroll_records_accrual_order
             ON payroll_records(personnel_id, payment_date, sequence, accrual_id);",
    )?;
    Ok(())
}

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
                accrual_id TEXT NOT NULL,
                accrual_type TEXT NOT NULL DEFAULT 'NORMAL',
                payment_date TEXT NOT NULL,
                sequence INTEGER NOT NULL DEFAULT 0,
                accrual_description TEXT,
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
                damga_snapshot_json TEXT
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
        // Keep this migration self-contained for callers that use get_migrations()
        // directly, while remaining safe for pre-release databases that already have the
        // additive column. The hook runs in the migration transaction after the no-op SQL.
        M::up_with_hook("SELECT 1;", |tx| {
            let exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('payroll_records') WHERE name = 'statutory_snapshot_json'",
                [],
                |row| row.get(0),
            )?;
            if exists == 0 {
                tx.execute(
                    "ALTER TABLE payroll_records ADD COLUMN statutory_snapshot_json TEXT",
                    [],
                )?;
            }
            Ok(())
        }),
        M::up_with_hook("SELECT 1;", |tx| {
            for column in [
                "dogum_askerlik_gv_indirim_tutar",
                "hayat_sigortasi_gv_prim_tutar",
                "saglik_sigortasi_gv_prim_tutar",
            ] {
                let exists: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('personnel') WHERE name = ?1",
                    [column],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    tx.execute(
                        &format!("ALTER TABLE personnel ADD COLUMN {column} INTEGER"),
                        [],
                    )?;
                }
            }
            Ok(())
        }),
        M::up_with_hook("SELECT 1;", |tx| {
            ensure_unique_tax_year_month(tx)?;
            Ok(())
        }),
        M::up_with_hook("SELECT 1;", |tx| {
            ensure_multi_accrual_schema(tx).map_err(|error| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                    error.to_string(),
                )))
            })?;
            Ok(())
        }),
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
    for (column, definition) in [
        ("dogum_askerlik_gv_indirim_tutar", "INTEGER"),
        ("hayat_sigortasi_gv_prim_tutar", "INTEGER"),
        ("saglik_sigortasi_gv_prim_tutar", "INTEGER"),
    ] {
        add_column_if_missing(&tx, "personnel", &mut personnel_columns, column, definition)?;
    }

    let mut payroll_columns = table_columns(&tx, "payroll_records")?;
    for (column, definition) in [
        ("raporlu_gun", "INTEGER"),
        ("odenen_raporlu_gun", "INTEGER"),
        ("is_primi_snapshot_json", "TEXT"),
        ("gv_snapshot_json", "TEXT"),
        ("statutory_snapshot_json", "TEXT"),
        ("damga_snapshot_json", "TEXT"),
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

    ensure_unique_tax_year_month(&tx)?;
    ensure_multi_accrual_schema(&tx)?;

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
