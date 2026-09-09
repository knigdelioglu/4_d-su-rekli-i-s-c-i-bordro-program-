use chrono::{Duration, NaiveDate};
use payroll_core::*;
use rust_decimal_macros::dec;
use serde_json::json;
use std::collections::HashMap;

pub fn request() -> PayrollCalculationRequest {
    let mut dataset = PayrollDatasetSnapshot::default();
    dataset.personnel.push(
        serde_json::from_value(json!({
            "id":"audit", "tcNo":"10000000000", "ad":"Audit", "soyad":"Fixture",
            "grup":"1. Grup", "sgkSicilNo":"", "iban":"", "hizmetYili":0
        }))
        .unwrap(),
    );
    dataset
        .annualPayrollParameters
        .push(AnnualPayrollParameters::default_for_2026());
    for (id, start, end, month) in [
        ("source", "2025-12-15", "2026-01-14", 1),
        ("payment", "2026-01-15", "2026-02-14", 2),
    ] {
        let mut day = NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        let last = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
        let mut days = HashMap::new();
        while day <= last {
            days.insert(day.to_string(), "Ç".to_owned());
            day += Duration::days(1);
        }
        dataset.periods.push(BordroDonemi {
            id: id.into(),
            yil: if month == 1 { 2025 } else { 2026 },
            ay: if month == 1 { 12 } else { 1 },
            baslangicTarihi: start.into(),
            bitisTarihi: end.into(),
            donemAdi: id.into(),
            taxYear: 2026,
            taxMonth: month,
        });
        dataset.attendances.push(PersonelPuantaj {
            id: id.into(),
            personelId: "audit".into(),
            donemId: id.into(),
            gunler: days,
        });
        let mut settings = DonemselKurumDegerleri {
            donemId: id.into(),
            gunlukTabanUcret: dec!(2000),
            gunlukYemek: dec!(300),
            ..Default::default()
        };
        settings.gunlukYemekIstisnasiGV = Some(dec!(300));
        dataset.institutionSettings.insert(id.into(), settings);
    }
    PayrollCalculationRequest {
        personnelId: "audit".into(),
        periodId: "source".into(),
        calculatedAt: "2026-02-20T00:00:00Z".into(),
        manualIncome: None,
        accrual: None,
        dataset,
    }
}
