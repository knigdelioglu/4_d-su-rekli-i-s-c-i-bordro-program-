use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

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

            CREATE INDEX IF NOT EXISTS idx_payroll_records_personnel ON payroll_records(personnel_id);
            CREATE INDEX IF NOT EXISTS idx_payroll_records_period ON payroll_records(period_id);
            CREATE INDEX IF NOT EXISTS idx_attendance_records_period ON attendance_records(period_id);
            CREATE INDEX IF NOT EXISTS idx_tax_opening_personnel_year ON personnel_tax_opening(personnel_id, year);
            "#,
        ),
    ])
}

pub fn initialize_db(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let migrations = get_migrations();
    migrations.to_latest(conn)?;
    Ok(())
}
