use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::{
    BordroDonemi, BordroKaydi, BordroStatus, DevredenPekKaydi, GelirKalemleri, GvHesapDetayi,
    KesintiKalemleri, PekDetayi, Personel, PuantajOzeti,
};
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Zincir".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "1".into(),
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

fn period(id: &str, yil: i32, ay: i32, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let (end_year, end_month) = if ay == 12 {
        (yil + 1, 1)
    } else {
        (yil, ay + 1)
    };
    BordroDonemi {
        id: id.into(),
        yil,
        ay,
        baslangicTarihi: format!("{yil}-{ay:02}-15"),
        bitisTarihi: format!("{end_year}-{end_month:02}-14"),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn gv_detay(gv_base: Decimal, previous: Decimal) -> GvHesapDetayi {
    GvHesapDetayi {
        cariGvMatrahi: gv_base,
        yeniKumulatifGvMatrahi: previous + gv_base,
        brutGelirVergisi: dec!(0),
        asgariUcretGvMatrahi: dec!(0),
        asgariUcretReferansKumulatifMatrahi: dec!(0),
        asgariUcretGvIstisnasi: dec!(0),
        uygulananGvIstisnasi: dec!(0),
        kesilenGelirVergisi: dec!(0),
        dogumAskerlikGvIndirimi: dec!(0),
        sigortaGvIndirimAdayi: dec!(0),
        sigortaGvAylikLimiti: dec!(0),
        sigortaGvYillikKalanLimiti: dec!(0),
        uygulanabilirSigortaGvIndirimi: dec!(0),
    }
}

fn pek_detay(final_pek: Decimal) -> PekDetayi {
    PekDetayi {
        hesaplananPek: final_pek,
        hamPek: final_pek,
        devredenPekKullanilan: dec!(0),
        primMatrahi: final_pek,
        finalPek: final_pek,
        devredenPekAşanTutar: dec!(0),
        pekAltSinir: dec!(0),
        pekUstSinir: dec!(999999999),
        altSinirTamamlamaFarki: dec!(0),
        fiiliYemekGunu: 0,
        yemekIstisnasiTutar: dec!(0),
        isverenSgkPrimi: None,
        isverenIssizlikPrimi: None,
        pekAltSinirTamamlamaIsverenPrimi: None,
        isverenPrimToplami: None,
        sgkIsverenOraniYuzde: None,
        isverenIssizlikOraniYuzde: None,
    }
}

fn payroll(
    personnel_id: &str,
    period_id: &str,
    gv_base: Decimal,
    previous_gv: Decimal,
    final_pek: Decimal,
    net: Decimal,
    status: BordroStatus,
) -> BordroKaydi {
    BordroKaydi {
        id: format!("{personnel_id}_{period_id}"),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000)),
            ..Default::default()
        },
        gelirToplam: dec!(100000),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(100000) - net,
        netOdeme: net,
        status,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(previous_gv),
        oncekiKumulatifAsgariGvMatrahi: None,
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: Some(pek_detay(final_pek)),
        isPrimiDetay: None,
        gvDetay: Some(gv_detay(gv_base, previous_gv)),
        statutorySnapshot: None,
        odenenRaporluGun: None,
        raporluGun: None,
    }
}

fn setup_three_periods() -> Result<(rusqlite::Connection, String), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let personnel_id = "p-chain".to_string();
    PersonnelRepository::save(&conn, &person(&personnel_id))?;
    for p in [
        period("2026-05", 2026, 5, 2026, 6),
        period("2026-06", 2026, 6, 2026, 7),
        period("2026-07", 2026, 7, 2026, 8),
    ] {
        PeriodRepository::save(&conn, &p)?;
    }
    Ok((conn, personnel_id))
}

fn status_of(
    conn: &rusqlite::Connection,
    personnel_id: &str,
    period_id: &str,
) -> Result<BordroStatus, Box<dyn std::error::Error>> {
    Ok(
        PayrollRepository::get_status_and_created_at(conn, personnel_id, period_id)?
            .expect("bordro kaydı bulunmalı")
            .0,
    )
}

#[test]
fn onceki_bordro_degisince_sonraki_calculated_bordrolar_stale_olur(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;

    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(50000),
            dec!(0),
            dec!(80000),
            dec!(70000),
            BordroStatus::CALCULATED,
        ),
    )?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(85000),
            dec!(71000),
            BordroStatus::CALCULATED,
        ),
    )?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-07",
            dec!(70000),
            dec!(110000),
            dec!(90000),
            dec!(72000),
            BordroStatus::CALCULATED,
        ),
    )?;

    let changed_may = payroll(
        &personnel_id,
        "2026-05",
        dec!(55000),
        dec!(0),
        dec!(82000),
        dec!(69000),
        BordroStatus::CALCULATED,
    );
    PayrollRepository::save(&conn, &changed_may)?;

    assert_eq!(
        status_of(&conn, &personnel_id, "2026-05")?,
        BordroStatus::CALCULATED
    );
    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::STALE
    );
    assert_eq!(
        status_of(&conn, &personnel_id, "2026-07")?,
        BordroStatus::STALE
    );
    Ok(())
}

#[test]
fn yalniz_pek_degisikligi_de_sonraki_bordroyu_stale_yapar() -> Result<(), Box<dyn std::error::Error>>
{
    let (conn, personnel_id) = setup_three_periods()?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(50000),
            dec!(0),
            dec!(80000),
            dec!(70000),
            BordroStatus::CALCULATED,
        ),
    )?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(85000),
            dec!(71000),
            BordroStatus::CALCULATED,
        ),
    )?;

    let changed = payroll(
        &personnel_id,
        "2026-05",
        dec!(50000),
        dec!(0),
        dec!(81000),
        dec!(70000),
        BordroStatus::CALCULATED,
    );
    PayrollRepository::save(&conn, &changed)?;

    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::STALE
    );
    Ok(())
}

#[test]
fn final_pek_ayni_kalsa_bile_devreden_pek_snapshot_degisikligi_stale_yapar(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;
    let mut may = payroll(
        &personnel_id,
        "2026-05",
        dec!(50000),
        dec!(0),
        dec!(90000),
        dec!(70000),
        BordroStatus::CALCULATED,
    );
    may.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
        tutar: dec!(10000),
        kalanAySayisi: 2,
        kaynakDonemId: Some("2026-05".into()),
    }]);
    PayrollRepository::save(&conn, &may)?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(90000),
            dec!(71000),
            BordroStatus::CALCULATED,
        ),
    )?;

    may.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
        tutar: dec!(15000),
        kalanAySayisi: 2,
        kaynakDonemId: Some("2026-05".into()),
    }]);
    PayrollRepository::save(&conn, &may)?;

    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::STALE
    );
    Ok(())
}

#[test]
fn stale_bordro_finalized_yapilamaz_ve_recalc_sonrasi_tekrar_calculated_olur(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(50000),
            dec!(0),
            dec!(80000),
            dec!(70000),
            BordroStatus::CALCULATED,
        ),
    )?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(85000),
            dec!(71000),
            BordroStatus::CALCULATED,
        ),
    )?;

    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(55000),
            dec!(0),
            dec!(82000),
            dec!(69000),
            BordroStatus::CALCULATED,
        ),
    )?;
    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::STALE
    );

    let finalize_stale =
        PayrollRepository::update_status(&conn, &personnel_id, "2026-06", BordroStatus::FINALIZED);
    assert!(matches!(
        finalize_stale,
        Err(DomainError::ValidationError(_))
    ));

    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(55000),
            dec!(85000),
            dec!(70500),
            BordroStatus::CALCULATED,
        ),
    )?;
    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::CALCULATED
    );

    let previous_for_july = CumulativeTaxService::get_previous_cumulative_gv(
        &conn,
        &personnel_id,
        &period("2026-07", 2026, 7, 2026, 8),
    )?;
    assert_eq!(previous_for_july, dec!(115000));
    Ok(())
}

#[test]
fn finalization_onceki_mevcut_vergi_zincirinin_finalized_olmasini_zorunlu_kilar(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(50000),
            dec!(0),
            dec!(80000),
            dec!(70000),
            BordroStatus::CALCULATED,
        ),
    )?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(85000),
            dec!(71000),
            BordroStatus::CALCULATED,
        ),
    )?;

    let june_first =
        PayrollRepository::update_status(&conn, &personnel_id, "2026-06", BordroStatus::FINALIZED);
    assert!(matches!(june_first, Err(DomainError::ValidationError(_))));

    PayrollRepository::update_status(&conn, &personnel_id, "2026-05", BordroStatus::FINALIZED)?;
    PayrollRepository::update_status(&conn, &personnel_id, "2026-06", BordroStatus::FINALIZED)?;
    assert_eq!(
        status_of(&conn, &personnel_id, "2026-06")?,
        BordroStatus::FINALIZED
    );
    Ok(())
}

#[test]
fn ileride_finalized_bordro_varsa_gecmis_mutable_bordro_degistirilemez(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;
    PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(50000),
            dec!(0),
            dec!(80000),
            dec!(70000),
            BordroStatus::CALCULATED,
        ),
    )?;

    // Legacy/restore kaynaklı tutarsız bir zinciri simüle ediyoruz. Production
    // finalization yolu bu durumu zaten reddeder; geçmişi değiştirme kilidi yine
    // de bu snapshot karşısında fail-closed olmalıdır.
    PayrollRepository::save_in_transaction(
        &conn,
        &payroll(
            &personnel_id,
            "2026-06",
            dec!(60000),
            dec!(50000),
            dec!(85000),
            dec!(71000),
            BordroStatus::FINALIZED,
        ),
    )?;

    let result = PayrollRepository::save(
        &conn,
        &payroll(
            &personnel_id,
            "2026-05",
            dec!(55000),
            dec!(0),
            dec!(82000),
            dec!(69000),
            BordroStatus::CALCULATED,
        ),
    );
    assert!(matches!(result, Err(DomainError::PayrollFinalized(_))));
    Ok(())
}

#[test]
fn draft_ve_stale_kayitlar_kumulatif_gv_icin_authoritative_degil(
) -> Result<(), Box<dyn std::error::Error>> {
    for status in [BordroStatus::DRAFT, BordroStatus::STALE] {
        let conn = create_in_memory_connection()?;
        let personnel_id = format!("p-{status:?}");
        PersonnelRepository::save(&conn, &person(&personnel_id))?;
        let may = period("2026-05", 2026, 5, 2026, 6);
        let june = period("2026-06", 2026, 6, 2026, 7);
        PeriodRepository::save(&conn, &may)?;
        PeriodRepository::save(&conn, &june)?;

        PayrollRepository::save_in_transaction(
            &conn,
            &payroll(
                &personnel_id,
                "2026-05",
                dec!(50000),
                dec!(0),
                dec!(80000),
                dec!(70000),
                status,
            ),
        )?;

        assert!(!PayrollRepository::has_personnel_tax_month_before(
            &conn,
            &personnel_id,
            2026,
            7,
        )?);

        let result = CumulativeTaxService::get_previous_cumulative_gv(&conn, &personnel_id, &june);
        assert!(matches!(result, Err(DomainError::ValidationError(_))));
    }
    Ok(())
}

#[test]
fn stale_onceki_bordronun_devreden_pek_snapshoti_kullanilamaz(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, personnel_id) = setup_three_periods()?;
    let mut may = payroll(
        &personnel_id,
        "2026-05",
        dec!(50000),
        dec!(0),
        dec!(90000),
        dec!(70000),
        BordroStatus::STALE,
    );
    may.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
        tutar: dec!(10000),
        kalanAySayisi: 2,
        kaynakDonemId: Some("2026-05".into()),
    }]);
    PayrollRepository::save_in_transaction(&conn, &may)?;

    let result = PayrollRepository::get_next_devreden_pek(&conn, &personnel_id, "2026-05");
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    Ok(())
}

#[test]
fn vergi_yili_degisiminde_onceki_yilin_stale_kaydi_yeni_yili_kirletmez(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let personnel_id = "p-year-reset";
    PersonnelRepository::save(&conn, &person(personnel_id))?;

    let november = period("2026-11", 2026, 11, 2026, 12);
    let december = period("2026-12", 2026, 12, 2027, 1);
    PeriodRepository::save(&conn, &november)?;
    PeriodRepository::save(&conn, &december)?;

    PayrollRepository::save_in_transaction(
        &conn,
        &payroll(
            personnel_id,
            "2026-11",
            dec!(80000),
            dec!(0),
            dec!(90000),
            dec!(70000),
            BordroStatus::STALE,
        ),
    )?;

    let previous =
        CumulativeTaxService::get_previous_cumulative_gv(&conn, personnel_id, &december)?;
    assert_eq!(previous, dec!(0));
    Ok(())
}
