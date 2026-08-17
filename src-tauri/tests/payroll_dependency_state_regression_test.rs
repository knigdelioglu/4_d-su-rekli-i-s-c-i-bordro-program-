use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
use bordro_programi_lib::services::payroll_preflight_service::PayrollPreflightService;
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

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Dependency".into(),
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
        kesintiler: Some(PersonelKesintileri::default()),
    }
}

fn person_with_asgari_opening(id: &str, year: i32, start_month: i32) -> Personel {
    let mut p = person(id);
    p.devirKumulatifAsgariGvMatrahi = Some(dec!(0));
    p.devirKumulatifAsgariGvMatrahiYili = Some(year);
    p.devirKumulatifGvMatrahiBaslangicAyi = Some(start_month);
    p
}

fn settings(period_id: &str, daily_minimum: rust_decimal::Decimal) -> DonemselKurumDegerleri {
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
        gunlukAsgariUcret: Some(daily_minimum),
        pekTavanKatsayisi: Some(dec!(3)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    }
}

fn attendance(personnel_id: &str, p: &BordroDonemi, code: &str) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&p.baslangicTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    for offset in 0..30 {
        gunler.insert(
            (start + Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string(),
            code.to_string(),
        );
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, p.id),
        personelId: personnel_id.into(),
        donemId: p.id.clone(),
        gunler,
    }
}

fn calculated_payroll(
    personnel_id: &str,
    period_id: &str,
    outgoing: Option<Vec<DevredenPekKaydi>>,
) -> BordroKaydi {
    BordroKaydi {
        id: format!("{}_{}", personnel_id, period_id),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(dec!(100)),
            ..GelirKalemleri::default()
        },
        gelirToplam: dec!(100),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(0),
        netOdeme: dec!(100),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(dec!(0)),
        oncekiKumulatifAsgariGvMatrahi: Some(dec!(0)),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: outgoing,
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
    }
}

fn status(
    conn: &rusqlite::Connection,
    personnel_id: &str,
    period_id: &str,
) -> BordroStatus {
    PayrollRepository::get_status_and_created_at(conn, personnel_id, period_id)
        .unwrap()
        .unwrap()
        .0
}

#[test]
fn attendance_mutation_marks_current_and_later_payrolls_stale() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p-att");
    PersonnelRepository::save(&conn, &p).unwrap();

    let january = period("2026-01", 2026, 1, 2026, 1);
    let february = period("2026-02", 2026, 2, 2026, 2);
    PeriodRepository::save(&conn, &january).unwrap();
    PeriodRepository::save(&conn, &february).unwrap();

    AttendanceRepository::save(&conn, &attendance(&p.id, &january, "Ç")).unwrap();
    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &january.id, None)).unwrap();
    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &february.id, None)).unwrap();

    let mut changed = attendance(&p.id, &january, "Ç");
    let first_date = changed.gunler.keys().next().unwrap().clone();
    changed.gunler.insert(first_date, "R".into());
    AttendanceRepository::save(&conn, &changed).unwrap();

    assert_eq!(status(&conn, &p.id, &january.id), BordroStatus::STALE);
    assert_eq!(status(&conn, &p.id, &february.id), BordroStatus::STALE);
}

#[test]
fn institution_setting_mutation_marks_calculated_payroll_stale() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p-settings");
    PersonnelRepository::save(&conn, &p).unwrap();
    let active = period("2026-03", 2026, 3, 2026, 3);
    PeriodRepository::save(&conn, &active).unwrap();

    let original = settings(&active.id, dec!(1000));
    SettingsRepository::save_institution_settings(&conn, &original).unwrap();
    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &active.id, None)).unwrap();

    let mut changed = original;
    changed.gunlukTabanUcret = dec!(1100);
    SettingsRepository::save_institution_settings(&conn, &changed).unwrap();

    assert_eq!(status(&conn, &p.id, &active.id), BordroStatus::STALE);
}

#[test]
fn sick_leave_and_tax_opening_mutations_invalidate_calculated_payroll() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p-source");
    PersonnelRepository::save(&conn, &p).unwrap();
    let active = period("2026-04", 2026, 4, 2026, 4);
    PeriodRepository::save(&conn, &active).unwrap();
    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &active.id, None)).unwrap();

    SickLeaveRepository::save(
        &conn,
        &SickLeaveRecord {
            id: "r-1".into(),
            personnelId: p.id.clone(),
            startDate: "2026-04-20".into(),
            endDate: "2026-04-21".into(),
            createdAt: None,
            updatedAt: None,
        },
    )
    .unwrap();
    assert_eq!(status(&conn, &p.id, &active.id), BordroStatus::STALE);

    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &active.id, None)).unwrap();
    TaxOpeningRepository::save(
        &conn,
        &PersonelTaxOpening {
            id: "opening-1".into(),
            personnelId: p.id.clone(),
            year: 2026,
            gvCumulativeOpening: dec!(1000),
            effectiveFromPeriodId: active.id.clone(),
            createdAt: None,
            updatedAt: None,
        },
    )
    .unwrap();
    assert_eq!(status(&conn, &p.id, &active.id), BordroStatus::STALE);
}

#[test]
fn annual_parameter_mutation_invalidates_same_tax_year() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p-annual");
    PersonnelRepository::save(&conn, &p).unwrap();
    let active = period("2026-05", 2026, 5, 2026, 5);
    PeriodRepository::save(&conn, &active).unwrap();

    let original = AnnualPayrollParameters::default_for_2026();
    AnnualPayrollParametersRepository::save(&conn, &original).unwrap();
    PayrollRepository::save(&conn, &calculated_payroll(&p.id, &active.id, None)).unwrap();

    let mut changed = original;
    changed.sigortaGvYillikBrutAsgariUcretTavani = Some(dec!(400000));
    AnnualPayrollParametersRepository::save(&conn, &changed).unwrap();

    assert_eq!(status(&conn, &p.id, &active.id), BordroStatus::STALE);
}

#[test]
fn reversed_tax_month_order_is_rejected() {
    let conn = create_in_memory_connection().unwrap();
    let first = period("work-jan", 2026, 1, 2026, 3);
    let second = period("work-feb", 2026, 2, 2026, 2);
    PeriodRepository::save(&conn, &first).unwrap();

    let error = PeriodRepository::save(&conn, &second).unwrap_err();
    assert!(error.to_string().contains("Vergi kronolojisi"));
}

#[test]
fn missing_tax_month_period_fails_closed() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("p-tax-gap");
    PersonnelRepository::save(&conn, &p).unwrap();

    let tax_month_1 = period("tax-1", 2026, 1, 2026, 1);
    let tax_month_3 = period("tax-3", 2026, 2, 2026, 3);
    PeriodRepository::save(&conn, &tax_month_1).unwrap();
    PeriodRepository::save(&conn, &tax_month_3).unwrap();

    let error = PayrollPreflightService::validate_for_calculation(&conn, &p.id, &tax_month_3.id)
        .unwrap_err();
    assert!(error.to_string().contains("referans zinciri eksik"));
}

#[test]
fn tax_month_minimum_wage_reference_mismatch_fails_closed() {
    let conn = create_in_memory_connection().unwrap();
    let p = person_with_asgari_opening("p-tax-ref", 2026, 12);
    PersonnelRepository::save(&conn, &p).unwrap();

    let active = period("2026-12", 2026, 12, 2026, 12);
    PeriodRepository::save(&conn, &active).unwrap();
    let mut k = settings(&active.id, dec!(1000));
    k.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
        effectiveFrom: "2027-01-01".into(),
        gunlukAsgariUcret: Some(dec!(1200)),
        pekTavanKatsayisi: None,
        gunlukYemekIstisnasiSGK: None,
        gunlukYemekIstisnasiGV: None,
    }]);
    SettingsRepository::save_institution_settings(&conn, &k).unwrap();

    let error = PayrollPreflightService::validate_for_calculation(&conn, &p.id, &active.id)
        .unwrap_err();
    assert!(error.to_string().contains("Yanlış GV/DV istisnası"));
}

#[test]
fn live_deferred_pek_cannot_silently_disappear_across_missing_payroll() {
    let conn = create_in_memory_connection().unwrap();
    let p = person_with_asgari_opening("p-pek-gap", 2026, 7);
    PersonnelRepository::save(&conn, &p).unwrap();

    let june = period("2026-06", 2026, 6, 2026, 7);
    let july = period("2026-07", 2026, 7, 2026, 8);
    let august = period("2026-08", 2026, 8, 2026, 9);
    PeriodRepository::save(&conn, &june).unwrap();
    PeriodRepository::save(&conn, &july).unwrap();
    PeriodRepository::save(&conn, &august).unwrap();
    SettingsRepository::save_institution_settings(&conn, &settings(&august.id, dec!(1000))).unwrap();

    PayrollRepository::save(
        &conn,
        &calculated_payroll(
            &p.id,
            &june.id,
            Some(vec![DevredenPekKaydi {
                tutar: dec!(20000),
                kalanAySayisi: 2,
                kaynakDonemId: Some(june.id.clone()),
            }]),
        ),
    )
    .unwrap();

    let error = PayrollPreflightService::validate_for_calculation(&conn, &p.id, &august.id)
        .unwrap_err();
    assert!(error.to_string().contains("devreden PEK"));
    assert!(error.to_string().contains("ara dönem bordrosunu"));
}
