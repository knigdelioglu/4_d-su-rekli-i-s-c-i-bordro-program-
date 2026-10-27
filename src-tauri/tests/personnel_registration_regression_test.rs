use bordro_programi_lib::db::{create_connection, create_in_memory_connection};
use bordro_programi_lib::domain::{models::*, DomainError};
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use rust_decimal_macros::dec;
use std::time::{SystemTime, UNIX_EPOCH};

const VALID_IBAN: &str = "TR330006100519786457841326";

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111110".into(),
        ad: "Ayşe".into(),
        soyad: "Yılmaz".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: VALID_IBAN.into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri::default()),
    }
}

#[test]
fn registration_normalizes_grouped_iban_and_survives_database_reopen() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let database_path = std::env::temp_dir().join(format!(
        "personnel-iban-{}-{nonce}.sqlite",
        std::process::id()
    ));
    let mut worker = person("worker-1");
    worker.tcNo = " 11111111110 ".into();
    worker.iban = " tr33 0006 1005 1978 6457 8413 26 ".into();

    {
        let conn = create_connection(Some(database_path.clone())).unwrap();
        PersonnelRepository::save(&conn, &worker).unwrap();
    }

    let reopened = create_connection(Some(database_path.clone())).unwrap();
    let loaded = PersonnelRepository::get_by_id(&reopened, "worker-1")
        .unwrap()
        .unwrap();
    assert_eq!(loaded.tcNo, "11111111110");
    assert_eq!(loaded.iban, VALID_IBAN);
    drop(reopened);
    std::fs::remove_file(database_path).unwrap();
}

#[test]
fn registration_rejects_invalid_turkish_ibans_but_legacy_restore_stays_readable() {
    let conn = create_in_memory_connection().unwrap();
    for iban in [
        "TR",
        "TR33000610051978645784132",
        "TR3300061005197864578413260",
        "TR3A0006100519786457841326",
        "TR3300061100519786457841326",
        "TR330006100519786457841325",
        "",
    ] {
        let mut invalid = person(&format!("invalid-{}", iban.len()));
        invalid.iban = iban.into();
        assert!(matches!(
            PersonnelRepository::save(&conn, &invalid),
            Err(DomainError::ValidationError(_))
        ));
    }

    let mut legacy = person("legacy-invalid-iban");
    legacy.iban = "TR".into();
    PersonnelRepository::save_in_transaction(&conn, &legacy).unwrap();
    assert_eq!(
        PersonnelRepository::get_by_id(&conn, &legacy.id)
            .unwrap()
            .unwrap()
            .iban,
        "TR"
    );
}

#[test]
fn negative_fixed_deductions_are_rejected_and_valid_values_round_trip() {
    let conn = create_in_memory_connection().unwrap();
    for negative in [dec!(-1), dec!(-0.01)] {
        for field in [
            "sabit_sendika",
            "sabit_bes",
            "icra",
            "nafaka",
            "kisi_borcu",
            "dogum_askerlik",
            "hayat_saglik",
            "diger",
            "gv_indirim",
        ] {
            let mut worker = person(&format!("negative-{field}-{}", negative));
            let deductions = worker.kesintiler.as_mut().unwrap();
            match field {
                "icra" => deductions.icraTutar = Some(negative),
                "nafaka" => deductions.nafakaTutar = Some(negative),
                "kisi_borcu" => deductions.kisiBorcuTutar = Some(negative),
                "sabit_sendika" => deductions.sabitSendikaAidati = Some(negative),
                "sabit_bes" => deductions.sabitBesTutar = Some(negative),
                "dogum_askerlik" => deductions.dogumAskerlikBorclanmasiTutar = Some(negative),
                "hayat_saglik" => deductions.hayatSaglikSigortasiTutar = Some(negative),
                "diger" => deductions.digerKesintiTutar = Some(negative),
                "gv_indirim" => {
                    deductions.gvIndirimleri = Some(GvIndirimGirdileri {
                        saglikSigortasiPrimiTutar: Some(negative),
                        ..Default::default()
                    })
                }
                _ => unreachable!(),
            }
            assert!(matches!(
                PersonnelRepository::save(&conn, &worker),
                Err(DomainError::ValidationError(_))
            ));
        }
    }

    let mut worker = person("valid-deductions");
    worker.kesintiler.as_mut().unwrap().icraTutar = Some(dec!(0));
    worker.kesintiler.as_mut().unwrap().nafakaTutar = Some(dec!(0.01));
    worker.kesintiler.as_mut().unwrap().kisiBorcuTutar = Some(dec!(1));
    PersonnelRepository::save(&conn, &worker).unwrap();
    let loaded = PersonnelRepository::get_by_id(&conn, "valid-deductions")
        .unwrap()
        .unwrap();
    let loaded_deductions = loaded.kesintiler.unwrap();
    assert_eq!(loaded_deductions.icraTutar, Some(dec!(0)));
    assert_eq!(loaded_deductions.nafakaTutar, Some(dec!(0.01)));
    assert_eq!(loaded_deductions.kisiBorcuTutar, Some(dec!(1)));
}

#[test]
fn duplicate_tc_no_is_a_structured_domain_error_and_same_person_can_edit() {
    let conn = create_in_memory_connection().unwrap();
    let first = person("first");
    PersonnelRepository::save(&conn, &first).unwrap();

    let mut same = first.clone();
    same.soyad = "Güncel".into();
    PersonnelRepository::save(&conn, &same).unwrap();

    let duplicate = person("second");
    let error = PersonnelRepository::save(&conn, &duplicate).unwrap_err();
    assert!(matches!(&error, DomainError::DuplicateTcNo(_)));
    let encoded = serde_json::to_value(error).unwrap();
    assert_eq!(encoded["type"], "DUPLICATE_TC_NO");
    assert!(encoded["message"].as_str().unwrap().contains("T.C. kimlik"));
}

#[test]
fn unrelated_sql_errors_are_not_reported_as_duplicate_tc_no() {
    let conn = create_in_memory_connection().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER reject_personnel_write BEFORE INSERT ON personnel
         WHEN NEW.id = 'trigger-error'
         BEGIN SELECT RAISE(ABORT, 'unrelated write failure'); END;",
    )
    .unwrap();
    let error = PersonnelRepository::save(&conn, &person("trigger-error")).unwrap_err();
    assert!(matches!(error, DomainError::DatabaseError(_)));
}

#[test]
fn registration_accepts_eleven_digit_tc_without_checksum_validation() {
    let conn = create_in_memory_connection().unwrap();
    let mut worker = person("checksum-not-required");
    worker.tcNo = "12345678901".into();
    PersonnelRepository::save(&conn, &worker).unwrap();
    assert_eq!(
        PersonnelRepository::get_by_id(&conn, &worker.id)
            .unwrap()
            .unwrap()
            .tcNo,
        "12345678901"
    );
}
