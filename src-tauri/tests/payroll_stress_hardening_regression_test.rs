use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::{
    resolve_statutory_snapshot_for_period_with_paid_sick_dates, PayrollService,
};
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period(id: &str, year: i32, month: i32, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let (end_year, end_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    BordroDonemi {
        id: id.into(),
        yil: year,
        ay: month,
        baslangicTarihi: format!("{year}-{month:02}-15"),
        bitisTarihi: format!("{end_year}-{end_month:02}-14"),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn person(id: &str, bes_uyesi: bool) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Stress".into(),
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
        kesintiler: Some(PersonelKesintileri {
            sendikaUyesi: Some(false),
            besUyesi: Some(bes_uyesi),
            sabitSendikaAidati: None,
            oksOraniYuzde: None,
            sabitBesTutar: None,
            icraTutar: None,
            kisiBorcuTutar: None,
            dogumAskerlikBorclanmasiTutar: None,
            hayatSaglikSigortasiTutar: None,
            digerKesintiTutar: None,
            gvIndirimleri: None,
        }),
    }
}

fn settings(period_id: &str) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        gunlukTabanUcret: dec!(1000),
        gunlukYemek: dec!(0),
        birlestirilmisSosyalYardim: dec!(0),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiGruplari: Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(0),
            aktif: true,
        }]),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: Some(dec!(0)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        sendikaAidatiYuzde: Some(dec!(0)),
        besOraniYuzde: Some(dec!(3)),
        gunlukYemekIstisnasiSGK: Some(dec!(0)),
        gunlukYemekIstisnasiGV: Some(dec!(0)),
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(3)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    }
}

fn attendance_from_codes(
    personnel_id: &str,
    period: &BordroDonemi,
    codes: &[&str],
) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let gunler = codes
        .iter()
        .enumerate()
        .map(|(offset, code)| {
            (
                (start + Duration::days(offset as i64))
                    .format("%Y-%m-%d")
                    .to_string(),
                (*code).to_string(),
            )
        })
        .collect::<HashMap<_, _>>();
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn prior_payroll_with_devreden(
    personnel_id: &str,
    period_id: &str,
    amount: rust_decimal::Decimal,
    remaining_months: i32,
) -> BordroKaydi {
    BordroKaydi {
        id: format!("{}_{}", personnel_id, period_id),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri::default(),
        gelirToplam: dec!(0),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(0),
        netOdeme: dec!(0),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(dec!(0)),
        oncekiKumulatifAsgariGvMatrahi: Some(dec!(0)),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: Some(vec![DevredenPekKaydi {
            tutar: amount,
            kalanAySayisi: remaining_months,
            kaynakDonemId: Some(period_id.into()),
        }]),
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
    }
}

fn setup_devreden_case(
    personnel_id: &str,
    bes_uyesi: bool,
    active_codes: &[&str],
) -> Result<(rusqlite::Connection, BordroDonemi), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person(personnel_id, bes_uyesi);
    PersonnelRepository::save(&conn, &p)?;

    // Önceki çalışma dönemi bilinçli olarak başka vergi yılına alınır; böylece
    // test yalnız devreden PEK davranışını sınar, kümülatif GV zinciri araya girmez.
    let prior = period("2026-06-stress", 2026, 6, 2025, 12);
    let active = period("2026-07-stress", 2026, 7, 2026, 8);
    PeriodRepository::save(&conn, &prior)?;
    PeriodRepository::save(&conn, &active)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&active.id))?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;

    PayrollRepository::save(
        &conn,
        &prior_payroll_with_devreden(personnel_id, &prior.id, dec!(20000), 2),
    )?;
    AttendanceRepository::save(&conn, &attendance_from_codes(personnel_id, &active, active_codes))?;

    Ok((conn, active))
}

#[test]
fn unpaid_r_is_not_sgk_day_but_paid_r_is() -> Result<(), Box<dyn std::error::Error>> {
    let p = period("2026-07-r", 2026, 7, 2026, 8);
    let k = settings(&p.id);
    let attendance = attendance_from_codes("r-person", &p, &["Ç", "R"]);

    let unpaid = resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        &attendance,
        &p,
        &k,
        &[],
    )?;
    assert_eq!(unpaid.sgkPrimGunSayisi, 1);

    let paid_date = NaiveDate::parse_from_str("2026-07-16", "%Y-%m-%d")?;
    let paid = resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        &attendance,
        &p,
        &k,
        &[paid_date],
    )?;
    assert_eq!(paid.sgkPrimGunSayisi, 2);
    Ok(())
}

#[test]
fn incoming_devreden_pek_is_not_aged_when_period_has_no_prim_day(
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, active) = setup_devreden_case("no-prim-day", false, &["R"])?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, "no-prim-day", &active.id)?;
    assert_eq!(
        payroll
            .statutorySnapshot
            .as_ref()
            .map(|snapshot| snapshot.sgkPrimGunSayisi),
        Some(0)
    );
    assert_eq!(
        payroll
            .pekDetay
            .as_ref()
            .map(|pek| pek.devredenPekKullanilan),
        Some(dec!(0))
    );
    let carried = payroll.sonrakiDevredenPek.as_ref().expect("devreden PEK korunmalı");
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].tutar, dec!(20000));
    assert_eq!(carried[0].kalanAySayisi, 2);
    Ok(())
}

#[test]
fn oks_uses_worker_pek_including_devreden_amount_used_this_period(
) -> Result<(), Box<dyn std::error::Error>> {
    let work_codes = vec!["Ç"; 30];
    let (conn, active) = setup_devreden_case("oks-devreden", true, &work_codes)?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, "oks-devreden", &active.id)?;
    let pek = payroll.pekDetay.as_ref().expect("PEK detayı olmalı");
    assert_eq!(pek.hesaplananPek, dec!(30000));
    assert_eq!(pek.devredenPekKullanilan, dec!(20000));
    assert_eq!(pek.primMatrahi, dec!(50000));
    assert_eq!(payroll.kesintiler.bes, Some(dec!(1500)));
    Ok(())
}

#[test]
fn stamp_tax_uses_resolved_minimum_wage_after_in_period_change(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let p = person("stamp-segment", false);
    let active = period("2025-12-stamp", 2025, 12, 2026, 1);
    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &active)?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;

    let mut k = settings(&active.id);
    k.gunlukAsgariUcret = Some(dec!(100));
    k.pekTavanKatsayisi = Some(dec!(10));
    k.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
        effectiveFrom: "2026-01-01".into(),
        gunlukAsgariUcret: Some(dec!(200)),
        pekTavanKatsayisi: None,
        gunlukYemekIstisnasiSGK: None,
        gunlukYemekIstisnasiGV: None,
    }]);
    SettingsRepository::save_institution_settings(&conn, &k)?;

    let work_codes = vec!["Ç"; 30];
    AttendanceRepository::save(&conn, &attendance_from_codes(&p.id, &active, &work_codes))?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &active.id)?;
    assert_eq!(payroll.gelirToplam, dec!(30000));
    assert_eq!(payroll.kesintiler.damgaVergisi, Some(dec!(182.16)));
    Ok(())
}

#[test]
fn period_validation_enforces_15_14_but_keeps_tax_month_configurable(
) -> Result<(), Box<dyn std::error::Error>> {
    let good = period("period-good", 2026, 7, 2026, 7);
    PeriodRepository::validate_period(&good)?;

    let bad_geometry = BordroDonemi {
        bitisTarihi: "2026-09-14".into(),
        ..good.clone()
    };
    assert!(PeriodRepository::validate_period(&bad_geometry).is_err());

    let bad_metadata = BordroDonemi {
        ay: 8,
        ..good
    };
    assert!(PeriodRepository::validate_period(&bad_metadata).is_err());
    Ok(())
}
