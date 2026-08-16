use bordro_programi_lib::db::connection::create_in_memory_connection;
use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::domain::{DomainError, Result};
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::payroll_service::PayrollService;
use rusqlite::params;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: format!("10000000{}", id.len()),
        ad: "Manuel".into(),
        soyad: "Gelir".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: String::new(),
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

fn setup(personnel_id: &str) -> Result<rusqlite::Connection> {
    let conn =
        create_in_memory_connection().map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    let p = person(personnel_id);
    let d = period();
    PersonnelRepository::save(&conn, &p)?;
    PeriodRepository::save(&conn, &d)?;
    let mut settings = DonemselKurumDegerleri {
        donemId: d.id.clone(),
        ..DonemselKurumDegerleri::default()
    };
    settings.tediyeListesi = Some(vec![TediyeKalemi {
        id: 1,
        ad: "Legacy aktif tediye".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 13,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(9999)),
    }]);
    settings.tisIkramiyeListesi = Some(vec![TisIkramiyeKalemi {
        id: 1,
        ad: "Legacy aktif TİS".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 30,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(8888)),
    }]);
    SettingsRepository::save_institution_settings(&conn, &settings)?;
    let mut gunler = HashMap::new();
    gunler.insert("2026-05-15".into(), "Ç".into());
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: format!("{}_{}", personnel_id, d.id),
            personelId: personnel_id.into(),
            donemId: d.id,
            gunler,
        },
    )?;
    Ok(conn)
}

#[test]
fn legacy_period_lists_do_not_auto_create_tediye_or_tis() -> Result<()> {
    let mut settings = DonemselKurumDegerleri::default();
    settings.tediyeListesi = Some(vec![TediyeKalemi {
        id: 1,
        ad: "Aktif".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 13,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(5000)),
    }]);
    settings.tisIkramiyeListesi = Some(vec![TisIkramiyeKalemi {
        id: 1,
        ad: "Aktif".into(),
        odemeAyi: "Haziran".into(),
        gunSayisi: 30,
        aktifDonemdeOdensin: true,
        sabitTutar: Some(dec!(6000)),
    }]);
    let summary = PuantajOzeti {
        c: 1,
        ..Default::default()
    };
    let (income, _) = auto_fill_gelirler_from_puantaj(&summary, &settings, 1, Some("1. Grup"))?;
    assert_eq!(income.tediye, None);
    assert_eq!(income.tisIkramiyesi, None);
    Ok(())
}

#[test]
fn production_payroll_uses_only_explicit_manual_tediye_and_tis() -> Result<()> {
    let conn = setup("manual-income")?;
    let manual = ManualPayrollIncomeInput {
        tediye: Some(dec!(1000.25)),
        tisIkramiyesi: Some(dec!(2000.75)),
    };
    let payroll = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "manual-income",
        "2026-05",
        Some(&manual),
    )?;
    assert_eq!(payroll.gelirler.tediye, Some(dec!(1000.25)));
    assert_eq!(payroll.gelirler.tisIkramiyesi, Some(dec!(2000.75)));

    for (kind, expected) in [("tediye", dec!(1000.25)), ("tisIkramiyesi", dec!(2000.75))] {
        let (amount, source): (i64, String) = conn
            .query_row(
                "SELECT amount, source FROM payroll_income_items WHERE payroll_id = ?1 AND item_type = ?2",
                params![payroll.id, kind],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        assert_eq!(
            amount,
            (expected * dec!(100))
                .round()
                .to_string()
                .parse::<i64>()
                .unwrap()
        );
        assert_eq!(source, "MANUAL");
    }
    Ok(())
}

#[test]
fn omitted_manual_input_does_not_fall_back_to_legacy_active_lists() -> Result<()> {
    let conn = setup("no-manual")?;
    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, "no-manual", "2026-05")?;
    assert_eq!(payroll.gelirler.tediye, None);
    assert_eq!(payroll.gelirler.tisIkramiyesi, None);
    Ok(())
}

#[test]
fn negative_manual_tediye_fails_before_persistence() -> Result<()> {
    let conn = setup("negative-tediye")?;
    let manual = ManualPayrollIncomeInput {
        tediye: Some(dec!(-0.01)),
        tisIkramiyesi: None,
    };
    let result = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "negative-tediye",
        "2026-05",
        Some(&manual),
    );
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    assert!(PayrollRepository::get_all(&conn)?.is_empty());
    Ok(())
}

#[test]
fn negative_manual_tis_fails_before_persistence() -> Result<()> {
    let conn = setup("negative-tis")?;
    let manual = ManualPayrollIncomeInput {
        tediye: None,
        tisIkramiyesi: Some(dec!(-0.01)),
    };
    let result = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "negative-tis",
        "2026-05",
        Some(&manual),
    );
    assert!(matches!(result, Err(DomainError::ValidationError(_))));
    assert!(PayrollRepository::get_all(&conn)?.is_empty());
    Ok(())
}

#[test]
fn explicit_blank_manual_input_clears_previous_manual_amounts() -> Result<()> {
    let conn = setup("clear-manual")?;
    let first = ManualPayrollIncomeInput {
        tediye: Some(dec!(1000)),
        tisIkramiyesi: Some(dec!(2000)),
    };
    PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "clear-manual",
        "2026-05",
        Some(&first),
    )?;

    let cleared = ManualPayrollIncomeInput::default();
    let payroll = PayrollService::calculate_payroll_for_personnel_with_manual_income(
        &conn,
        "clear-manual",
        "2026-05",
        Some(&cleared),
    )?;
    assert_eq!(payroll.gelirler.tediye, None);
    assert_eq!(payroll.gelirler.tisIkramiyesi, None);
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM payroll_income_items WHERE payroll_id = ?1 AND item_type IN ('tediye', 'tisIkramiyesi')",
            params![payroll.id],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    assert_eq!(count, 0);
    Ok(())
}
