use payroll_core::{
    calculate_legacy_backup_payroll_checked, calculate_payroll_checked,
    calculate_payroll_with_compensation_policy_checked, evaluate_payroll_invalidation,
    finalize_payroll, finalize_payroll_with_compensation_policy_checked, AnnualPayrollParameters,
    BordroDonemi, BordroKaydi, CompensationPolicyVersion, LegacyBackupPayrollInput,
    PayrollCalculationRequest, PayrollDatasetSnapshot, PayrollMutation, Result as CoreResult,
    RetroCalculationRequest, RetroEntitlementEngine,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use wasm_bindgen::prelude::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MutationPolicyRequest {
    dataset: PayrollDatasetSnapshot,
    mutation: PayrollMutation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyBackupPayrollReplayRequest {
    request: PayrollCalculationRequest,
    legacy_input: LegacyBackupPayrollInput,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PayrollMetadataValidationPayload {
    compensation_policy_versions: Vec<CompensationPolicyVersion>,
    bordrolar: Vec<BordroKaydi>,
    donemler: Vec<BordroDonemi>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompensationTimelineRequest {
    start_inclusive: String,
    end_inclusive: String,
    versions: Vec<CompensationPolicyVersion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompensationPolicyPeriodValidationRequest {
    periods: Vec<BordroDonemi>,
    existing_versions: Vec<CompensationPolicyVersion>,
    candidate_versions: Vec<CompensationPolicyVersion>,
}

fn deserialize_contract_json<T: DeserializeOwned>(json: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(json)
}

fn parse_request(request_json: &str) -> CoreResult<PayrollCalculationRequest> {
    deserialize_contract_json(request_json).map_err(|error| {
        payroll_core::DomainError::InvalidData(format!(
            "WASM bordro isteği geçersiz JSON içeriyor: {}",
            error
        ))
    })
}

fn validate_payroll_backup_metadata_json_inner(payload_json: &str) -> CoreResult<()> {
    let payload: PayrollMetadataValidationPayload = deserialize_contract_json(payload_json)
        .map_err(|error| {
            payroll_core::DomainError::InvalidData(format!(
                "V6 bordro metadata isteği geçersiz JSON içeriyor: {error}"
            ))
        })?;
    let ordered_versions =
        payroll_core::validate_compensation_policy_timeline(&payload.compensation_policy_versions)?;
    let periods: HashMap<&str, &BordroDonemi> = payload
        .donemler
        .iter()
        .map(|period| (period.id.as_str(), period))
        .collect();
    if periods.len() != payload.donemler.len() {
        return Err(payroll_core::DomainError::InvalidData(
            "V6 payload yinelenen dönem kimliği içeriyor.".into(),
        ));
    }
    let mut payroll_ids = BTreeSet::new();
    for payroll in &payload.bordrolar {
        if !payroll_ids.insert(payroll.id.as_str()) {
            return Err(payroll_core::DomainError::InvalidData(format!(
                "V6 payload yinelenen bordro kimliği içeriyor: {}.",
                payroll.id
            )));
        }
        let period = periods.get(payroll.donemId.as_str()).ok_or_else(|| {
            payroll_core::DomainError::InvalidData(format!(
                "V6 bordro {} için parent dönem bulunamadı: {}.",
                payroll.id, payroll.donemId
            ))
        })?;
        payroll_core::validate_payroll_calculation_metadata_with_context(
            payroll,
            period,
            &payload.bordrolar,
            &payload.donemler,
        )
        .map_err(|error| payroll_core::DomainError::InvalidData(error.to_string()))?;
        payroll_core::validate_compensation_snapshot_policy_links(payroll, &ordered_versions)?;
    }
    Ok(())
}

fn error_to_js(error: payroll_core::DomainError) -> JsValue {
    serde_wasm_bindgen::to_value(&error).unwrap_or_else(|_| JsValue::from_str(&error.to_string()))
}

/// Calculates one payroll from a complete, JSON-serializable snapshot. NORMAL
/// payrolls use the authoritative compensation timeline through the default
/// checked core entry point.
///
/// Decimal inputs are expected as JSON strings. Returning JSON rather than
/// `JsValue` keeps the boundary explicit and auditable; the UI converts the
/// result into its display/storage representation only after this call.
#[wasm_bindgen]
pub fn calculate_payroll_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    let payroll = calculate_payroll_checked(&request).map_err(error_to_js)?;
    serde_json::to_string(&payroll)
        .map_err(|error| JsValue::from_str(&format!("Bordro sonucu serileştirilemedi: {}", error)))
}

/// Import-only exact V1-V5 replay. Missing legacy sources return JSON null so
/// callers can retain the historical snapshot without rejecting the import.
#[wasm_bindgen]
pub fn calculate_legacy_backup_payroll_json(replay_json: &str) -> Result<String, JsValue> {
    let replay: LegacyBackupPayrollReplayRequest =
        deserialize_contract_json(replay_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "Legacy yedek replay isteği geçersiz JSON içeriyor: {error}"
            )))
        })?;
    let payroll = calculate_legacy_backup_payroll_checked(&replay.request, &replay.legacy_input)
        .map_err(error_to_js)?;
    serde_json::to_string(&payroll).map_err(|error| {
        JsValue::from_str(&format!(
            "Legacy yedek replay sonucu serileştirilemedi: {error}"
        ))
    })
}

/// Calculates one payroll through the explicit compensation-policy engine.
/// Compatibility export for callers that explicitly select the policy API.
/// The ordinary calculation export already uses the same resolver for NORMAL.
#[wasm_bindgen]
pub fn calculate_compensation_policy_payroll_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    let payroll =
        calculate_payroll_with_compensation_policy_checked(&request).map_err(error_to_js)?;
    serde_json::to_string(&payroll).map_err(|error| {
        JsValue::from_str(&format!(
            "Politika bordro sonucu serileştirilemedi: {}",
            error
        ))
    })
}

/// Replays historical service periods and returns an auditable retro batch
/// preview. The function is pure; browser persistence happens in TypeScript
/// after the result crosses the explicit Decimal JSON boundary.
#[wasm_bindgen]
pub fn calculate_retro_preview_json(request_json: &str) -> Result<String, JsValue> {
    let request: RetroCalculationRequest =
        deserialize_contract_json(request_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "WASM retro hesap isteği geçersiz JSON içeriyor: {}",
                error
            )))
        })?;
    let result = RetroEntitlementEngine::calculate(&request).map_err(error_to_js)?;
    serde_json::to_string(&result).map_err(|error| {
        JsValue::from_str(&format!("Retro hesap sonucu serileştirilemedi: {}", error))
    })
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

/// Recalculates and finalizes a schema-2 NORMAL payroll through the explicit
/// compensation-policy engine.
#[wasm_bindgen]
pub fn finalize_compensation_policy_payroll_json(request_json: &str) -> Result<String, JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    let payroll =
        finalize_payroll_with_compensation_policy_checked(&request).map_err(error_to_js)?;
    serde_json::to_string(&payroll).map_err(|error| {
        JsValue::from_str(&format!(
            "Politika bordro kesinleştirme sonucu serileştirilemedi: {error}"
        ))
    })
}

/// Validates and parses the same request boundary without persisting anything.
#[wasm_bindgen]
pub fn validate_payroll_json(request_json: &str) -> Result<(), JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    payroll_core::validate_payroll_request(&request).map_err(error_to_js)
}

/// Validates the annual tariff with the same core-owned semantic contract
/// used by checked payroll calculation. Browser persistence tests use this
/// narrow endpoint for direct Rust/WASM parity without duplicating tax logic.
#[wasm_bindgen]
pub fn validate_annual_payroll_parameters_json(parameters_json: &str) -> Result<(), JsValue> {
    let parameters: AnnualPayrollParameters =
        serde_json::from_str(parameters_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "Yıllık bordro parametresi geçersiz JSON içeriyor: {}",
                error
            )))
        })?;
    payroll_core::validate_annual_payroll_parameters(&parameters).map_err(error_to_js)
}

/// Validates V6 compensation policy graph links and every payroll envelope
/// against its parent period with the same core authority as native restore.
/// This checks structure and input consistency only; it does not replay payroll.
#[wasm_bindgen]
pub fn validate_payroll_backup_metadata_json(payload_json: &str) -> Result<(), JsValue> {
    validate_payroll_backup_metadata_json_inner(payload_json).map_err(error_to_js)
}

/// Resolves the same typed compensation timeline used by the Rust payroll
/// engine. Browser policy previews use this instead of duplicating the
/// supersession graph or ROAD_ONLY validation rules in TypeScript.
#[wasm_bindgen]
pub fn resolve_compensation_policy_timeline_json(request_json: &str) -> Result<String, JsValue> {
    let request: CompensationTimelineRequest =
        deserialize_contract_json(request_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "Ücret politikası timeline isteği geçersiz JSON içeriyor: {error}"
            )))
        })?;
    let versions = payroll_core::validate_compensation_policy_timeline(&request.versions)
        .map_err(error_to_js)?;
    let segments = payroll_core::resolve_compensation_segments(
        &request.start_inclusive,
        &request.end_inclusive,
        &versions,
    )
    .map_err(error_to_js)?;
    serde_json::to_string(&segments).map_err(|error| {
        JsValue::from_str(&format!(
            "Ücret politikası timeline sonucu serileştirilemedi: {error}"
        ))
    })
}

/// Returns conflicts between active policy rounding modes for the supplied
/// payroll service period. This shares the exact period and supersession rules
/// used by payroll resolution.
#[wasm_bindgen]
pub fn find_compensation_policy_rounding_issues_json(
    request_json: &str,
) -> Result<String, JsValue> {
    let request: CompensationTimelineRequest =
        deserialize_contract_json(request_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "Ücret politikası timeline isteği geçersiz JSON içeriyor: {error}"
            )))
        })?;
    let issues = payroll_core::find_road_rounding_issues(
        &request.start_inclusive,
        &request.end_inclusive,
        &request.versions,
    )
    .map_err(error_to_js)?;
    serde_json::to_string(&issues).map_err(|error| {
        JsValue::from_str(&format!(
            "Ücret politikası rounding diagnostics sonucu serileştirilemedi: {error}"
        ))
    })
}

/// Rejects only newly introduced rounding conflicts across all persisted
/// service periods. Existing conflicts remain eligible for explicit repair.
#[wasm_bindgen]
pub fn validate_no_new_compensation_policy_rounding_conflicts_json(
    request_json: &str,
) -> Result<(), JsValue> {
    let request: CompensationPolicyPeriodValidationRequest =
        deserialize_contract_json(request_json).map_err(|error| {
            error_to_js(payroll_core::DomainError::InvalidData(format!(
                "Ücret politikası dönem doğrulama isteği geçersiz JSON içeriyor: {error}"
            )))
        })?;
    payroll_core::validate_no_new_road_rounding_conflicts(
        &request.periods,
        &request.existing_versions,
        &request.candidate_versions,
    )
    .map_err(error_to_js)
}

/// Returns the core-owned invalidation impact for one browser mutation.
#[wasm_bindgen]
pub fn evaluate_mutation_policy_json(request_json: &str) -> Result<String, JsValue> {
    let request: MutationPolicyRequest =
        deserialize_contract_json(request_json).map_err(|error| {
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
        CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
        CompensationPolicyVersion, DevredenPekKaydi, DomainError, DonemselKurumDegerleri,
        ManualPayrollIncomeInput, PayrollCalculationRequest, PayrollDatasetSnapshot,
        PayrollPeriodInstitutionSettings, Personel, PersonelKesintileri, PersonelPuantaj,
        RoadRoundingMode, SickLeaveRecord, StatutoryParameterSegment,
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
        let legacy_settings = DonemselKurumDegerleri {
            donemId: period.id.clone(),
            ..Default::default()
        };
        let settings = PayrollPeriodInstitutionSettings::from(&legacy_settings);
        let baseline = policy_version(
            "policy-baseline",
            &period.baslangicTarihi,
            policy_state_from_legacy_settings(&legacy_settings),
        );
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
            accrual: None,
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
                compensationRevisions: Vec::new(),
                compensationRevisionOverrides: Vec::new(),
                retroBatches: Vec::new(),
                retroAllocations: Vec::new(),
                compensationPolicyVersions: vec![baseline],
            },
        })
        .expect("fixture should serialize")
    }

    fn standard_request() -> PayrollCalculationRequest {
        serde_json::from_str(&request_json()).expect("standard fixture should deserialize")
    }

    fn policy_state_from_legacy_settings(
        settings: &DonemselKurumDegerleri,
    ) -> CompensationPolicyState {
        CompensationPolicyState {
            gunlukTabanUcret: settings.gunlukTabanUcret,
            gunlukYemek: settings.gunlukYemek,
            birlestirilmisSosyalYardim: settings.birlestirilmisSosyalYardim,
            gunlukVasitaYol: settings.gunlukVasitaYol,
            giyimYardimi: settings.giyimYardimi,
            hizmetZammiBirimi: settings.hizmetZammiBirimi,
            isPrimiYuzde: settings.isPrimiYuzde,
            isPrimiGruplari: settings
                .isPrimiGruplari
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|group| CompensationPolicyGroup {
                    id: group.id,
                    ad: group.ad,
                    oran: group.oran,
                    aktif: group.aktif,
                })
                .collect(),
            geceCalismaPrimiYuzde: settings.geceCalismaPrimiYuzde.unwrap_or_default(),
            geceCalismaTatiliPrimiYuzde: settings.geceCalismaTatiliPrimiYuzde.unwrap_or_default(),
            ekOdeme: settings.ekOdeme,
            digerGelirVarsayilan: settings.digerGelirVarsayilan,

            socialBenefitAccrualPolicies: None,
        }
    }

    fn policy_version(
        id: &str,
        effective_from: &str,
        state: CompensationPolicyState,
    ) -> CompensationPolicyVersion {
        CompensationPolicyVersion {
            id: id.into(),
            effectiveFrom: effective_from.into(),
            state,
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PeriodTotal,
            createdAt: "2026-09-03T00:00:00.000Z".into(),
            source: "WASM_TEST".into(),
            reason: "explicit policy export test".into(),
            supersedesVersionId: None,
        }
    }

    fn baseline_policy_from_settings(
        request: &PayrollCalculationRequest,
    ) -> CompensationPolicyVersion {
        request
            .dataset
            .compensationPolicyVersions
            .iter()
            .find(|version| version.effectiveFrom == "2026-01-15")
            .expect("period baseline policy should exist")
            .clone()
    }

    fn set_active_base_wage(request: &mut PayrollCalculationRequest, wage: rust_decimal::Decimal) {
        let start = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == request.periodId)
            .expect("active period should exist")
            .baslangicTarihi
            .clone();
        request
            .dataset
            .compensationPolicyVersions
            .iter_mut()
            .find(|version| version.effectiveFrom == start)
            .expect("active period baseline should exist")
            .state
            .gunlukTabanUcret = wage;
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
        let legacy_settings: Vec<DonemselKurumDegerleri> = request
            .dataset
            .periods
            .iter()
            .map(|period| DonemselKurumDegerleri {
                donemId: period.id.clone(),
                ..Default::default()
            })
            .collect();
        request.dataset.institutionSettings = legacy_settings
            .iter()
            .map(|settings| {
                (
                    settings.donemId.clone(),
                    PayrollPeriodInstitutionSettings::from(settings),
                )
            })
            .collect();
        request.dataset.compensationPolicyVersions = legacy_settings
            .iter()
            .map(|settings| {
                let period = request
                    .dataset
                    .periods
                    .iter()
                    .find(|period| period.id == settings.donemId)
                    .expect("matching period should exist");
                policy_version(
                    &format!("policy-baseline-{}", settings.donemId),
                    &period.baslangicTarihi,
                    policy_state_from_legacy_settings(settings),
                )
            })
            .collect();
        request.dataset.attendances = vec![fixture_attendance(active, "Ç")];
        request.dataset.payrolls.clear();
        request.dataset.sickLeaveRecords.clear();
        request.dataset.taxOpenings.clear();
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
        configure_active_period(&mut prior_request, vec![previous.clone()], previous);
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
    fn independent_payment_events_and_delete_policy_match_native() {
        use payroll_core::{AccrualType, PayrollAccrualInput};
        let mut request = standard_request();
        request.manualIncome = None;
        let period = request
            .dataset
            .periods
            .iter()
            .find(|p| p.id == request.periodId)
            .unwrap()
            .clone();
        for (index, kind) in [
            AccrualType::TEDIYE,
            AccrualType::NORMAL,
            AccrualType::TIS_IKRAMIYE,
        ]
        .into_iter()
        .enumerate()
        {
            request.accrual = Some(PayrollAccrualInput {
                accrualId: format!("event-{index}"),
                accrualType: kind,
                paymentDate: format!("{}-{:02}-10", period.taxYear, period.taxMonth),
                sequence: index as i32,
                grossAmount: if kind == AccrualType::NORMAL {
                    None
                } else {
                    Some(dec!(2000))
                },
                description: None,
            });
            assert_calculation_parity("independent-payment-event", request.clone(), true);
            request
                .dataset
                .payrolls
                .push(calculate_payroll(&request).unwrap());
        }
        let mutation = PayrollMutation::AccrualDelete {
            personnelId: request.personnelId.clone(),
            periodId: request.periodId.clone(),
            accrualId: "event-0".into(),
        };
        let input = serde_json::json!({ "dataset": request.dataset, "mutation": mutation });
        let wasm: serde_json::Value =
            serde_json::from_str(&evaluate_mutation_policy_json(&input.to_string()).unwrap())
                .unwrap();
        let native = evaluate_payroll_invalidation(&request.dataset, &mutation).unwrap();
        assert_eq!(wasm, serde_json::to_value(native).unwrap());
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
        thirty_days.dataset.personnel[0].devirKumulatifAsgariGvMatrahi = Some(dec!(1));
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
        raise_segment.dataset.personnel[0].devirKumulatifAsgariGvMatrahi = Some(dec!(1));
        raise_segment.dataset.personnel[0].devirKumulatifAsgariGvMatrahiYili = Some(2026);
        raise_segment.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(2);
        let mut raised_state = raise_segment
            .dataset
            .compensationPolicyVersions
            .iter()
            .find(|version| version.effectiveFrom == active.baslangicTarihi)
            .expect("active baseline should exist")
            .state
            .clone();
        raised_state.gunlukTabanUcret *= dec!(1.1);
        raise_segment
            .dataset
            .compensationPolicyVersions
            .push(policy_version(
                "policy-raise-2026-02-01",
                "2026-02-01",
                raised_state,
            ));
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
        set_active_base_wage(&mut bracket_transition, dec!(8000));
        fixtures.push(("income-tax-bracket-transition", bracket_transition, true));

        // 9. Kümülatif GV opening.
        let mut gv_opening = standard_request();
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahi = Some(dec!(120000));
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahiYili = Some(2026);
        gv_opening.dataset.personnel[0].devirKumulatifGvMatrahiBaslangicAyi = Some(1);
        fixtures.push(("cumulative-gv-opening", gv_opening, true));

        // 10. Asgari ücret GV istisnası.
        let mut minimum_wage_exemption = standard_request();
        set_active_base_wage(&mut minimum_wage_exemption, dec!(1101));
        fixtures.push((
            "minimum-wage-income-tax-exemption",
            minimum_wage_exemption,
            true,
        ));

        // 11. PEK alt sınırı.
        let mut pek_floor = standard_request();
        set_active_base_wage(&mut pek_floor, dec!(100));
        set_manual_zero(&mut pek_floor);
        fixtures.push(("pek-floor", pek_floor, true));

        // 12. PEK üst sınırı.
        let mut pek_ceiling = standard_request();
        set_active_base_wage(&mut pek_ceiling, dec!(10000));
        pek_ceiling
            .dataset
            .institutionSettings
            .get_mut("2026-01")
            .expect("standard settings should exist")
            .pekTavanKatsayisi = Some(dec!(1));
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
            nafakaTutar: None,
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
    fn adapter_finalizes_schema2_payroll_through_the_policy_engine() {
        let mut request = standard_request();
        let baseline = baseline_policy_from_settings(&request);
        request.dataset.compensationPolicyVersions = vec![baseline];
        let calculated = calculate_payroll_with_compensation_policy_checked(&request)
            .expect("explicit policy fixture should calculate");
        request.dataset.payrolls.push(calculated);
        let json = serde_json::to_string(&request).expect("request should serialize");

        let wasm_json = finalize_compensation_policy_payroll_json(&json)
            .expect("schema-2 policy record should finalize");
        let wasm_result: BordroKaydi = serde_json::from_str(&wasm_json).unwrap();
        let native_result =
            payroll_core::finalize_payroll_with_compensation_policy_checked(&request).unwrap();
        assert_eq!(wasm_result.status, BordroStatus::FINALIZED);
        assert_eq!(wasm_result.netOdeme, native_result.netOdeme);
        assert_eq!(
            serde_json::to_value(&wasm_result.calculationMetadata.as_ref().unwrap().snapshot)
                .unwrap(),
            serde_json::to_value(&native_result.calculationMetadata.as_ref().unwrap().snapshot)
                .unwrap()
        );

        let mut blocked = request.clone();
        let mut later_period = blocked.dataset.periods[0].clone();
        later_period.id = "2026-02".into();
        later_period.ay = 2;
        later_period.baslangicTarihi = "2026-02-15".into();
        later_period.bitisTarihi = "2026-03-14".into();
        later_period.taxMonth = 2;
        blocked.dataset.periods.push(later_period);
        let mut later_payroll = blocked.dataset.payrolls[0].clone();
        later_payroll.id = "person-1_2026-02".into();
        later_payroll.donemId = "2026-02".into();
        later_payroll.status = BordroStatus::FINALIZED;
        blocked.dataset.payrolls.push(later_payroll);
        let blocked_json = serde_json::to_string(&blocked).unwrap();
        let wasm_error = finalize_compensation_policy_payroll_json(&blocked_json)
            .expect_err("explicit policy finalization must reject immutable downstream payroll");
        let wasm_error: DomainError = serde_wasm_bindgen::from_value(wasm_error).unwrap();
        let native_error =
            payroll_core::finalize_payroll_with_compensation_policy_checked(&blocked)
                .expect_err("core should reject the explicit downstream dependency");
        assert_eq!(wasm_error.to_string(), native_error.to_string());

        request.dataset.payrolls[0].calculationMetadata = None;
        let legacy_json = serde_json::to_string(&request).unwrap();
        assert!(finalize_compensation_policy_payroll_json(&legacy_json).is_err());
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

    #[test]
    fn contract_boundary_uses_canonical_uppercase_snapshot_enum_tokens() {
        fn assert_token<T>(token: &str, expected: T)
        where
            T: DeserializeOwned + serde::Serialize + PartialEq + std::fmt::Debug,
        {
            let json = serde_json::to_string(token).expect("enum literal should encode");
            let decoded: T = deserialize_contract_json(&json)
                .expect("canonical enum token should deserialize at the WASM boundary");
            assert_eq!(decoded, expected);
            assert_eq!(
                serde_json::to_string(&decoded).expect("enum should serialize"),
                json
            );
        }

        assert_token(
            "COMPENSATION_POLICY_ENGINE",
            payroll_core::PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE,
        );
        assert_token(
            "COMPLETE",
            payroll_core::PayrollAttendanceCoverage::COMPLETE,
        );
        assert_token("PARTIAL", payroll_core::PayrollAttendanceCoverage::PARTIAL);
        assert_token(
            "EXPLICIT_OPENING",
            payroll_core::PayrollTaxOpeningSource::EXPLICIT_OPENING,
        );
        assert_token(
            "LEGACY_PERSONNEL_VALUE",
            payroll_core::PayrollTaxOpeningSource::LEGACY_PERSONNEL_VALUE,
        );
        assert_token(
            "IMPLICIT_ZERO",
            payroll_core::PayrollTaxOpeningSource::IMPLICIT_ZERO,
        );
        assert_token("NONE", payroll_core::PayrollPekCarrySource::NONE);
        assert_token(
            "PAYMENT_EVENT",
            payroll_core::PayrollPekCarrySource::PAYMENT_EVENT,
        );
        assert_token(
            "RETRO_OVERRIDE",
            payroll_core::PayrollPekCarrySource::RETRO_OVERRIDE,
        );
        assert_token(
            "FULL_EARNING",
            payroll_core::CompensationBoundaryScope::FullEarning,
        );
        assert_token(
            "ROAD_ONLY",
            payroll_core::CompensationBoundaryScope::RoadOnly,
        );
        assert_token(
            "PER_COMPENSATION_SLICE",
            payroll_core::RoadRoundingMode::PerCompensationSlice,
        );
        assert_token("PERIOD_TOTAL", payroll_core::RoadRoundingMode::PeriodTotal);
    }

    #[test]
    fn metadata_wasm_boundary_accepts_and_round_trips_the_shared_current_snapshot_fixture() {
        let json =
            include_str!("../../payroll-core/tests/fixtures/v6-current-engine-snapshot.json");
        let decoded: PayrollMetadataValidationPayload =
            deserialize_contract_json(json).expect("canonical V6 snapshot should deserialize");
        validate_payroll_backup_metadata_json_inner(json)
            .expect("canonical V6 snapshot should pass full financial replay");

        let fixture: serde_json::Value = serde_json::from_str(json).unwrap();
        let encoded = serde_json::to_value(decoded).expect("typed V6 payload should serialize");
        assert_eq!(
            encoded["bordrolar"][0]["calculationMetadata"]["snapshot"]["usedInputs"],
            fixture["bordrolar"][0]["calculationMetadata"]["snapshot"]["usedInputs"]
        );
        assert_eq!(
            encoded["bordrolar"][0]["calculationMetadata"]["provenance"],
            "COMPENSATION_POLICY_ENGINE"
        );

        let mut forged_financial_projection: serde_json::Value = fixture.clone();
        let net: rust_decimal::Decimal = forged_financial_projection["bordrolar"][0]["netOdeme"]
            .as_str()
            .expect("decimal net should use canonical string JSON")
            .parse()
            .expect("canonical net decimal should parse");
        forged_financial_projection["bordrolar"][0]["netOdeme"] =
            serde_json::json!((net + rust_decimal::Decimal::ONE).to_string());
        let error =
            validate_payroll_backup_metadata_json_inner(&forged_financial_projection.to_string())
                .expect_err("V6 boundary must reject a forged financial projection");
        assert!(error.to_string().contains("netOdeme"));

        let mut unsupported_schema: serde_json::Value = fixture.clone();
        unsupported_schema["bordrolar"][0]["calculationMetadata"]["snapshot"]
            ["snapshotSchemaVersion"] = serde_json::json!(1);
        assert!(
            validate_payroll_backup_metadata_json_inner(&unsupported_schema.to_string()).is_err()
        );

        let mut missing_flag: serde_json::Value = fixture.clone();
        missing_flag["compensationPolicyVersions"][0]
            .as_object_mut()
            .unwrap()
            .remove("boundaryScope");
        assert!(validate_payroll_backup_metadata_json_inner(&missing_flag.to_string()).is_err());

        let mut invalid_enum: serde_json::Value = fixture.clone();
        invalid_enum["bordrolar"][0]["calculationMetadata"]["snapshot"]["compensation"]
            ["segments"][0]["roadRoundingMode"] = serde_json::json!("PER_POLICY_SLICE");
        assert!(validate_payroll_backup_metadata_json_inner(&invalid_enum.to_string()).is_err());

        let mut mismatched_policy_link: serde_json::Value = fixture;
        mismatched_policy_link["bordrolar"][0]["calculationMetadata"]["snapshot"]["compensation"]
            ["segments"][0]["boundaryScope"] = serde_json::json!("ROAD_ONLY");
        assert!(
            validate_payroll_backup_metadata_json_inner(&mismatched_policy_link.to_string())
                .is_err()
        );
    }

    #[wasm_bindgen_test]
    fn metadata_wasm_export_accepts_the_shared_fixture_and_rejects_boundary_mismatch() {
        let fixture =
            include_str!("../../payroll-core/tests/fixtures/v6-current-engine-snapshot.json");
        assert!(validate_payroll_backup_metadata_json(fixture).is_ok());

        let mut forged_financial_projection: serde_json::Value =
            serde_json::from_str(fixture).unwrap();
        let net: rust_decimal::Decimal = forged_financial_projection["bordrolar"][0]["netOdeme"]
            .as_str()
            .expect("decimal net should use canonical string JSON")
            .parse()
            .expect("canonical net decimal should parse");
        forged_financial_projection["bordrolar"][0]["netOdeme"] =
            serde_json::json!((net + rust_decimal::Decimal::ONE).to_string());
        assert!(
            validate_payroll_backup_metadata_json(&forged_financial_projection.to_string())
                .is_err()
        );

        let mut mismatched: serde_json::Value = serde_json::from_str(fixture).unwrap();
        mismatched["bordrolar"][0]["calculationMetadata"]["snapshot"]["compensation"]["segments"]
            [0]["boundaryScope"] = serde_json::json!("ROAD_ONLY");
        assert!(validate_payroll_backup_metadata_json(&mismatched.to_string()).is_err());
    }

    #[wasm_bindgen_test]
    fn explicit_compensation_exports_resolve_and_calculate_with_policy_provenance() {
        let mut request = standard_request();
        let baseline = baseline_policy_from_settings(&request);
        request.dataset.compensationPolicyVersions = vec![baseline];
        let native = payroll_core::calculate_payroll_with_compensation_policy_checked(&request)
            .expect("explicit compensation policy should calculate");
        let request_json = serde_json::to_string(&request).expect("request should serialize");
        let wasm_json = calculate_compensation_policy_payroll_json(&request_json)
            .expect("WASM explicit policy export should calculate");
        let wasm: BordroKaydi =
            serde_json::from_str(&wasm_json).expect("WASM payroll should deserialize");
        assert_eq!(
            serde_json::to_value(&native).expect("native payroll should serialize"),
            serde_json::to_value(&wasm).expect("WASM payroll should serialize")
        );
        let metadata = wasm
            .calculationMetadata
            .as_ref()
            .expect("explicit policy calculation should persist provenance");
        assert_eq!(
            metadata.provenance,
            payroll_core::PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE
        );
        assert_eq!(metadata.snapshot.as_ref().unwrap().snapshotSchemaVersion, 2);

        let timeline_json = serde_json::json!({
            "startInclusive": "2026-01-15",
            "endInclusive": "2026-02-14",
            "versions": &request.dataset.compensationPolicyVersions,
        })
        .to_string();
        let resolved_json = resolve_compensation_policy_timeline_json(&timeline_json)
            .expect("timeline export should resolve using the core resolver");
        let resolved: serde_json::Value =
            serde_json::from_str(&resolved_json).expect("resolved timeline should be JSON");
        assert_eq!(resolved.as_array().unwrap().len(), 1);
        assert_eq!(resolved[0]["versionId"], "policy-baseline");
        assert_eq!(resolved[0]["endDate"], "2026-02-14");

        request.dataset.compensationPolicyVersions.clear();
        let missing_baseline = serde_json::to_string(&request).unwrap();
        assert!(calculate_compensation_policy_payroll_json(&missing_baseline).is_err());
    }
}
