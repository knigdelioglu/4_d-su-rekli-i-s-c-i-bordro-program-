use chrono::{Duration, NaiveDate};
use payroll_core::*;
use rust_decimal_macros::dec;
use serde_json::json;

pub fn request() -> PayrollCalculationRequest {
    let person: Personel = serde_json::from_value(json!({
        "id":"audit", "tcNo":"10000000000", "ad":"Audit", "soyad":"Fixture",
        "grup":"1. Grup", "sgkSicilNo":"", "iban":"", "hizmetYili":0,
        "kesintiler":{"besUyesi":true}
    }))
    .unwrap();
    let period = BordroDonemi {
        id: "audit-jan".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Audit January".into(),
        taxYear: 2026,
        taxMonth: 1,
    };
    let mut settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        gunlukTabanUcret: dec!(2000),
        gunlukYemek: dec!(200),
        gunlukVasitaYol: dec!(0),
        birlestirilmisSosyalYardim: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        ..Default::default()
    };
    for group in settings.isPrimiGruplari.as_mut().unwrap() {
        group.oran = dec!(0);
    }
    let mut attendance = PersonelPuantaj {
        id: "audit-attendance".into(),
        personelId: person.id.clone(),
        donemId: period.id.clone(),
        gunler: Default::default(),
    };
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    while date <= NaiveDate::from_ymd_opt(2026, 2, 14).unwrap() {
        attendance.gunler.insert(date.to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PayrollCalculationRequest {
        personnelId: person.id.clone(),
        periodId: period.id.clone(),
        calculatedAt: "2026-09-08T00:00:00Z".into(),
        manualIncome: None,
        accrual: None,
        dataset: PayrollDatasetSnapshot {
            personnel: vec![person],
            periods: vec![period.clone()],
            institutionSettings: [(period.id, settings)].into(),
            attendances: vec![attendance],
            annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
            ..Default::default()
        },
    }
}

#[allow(dead_code)]
pub fn supplementary(
    mut req: PayrollCalculationRequest,
    kind: AccrualType,
) -> PayrollCalculationRequest {
    req.accrual = Some(PayrollAccrualInput {
        accrualId: "audit-extra".into(),
        accrualType: kind,
        paymentDate: "2026-01-31".into(),
        sequence: 1,
        grossAmount: Some(dec!(10000)),
        description: None,
    });
    req
}

pub fn dated_request(year: i32, month: u32) -> PayrollCalculationRequest {
    let mut req = request();
    let start = NaiveDate::from_ymd_opt(year, month, 15).unwrap();
    let end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 14)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 14)
    }
    .unwrap();
    let id = format!("audit-{year}-{month:02}");
    let period = &mut req.dataset.periods[0];
    period.id = id.clone();
    period.yil = year;
    period.ay = month as i32;
    period.taxYear = year;
    period.taxMonth = month as i32;
    period.baslangicTarihi = start.to_string();
    period.bitisTarihi = end.to_string();
    let mut settings = req
        .dataset
        .institutionSettings
        .remove(&req.periodId)
        .unwrap();
    settings.donemId = id.clone();
    req.dataset.institutionSettings.insert(id.clone(), settings);
    req.periodId = id.clone();
    let attendance = &mut req.dataset.attendances[0];
    attendance.id = format!("attendance-{id}");
    attendance.donemId = id;
    attendance.gunler.clear();
    let mut date = start;
    while date <= end {
        attendance.gunler.insert(date.to_string(), "Ç".into());
        date += Duration::days(1);
    }
    req.dataset.annualPayrollParameters[0].year = year;
    req
}
