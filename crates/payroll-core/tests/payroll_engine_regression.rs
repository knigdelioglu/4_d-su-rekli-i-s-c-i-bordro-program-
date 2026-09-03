use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, validate_payroll_request, AnnualPayrollParameters, BordroDonemi,
    BordroStatus, DonemselKurumDegerleri, ManualPayrollIncomeInput, PayrollCalculationRequest,
    PayrollDatasetSnapshot, Personel, PersonelPuantaj,
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

#[test]
fn strict_preflight_fails_closed_when_tax_chain_is_incomplete() {
    let request = fixture_request();
    let error =
        validate_payroll_request(&request).expect_err("tax month 1 is intentionally absent");
    assert!(error.to_string().contains("vergi") || error.to_string().contains("zinciri"));
}
