//! Boundary regression cases for the statutory snapshot and sick-leave paths.
//! Expected totals are calculated from the dates and rates in this fixture.
use chrono::NaiveDate;
use payroll_core::{
    calculate_paid_sick_dates_from_records, resolve_statutory_snapshot_for_period,
    resolve_statutory_snapshot_for_period_with_paid_sick_dates, BordroDonemi, SickLeaveRecord,
    StatutoryParameterSegment,
};
use rust_decimal_macros::dec;

#[path = "support/independent_audit.rs"]
mod audit_support;

fn sick(id: &str, start: &str, end: &str) -> SickLeaveRecord {
    SickLeaveRecord {
        id: id.into(),
        personnelId: "audit".into(),
        startDate: start.into(),
        endDate: end.into(),
        createdAt: None,
        updatedAt: None,
    }
}

#[test]
fn statutory_segment_boundary_counts_each_date_once() {
    let mut request = audit_support::dated_request(2026, 1);
    let period = request.dataset.periods[0].clone();
    let settings = request
        .dataset
        .institutionSettings
        .get_mut(&period.id)
        .unwrap();
    settings.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
        effectiveFrom: "2026-01-30".into(),
        gunlukAsgariUcret: Some(dec!(2200)),
        pekTavanKatsayisi: Some(dec!(2)),
        gunlukYemekIstisnasiSGK: Some(dec!(105)),
        gunlukYemekIstisnasiGV: Some(dec!(110)),
    }]);
    let snapshot =
        resolve_statutory_snapshot_for_period(&request.dataset.attendances[0], &period, settings)
            .unwrap();
    assert_eq!(snapshot.segments.len(), 2);
    assert_eq!(snapshot.segments[0].effectiveTo, "2026-01-29");
    assert_eq!(snapshot.segments[0].sgkPrimGunSayisi, 15);
    assert_eq!(snapshot.segments[1].sgkPrimGunSayisi, 15);
    assert_eq!(snapshot.sgkPrimGunSayisi, 30);
    assert_eq!(snapshot.pekAltSinir, dec!(49515));
    assert_eq!(snapshot.sgkYemekIstisnasiToplam, dec!(6180));
}

#[test]
fn incomplete_attendance_uses_recorded_days_and_paid_sick_restores_prim_day() {
    let mut request = audit_support::dated_request(2026, 1);
    let period = request.dataset.periods[0].clone();
    let attendance = &mut request.dataset.attendances[0];
    attendance.gunler.remove("2026-01-16");
    attendance.gunler.insert("2026-01-17".into(), "R".into());
    let settings = request.dataset.institutionSettings.get(&period.id).unwrap();
    let without = resolve_statutory_snapshot_for_period(attendance, &period, settings).unwrap();
    let paid = resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        attendance,
        &period,
        settings,
        &[NaiveDate::from_ymd_opt(2026, 1, 17).unwrap()],
    )
    .unwrap();
    assert_eq!(without.sgkPrimGunSayisi, 29);
    assert_eq!(paid.sgkPrimGunSayisi, 30);
    assert_eq!(without.segments[0].fiiliYemekGunu, 29);
    assert_eq!(paid.segments[0].fiiliYemekGunu, 29);
}

#[test]
fn sick_rule_accepts_only_first_two_days_of_first_five_episodes_per_year() {
    let period = BordroDonemi {
        id: "sick-boundary".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Ocak".into(),
        taxYear: 2026,
        taxMonth: 1,
    };
    let mut records = Vec::new();
    for index in 0..6 {
        let day = 15 + index * 3;
        records.push(sick(
            &format!("s{index}"),
            &format!("2026-01-{day:02}"),
            &format!("2026-01-{:02}", day + 1),
        ));
    }
    let dates = calculate_paid_sick_dates_from_records(&records, &period).unwrap();
    assert_eq!(dates.len(), 10);
    assert!(dates
        .iter()
        .any(|date| *date == NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()));
    assert!(!dates
        .iter()
        .any(|date| *date == NaiveDate::from_ymd_opt(2026, 1, 30).unwrap()));
}

#[test]
fn invalid_attendance_date_and_out_of_period_segment_fail_closed() {
    let mut request = audit_support::dated_request(2026, 1);
    let period = request.dataset.periods[0].clone();
    request.dataset.attendances[0]
        .gunler
        .insert("2026-02-15".into(), "Ç".into());
    let settings = request.dataset.institutionSettings.get(&period.id).unwrap();
    assert!(resolve_statutory_snapshot_for_period(
        &request.dataset.attendances[0],
        &period,
        settings
    )
    .is_err());

    let mut settings = settings.clone();
    settings.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
        effectiveFrom: "2026-02-15".into(),
        gunlukAsgariUcret: Some(dec!(2200)),
        pekTavanKatsayisi: None,
        gunlukYemekIstisnasiSGK: None,
        gunlukYemekIstisnasiGV: None,
    }]);
    assert!(resolve_statutory_snapshot_for_period(
        &request.dataset.attendances[0],
        &period,
        &settings
    )
    .is_err());
}
