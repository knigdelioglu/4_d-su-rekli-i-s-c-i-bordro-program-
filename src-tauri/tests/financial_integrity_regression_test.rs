use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person() -> Personel {
    Personel {
        id: "p-integrity".into(),
        tcNo: "11111111111".into(),
        ad: "Finans".into(),
        soyad: "Bütünlük".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "sgk-integrity".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: None,
    }
}

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-05".into(),
        yil: 2026,
        ay: 5,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Mayıs 2026".into(),
        taxYear: 2026,
        taxMonth: 6,
    }
}

fn payroll(accrual_type: AccrualType, sequence: i32, net: rust_decimal::Decimal) -> BordroKaydi {
    BordroKaydi {
        id: format!("p-integrity-{sequence}"),
        personelId: "p-integrity".into(),
        donemId: "2026-05".into(),
        accrualId: format!("accrual-{sequence}"),
        accrualType: accrual_type,
        paymentDate: String::new(),
        sequence,
        accrualDescription: None,
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(dec!(100)),
            ..Default::default()
        },
        gelirToplam: dec!(100),
        kesintiler: KesintiKalemleri {
            digerKesinti: Some(dec!(10)),
            ..Default::default()
        },
        kesintiToplam: dec!(10),
        netOdeme: net,
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: None,
        oncekiKumulatifAsgariGvMatrahi: None,
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        damgaDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: None,
        raporluGun: None,
    }
}

#[test]
fn source_mutation_rolls_back_when_invalidation_fails() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person();
    let d = period();
    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &d)?;

    let original = PersonelPuantaj {
        id: "p-integrity_2026-05".into(),
        personelId: p.id.clone(),
        donemId: d.id.clone(),
        gunler: HashMap::from([(String::from("2026-05-15"), String::from("Ç"))]),
    };
    AttendanceRepository::save(&conn, &original)?;
    PayrollRepository::save(&conn, &payroll(AccrualType::NORMAL, 0, dec!(90)))?;

    conn.execute_batch(
        "CREATE TRIGGER fail_payroll_stale
         BEFORE UPDATE OF status ON payroll_records
         WHEN NEW.status = 'STALE'
         BEGIN
             SELECT RAISE(ABORT, 'forced invalidation failure');
         END;",
    )?;

    let changed = PersonelPuantaj {
        gunler: HashMap::from([(String::from("2026-05-16"), String::from("Ç"))]),
        ..original.clone()
    };
    let error = AttendanceRepository::save(&conn, &changed).unwrap_err();
    assert!(matches!(error, DomainError::DatabaseError(_)));

    let persisted =
        AttendanceRepository::get_by_personnel_and_period(&conn, &p.id, &d.id)?.unwrap();
    assert_eq!(persisted.gunler, original.gunler);
    assert_eq!(PayrollRepository::get_all(&conn)?.len(), 1);
    assert_eq!(
        PayrollRepository::get_all(&conn)?[0].status,
        BordroStatus::CALCULATED
    );
    Ok(())
}

#[test]
fn strict_restore_boundary_rejects_tampered_totals_for_every_accrual_type() {
    let conn = create_in_memory_connection().unwrap();
    PersonnelRepository::save(&conn, &person()).unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();

    for (sequence, accrual_type) in [
        (0, AccrualType::NORMAL),
        (1, AccrualType::TEDIYE),
        (2, AccrualType::TIS_IKRAMIYE),
        (3, AccrualType::SUPPLEMENTAL),
    ] {
        let error = PayrollRepository::save_in_transaction(
            &conn,
            &payroll(accrual_type, sequence, dec!(91)),
        )
        .unwrap_err();
        assert!(matches!(error, DomainError::InvalidData(_)));
    }

    assert!(PayrollRepository::get_all(&conn).unwrap().is_empty());
}
