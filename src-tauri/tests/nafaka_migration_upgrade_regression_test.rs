use bordro_programi_lib::db::migrations::{
    get_migrations, initialize_db, COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
};
use bordro_programi_lib::domain::models::{Personel, PersonelKesintileri};
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use rusqlite::Connection;
use rust_decimal_macros::dec;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const PRE_COMPENSATION_POLICY_USER_VERSION: u32 = 20;

struct TemporaryDatabase(PathBuf);

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn legacy_personnel() -> Personel {
    Personel {
        id: "p-existing".into(),
        tcNo: "10000000001".into(),
        ad: "Eski".into(),
        soyad: "Kayıt".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "SGK-OLD".into(),
        iban: "TR00".into(),
        hizmetYili: 7,
        aciklama: Some("upgrade sırasında korunmalı".into()),
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri::default()),
    }
}

fn column_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    let mut statement = conn.prepare("PRAGMA table_info(personnel)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

#[test]
fn disk_database_at_previous_latest_version_upgrades_without_losing_personnel(
) -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let database = TemporaryDatabase(std::env::temp_dir().join(format!(
        "nafaka-upgrade-{}-{nonce}.sqlite",
        std::process::id()
    )));

    {
        let mut conn = Connection::open(&database.0)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        get_migrations().to_version(&mut conn, PRE_COMPENSATION_POLICY_USER_VERSION as usize)?;
        let latest: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(latest, PRE_COMPENSATION_POLICY_USER_VERSION);

        PersonnelRepository::save(&conn, &legacy_personnel())?;

        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM personnel WHERE id = 'p-existing'",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            1
        );
        assert!(column_exists(&conn, "nafaka_tutar")?);
    }

    {
        let mut conn = Connection::open(&database.0)?;
        let old_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(old_version, PRE_COMPENSATION_POLICY_USER_VERSION);

        initialize_db(&mut conn)?;

        let upgraded_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert!(
            upgraded_version >= COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
            "the upgrade must include the compensation-policy flags migration"
        );
        assert!(column_exists(&conn, "nafaka_tutar")?);

        let mut restored = PersonnelRepository::get_by_id(&conn, "p-existing")?
            .expect("eski personel kaydı upgrade sonrası okunmalı");
        assert_eq!(restored.ad, "Eski");
        assert_eq!(restored.soyad, "Kayıt");
        assert_eq!(restored.hizmetYili, 7);
        assert_eq!(
            restored
                .kesintiler
                .as_ref()
                .and_then(|items| items.nafakaTutar),
            None
        );

        restored
            .kesintiler
            .get_or_insert_with(PersonelKesintileri::default)
            .nafakaTutar = Some(dec!(125.50));
        PersonnelRepository::save(&conn, &restored)?;
        let saved = PersonnelRepository::get_by_id(&conn, "p-existing")?
            .expect("personel nafaka ile tekrar okunmalı");
        assert_eq!(
            saved
                .kesintiler
                .as_ref()
                .and_then(|items| items.nafakaTutar),
            Some(dec!(125.50))
        );
    }

    {
        let mut reopened = Connection::open(&database.0)?;
        initialize_db(&mut reopened)?;
        let version: u32 = reopened.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert!(
            version >= COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
            "the reopened database must retain the compensation-policy flags migration"
        );
        let saved = PersonnelRepository::get_by_id(&reopened, "p-existing")?
            .expect("ikinci açılışta personel korunmalı");
        assert_eq!(
            saved
                .kesintiler
                .as_ref()
                .and_then(|items| items.nafakaTutar),
            Some(dec!(125.50))
        );
        let integrity: String =
            reopened.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        assert_eq!(integrity, "ok");
    }

    Ok(())
}
