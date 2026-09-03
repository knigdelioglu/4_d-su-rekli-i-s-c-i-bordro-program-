use payroll_core::{calculate_payroll, PayrollCalculationRequest, Result as CoreResult};
use wasm_bindgen::prelude::*;

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

/// Validates and parses the same request boundary without persisting anything.
#[wasm_bindgen]
pub fn validate_payroll_json(request_json: &str) -> Result<(), JsValue> {
    let request = parse_request(request_json).map_err(error_to_js)?;
    payroll_core::validate_payroll_request(&request).map_err(error_to_js)
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
        calculate_payroll, AnnualPayrollParameters, BordroDonemi, BordroKaydi,
        DonemselKurumDegerleri, ManualPayrollIncomeInput, PayrollCalculationRequest,
        PayrollDatasetSnapshot, Personel, PersonelPuantaj,
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
        gunler.insert("2026-01-15".into(), "Ç".into());

        serde_json::to_string(&PayrollCalculationRequest {
            personnelId: "person-1".into(),
            periodId: period.id.clone(),
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

    #[wasm_bindgen_test]
    fn adapter_calculates_through_the_shared_contract() {
        let json = request_json();
        validate_payroll_json(&json).expect("shared preflight should pass");
        let result = calculate_payroll_json(&json).expect("adapter should calculate");
        let value: serde_json::Value = serde_json::from_str(&result).expect("valid result JSON");

        assert_eq!(value["status"], serde_json::json!("CALCULATED"));
        assert_eq!(value["gelirler"]["tediye"], serde_json::json!(123.45));

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
}
