//! Road allowance dates are independent from the institution's wage raise calendar.
use crate::{
    round_currency_amount, BordroDonemi, DomainError, DonemselKurumDegerleri, PersonelPuantaj,
    Result,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;

pub fn validate_transport_rates(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<()> {
    let Some(changes) = &settings.transportRateChanges else {
        return Ok(());
    };
    let mut previous = None;
    for change in changes {
        let date = NaiveDate::parse_from_str(&change.effectiveFrom, "%Y-%m-%d").map_err(|_| {
            DomainError::ValidationError("Yol yardımı yürürlük tarihi geçersiz.".into())
        })?;
        if date.format("%Y-%m-%d").to_string() != change.effectiveFrom
            || change.effectiveFrom < period.baslangicTarihi
            || change.effectiveFrom > period.bitisTarihi
            || previous.is_some_and(|previous| date <= previous)
            || change.gunlukVasitaYol < Decimal::ZERO
        {
            return Err(DomainError::ValidationError("Yol yardımı tarihleri dönem içinde, tekrarsız ve artan sırada; günlük tutarlar sıfır veya pozitif olmalıdır.".into()));
        }
        previous = Some(date);
    }
    Ok(())
}

/// Caller validates the schedule once before resolving attendance/replay dates.
pub fn transport_rate_on(settings: &DonemselKurumDegerleri, date: &str) -> Decimal {
    settings
        .transportRateChanges
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|change| change.effectiveFrom.as_str() <= date)
        .map_or(settings.gunlukVasitaYol, |change| change.gunlukVasitaYol)
}

pub fn calculate_dated_transport(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
    attendance: &PersonelPuantaj,
) -> Result<Decimal> {
    validate_transport_rates(period, settings)?;
    let total: Decimal = attendance
        .gunler
        .iter()
        .filter(|(_, code)| matches!(code.as_str(), "Ç" | "GÇ"))
        .map(|(date, _)| transport_rate_on(settings, date))
        .sum();
    Ok(round_currency_amount(total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportRateChange;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn fixture() -> (BordroDonemi, DonemselKurumDegerleri) {
        (
            BordroDonemi {
                id: "2026-08".into(),
                yil: 2026,
                ay: 8,
                baslangicTarihi: "2026-08-15".into(),
                bitisTarihi: "2026-09-14".into(),
                donemAdi: "test".into(),
                taxYear: 2026,
                taxMonth: 9,
            },
            DonemselKurumDegerleri {
                gunlukVasitaYol: dec!(100),
                transportRateChanges: Some(vec![
                    TransportRateChange {
                        effectiveFrom: "2026-08-20".into(),
                        gunlukVasitaYol: dec!(120),
                    },
                    TransportRateChange {
                        effectiveFrom: "2026-09-05".into(),
                        gunlukVasitaYol: dec!(140),
                    },
                ]),
                ..Default::default()
            },
        )
    }
    #[test]
    fn dated_transport_counts_only_work_days_and_includes_effective_dates() {
        let (period, settings) = fixture();
        let attendance = PersonelPuantaj {
            id: "test".into(),
            personelId: "p".into(),
            donemId: period.id.clone(),
            gunler: HashMap::from([
                ("2026-08-19".into(), "Ç".into()),
                ("2026-08-20".into(), "GÇ".into()),
                ("2026-09-04".into(), "Ç".into()),
                ("2026-09-05".into(), "Ç".into()),
                ("2026-09-06".into(), "İ".into()),
                ("2026-09-07".into(), "GÇT".into()),
                ("2026-09-08".into(), "R".into()),
                ("2026-09-09".into(), "T".into()),
            ]),
        };
        assert_eq!(
            calculate_dated_transport(&period, &settings, &attendance).unwrap(),
            dec!(480)
        );
    }
    #[test]
    fn dated_transport_rejects_bad_dates_order_duplicates_and_negative_rates() {
        let (period, mut settings) = fixture();
        for date in ["2026-08-14", "2026-09-15", "2026-02-30", "2026-8-20", ""] {
            settings.transportRateChanges.as_mut().unwrap()[0].effectiveFrom = date.into();
            assert!(
                validate_transport_rates(&period, &settings).is_err(),
                "{date}"
            );
        }
        settings = fixture().1;
        settings.transportRateChanges.as_mut().unwrap().swap(0, 1);
        assert!(validate_transport_rates(&period, &settings).is_err());
        settings = fixture().1;
        settings.transportRateChanges.as_mut().unwrap()[1].effectiveFrom = "2026-08-20".into();
        assert!(validate_transport_rates(&period, &settings).is_err());
        settings = fixture().1;
        settings.transportRateChanges.as_mut().unwrap()[0].gunlukVasitaYol = dec!(-1);
        assert!(validate_transport_rates(&period, &settings).is_err());
    }
    #[test]
    fn dated_transport_allows_period_edges_zero_and_rate_decreases() {
        let (period, mut settings) = fixture();
        settings.transportRateChanges = Some(vec![
            TransportRateChange {
                effectiveFrom: period.baslangicTarihi.clone(),
                gunlukVasitaYol: dec!(90),
            },
            TransportRateChange {
                effectiveFrom: period.bitisTarihi.clone(),
                gunlukVasitaYol: Decimal::ZERO,
            },
        ]);
        validate_transport_rates(&period, &settings).unwrap();
        assert_eq!(transport_rate_on(&settings, "2026-08-15"), dec!(90));
        assert_eq!(transport_rate_on(&settings, "2026-09-14"), Decimal::ZERO);
    }
    #[test]
    fn old_settings_json_and_explicit_empty_schedule_keep_distinct_meanings() {
        let mut value = serde_json::to_value(fixture().1).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("transportRateChanges");
        let legacy: DonemselKurumDegerleri = serde_json::from_value(value.clone()).unwrap();
        assert!(legacy.transportRateChanges.is_none());
        assert_eq!(serde_json::to_value(&legacy).unwrap(), value);
        value["transportRateChanges"] = serde_json::json!([]);
        let current: DonemselKurumDegerleri = serde_json::from_value(value).unwrap();
        assert_eq!(current.transportRateChanges.unwrap().len(), 0);
    }
}
