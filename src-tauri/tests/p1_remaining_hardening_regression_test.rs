use bordro_programi_lib::db::{create_in_memory_connection, migrations::initialize_db};
use bordro_programi_lib::domain::calculations::{
    calculate_gv_indirimleri, validate_kurum_degerleri_for_payroll, validate_pek_bounds,
};
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::DomainError;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::repositories::{dec_to_kurus, opt_dec_to_kurus};
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Duration, NaiveDate};
use rusqlite::params;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period(id: &str, yil: i32, ay: i32, tax_year: i32, tax_month: i32) -> BordroDonemi {
    let start = NaiveDate::from_ymd_opt(yil, ay as u32, 15).unwrap();
    let (end_y, end_m) = if ay == 12 {
        (yil + 1, 1)
    } else {
        (yil, ay + 1)
    };
    let end = NaiveDate::from_ymd_opt(end_y, end_m as u32, 14).unwrap();
    BordroDonemi {
        id: id.into(),
        yil,
        ay,
        baslangicTarihi: start.format("%Y-%m-%d").to_string(),
        bitisTarihi: end.format("%Y-%m-%d").to_string(),
        donemAdi: id.into(),
        taxYear: tax_year,
        taxMonth: tax_month,
    }
}

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: format!(
            "1{:010}",
            id.bytes().map(u64::from).sum::<u64>() % 10_000_000_000
        ),
        ad: "P1".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: format!("sgk-{id}"),
        iban: "TR000000000000000000000000".into(),
        hizmetYili: 5,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri {
            sendikaUyesi: Some(false),
            sabitSendikaAidati: None,
            besUyesi: Some(false),
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

fn attendance_for(period: &BordroDonemi, personnel_id: &str) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    for offset in 0..30 {
        gunler.insert(
            (start + Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string(),
            "Ç".to_string(),
        );
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, period.id),
        personelId: personnel_id.into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn setup_payroll_inputs(
    conn: &rusqlite::Connection,
    personnel: &Personel,
    p: &BordroDonemi,
) -> Result<(), DomainError> {
    PersonnelRepository::save(conn, personnel)?;
    PeriodRepository::save(conn, p)?;
    let settings = DonemselKurumDegerleri {
        donemId: p.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    SettingsRepository::save_institution_settings(conn, &settings)?;
    let mut annual = AnnualPayrollParameters::default_for_2026();
    annual.year = p.taxYear;
    if p.taxYear != 2026 {
        annual.sigortaGvYillikBrutAsgariUcretTavani = Some(dec!(396360));
    }
    AnnualPayrollParametersRepository::save(conn, &annual)?;
    AttendanceRepository::save(conn, &attendance_for(p, &personnel.id))?;
    Ok(())
}

#[test]
fn period_repository_rejects_duplicate_tax_year_month() {
    let conn = create_in_memory_connection().unwrap();
    let p1 = period("2026-05", 2026, 5, 2026, 6);
    let p2 = period("2026-06", 2026, 6, 2026, 6);
    PeriodRepository::save(&conn, &p1).unwrap();
    let err = PeriodRepository::save(&conn, &p2).unwrap_err();
    assert!(matches!(err, DomainError::ValidationError(message) if message.contains("çakışması")));
}

#[test]
fn unique_index_rejects_raw_duplicate_tax_month() {
    let conn = create_in_memory_connection().unwrap();
    let p1 = period("2026-05", 2026, 5, 2026, 6);
    PeriodRepository::save(&conn, &p1).unwrap();
    let p2 = period("2026-06", 2026, 6, 2026, 6);
    let result = conn.execute(
        "INSERT INTO payroll_periods (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'x')",
        params![p2.id,p2.yil,p2.ay,p2.baslangicTarihi,p2.bitisTarihi,p2.donemAdi,p2.taxYear,p2.taxMonth],
    );
    assert!(result.is_err());
}

#[test]
fn initialize_db_fails_explicitly_when_legacy_tax_month_duplicates_exist() {
    let mut conn = create_in_memory_connection().unwrap();
    conn.execute("DROP INDEX idx_payroll_periods_tax_year_month", [])
        .unwrap();
    let p1 = period("2026-05", 2026, 5, 2026, 6);
    let p2 = period("2026-06", 2026, 6, 2026, 6);
    for p in [p1, p2] {
        conn.execute(
            "INSERT INTO payroll_periods (id,yil,ay,baslangic_tarihi,bitis_tarihi,donem_adi,tax_year,tax_month,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'x')",
            params![p.id,p.yil,p.ay,p.baslangicTarihi,p.bitisTarihi,p.donemAdi,p.taxYear,p.taxMonth],
        ).unwrap();
    }
    conn.pragma_update(None, "user_version", 10u32).unwrap();
    let err = initialize_db(&mut conn).unwrap_err().to_string();
    assert!(err.contains("çakışması"));
}

#[test]
fn insurance_and_borrowing_gv_deductions_apply_legal_limits() {
    let det = calculate_gv_indirimleri(
        dec!(100000),
        dec!(5000),
        dec!(10000),
        dec!(12000),
        dec!(396360),
        dec!(390000),
    );
    assert_eq!(det.dogum_askerlik_indirimi, dec!(5000));
    assert_eq!(det.sigorta_adayi, dec!(17000));
    assert_eq!(det.sigorta_aylik_limiti, dec!(15000));
    assert_eq!(det.sigorta_yillik_kalan_limiti, dec!(6360));
    assert_eq!(det.uygulanabilir_sigorta_indirimi, dec!(6360));
}

#[test]
fn pek_bounds_fail_closed_when_ceiling_is_below_floor() {
    let err = validate_pek_bounds(dec!(1000), dec!(999)).unwrap_err();
    assert!(
        matches!(err, DomainError::ValidationError(message) if message.contains("PEK sınırları"))
    );
}

#[test]
fn institution_validation_rejects_pek_multiplier_below_one() {
    let mut settings = DonemselKurumDegerleri::default();
    settings.pekTavanKatsayisi = Some(dec!(0.5));
    let err = validate_kurum_degerleri_for_payroll(&settings).unwrap_err();
    assert!(
        matches!(err, DomainError::ValidationError(message) if message.contains("PEK tavan katsayısı"))
    );
}

#[test]
fn personnel_repository_rejects_oks_rate_below_three_percent() {
    let conn = create_in_memory_connection().unwrap();
    let mut p = person("oks-low");
    p.kesintiler = Some(PersonelKesintileri {
        sendikaUyesi: Some(false),
        sabitSendikaAidati: None,
        besUyesi: Some(true),
        oksOraniYuzde: Some(dec!(2.99)),
        sabitBesTutar: None,
        icraTutar: None,
        kisiBorcuTutar: None,
        dogumAskerlikBorclanmasiTutar: None,
        hayatSaglikSigortasiTutar: None,
        digerKesintiTutar: None,
        gvIndirimleri: None,
    });
    let err = PersonnelRepository::save(&conn, &p).unwrap_err();
    assert!(
        matches!(err, DomainError::ValidationError(message) if message.contains("OKS özel oranı"))
    );
}

#[test]
fn decimal_to_kurus_overflow_is_an_error_not_zero() {
    assert!(dec_to_kurus(Some(Decimal::MAX)).is_err());
    assert!(opt_dec_to_kurus(Some(Decimal::MAX)).is_err());
}

fn payroll_record(
    personnel_id: &str,
    period_id: &str,
    gross: Decimal,
    deductions: Decimal,
) -> BordroKaydi {
    let net = (gross - deductions).round_dp(2);
    BordroKaydi {
        id: format!("{}_{}", personnel_id, period_id),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(gross),
            ..GelirKalemleri::default()
        },
        gelirToplam: gross,
        kesintiler: KesintiKalemleri {
            digerKesinti: Some(deductions),
            ..KesintiKalemleri::default()
        },
        kesintiToplam: deductions,
        netOdeme: net,
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: "2026-08-16T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-08-16T00:00:00Z".into(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(Decimal::ZERO),
        oncekiKumulatifAsgariGvMatrahi: Some(Decimal::ZERO),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: Some(vec![]),
        sonrakiDevredenPek: Some(vec![]),
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
    }
}

#[test]
fn payroll_persistence_rejects_negative_net() {
    let conn = create_in_memory_connection().unwrap();
    let p = person("neg-net");
    let donem = period("2026-05", 2026, 5, 2026, 6);
    PersonnelRepository::save(&conn, &p).unwrap();
    PeriodRepository::save(&conn, &donem).unwrap();
    let bordro = payroll_record(&p.id, &donem.id, dec!(1000), dec!(1200));
    let err = PayrollRepository::save_in_transaction(&conn, &bordro).unwrap_err();
    assert!(matches!(err, DomainError::NegativeNetPayment { .. }));
}

#[test]
fn annual_2026_parameters_persist_insurance_gv_cap() {
    let conn = create_in_memory_connection().unwrap();
    let annual = AnnualPayrollParameters::default_for_2026();
    AnnualPayrollParametersRepository::save(&conn, &annual).unwrap();
    let loaded = AnnualPayrollParametersRepository::get_by_year(&conn, 2026)
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.sigortaGvYillikBrutAsgariUcretTavani,
        Some(dec!(396360))
    );
}

#[test]
fn payroll_service_surfaces_negative_net_instead_of_saving_debt_as_salary() {
    let conn = create_in_memory_connection().unwrap();
    let mut p = person("svc-neg");
    if let Some(k) = p.kesintiler.as_mut() {
        k.digerKesintiTutar = Some(dec!(1_000_000));
    }
    let donem = period("2026-05", 2026, 5, 2026, 6);
    setup_payroll_inputs(&conn, &p, &donem).unwrap();
    let err = PayrollService::calculate_payroll_for_personnel(&conn, &p.id, &donem.id).unwrap_err();
    assert!(matches!(err, DomainError::NegativeNetPayment { .. }));
    assert!(
        PayrollRepository::get_status_and_created_at(&conn, &p.id, &donem.id)
            .unwrap()
            .is_none()
    );
}
