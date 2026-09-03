use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, finalize_payroll, validate_payroll_request, AnnualPayrollParameters,
    BordroDonemi, BordroStatus, DonemselKurumDegerleri, ManualPayrollIncomeInput,
    PayrollCalculationRequest, PayrollDatasetSnapshot, Personel, PersonelPuantaj,
};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn fixture_request() -> PayrollCalculationRequest {
    let period = BordroDonemi {
        id: "2026-01".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Ocak 2026".into(),
        taxYear: 2026,
        taxMonth: 2,
    };

    let mut gunler = HashMap::new();
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid fixture date");
    let end = NaiveDate::from_ymd_opt(2026, 2, 14).expect("valid fixture date");
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }

    let settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        ..Default::default()
    };

    PayrollCalculationRequest {
        personnelId: "person-1".into(),
        periodId: period.id.clone(),
        calculatedAt: "2026-09-03T00:00:00.000Z".into(),
        manualIncome: Some(ManualPayrollIncomeInput {
            tediye: Some(dec!(75000.10)),
            tisIkramiyesi: Some(dec!(0.15)),
        }),
        dataset: PayrollDatasetSnapshot {
            personnel: vec![Personel {
                id: "person-1".into(),
                tcNo: "10000000000".into(),
                ad: "Test".into(),
                soyad: "Personel".into(),
                grup: "1. Grup".into(),
                unvan: None,
                sgkSicilNo: "SGK-1".into(),
                iban: "TR000000000000000000000000".into(),
                hizmetYili: 5,
                aciklama: None,
                devirKumulatifGvMatrahi: None,
                devirKumulatifGvMatrahiYili: None,
                devirKumulatifGvMatrahiBaslangicAyi: None,
                devirKumulatifAsgariGvMatrahi: None,
                devirKumulatifAsgariGvMatrahiYili: None,
                kesintiler: None,
            }],
            periods: vec![period],
            institutionSettings: HashMap::from([("2026-01".into(), settings)]),
            attendances: vec![PersonelPuantaj {
                id: "person-1_2026-01".into(),
                personelId: "person-1".into(),
                donemId: "2026-01".into(),
                gunler,
            }],
            payrolls: Vec::new(),
            taxOpenings: Vec::new(),
            sickLeaveRecords: Vec::new(),
            annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
            zamAylari: Vec::new(),
        },
    }
}

#[test]
fn calculation_uses_shared_engine_and_preserves_manual_decimal_values() {
    let request = fixture_request();
    let result = calculate_payroll(&request).expect("fixture should calculate");

    assert_eq!(result.status, BordroStatus::CALCULATED);
    assert_eq!(result.gelirler.tediye, Some(dec!(75000.10)));
    assert_eq!(result.gelirler.tisIkramiyesi, Some(dec!(0.15)));
    assert_eq!(
        result.netOdeme,
        (result.gelirToplam - result.kesintiToplam).round_dp(2)
    );
}

#[test]
fn decimal_json_boundary_accepts_exact_strings() {
    let input: ManualPayrollIncomeInput =
        serde_json::from_str(r#"{"tediye":"75000.10","tisIkramiyesi":"0.15"}"#)
            .expect("Decimal strings should deserialize at the shared boundary");

    assert_eq!(input.tediye, Some(dec!(75000.10)));
    assert_eq!(input.tisIkramiyesi, Some(dec!(0.15)));
}

#[test]
fn finalized_existing_payroll_is_immutable() {
    let mut request = fixture_request();
    let mut finalized = calculate_payroll(&request).expect("fixture should calculate");
    finalized.status = BordroStatus::FINALIZED;
    request.dataset.payrolls.push(finalized);

    let error = calculate_payroll(&request).expect_err("FINALIZED payroll must be immutable");
    assert!(error.to_string().contains("FINALIZED"));
}

fn finalization_request(status: BordroStatus) -> PayrollCalculationRequest {
    let mut request = fixture_request();
    request.dataset.periods[0].taxMonth = 1;
    let mut existing = calculate_payroll(&request).expect("fixture should calculate");
    existing.status = status;
    request.dataset.payrolls.push(existing);
    request
}

#[test]
fn finalize_recalculates_and_returns_finalized_result() {
    let request = finalization_request(BordroStatus::CALCULATED);
    let result = finalize_payroll(&request).expect("calculated payroll should finalize");

    assert_eq!(result.status, BordroStatus::FINALIZED);
    assert_eq!(result.gelirler.tediye, Some(dec!(75000.10)));
    assert_eq!(result.olusturulmaTarihi, "2026-09-03T00:00:00.000Z");
}

#[test]
fn finalize_rejects_stale_and_draft_but_recalculates_calculated_source() {
    let mut calculated = finalization_request(BordroStatus::CALCULATED);
    calculated.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(dec!(0.1)),
        tisIkramiyesi: Some(dec!(0.15)),
    });
    let result = finalize_payroll(&calculated).expect("calculated source should be recalculated");
    assert_eq!(result.status, BordroStatus::FINALIZED);
    assert_eq!(result.gelirler.tediye, Some(dec!(0.1)));

    let stale_error = finalize_payroll(&finalization_request(BordroStatus::STALE))
        .expect_err("stale records must be recalculated before finalization");
    assert!(stale_error.to_string().contains("STALE"));

    let draft_error = finalize_payroll(&finalization_request(BordroStatus::DRAFT))
        .expect_err("draft has no authoritative result to finalize");
    assert!(draft_error.to_string().contains("DRAFT"));

    let finalized_error = finalize_payroll(&finalization_request(BordroStatus::FINALIZED))
        .expect_err("finalized records are fail-closed");
    assert!(finalized_error.to_string().contains("FINALIZED"));
}

#[test]
fn finalize_rejects_a_finalized_downstream_dependency() {
    let mut request = finalization_request(BordroStatus::CALCULATED);
    let mut later_period = request.dataset.periods[0].clone();
    later_period.id = "2026-02".into();
    later_period.ay = 2;
    later_period.baslangicTarihi = "2026-02-15".into();
    later_period.bitisTarihi = "2026-03-14".into();
    later_period.taxMonth = 2;
    request.dataset.periods.push(later_period);

    let mut later_payroll = request.dataset.payrolls[0].clone();
    later_payroll.id = "person-1_2026-02".into();
    later_payroll.donemId = "2026-02".into();
    later_payroll.status = BordroStatus::FINALIZED;
    request.dataset.payrolls.push(later_payroll);

    let error = finalize_payroll(&request)
        .expect_err("finalization must not invalidate a downstream FINALIZED payroll");
    assert!(error.to_string().contains("downstream"));
    assert!(error.to_string().contains("FINALIZED"));
}

#[test]
fn calculation_timestamp_is_input_driven_and_existing_creation_is_preserved() {
    let mut first = fixture_request();
    first.calculatedAt = "2026-01-01T00:00:00Z".into();
    let first_result = calculate_payroll(&first).expect("first calculation should pass");

    let mut second = first.clone();
    second.calculatedAt = "2026-02-01T00:00:00Z".into();
    let second_result = calculate_payroll(&second).expect("second calculation should pass");
    assert_eq!(first_result.netOdeme, second_result.netOdeme);
    assert_eq!(first_result.gelirToplam, second_result.gelirToplam);
    assert_ne!(
        first_result.sonGuncellemeTarihi,
        second_result.sonGuncellemeTarihi
    );

    second.dataset.payrolls.push(first_result.clone());
    let updated = calculate_payroll(&second).expect("existing calculation should pass");
    assert_eq!(updated.olusturulmaTarihi, first_result.olusturulmaTarihi);
    assert_eq!(updated.sonGuncellemeTarihi, "2026-02-01T00:00:00Z");
}

#[test]
fn decimal_results_serialize_as_strings() {
    let result = calculate_payroll(&fixture_request()).expect("fixture should calculate");
    let value = serde_json::to_value(result).expect("result should serialize");

    assert!(value["gelirToplam"].is_string());
    assert!(value["netOdeme"].is_string());
    assert_eq!(value["gelirler"]["tediye"], serde_json::json!("75000.10"));
}

#[test]
fn strict_preflight_fails_closed_when_tax_chain_is_incomplete() {
    let request = fixture_request();
    let error =
        validate_payroll_request(&request).expect_err("tax month 1 is intentionally absent");
    assert!(error.to_string().contains("vergi") || error.to_string().contains("zinciri"));
}
