use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::{
    SettingsRepository, ZAM_AYLARI_SETTING_KEY,
};
use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
use bordro_programi_lib::services::payroll_notice_service::{
    PayrollNoticeService, PayrollNoticeSeverity,
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

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Notice".into(),
        soyad: "Worker".into(),
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

fn settings(period_id: &str, daily_wage: rust_decimal::Decimal) -> DonemselKurumDegerleri {
    DonemselKurumDegerleri {
        donemId: period_id.into(),
        gunlukTabanUcret: daily_wage,
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
        geceCalismaPrimiYuzde: Some(dec!(0)),
        geceCalismaTatiliPrimiYuzde: Some(dec!(0)),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: Some(dec!(0)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        sendikaAidatiYuzde: Some(dec!(0)),
        sabitSendikaAidati: Some(dec!(0)),
        besOraniYuzde: Some(dec!(3)),
        sabitBesTutar: Some(dec!(0)),
        gunlukYemekIstisnasiSGK: Some(dec!(0)),
        gunlukYemekIstisnasiGV: Some(dec!(0)),
        gunlukAsgariUcret: Some(dec!(1101)),
        pekTavanKatsayisi: Some(dec!(9)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    }
}

fn full_attendance(personnel_id: &str, p: &BordroDonemi) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&p.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&p.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut gunler = HashMap::new();
    let mut current = start;
    while current <= end {
        gunler.insert(current.format("%Y-%m-%d").to_string(), "Ç".into());
        current += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{}_{}", personnel_id, p.id),
        personelId: personnel_id.into(),
        donemId: p.id.clone(),
        gunler,
    }
}

fn payroll_with_snapshots(personnel_id: &str, period_id: &str) -> BordroKaydi {
    BordroKaydi {
        id: format!("{}_{}", personnel_id, period_id),
        personelId: personnel_id.into(),
        donemId: period_id.into(),
        accrualId: format!("{}_{}", personnel_id, period_id),
        accrualType: AccrualType::NORMAL,
        paymentDate: String::new(),
        sequence: 0,
        accrualDescription: None,
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000)),
            ..GelirKalemleri::default()
        },
        gelirToplam: dec!(100000),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(0),
        netOdeme: dec!(100000),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: "2026-08-01T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-08-01T00:00:00Z".into(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(dec!(180000)),
        oncekiKumulatifAsgariGvMatrahi: Some(dec!(0)),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: Some(vec![DevredenPekKaydi {
            tutar: dec!(12000),
            kalanAySayisi: 1,
            kaynakDonemId: Some("2026-06".into()),
        }]),
        sonrakiDevredenPek: Some(Vec::new()),
        pekDetay: Some(PekDetayi {
            hesaplananPek: dec!(100000),
            hamPek: dec!(100000),
            devredenPekKullanilan: dec!(5000),
            primMatrahi: dec!(105000),
            finalPek: dec!(105000),
            devredenPekAşanTutar: dec!(0),
            pekAltSinir: dec!(33030),
            pekUstSinir: dec!(297270),
            altSinirTamamlamaFarki: dec!(0),
            fiiliYemekGunu: 30,
            yemekIstisnasiTutar: dec!(0),
            isverenSgkPrimi: Some(dec!(0)),
            isverenIssizlikPrimi: Some(dec!(0)),
            pekAltSinirTamamlamaIsverenPrimi: Some(dec!(0)),
            isverenPrimToplami: Some(dec!(0)),
            sgkIsverenOraniYuzde: Some(dec!(21.75)),
            isverenIssizlikOraniYuzde: Some(dec!(2)),
        }),
        isPrimiDetay: None,
        gvDetay: Some(GvHesapDetayi {
            oncekiKumulatifGvMatrahi: dec!(180000),
            cariGvMatrahi: dec!(50000),
            yeniKumulatifGvMatrahi: dec!(230000),
            brutGelirVergisi: dec!(8500),
            asgariUcretGvMatrahi: dec!(0),
            asgariUcretReferansKumulatifMatrahi: dec!(0),
            asgariUcretGvIstisnasi: dec!(0),
            ayniAyOncekiKullanilanGvIstisnasi: dec!(0),
            tahakkukOncesiKalanGvIstisnasi: dec!(0),
            uygulananGvIstisnasi: dec!(0),
            tahakkukSonrasiKalanGvIstisnasi: dec!(0),
            kesilenGelirVergisi: dec!(8500),
            dogumAskerlikGvIndirimi: dec!(0),
            sigortaGvIndirimAdayi: dec!(0),
            sigortaGvAylikLimiti: dec!(0),
            sigortaGvYillikKalanLimiti: dec!(0),
            uygulanabilirSigortaGvIndirimi: dec!(0),
        }),
        damgaDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
    }
}

fn setup_period_basics(conn: &rusqlite::Connection, p: &BordroDonemi, worker: &Personel) {
    PersonnelRepository::save(conn, worker).unwrap();
    PeriodRepository::save(conn, p).unwrap();
    SettingsRepository::save_institution_settings(conn, &settings(&p.id, dec!(1000))).unwrap();
    AnnualPayrollParametersRepository::save(conn, &AnnualPayrollParameters::default_for_2026())
        .unwrap();
    SettingsRepository::set_app_setting(conn, ZAM_AYLARI_SETTING_KEY, "[1,7]").unwrap();
}

#[test]
fn missing_attendance_is_a_critical_notice() {
    let conn = create_in_memory_connection().unwrap();
    let active = period("2026-04", 2026, 4, 2026, 5);
    let worker = person("p-missing-attendance");
    setup_period_basics(&conn, &active, &worker);

    let notices = PayrollNoticeService::get_period_notices(&conn, &active.id).unwrap();
    let missing = notices
        .iter()
        .find(|notice| notice.code == "MISSING_ATTENDANCE")
        .expect("missing attendance notice should exist");

    assert_eq!(missing.severity, PayrollNoticeSeverity::Critical);
    assert_eq!(missing.personnel_id.as_deref(), Some(worker.id.as_str()));
}

#[test]
fn raise_transition_notice_shows_previous_and_current_daily_wages() {
    let conn = create_in_memory_connection().unwrap();
    let worker = person("p-raise");
    PersonnelRepository::save(&conn, &worker).unwrap();

    let previous = period("2026-06", 2026, 6, 2026, 7);
    let active = period("2026-07", 2026, 7, 2026, 8);
    PeriodRepository::save(&conn, &previous).unwrap();
    PeriodRepository::save(&conn, &active).unwrap();
    SettingsRepository::save_institution_settings(&conn, &settings(&previous.id, dec!(1000)))
        .unwrap();
    SettingsRepository::save_institution_settings(&conn, &settings(&active.id, dec!(1100)))
        .unwrap();
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())
        .unwrap();
    SettingsRepository::set_app_setting(&conn, ZAM_AYLARI_SETTING_KEY, "[8]").unwrap();
    AttendanceRepository::save(&conn, &full_attendance(&worker.id, &active)).unwrap();

    let notices = PayrollNoticeService::get_period_notices(&conn, &active.id).unwrap();
    let raise = notices
        .iter()
        .find(|notice| notice.code == "RAISE_TRANSITION_PERIOD")
        .expect("raise transition notice should exist");

    assert_eq!(raise.severity, PayrollNoticeSeverity::Warning);
    assert!(raise.details.iter().any(|detail| detail.contains("1000")));
    assert!(raise.details.iter().any(|detail| detail.contains("1100")));
}

#[test]
fn fifth_and_sixth_sick_leave_episodes_are_reported_with_quota_warnings() {
    let conn = create_in_memory_connection().unwrap();
    let active = period("2026-03", 2026, 3, 2026, 4);
    let worker = person("p-sick-quota");
    setup_period_basics(&conn, &active, &worker);
    AttendanceRepository::save(&conn, &full_attendance(&worker.id, &active)).unwrap();

    for (index, (start, end)) in [
        ("2026-01-02", "2026-01-03"),
        ("2026-01-10", "2026-01-11"),
        ("2026-02-02", "2026-02-03"),
        ("2026-02-10", "2026-02-11"),
        ("2026-03-20", "2026-03-21"),
        ("2026-03-25", "2026-03-26"),
    ]
    .into_iter()
    .enumerate()
    {
        SickLeaveRepository::save(
            &conn,
            &SickLeaveRecord {
                id: format!("sick-{index}"),
                personnelId: worker.id.clone(),
                startDate: start.into(),
                endDate: end.into(),
                createdAt: None,
                updatedAt: None,
            },
        )
        .unwrap();
    }

    let notices = PayrollNoticeService::get_period_notices(&conn, &active.id).unwrap();
    assert!(notices
        .iter()
        .any(|notice| notice.code == "SICK_LEAVE_QUOTA_LAST_PAID"));
    assert!(notices
        .iter()
        .any(|notice| notice.code == "SICK_LEAVE_QUOTA_EXHAUSTED"));
}

#[test]
fn calculated_snapshot_reports_incoming_pek_last_month_and_tax_bracket_transition() {
    let conn = create_in_memory_connection().unwrap();
    let active = period("2026-07", 2026, 7, 2026, 8);
    let worker = person("p-result-notices");
    setup_period_basics(&conn, &active, &worker);
    AttendanceRepository::save(&conn, &full_attendance(&worker.id, &active)).unwrap();
    PayrollRepository::save(&conn, &payroll_with_snapshots(&worker.id, &active.id)).unwrap();

    let notices = PayrollNoticeService::get_period_notices(&conn, &active.id).unwrap();

    assert!(notices
        .iter()
        .any(|notice| notice.code == "INCOMING_PEK_CARRY"));
    assert!(notices
        .iter()
        .any(|notice| notice.code == "PEK_CARRY_LAST_MONTH"));
    let tax = notices
        .iter()
        .find(|notice| notice.code == "INCOME_TAX_BRACKET_TRANSITION")
        .expect("tax bracket transition notice should exist");
    assert_eq!(tax.severity, PayrollNoticeSeverity::Warning);
    assert!(tax.message.contains("%15"));
    assert!(tax.message.contains("%20"));
}
