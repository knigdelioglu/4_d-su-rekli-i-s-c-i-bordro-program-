use payroll_core::{
    calculate_payroll, evaluate_payroll_invalidation, finalize_payroll, PayrollCalculationRequest,
    PayrollDatasetSnapshot, PayrollMutation, Result as CoreResult,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MutationPolicyRequest {
    dataset: PayrollDatasetSnapshot,
    mutation: PayrollMutation,
}

fn parse_request(request_json: &str) -> CoreResult<PayrollCalculationRequest> {
    serde_json::from_str(request_json).map_err(|error| {
        payroll_core::DomainError::InvalidData(format!(
            "WASM bordro isteği geçersiz JSON içeriyor: {}",
            error
        ))
    })
}

fn error_to_js(error: payroll_core::DomainError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(&error.to_string()))
}

/// Calculates one payroll from a complete, JSON-serializable snapshot.
///
/// Decimal inputs are expected as JSON strings. Returning JSON rather than
/// `JsValue` keeps the boundary explicit and auditable; the UI converts the
/// result into its display/storage representation only after this call.
#[wasm_bindgen]
pub fn calculate_payroll_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    payroll_core::validate_payroll_request(&request).map_err(error_to_js)?;
    let payroll = calculate_payroll(&request).map_err(error_to_js)?;
    serde_json::to_string(&payroll)
        .map_err(|error| JsValue::from_str(&format!("Bordro sonucu serileştirilemedi: {}", error)))
}

/// Recalculates and finalizes a payroll from the supplied authoritative
/// snapshot. Persistence remains the responsibility of the browser adapter.
#[wasm_bindgen]
pub fn finalize_payroll_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    let payroll = finalize_payroll(&request).map_err(error_to_js)?;
    serde_json::to_string(&payroll)
        .map_err(|error| JsValue::from_str(&format!("Bordro sonucu serileştirilemedi: {}", error)))
}

/// Validates and parses the same request boundary without persisting anything.
#[wasm_bindgen]
pub fn validate_payroll_json(request_json: &str) -> Result<(), JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    payroll_core::validate_payroll_request(&request).map_err(error_to_js)
}

/// Returns the core-owned invalidation impact for one browser mutation.
#[wasm_bindgen]
pub fn evaluate_mutation_policy_json(request_json: &str) -> Result<String, JsValue> {
    let request: MutationPolicyRequest = serde_json::from_str(request_json).map_err(|error| {
        error_to_js(payroll_core::DomainError::InvalidData(format!(
            "WASM mutation policy isteği geçersiz JSON içeriyor: {}",
            error
        )))
    })?;
    let impact =
        evaluate_payroll_invalidation(&request.dataset, &request.mutation).map_err(error_to_js)?;
    serde_json::to_string(&impact).map_err(|error| {
        JsValue::from_str(&format!(
            "Bordro mutation policy sonucu serileştirilemedi: {}",
            error
        ))
    })
}

/// Returns period/person data-quality notices from the same browser snapshot.
#[wasm_bindgen]
pub fn get_payroll_notices_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    let notices = payroll_core::get_period_notices(&request.dataset, &request.periodId)
        .map_err(error_to_js)?;
    serde_json::to_string(&notices).map_err(|error| {
        JsValue::from_str(&format!("Bordro uyarıları serileştirilemedi: {}", error))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use payroll_core::{
        calculate_payroll, AnnualPayrollParameters, BordroDonemi, BordroKaydi, BordroStatus,
        DevredenPekKaydi, DomainError, DonemselKurumDegerleri, ManualPayrollIncomeInput,
        PayrollCalculationRequest, PayrollDatasetSnapshot, Personel, PersonelKesintileri,
        PersonelPuantaj, SickLeaveRecord, StatutoryParameterSegment,
    };
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use wasm_bindgen_test::*;

    fn request_json() -> String {
        let period = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        let settings = DonemselKurumDegerleri {
            donemId: period.id.clone(),
            ..Default::default()
        };
        let mut gunler = HashMap::new();
        let mut date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        while date <= end {
            gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
            date += chrono::Duration::days(1);
        }

        serde_json::to_string(&PayrollCalculationRequest {
            personnelId: "person-1".into(),
            periodId: period.id.clone(),
            calculatedAt: "2026-09-03T00:00:00.000Z".into(),
            manualIncome: Some(ManualPayrollIncomeInput {
                tediye: Some(dec!(123.45)),
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
        })
        .expect("fixture should serialize")
    }

    fn standard_request() -> PayrollCalculationRequest {
        serde_json::from_str(&request_json()).expect("standard fixture should deserialize")
    }

    fn fixture_period(
        id: &str,
        year: i32,
        month: i32,
        tax_year: i32,
        tax_month: i32,
        start: &str,
        end: &str,
    ) -> BordroDonemi {
        BordroDonemi {
            id: id.into(),
            yil: year,
            ay: month,
            baslangicTarihi: start.into(),
            bitisTarihi: end.into(),
            donemAdi: id.into(),
            taxYear: tax_year,
            taxMonth: tax_month,
        }
    }

    fn fixture_attendance(period: &BordroDonemi, code: &str) -> PersonelPuantaj {
        let mut gunler = HashMap::new();
        let mut date = chrono::NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .expect("fixture start should be valid");
        let end = chrono::NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .expect("fixture end should be valid");
        while date <= end {
            gunler.insert(date.format("%Y-%m-%d").to_string(), code.into());
            date += chrono::Duration::days(1);
        }
        PersonelPuantaj {
            id: format!("person-1_{}", period.id),
            personelId: "person-1".into(),
            donemId: period.id.clone(),
            gunler,
        }
    }

    fn configure_active_period(
        request: &mut PayrollCalculationRequest,
        periods: Vec<BordroDonemi>,
        active: &BordroDonemi,
    ) {
        request.periodId = active.id.clone();
        request.dataset.periods = periods;
        request.dataset.institutionSettings = request
            .dataset
            .periods
            .iter()
            .map(|period| {
                (
                    period.id.clone(),
                    DonemselKurumDegerleri {
                        donemId: period.id.clone(),
                        ..Default::default()
                    },
                )
            })
            .collect();
        request.dataset.attendances = vec![fixture_attendance(active, "Ç")];
        request.dataset.payrolls.clear();
        request.dataset.sickLeaveRecords.clear();
        request.dataset.taxOpenings.clear();
        request.dataset.zamAylari.clear();
    }

    fn configure_two_periods(
        request: &mut PayrollCalculationRequest,
        previous: BordroDonemi,
        active: BordroDonemi,
    ) {
        configure_active_period(request, vec![previous, active.clone()], &active);
    }

    fn prior_calculated_payroll(
        request: &PayrollCalculationRequest,
        previous: &BordroDonemi,
    ) -> BordroKaydi {
        let mut prior_request = request.clone();
        prior_request.periodId = previous.id.clone();
        prior_request.manualIncome = Some(ManualPayrollIncomeInput {
            tediye: Some(dec!(0)),
            tisIkramiyesi: Some(dec!(0)),
        });
        prior_request.dataset.periods = vec![previous.clone()];
        prior_request.dataset.institutionSettings = HashMap::from([(
            previous.id.clone(),
            DonemselKurumDegerleri {
                donemId: previous.id.clone(),
                ..Default::default()
            },
        )]);
        prior_request.dataset.attendances = vec![fixture_attendance(previous, "Ç")];
        prior_request.dataset.payrolls.clear();
        prior_request.dataset.zamAylari.clear();
        calculate_payroll(&prior_request).expect("prior fixture should calculate")
    }

    fn add_sick_leave(request: &mut PayrollCalculationRequest, start: &str, end: &str) {
        let attendance = request
            .dataset
            .attendances
            .iter_mut()
            .find(|attendance| attendance.donemId == request.periodId)
            .expect("active attendance should exist");
        let mut date = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d")
            .expect("sick start should be valid");
        let end_date =
            chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").expect("sick end should be valid");
        while date <= end_date {
            attendance
                .gunler
                .insert(date.format("%Y-%m-%d").to_string(), "R".into());
            date += chrono::Duration::days(1);
        }
        request.dataset.sickLeaveRecords.push(SickLeaveRecord {
            id: format!("sick-{}-{}", start, end),
            personnelId: "person-1".into(),
            startDate: start.into(),
            endDate: end.into(),
            createdAt: None,
            updatedAt: None,
        });
    }

    fn set_manual_zero(request: &mut PayrollCalculationRequest) {
        request.manualIncome = Some(ManualPayrollIncomeInput {
            tediye: Some(dec!(0)),
            tisIkramiyesi: Some(dec!(0)),
        });
    }

    fn assert_calculation_parity(
        name: &str,
        request: PayrollCalculationRequest,
        expected_success: bool,
    ) -> Option<BordroKaydi> {
        let native = payroll_core::validate_payroll_request(&request)
            .and_then(|_| payroll_core::calculate_payroll(&request));
        let json = serde_json::to_string(&request).expect("fixture request should serialize");
        let wasm = calculate_payroll_json(&json);

        assert_eq!(native.is_ok(), expected_success, "native result: {name}");
        assert_eq!(wasm.is_ok(), expected_success, "WASM result: {name}");
        match (native, wasm) {
            (Ok(native_result), Ok(wasm_json)) => {
                let wasm_result: BordroKaydi =
                    serde_json::from_str(&wasm_json).expect("WASM result should deserialize");
                assert_eq!(
                    serde_json::to_value(&native_result).expect("native result should serialize"),
                    serde_json::to_value(&wasm_result).expect("WASM result should serialize"),
                    "financial/status/detail parity: {name}"
                );
                Some(native_result)
            }
            (Err(native_error), Err(wasm_error)) => {
                let wasm_error: DomainError = serde_wasm_bindgen::from_value(wasm_error)
                    .expect("WASM error should keep the shared DomainError shape");
                assert_eq!(
                    serde_json::to_value(native_error).expect("native error should serialize"),
                    serde_json::to_value(wasm_error).expect("WASM error should serialize"),
                    "error semantic parity: {name}"
                );
                None
            }
            (native, wasm) => panic!(
                "success/error parity mismatch for {name}: native_ok={}, wasm_ok={}",
                native.is_ok(),
                wasm.is_ok()
            ),
        }
    }

    #[wasm_bindgen_test]
    fn adapter_calculation_parity_fixture_matrix() {
        let mut fixtures: Vec<(&str, PayrollCalculationRequest, bool)> = Vec::new();

        // 1. Basit standart ay.
        fixtures.push(("simple-standard-month", standard_request(), true));

        // 2. Şubat dönemi.
        let mut february = standard_request();
        configure_two_periods(
            &mut february,
            fixture_period("2026-01", 2026, 1, 2026, 1, "2026-01-15", "2026-02-14"),
            fixture_period("2026-02", 2026, 2, 2026, 2, "2026-02-15", "2026-03-14"),
        );
        fixtures.push(("february-period", february, true));

        // 3. 30 günlük dönem.
        let mut thirty_days = standard_request();
        let active = fixture_period("2026-04", 2026, 4, 2026, 4, "2026-04-15", "2026-05-14");
        configure_active_period(&mut thirty_days, vec![active.clone()], &active);
        thirty_days.dataset.personnel[0].devirKumulatifAsgariGvMatrahi = Some(dec!(0));
        thirty_days.dataset.personnel[0].devirKumulatifAsgariGvMatrahiYili = Some(2026);
        thirty_days.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(4);
        fixtures.push(("thirty-day-period", thirty_days, true));

        // 4. 31 günlük dönem.
        fixtures.push(("thirty-one-day-period", standard_request(), true));

        // 5. Raporlu gün.
        let mut reported_day = standard_request();
        add_sick_leave(&mut reported_day, "2026-01-20", "2026-01-20");
        fixtures.push(("reported-day", reported_day, true));

        // 6. Yıl içi rapor hakediş / ilk iki gün kuralı.
        let mut sick_quota = standard_request();
        add_sick_leave(&mut sick_quota, "2026-01-20", "2026-01-22");
        fixtures.push(("annual-sick-leave-first-two-days", sick_quota, true));

        // 7. Zam ve statutory segment geçişi dönem ortasında.
        let mut raise_segment = standard_request();
        let previous = fixture_period("2025-12", 2025, 12, 2025, 12, "2025-12-15", "2026-01-14");
        let active = fixture_period("2026-01", 2026, 1, 2026, 2, "2026-01-15", "2026-02-14");
        configure_two_periods(&mut raise_segment, previous, active.clone());
        raise_segment.dataset.personnel[0].devirKumulatifAsgariGvMatrahi = Some(dec!(0));
        raise_segment.dataset.personnel[0].devirKumulatifAsgariGvMatrahiYili = Some(2026);
        raise_segment.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(2);
        raise_segment.dataset.zamAylari = vec![2];
        raise_segment
            .dataset
            .institutionSettings
            .get_mut(&active.id)
            .expect("active settings should exist")
            .statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
            effectiveFrom: "2026-02-01".into(),
            gunlukAsgariUcret: Some(dec!(1200)),
            pekTavanKatsayisi: Some(dec!(7.5)),
            gunlukYemekIstisnasiSGK: Some(dec!(300)),
            gunlukYemekIstisnasiGV: Some(dec!(300)),
        }]);
        fixtures.push(("mid-period-raise-statutory-segment", raise_segment, true));

        // 8. Gelir vergisi dilimi geçişi.
        let mut bracket_transition = standard_request();
        bracket_transition
            .dataset
            .institutionSettings
            .get_mut("2026-01")
            .expect("standard settings should exist")
            .gunlukTabanUcret = dec!(8000);
        fixtures.push(("income-tax-bracket-transition", bracket_transition, true));

        // 9. Kümülatif GV opening.
        let mut gv_opening = standard_request();
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahi = Some(dec!(120000));
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahiYili = Some(2026);
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(1);
        fixtures.push(("cumulative-gv-opening", gv_opening, true));

        // 10. Asgari ücret GV istisnası.
        let mut minimum_wage_exemption = standard_request();
        minimum_wage_exemption
            .dataset
            .institutionSettings
            .get_mut("2026-01")
            .expect("standard settings should exist")
            .gunlukTabanUcret = dec!(1101);
        fixtures.push((
            "minimum-wage-income-tax-exemption",
            minimum_wage_exemption,
            true,
        ));

        // 11. PEK alt sınırı.
        let mut pek_floor = standard_request();
        pek_floor
            .dataset
            .institutionSettings
            .get_mut("2026-01")
            .expect("standard settings should exist")
            .gunlukTabanUcret = dec!(100);
        set_manual_zero(&mut pek_floor);
        fixtures.push(("pek-floor", pek_floor, true));

        // 12. PEK üst sınırı.
        let mut pek_ceiling = standard_request();
        let settings = pek_ceiling
            .dataset
            .institutionSettings
            .get_mut("2026-01")
            .expect("standard settings should exist");
        settings.gunlukTabanUcret = dec!(10000);
        settings.pekTavanKatsayisi = Some(dec!(1));
        set_manual_zero(&mut pek_ceiling);
        fixtures.push(("pek-ceiling", pek_ceiling, true));

        // 13. Devreden PEK.
        let mut carried_pek = standard_request();
        let previous = fixture_period("2026-01", 2026, 1, 2026, 1, "2026-01-15", "2026-02-14");
        let active = fixture_period("2026-02", 2026, 2, 2026, 2, "2026-02-15", "2026-03-14");
        configure_two_periods(&mut carried_pek, previous.clone(), active);
        set_manual_zero(&mut carried_pek);
        let mut previous_payroll = prior_calculated_payroll(&carried_pek, &previous);
        previous_payroll.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
            tutar: dec!(20000),
            kalanAySayisi: 2,
            kaynakDonemId: Some(previous.id.clone()),
        }]);
        carried_pek.dataset.payrolls = vec![previous_payroll];
        fixtures.push(("carried-pek", carried_pek, true));

        // 14. Devreden PEK gap / previous authoritative payroll requirement.
        let mut carried_pek_gap = standard_request();
        let previous = fixture_period("2026-01", 2026, 1, 2026, 1, "2026-01-15", "2026-02-14");
        let active = fixture_period("2026-03", 2026, 3, 2026, 3, "2026-03-15", "2026-04-14");
        configure_two_periods(&mut carried_pek_gap, previous.clone(), active);
        carried_pek_gap.dataset.personnel[0].devirKumulatifAsgariGvMatrahi = Some(dec!(0));
        carried_pek_gap.dataset.personnel[0].devirKumulatifAsgariGvMatrahiYili = Some(2026);
        carried_pek_gap.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(3);
        set_manual_zero(&mut carried_pek_gap);
        let mut previous_payroll = prior_calculated_payroll(&carried_pek_gap, &previous);
        previous_payroll.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
            tutar: dec!(20000),
            kalanAySayisi: 3,
            kaynakDonemId: Some(previous.id),
        }]);
        carried_pek_gap.dataset.payrolls = vec![previous_payroll];
        fixtures.push(("carried-pek-gap", carried_pek_gap, false));

        // 15. Manual income.
        let mut manual_income = standard_request();
        manual_income.manualIncome = Some(ManualPayrollIncomeInput {
            tediye: Some(dec!(0.1)),
            tisIkramiyesi: Some(dec!(0.15)),
        });
        fixtures.push(("manual-income", manual_income, true));

        // 16. Manual deductions.
        let mut manual_deductions = standard_request();
        manual_deductions.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
            besUyesi: Some(true),
            sabitBesTutar: Some(dec!(0.15)),
            icraTutar: Some(dec!(1)),
            digerKesintiTutar: Some(dec!(2)),
            ..Default::default()
        });
        fixtures.push(("manual-deductions", manual_deductions, true));

        // 17. Negative net edge case.
        let mut negative_net = standard_request();
        negative_net.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
            kisiBorcuTutar: Some(dec!(999999)),
            ..Default::default()
        });
        set_manual_zero(&mut negative_net);
        fixtures.push(("negative-net", negative_net, false));

        // 18. Zero net edge case.
        let mut zero_net = standard_request();
        zero_net.dataset.attendances[0] = fixture_attendance(&zero_net.dataset.periods[0], "R");
        set_manual_zero(&mut zero_net);
        fixtures.push(("zero-net", zero_net, true));

        // 19. Missing attendance.
        let mut missing_attendance = standard_request();
        missing_attendance.dataset.attendances.clear();
        fixtures.push(("missing-attendance", missing_attendance, false));

        // 20. Incomplete attendance (calculation remains backward-compatible;
        // finalization is the stricter completeness gate).
        let mut incomplete_attendance = standard_request();
        incomplete_attendance.dataset.attendances[0]
            .gunler
            .remove("2026-02-14");
        fixtures.push(("incomplete-attendance", incomplete_attendance, true));

        // 21. STALE previous payroll.
        let mut stale_previous = standard_request();
        let previous = fixture_period("2026-01", 2026, 1, 2026, 1, "2026-01-15", "2026-02-14");
        let active = fixture_period("2026-02", 2026, 2, 2026, 2, "2026-02-15", "2026-03-14");
        configure_two_periods(&mut stale_previous, previous.clone(), active);
        let mut previous_payroll = prior_calculated_payroll(&stale_previous, &previous);
        previous_payroll.status = BordroStatus::STALE;
        stale_previous.dataset.payrolls = vec![previous_payroll];
        fixtures.push(("stale-previous-payroll", stale_previous, false));

        // 22. FINALIZED previous payroll.
        let mut finalized_previous = standard_request();
        let previous = fixture_period("2026-01", 2026, 1, 2026, 1, "2026-01-15", "2026-02-14");
        let active = fixture_period("2026-02", 2026, 2, 2026, 2, "2026-02-15", "2026-03-14");
        configure_two_periods(&mut finalized_previous, previous.clone(), active);
        let mut previous_payroll = prior_calculated_payroll(&finalized_previous, &previous);
        previous_payroll.status = BordroStatus::FINALIZED;
        finalized_previous.dataset.payrolls = vec![previous_payroll];
        fixtures.push(("finalized-previous-payroll", finalized_previous, true));

        // 23. FINALIZED current payroll.
        let mut finalized_current = standard_request();
        let mut current_payroll =
            calculate_payroll(&finalized_current).expect("current fixture should calculate");
        current_payroll.status = BordroStatus::FINALIZED;
        finalized_current.dataset.payrolls = vec![current_payroll];
        fixtures.push(("finalized-current-payroll", finalized_current, false));

        // 24. Tax month chronology error.
        let mut chronology_error = standard_request();
        let previous = fixture_period("2026-01", 2026, 1, 2026, 2, "2026-01-15", "2026-02-14");
        let active = fixture_period("2026-02", 2026, 2, 2026, 2, "2026-02-15", "2026-03-14");
        configure_two_periods(&mut chronology_error, previous, active);
        fixtures.push(("tax-month-chronology-error", chronology_error, false));

        // 25. Missing annual parameters.
        let mut missing_annual = standard_request();
        missing_annual.dataset.annualPayrollParameters.clear();
        fixtures.push(("missing-annual-parameters", missing_annual, false));

        // 26. Missing institution parameters.
        let mut missing_institution = standard_request();
        missing_institution.dataset.institutionSettings.clear();
        fixtures.push(("missing-institution-parameters", missing_institution, false));

        assert_eq!(fixtures.len(), 26);
        for (name, request, expected_success) in fixtures {
            assert_calculation_parity(name, request, expected_success);
        }
    }

    #[wasm_bindgen_test]
    fn adapter_calculates_through_the_shared_contract() {
        let json = request_json();
        validate_payroll_json(&json).expect("shared preflight should pass");
        let result = calculate_payroll_json(&json).expect("adapter should calculate");
        let value: serde_json::Value = serde_json::from_str(&result).expect("valid result JSON");

        assert_eq!(value["status"], serde_json::json!("CALCULATED"));
        assert_eq!(value["gelirler"]["tediye"], serde_json::json!("123.45"));

        // Native Tauri also calls `payroll_core::calculate_payroll`; compare
        // the adapter result to that same core result so the WASM boundary
        // cannot accidentally acquire a second calculation path.
        let request: PayrollCalculationRequest =
            serde_json::from_str(&json).expect("request JSON should round-trip");
        let native_core_result = calculate_payroll(&request).expect("core should calculate");
        let wasm_result: BordroKaydi =
            serde_json::from_str(&result).expect("adapter result should deserialize");

        assert_eq!(wasm_result.status, native_core_result.status);
        assert_eq!(wasm_result.gelirToplam, native_core_result.gelirToplam);
        assert_eq!(wasm_result.kesintiToplam, native_core_result.kesintiToplam);
        assert_eq!(wasm_result.netOdeme, native_core_result.netOdeme);
        assert_eq!(
            wasm_result.gelirler.tediye,
            native_core_result.gelirler.tediye
        );
        assert_eq!(
            wasm_result
                .gvDetay
                .as_ref()
                .map(|detail| detail.kesilenGelirVergisi),
            native_core_result
                .gvDetay
                .as_ref()
                .map(|detail| detail.kesilenGelirVergisi)
        );
    }

    #[wasm_bindgen_test]
    fn adapter_finalizes_through_the_shared_contract() {
        let mut request: PayrollCalculationRequest =
            serde_json::from_str(&request_json()).expect("request JSON should round-trip");
        let existing = calculate_payroll(&request).expect("fixture should calculate");
        request.dataset.payrolls.push(existing);
        let json = serde_json::to_string(&request).expect("finalization request should serialize");

        let wasm_json = finalize_payroll_json(&json).expect("adapter should finalize");
        let wasm_result: BordroKaydi =
            serde_json::from_str(&wasm_json).expect("adapter result should deserialize");
        let native_result = finalize_payroll(&request).expect("core should finalize");

        assert_eq!(wasm_result.status, BordroStatus::FINALIZED);
        assert_eq!(wasm_result.status, native_result.status);
        assert_eq!(wasm_result.netOdeme, native_result.netOdeme);
        assert_eq!(
            serde_json::to_value(&wasm_result.pekDetay).expect("PEK should serialize"),
            serde_json::to_value(&native_result.pekDetay).expect("PEK should serialize")
        );
        assert_eq!(
            serde_json::to_value(&wasm_result.gvDetay).expect("GV should serialize"),
            serde_json::to_value(&native_result.gvDetay).expect("GV should serialize")
        );

        let value: serde_json::Value =
            serde_json::from_str(&wasm_json).expect("finalized result should be JSON");
        assert!(value["netOdeme"].is_string());
        assert_eq!(value["status"], serde_json::json!("FINALIZED"));
    }

    #[wasm_bindgen_test]
    fn adapter_finalization_matches_core_downstream_finalized_blocker() {
        let mut request = standard_request();
        let current = calculate_payroll(&request).expect("fixture should calculate");
        request.dataset.payrolls.push(current);

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

        let json = serde_json::to_string(&request).expect("finalization request should serialize");
        let wasm_error = finalize_payroll_json(&json).expect_err("adapter should fail closed");
        let wasm_error: DomainError =
            serde_wasm_bindgen::from_value(wasm_error).expect("error should remain structured");
        let native_error = payroll_core::finalize_payroll(&request)
            .expect_err("core should reject the downstream FINALIZED dependency");

        assert_eq!(wasm_error.to_string(), native_error.to_string());
    }

    #[wasm_bindgen_test]
    fn adapter_returns_the_core_mutation_impact() {
        let mut request: PayrollCalculationRequest =
            serde_json::from_str(&request_json()).expect("request JSON should round-trip");
        let mut later_period = request.dataset.periods[0].clone();
        later_period.id = "2026-02".into();
        later_period.ay = 2;
        later_period.taxMonth = 2;
        later_period.baslangicTarihi = "2026-02-15".into();
        later_period.bitisTarihi = "2026-03-14".into();
        request.dataset.periods.push(later_period);

        let mut later_payroll = calculate_payroll(&request).expect("fixture should calculate");
        later_payroll.id = "person-1_2026-02".into();
        later_payroll.donemId = "2026-02".into();
        later_payroll.status = payroll_core::BordroStatus::FINALIZED;
        request.dataset.payrolls = vec![later_payroll];

        let mutation = PayrollMutation::PayrollCalculation {
            personnelId: "person-1".into(),
            periodId: "2026-01".into(),
        };
        let policy_request = serde_json::json!({
            "dataset": request.dataset,
            "mutation": mutation,
        });
        let json = serde_json::to_string(&policy_request).expect("policy request should serialize");
        let wasm_json = evaluate_mutation_policy_json(&json).expect("policy should evaluate");
        let wasm_impact: payroll_core::MutationImpact =
            serde_json::from_str(&wasm_json).expect("impact should deserialize");
        let parsed_policy: MutationPolicyRequest =
            serde_json::from_str(&json).expect("policy request should deserialize");
        let native_impact = evaluate_payroll_invalidation(
            &parsed_policy.dataset,
            &PayrollMutation::PayrollCalculation {
                personnelId: "person-1".into(),
                periodId: "2026-01".into(),
            },
        )
        .expect("core policy should evaluate");

        assert_eq!(wasm_impact, native_impact);
        assert_eq!(wasm_impact.blockedByFinalized.len(), 1);
    }
}
