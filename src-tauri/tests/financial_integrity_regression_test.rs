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

fn reconciled_payroll(accrual_type: AccrualType, sequence: i32) -> BordroKaydi {
    let pek = PekDetayi {
        hesaplananPek: dec!(100),
        hamPek: dec!(100),
        devredenPekKullanilan: dec!(0),
        primMatrahi: dec!(100),
        finalPek: dec!(100),
        devredenPekAşanTutar: dec!(0),
        pekAltSinir: dec!(0),
        pekUstSinir: dec!(1000),
        altSinirTamamlamaFarki: dec!(0),
        fiiliYemekGunu: 0,
        yemekIstisnasiTutar: dec!(0),
        isverenSgkPrimi: None,
        isverenIssizlikPrimi: None,
        pekAltSinirTamamlamaIsverenPrimi: None,
        isverenPrimToplami: None,
        sgkIsverenOraniYuzde: None,
        isverenIssizlikOraniYuzde: None,
    };
    let gv = GvHesapDetayi {
        oncekiKumulatifGvMatrahi: dec!(0),
        cariGvMatrahi: dec!(85),
        yeniKumulatifGvMatrahi: dec!(85),
        brutGelirVergisi: dec!(0),
        asgariUcretGvMatrahi: dec!(0),
        asgariUcretReferansKumulatifMatrahi: dec!(0),
        asgariUcretGvIstisnasi: dec!(0),
        ayniAyOncekiKullanilanGvIstisnasi: dec!(0),
        tahakkukOncesiKalanGvIstisnasi: dec!(0),
        uygulananGvIstisnasi: dec!(0),
        tahakkukSonrasiKalanGvIstisnasi: dec!(0),
        kesilenGelirVergisi: dec!(0),
        dogumAskerlikGvIndirimi: dec!(0),
        sigortaGvIndirimAdayi: dec!(0),
        sigortaGvAylikLimiti: dec!(0),
        sigortaGvYillikKalanLimiti: dec!(0),
        uygulanabilirSigortaGvIndirimi: dec!(0),
    };
    let damga = DamgaVergisiHesapDetayi {
        brutDamgaVergisi: dec!(0),
        aylikDamgaIstisnaHakki: dec!(0),
        ayniAyOncekiKullanilanDamgaIstisnasi: dec!(0),
        uygulananDamgaIstisnasi: dec!(0),
        kalanDamgaIstisnasi: dec!(0),
        kesilenDamgaVergisi: dec!(0),
    };
    let statutory = ResolvedStatutorySnapshot {
        source: StatutorySnapshotSource::AttendanceBacked,
        segments: Vec::new(),
        sgkPrimGunSayisi: 1,
        pekAltSinir: dec!(0),
        pekUstSinir: dec!(1000),
        sgkYemekIstisnasiToplam: dec!(0),
        gvYemekIstisnasiToplam: dec!(0),
        gvReferansGunlukAsgariUcret: dec!(1000),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
    };

    BordroKaydi {
        id: format!("p-integrity-reconciled-{sequence}"),
        personelId: "p-integrity".into(),
        donemId: "2026-05".into(),
        accrualId: format!("accrual-reconciled-{sequence}"),
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
            isciSgkPrimi: Some(dec!(14)),
            isciIssizlikPrimi: Some(dec!(1)),
            gelirVergisi: Some(dec!(0)),
            damgaVergisi: Some(dec!(0)),
            ..Default::default()
        },
        kesintiToplam: dec!(15),
        netOdeme: dec!(85),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(dec!(0)),
        oncekiKumulatifAsgariGvMatrahi: Some(dec!(0)),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: Some(pek),
        isPrimiDetay: None,
        gvDetay: Some(gv),
        damgaDetay: Some(damga),
        statutorySnapshot: Some(statutory),
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
    }
}

fn reconciliation_connection() -> rusqlite::Connection {
    let conn = create_in_memory_connection().unwrap();
    PersonnelRepository::save(&conn, &person()).unwrap();
    PeriodRepository::save(&conn, &period()).unwrap();
    conn
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

#[test]
fn strict_restore_reconciles_gv_damga_and_pek_snapshots() {
    let conn = reconciliation_connection();

    let mut gv_snapshot = reconciled_payroll(AccrualType::NORMAL, 10);
    gv_snapshot.gvDetay.as_mut().unwrap().kesilenGelirVergisi = dec!(1);
    assert!(matches!(
        PayrollRepository::save_in_transaction(&conn, &gv_snapshot),
        Err(DomainError::InvalidData(_))
    ));

    let mut gv_deduction = reconciled_payroll(AccrualType::NORMAL, 11);
    gv_deduction.kesintiler.gelirVergisi = Some(dec!(1));
    gv_deduction.kesintiToplam = dec!(16);
    gv_deduction.netOdeme = dec!(84);
    assert!(matches!(
        PayrollRepository::save_in_transaction(&conn, &gv_deduction),
        Err(DomainError::InvalidData(_))
    ));

    let mut stamp_snapshot = reconciled_payroll(AccrualType::NORMAL, 12);
    stamp_snapshot.damgaDetay.as_mut().unwrap().kesilenDamgaVergisi = dec!(1);
    assert!(matches!(
        PayrollRepository::save_in_transaction(&conn, &stamp_snapshot),
        Err(DomainError::InvalidData(_))
    ));

    let mut pek_snapshot = reconciled_payroll(AccrualType::NORMAL, 13);
    pek_snapshot.pekDetay.as_mut().unwrap().primMatrahi = dec!(101);
    pek_snapshot.pekDetay.as_mut().unwrap().finalPek = dec!(101);
    assert!(matches!(
        PayrollRepository::save_in_transaction(&conn, &pek_snapshot),
        Err(DomainError::InvalidData(_))
    ));

    let mut unemployment_snapshot = reconciled_payroll(AccrualType::NORMAL, 14);
    unemployment_snapshot.kesintiler.isciIssizlikPrimi = Some(dec!(2));
    unemployment_snapshot.kesintiToplam = dec!(16);
    unemployment_snapshot.netOdeme = dec!(84);
    assert!(matches!(
        PayrollRepository::save_in_transaction(&conn, &unemployment_snapshot),
        Err(DomainError::InvalidData(_))
    ));
}

#[test]
fn strict_restore_accepts_valid_payment_events_and_retro_source_deltas() {
    let conn = reconciliation_connection();
    for (sequence, accrual_type) in [
        (20, AccrualType::NORMAL),
        (21, AccrualType::TEDIYE),
        (22, AccrualType::SUPPLEMENTAL),
    ] {
        PayrollRepository::save_in_transaction(&conn, &reconciled_payroll(accrual_type, sequence))
            .unwrap();
    }

    let mut retro = reconciled_payroll(AccrualType::RETRO_ADJUSTMENT, 23);
    retro.kesintiler.isciSgkPrimi = Some(dec!(3.80));
    retro.kesintiler.isciIssizlikPrimi = Some(dec!(0.70));
    retro.kesintiToplam = dec!(4.50);
    retro.netOdeme = dec!(95.50);
    retro.pekDetay.as_mut().unwrap().primMatrahi = dec!(20);
    retro.pekDetay.as_mut().unwrap().finalPek = dec!(20);
    retro.pekDetay.as_mut().unwrap().hamPek = dec!(20);
    PayrollRepository::save_in_transaction(&conn, &retro).unwrap();
}

#[test]
fn legacy_sparse_normal_snapshot_remains_writable_and_full_snapshot_roundtrips() {
    let conn = reconciliation_connection();
    let legacy = payroll(AccrualType::NORMAL, 30, dec!(90));
    PayrollRepository::save_in_transaction(&conn, &legacy).unwrap();

    let mut loaded = PayrollRepository::get_all(&conn)
        .unwrap()
        .into_iter()
        .find(|item| item.id == legacy.id)
        .unwrap();
    loaded.kesintiler.digerKesinti = Some(dec!(11));
    loaded.kesintiToplam = dec!(11);
    loaded.netOdeme = dec!(89);
    PayrollRepository::save(&conn, &loaded).unwrap();

    let second_period = BordroDonemi {
        id: "2026-06".into(),
        yil: 2026,
        ay: 6,
        baslangicTarihi: "2026-06-15".into(),
        bitisTarihi: "2026-07-14".into(),
        donemAdi: "Haziran 2026".into(),
        taxYear: 2026,
        taxMonth: 7,
    };
    PeriodRepository::save(&conn, &second_period).unwrap();
    let mut full = reconciled_payroll(AccrualType::NORMAL, 31);
    full.id = "p-integrity-reconciled-31-full".into();
    full.donemId = second_period.id.clone();
    full.accrualId = "accrual-reconciled-31-full".into();
    PayrollRepository::save_in_transaction(&conn, &full).unwrap();
    let roundtripped = PayrollRepository::get_all(&conn)
        .unwrap()
        .into_iter()
        .find(|item| item.id == full.id)
        .unwrap();
    PayrollRepository::save_in_transaction(&conn, &roundtripped).unwrap();
    assert_eq!(roundtripped.gvDetay.unwrap().yeniKumulatifGvMatrahi, dec!(85));
    assert_eq!(roundtripped.pekDetay.unwrap().primMatrahi, dec!(100));
}
