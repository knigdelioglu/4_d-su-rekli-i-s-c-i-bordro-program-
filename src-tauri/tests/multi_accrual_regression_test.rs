use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period(id: &str, month: i32, tax_month: i32) -> BordroDonemi {
    let (end_year, end_month) = if month == 12 {
        (2027, 1)
    } else {
        (2026, month + 1)
    };
    BordroDonemi {
        id: id.into(),
        yil: 2026,
        ay: month,
        baslangicTarihi: format!("2026-{month:02}-15"),
        bitisTarihi: format!("{end_year}-{end_month:02}-14"),
        donemAdi: id.into(),
        taxYear: 2026,
        taxMonth: tax_month,
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
        gunlukAsgariUcret: Some(dec!(1101)),
        pekTavanKatsayisi: Some(dec!(3)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    }
}

fn attendance(personnel_id: &str, active: &BordroDonemi) -> PersonelPuantaj {
    let start = NaiveDate::parse_from_str(&active.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&active.bitisTarihi, "%Y-%m-%d").unwrap();
    let mut date = start;
    let mut gunler = HashMap::new();
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{personnel_id}_{}", active.id),
        personelId: personnel_id.into(),
        donemId: active.id.clone(),
        gunler,
    }
}

fn person(id: &str) -> Personel {
    Personel {
        id: id.into(),
        tcNo: "11111111111".into(),
        ad: "Multi".into(),
        soyad: "Accrual".into(),
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

fn serialized<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

#[test]
fn native_multi_accrual_keeps_finalized_normal_immutable_and_continues_into_october(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let personnel_id = "p-multi-native";
    let prior = BordroDonemi {
        id: "2025-12".into(),
        yil: 2025,
        ay: 12,
        baslangicTarihi: "2025-12-15".into(),
        bitisTarihi: "2026-01-14".into(),
        donemAdi: "2025-12".into(),
        taxYear: 2026,
        taxMonth: 1,
    };
    let active = period("2026-01", 1, 2);
    let next = period("2026-02", 2, 3);

    PersonnelRepository::save(&conn, &person(personnel_id))?;
    PeriodRepository::save(&conn, &prior)?;
    PeriodRepository::save(&conn, &active)?;
    PeriodRepository::save(&conn, &next)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&prior.id))?;
    SettingsRepository::save_institution_settings(&conn, &settings(&active.id))?;
    SettingsRepository::save_institution_settings(&conn, &settings(&next.id))?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;
    AttendanceRepository::save(&conn, &attendance(personnel_id, &active))?;
    AttendanceRepository::save(&conn, &attendance(personnel_id, &next))?;

    let calculated_normal =
        PayrollService::calculate_payroll_for_personnel(&conn, personnel_id, &active.id)?;
    let finalized_normal =
        PayrollService::finalize_payroll_for_personnel(&conn, personnel_id, &active.id)?;
    assert_eq!(finalized_normal.status, BordroStatus::FINALIZED);
    let normal_before = PayrollRepository::get_all(&conn)?
        .into_iter()
        .find(|record| record.accrualType == AccrualType::NORMAL)
        .expect("normal accrual should be persisted");

    let tediye_input = PayrollAccrualInput {
        accrualId: format!("{personnel_id}_2026-08_tediye_1"),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-02-20".into(),
        sequence: 1,
        grossAmount: Some(dec!(2000)),
        description: Some("Şubat tediye".into()),
    };
    let calculated_tediye = PayrollService::calculate_payroll_for_accrual(
        &conn,
        personnel_id,
        &active.id,
        Some(&tediye_input),
        None,
    )?;
    assert_eq!(calculated_tediye.status, BordroStatus::CALCULATED);
    assert_eq!(calculated_tediye.gelirler.tediye, Some(dec!(2000)));
    assert_eq!(calculated_tediye.gelirler.tabanBrutAylik, None);
    assert_eq!(
        calculated_tediye
            .gvDetay
            .as_ref()
            .unwrap()
            .oncekiKumulatifGvMatrahi,
        finalized_normal
            .gvDetay
            .as_ref()
            .unwrap()
            .yeniKumulatifGvMatrahi
    );
    assert_eq!(
        CumulativeTaxService::get_previous_cumulative_gv_for_accrual(
            &conn,
            personnel_id,
            &active,
            &tediye_input,
        )?,
        finalized_normal
            .gvDetay
            .as_ref()
            .unwrap()
            .yeniKumulatifGvMatrahi
    );

    let finalized_tediye = PayrollService::finalize_payroll_for_accrual(
        &conn,
        personnel_id,
        &active.id,
        Some(&tediye_input.accrualId),
    )?;
    assert_eq!(finalized_tediye.status, BordroStatus::FINALIZED);

    let normal_after = PayrollRepository::get_all(&conn)?
        .into_iter()
        .find(|record| record.accrualType == AccrualType::NORMAL)
        .expect("normal accrual should remain persisted");
    assert_eq!(normal_after.id, normal_before.id);
    assert_eq!(normal_after.status, BordroStatus::FINALIZED);
    assert_eq!(normal_after.netOdeme, normal_before.netOdeme);
    assert_eq!(
        serialized(&normal_after.gvDetay),
        serialized(&normal_before.gvDetay)
    );
    assert_eq!(
        serialized(&normal_after.damgaDetay),
        serialized(&normal_before.damgaDetay)
    );
    assert_eq!(
        serialized(&normal_after.pekDetay),
        serialized(&normal_before.pekDetay)
    );

    let october = PayrollService::calculate_payroll_for_personnel(&conn, personnel_id, &next.id)?;
    assert_eq!(october.status, BordroStatus::CALCULATED);
    assert_eq!(
        october.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
        finalized_tediye
            .gvDetay
            .as_ref()
            .unwrap()
            .yeniKumulatifGvMatrahi
    );

    // Keep the existing single-record assertion in the test as evidence that
    // the normal calculation itself was not replaced by the supplemental path.
    assert_eq!(calculated_normal.accrualType, AccrualType::NORMAL);
    Ok(())
}

/// A/C/G/H/I/J/O: actual SQLite saves/deletes, transaction rollback and frozen snapshots.
#[test]
fn payment_event_backdated_insert_delete_and_finalized_protection() -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
    let active = period("2026-07", 7, 8);
    let personnel_id = "payment-event-native";
    PersonnelRepository::save(&conn, &person(personnel_id))?;
    PeriodRepository::save(&conn, &active)?;
    SettingsRepository::save_institution_settings(&conn, &settings(&active.id))?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;
    AttendanceRepository::save(&conn, &attendance(personnel_id, &active))?;
    // Statutory reference months exist without requiring any salary event.
    for tax_month in 1..8 {
        let previous = BordroDonemi {
            id: format!("reference-{tax_month}"),
            yil: if tax_month == 1 { 2025 } else { 2026 },
            ay: if tax_month == 1 { 12 } else { tax_month - 1 },
            baslangicTarihi: if tax_month == 1 { "2025-12-15".into() } else { format!("2026-{:02}-15", tax_month - 1) },
            bitisTarihi: format!("2026-{tax_month:02}-14"),
            donemAdi: format!("Reference {tax_month}"), taxYear: 2026, taxMonth: tax_month,
        };
        PeriodRepository::save(&conn, &previous)?;
        SettingsRepository::save_institution_settings(&conn, &settings(&previous.id))?;
    }
    let input = |id: &str, kind, date: &str| PayrollAccrualInput {
        accrualId: id.into(), accrualType: kind, paymentDate: date.into(), sequence: 0,
        grossAmount: if kind == AccrualType::NORMAL { None } else { Some(dec!(2000)) }, description: None,
    };
    let normal = input("normal", AccrualType::NORMAL, "2026-08-14");
    let tis = input("tis", AccrualType::TIS_IKRAMIYE, "2026-08-25");
    let tediye = input("tediye", AccrualType::TEDIYE, "2026-08-10");
    let calculate = |event: &PayrollAccrualInput| {
        PayrollService::validate_payroll_request_for_accrual(&conn, personnel_id, &active.id, Some(event))?;
        PayrollService::calculate_payroll_for_accrual(&conn, personnel_id, &active.id, Some(event), None)
    };
    calculate(&normal)?;
    calculate(&tis)?;
    calculate(&tediye)?;
    let records = PayrollRepository::get_all(&conn)?;
    for id in ["normal", "tis"] {
        assert_eq!(records.iter().find(|record| record.accrualId == id).unwrap().status, BordroStatus::STALE);
    }
    calculate(&normal)?;
    calculate(&tis)?;
    PayrollRepository::delete_accrual(&conn, personnel_id, &active.id, "tediye")?;
    let records = PayrollRepository::get_all(&conn)?;
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.status == BordroStatus::STALE));
    // The first event can be created/calculated with no NORMAL at all.
    PayrollRepository::delete_accrual(&conn, personnel_id, &active.id, "normal")?;
    calculate(&tediye)?;
    let late_normal = calculate(&normal)?;
    let detail = late_normal.pekDetay.as_ref().unwrap();
    let first = PayrollRepository::get_all(&conn)?.into_iter().find(|r| r.accrualId == "tediye").unwrap();
    let used = first.pekDetay.as_ref().unwrap().primMatrahi;
    assert!(detail.pekAltSinir > used + detail.primMatrahi);
    assert_eq!(detail.altSinirTamamlamaFarki, detail.pekAltSinir - used - detail.primMatrahi);
    assert_eq!(detail.finalPek, detail.primMatrahi + detail.altSinirTamamlamaFarki);
    calculate(&tis)?;
    // Persist a finalized snapshot to isolate mutation protection from finalization preflight.
    let mut finalized = PayrollRepository::get_all(&conn)?.into_iter().find(|r| r.accrualId == "normal").unwrap();
    finalized.status = BordroStatus::FINALIZED;
    PayrollRepository::save_in_transaction(&conn, &finalized)?;
    let before = serialized(&PayrollRepository::get_all(&conn)?);
    let backdated = input("earlier", AccrualType::SUPPLEMENTAL, "2026-08-09");
    assert!(calculate(&backdated).is_err());
    assert_eq!(serialized(&PayrollRepository::get_all(&conn)?), before);
    assert!(calculate(&tediye).is_err());
    assert_eq!(serialized(&PayrollRepository::get_all(&conn)?), before);
    assert!(PayrollRepository::delete_accrual(&conn, personnel_id, &active.id, "tediye").is_err());
    assert_eq!(serialized(&PayrollRepository::get_all(&conn)?), before);
    assert!(PayrollRepository::delete_accrual(&conn, personnel_id, &active.id, "normal").is_err());
    assert_eq!(serialized(&PayrollRepository::get_all(&conn)?), before);
    Ok(())
}
