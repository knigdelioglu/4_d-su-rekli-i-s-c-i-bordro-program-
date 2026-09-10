use super::{base_payment_request, decimal_from_cents};
use payroll_core::{
    calculate_payroll_checked, AccrualType, AnnualPayrollParameters, BordroStatus,
    ManualPayrollIncomeInput, PayrollAccrualInput, PersonelTaxOpening,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

fn cross_year_request(opening: Decimal) -> payroll_core::PayrollCalculationRequest {
    let mut request = crate::audit_fixture::request();
    request.dataset.periods[0].taxYear = 2025;
    request.dataset.periods[0].taxMonth = 12;
    request.dataset.periods[1].taxMonth = 1;
    let mut previous_year = AnnualPayrollParameters::default_for_2026();
    previous_year.year = 2025;
    request.dataset.annualPayrollParameters.push(previous_year);
    request.dataset.taxOpenings.push(PersonelTaxOpening {
        id: "property-year-opening".into(),
        personnelId: "audit".into(),
        year: 2025,
        gvCumulativeOpening: Some(opening),
        effectiveFromPeriodId: Some("source".into()),
        asgariGvCumulativeOpening: Some(opening),
        asgariGvEffectiveFromPeriodId: Some("source".into()),
        createdAt: None,
        updatedAt: None,
    });
    request.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(decimal_from_cents(40_000_000)),
        tisIkramiyesi: None,
    });
    request
}

fn supplementary_request(
    gross_cents: i64,
    accrual_id: &str,
    payment_date: &str,
    sequence: i32,
) -> payroll_core::PayrollCalculationRequest {
    let mut request = base_payment_request();
    request.accrual = Some(PayrollAccrualInput {
        accrualId: accrual_id.into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: payment_date.into(),
        sequence,
        grossAmount: Some(decimal_from_cents(gross_cents)),
        description: Some("Property payment event".into()),
    });
    request
}

proptest! {
    #![proptest_config(crate::property::proptest_config(96))]

    #[test]
    fn supplementary_event_is_deterministic_for_identical_input(
        gross_cents in 0i64..=15_000_000i64,
    ) {
        let request = supplementary_request(gross_cents, "property-event", "2026-01-20", 0);
        let first = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("generated supplementary request rejected: {error}"));
        let second = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("replayed supplementary request rejected: {error}"));

        prop_assert_eq!(serde_json::to_value(&first).unwrap(), serde_json::to_value(&second).unwrap());
        prop_assert_eq!(first.status, BordroStatus::CALCULATED);
        prop_assert_eq!(first.accrualId, "property-event");
        prop_assert_eq!(first.sequence, 0);
        prop_assert_eq!(first.gelirToplam, decimal_from_cents(gross_cents));
        prop_assert_eq!(first.netOdeme, (first.gelirToplam - first.kesintiToplam).round_dp(2));
    }

    #[test]
    fn same_month_authoritative_events_extend_tax_and_pek_state(
        first_cents in 0i64..=15_000_000i64,
        second_cents in 0i64..=15_000_000i64,
    ) {
        let first_request = supplementary_request(first_cents, "property-event-1", "2026-01-20", 0);
        let first = calculate_payroll_checked(&first_request)
            .unwrap_or_else(|error| panic!("first property event rejected: {error}"));
        let mut second_request = first_request;
        second_request.dataset.payrolls.push(first.clone());
        second_request.accrual = Some(PayrollAccrualInput {
            accrualId: "property-event-2".into(),
            accrualType: AccrualType::TEDIYE,
            paymentDate: "2026-01-21".into(),
            sequence: 1,
            grossAmount: Some(decimal_from_cents(second_cents)),
            description: Some("Property payment event 2".into()),
        });
        let second = calculate_payroll_checked(&second_request)
            .unwrap_or_else(|error| panic!("second property event rejected: {error}"));

        let first_gv = first.gvDetay.as_ref().expect("first GV detail");
        let second_gv = second.gvDetay.as_ref().expect("second GV detail");
        prop_assert_eq!(second_gv.oncekiKumulatifGvMatrahi, first_gv.cariGvMatrahi);
        prop_assert_eq!(
            second_gv.ayniAyOncekiKullanilanGvIstisnasi,
            first_gv.uygulananGvIstisnasi,
        );

        let first_stamp = first.damgaDetay.as_ref().expect("first stamp detail");
        let second_stamp = second.damgaDetay.as_ref().expect("second stamp detail");
        prop_assert_eq!(
            second_stamp.ayniAyOncekiKullanilanDamgaIstisnasi,
            first_stamp.uygulananDamgaIstisnasi,
        );

        let first_pek = first.pekDetay.as_ref().expect("first PEK detail");
        let second_pek = second.pekDetay.as_ref().expect("second PEK detail");
        prop_assert_eq!(
            second_pek.aylikOncekiPekTuketimi,
            first_pek.aylikSonrasiPekTuketimi,
        );
        prop_assert_eq!(second.status, BordroStatus::CALCULATED);
        prop_assert_eq!(second.netOdeme, (second.gelirToplam - second.kesintiToplam).round_dp(2));
    }

    #[test]
    fn new_tax_year_resets_cumulative_tax_but_keeps_pek_carry(
        opening_cents in 0i64..=50_000_000i64,
    ) {
        let mut request = cross_year_request(decimal_from_cents(opening_cents));
        let previous = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("previous tax-year payroll rejected: {error}"));
        let outgoing = previous
            .sonrakiDevredenPek
            .clone()
            .expect("previous year should produce carry");
        request.dataset.payrolls.push(previous);
        request.periodId = "payment".into();
        request.manualIncome = None;

        let current = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("new tax-year payroll rejected: {error}"));
        prop_assert_eq!(
            current
                .gvDetay
                .expect("new-year GV detail")
                .oncekiKumulatifGvMatrahi,
            Decimal::ZERO,
        );
        prop_assert_eq!(current.oncekiKumulatifAsgariGvMatrahi, Some(Decimal::ZERO));
        prop_assert_eq!(current.devredenPekGelen, Some(outgoing));
    }

    #[test]
    fn stale_prior_event_is_not_silently_skipped(
        opening_cents in 0i64..=50_000_000i64,
    ) {
        let mut request = cross_year_request(decimal_from_cents(opening_cents));
        let mut previous = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("previous tax-year payroll rejected: {error}"));
        previous.status = BordroStatus::STALE;
        request.dataset.payrolls.push(previous);
        request.periodId = "payment".into();
        request.manualIncome = None;

        prop_assert!(calculate_payroll_checked(&request).is_err());
    }
}
