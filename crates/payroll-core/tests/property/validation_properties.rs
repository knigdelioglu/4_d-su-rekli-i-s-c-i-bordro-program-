use super::{attendance_with_entry, decimal_from_cents, valid_period};
use chrono::{Duration, NaiveDate};
use payroll_core::{
    validate_attendance_for_period, validate_monetary_amount, validate_period,
    validate_sick_leave_records, SickLeaveRecord,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

proptest! {
    #![proptest_config(crate::property::proptest_config(96))]

    #[test]
    fn period_start_outside_fifteenth_is_rejected(day in 1u32..=14u32) {
        let mut period = valid_period();
        period.baslangicTarihi = format!("2026-01-{day:02}");
        prop_assert!(validate_period(&period).is_err());
    }

    #[test]
    fn attendance_outside_period_is_rejected(before_period in any::<bool>()) {
        let period = valid_period();
        let date = if before_period { "2026-01-14" } else { "2026-02-15" };
        let attendance = attendance_with_entry(date, "Ç");
        prop_assert!(validate_attendance_for_period(&attendance, &period).is_err());
    }

    #[test]
    fn malformed_attendance_date_is_rejected(invalid_date in prop::sample::select(vec![
        "2026-02-30".to_string(),
        "2026/01/15".to_string(),
        "not-a-date".to_string(),
    ])) {
        let period = valid_period();
        let attendance = attendance_with_entry(&invalid_date, "Ç");
        prop_assert!(validate_attendance_for_period(&attendance, &period).is_err());
    }

    #[test]
    fn unsupported_attendance_code_is_rejected(code in prop::sample::select(vec![
        "X".to_string(),
        "".to_string(),
        "ÇALIŞTI".to_string(),
    ])) {
        let period = valid_period();
        let attendance = attendance_with_entry("2026-01-15", &code);
        prop_assert!(validate_attendance_for_period(&attendance, &period).is_err());
    }

    #[test]
    fn overlapping_sick_episodes_are_rejected(start_offset in 0i64..=10i64) {
        let start = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
            + Duration::days(start_offset);
        let first_end = start + Duration::days(2);
        let second_start = start + Duration::days(1);
        let second_end = start + Duration::days(3);
        let records = vec![
            SickLeaveRecord {
                id: "sick-a".into(),
                personnelId: "property-person".into(),
                startDate: start.to_string(),
                endDate: first_end.to_string(),
                createdAt: None,
                updatedAt: None,
            },
            SickLeaveRecord {
                id: "sick-b".into(),
                personnelId: "property-person".into(),
                startDate: second_start.to_string(),
                endDate: second_end.to_string(),
                createdAt: None,
                updatedAt: None,
            },
        ];
        prop_assert!(validate_sick_leave_records(&records).is_err());
    }

    #[test]
    fn decimal_json_round_trip_preserves_exact_money(cents in -100_000_000i64..=100_000_000i64) {
        let original = decimal_from_cents(cents);
        let encoded = serde_json::to_string(&original).unwrap();
        let decoded: Decimal = serde_json::from_str(&encoded).unwrap();
        prop_assert_eq!(decoded, original);
        prop_assert!(!encoded.contains('e'));
        prop_assert!(!encoded.contains('E'));
    }

    #[test]
    fn monetary_values_with_thousandths_are_rejected(
        whole_cents in 0i64..=1_000_000i64,
        extra_thousandths in 1i64..=9i64,
    ) {
        let amount = Decimal::new(whole_cents * 10 + extra_thousandths, 3);
        prop_assert!(validate_monetary_amount("property", amount).is_err());
    }
}
