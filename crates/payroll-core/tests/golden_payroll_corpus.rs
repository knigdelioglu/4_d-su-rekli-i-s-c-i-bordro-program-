use chrono::NaiveDate;
use payroll_core::{calculate_payroll_checked, BordroDonemi, PayrollCalculationRequest};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const GOLDEN_SCHEMA_VERSION: u32 = 1;
const MINIMUM_GOLDEN_CASES: usize = 30;
const EXPECTED_ASSERTION_PROFILE: &str = "FULL_FINANCIAL";
const EXPECTED_SOURCE_TYPE: &str = "independent_manual_calculation";
const VERIFIED_EVIDENCE_IDS: &[&str] = &[
    "G001", "G003", "G006", "G007", "G009", "G010", "G011", "G013", "G014", "G016", "G017", "G029",
    "G030", "G031", "G032",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenFixture {
    schema_version: u32,
    id: String,
    description: String,
    legal_year: i32,
    parameters_version: String,
    source: GoldenSource,
    request: PayrollCalculationRequest,
    expected: GoldenExpected,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenSource {
    #[serde(rename = "type")]
    source_type: String,
    reference_id: String,
    verified_by: String,
    verified_at: String,
    verification_status: String,
    notes: String,
    evidence: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenExpected {
    assertion_profile: String,
    result: Value,
}

fn golden_fixture_paths() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut paths = Vec::new();

    let years = fs::read_dir(&root)
        .unwrap_or_else(|error| {
            panic!(
                "Golden corpus dizini okunamadı ({}): {error}",
                root.display()
            )
        })
        .map(|entry| {
            entry
                .expect("Golden corpus directory entry okunmalı")
                .path()
        })
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();

    for year_dir in years {
        let entries = fs::read_dir(&year_dir).unwrap_or_else(|error| {
            panic!(
                "Golden corpus yıl dizini okunamadı ({}): {error}",
                year_dir.display()
            )
        });
        for entry in entries {
            let path = entry
                .expect("Golden corpus fixture directory entry okunmalı")
                .path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                paths.push(path);
            }
        }
    }

    paths.sort();
    paths
}

fn required_result_fields() -> &'static [&'static str] {
    &[
        "id",
        "personelId",
        "donemId",
        "accrualId",
        "accrualType",
        "paymentDate",
        "sequence",
        "puantajOzeti",
        "gelirler",
        "gelirToplam",
        "kesintiler",
        "kesintiToplam",
        "netOdeme",
        "status",
        "pekDetay",
        "gvDetay",
        "damgaDetay",
        "devredenPekGelen",
        "sonrakiDevredenPek",
        "statutorySnapshot",
    ]
}

fn period_for_request(request: &PayrollCalculationRequest) -> &BordroDonemi {
    request
        .dataset
        .periods
        .iter()
        .find(|period| period.id == request.periodId)
        .unwrap_or_else(|| panic!("{} request'i aktif period içermiyor", request.periodId))
}

fn assert_evidence(path: &Path, fixture: &GoldenFixture) {
    let Some(evidence) = fixture.source.evidence.as_deref() else {
        assert!(
            !VERIFIED_EVIDENCE_IDS.contains(&fixture.id.as_str()),
            "{} kritik fixture için evidence yolu eksik",
            path.display()
        );
        return;
    };

    assert!(
        VERIFIED_EVIDENCE_IDS.contains(&fixture.id.as_str()),
        "{} kritik liste dışında evidence işaretliyor",
        path.display()
    );
    let evidence_path = Path::new(evidence);
    assert!(
        !evidence_path.is_absolute()
            && !evidence_path
                .components()
                .any(|component| component == std::path::Component::ParentDir),
        "{} evidence yolu repo-relative ve parent traversal içermemeli",
        path.display()
    );
    assert!(
        evidence.starts_with("evidence/"),
        "{} evidence yolu evidence/ altında olmalı",
        path.display()
    );

    let golden_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let evidence_path = golden_root.join(evidence_path);
    let content = fs::read_to_string(&evidence_path).unwrap_or_else(|error| {
        panic!(
            "{} ({}) bağımsız evidence okunamadı: {error}",
            path.display(),
            fixture.id
        )
    });
    assert!(
        content.contains(&format!("# {}", fixture.id)),
        "{} evidence fixture id'sini açıkça taşımalı",
        evidence_path.display()
    );
    for marker in [
        "## Sabit girdiler",
        "## Gelir hesabı",
        "## PEK hesabı",
        "## İşçi primleri",
        "## GV hesabı",
        "## DV hesabı",
        "## Kesinti toplamı ve net ödeme",
        "## Kaynak / referans notu",
        "## Doğrulama tarihi",
    ] {
        assert!(
            content.contains(marker),
            "{} evidence bölümü eksik: {marker}",
            evidence_path.display()
        );
    }
    for forbidden in ["calculate_payroll_checked(", "calculate_payroll("] {
        assert!(
            !content.contains(forbidden),
            "{} production hesap çağrısı evidence içine yazılamaz: {forbidden}",
            evidence_path.display()
        );
    }
}

fn schema_pattern_matches(pattern: &str, value: &str) -> bool {
    match pattern {
        "^G[0-9]{3}$" => {
            let bytes = value.as_bytes();
            bytes.len() == 4 && bytes[0] == b'G' && bytes[1..].iter().all(u8::is_ascii_digit)
        }
        "^[0-9]{4}-.+" => {
            let bytes = value.as_bytes();
            bytes.len() > 5 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-'
        }
        "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" => {
            let bytes = value.as_bytes();
            bytes.len() == 10
                && bytes[4] == b'-'
                && bytes[7] == b'-'
                && bytes[..4].iter().all(u8::is_ascii_digit)
                && bytes[5..7].iter().all(u8::is_ascii_digit)
                && bytes[8..].iter().all(u8::is_ascii_digit)
        }
        "^evidence/[^/].+" => value
            .strip_prefix("evidence/")
            .is_some_and(|rest| !rest.starts_with('/') && rest.chars().count() >= 2),
        _ => panic!("Golden schema validator pattern desteği olmayan ifade içeriyor: {pattern}"),
    }
}

fn schema_type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => panic!("Golden schema validator type desteği olmayan ifade içeriyor: {expected}"),
    }
}

fn validate_schema_instance(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let schema_object = schema
        .as_object()
        .ok_or_else(|| format!("{path}: schema düğümü object olmalı"))?;

    if let Some(expected_type) = schema_object.get("type").and_then(Value::as_str) {
        if !schema_type_matches(expected_type, value) {
            return Err(format!("{path}: type={expected_type} bekleniyordu"));
        }
    }

    if let Some(expected) = schema_object.get("const") {
        if value != expected {
            return Err(format!("{path}: const değeri eşleşmiyor"));
        }
    }

    if let Some(options) = schema_object.get("enum").and_then(Value::as_array) {
        if !options.iter().any(|option| option == value) {
            return Err(format!("{path}: enum değeri geçersiz"));
        }
    }

    if let Some(minimum) = schema_object.get("minimum").and_then(Value::as_f64) {
        let actual = value
            .as_f64()
            .ok_or_else(|| format!("{path}: minimum için numeric değer gerekli"))?;
        if actual < minimum {
            return Err(format!("{path}: minimum={minimum} altında"));
        }
    }

    if let Some(min_length) = schema_object.get("minLength").and_then(Value::as_u64) {
        let actual = value
            .as_str()
            .ok_or_else(|| format!("{path}: minLength için string değer gerekli"))?;
        if actual.chars().count() < min_length as usize {
            return Err(format!("{path}: minLength={min_length} altında"));
        }
    }

    if let Some(pattern) = schema_object.get("pattern").and_then(Value::as_str) {
        let actual = value
            .as_str()
            .ok_or_else(|| format!("{path}: pattern için string değer gerekli"))?;
        if !schema_pattern_matches(pattern, actual) {
            return Err(format!("{path}: pattern eşleşmiyor: {pattern}"));
        }
    }

    if let Some(required) = schema_object.get("required").and_then(Value::as_array) {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path}: required için object değer gerekli"))?;
        for field in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(field) {
                return Err(format!("{path}: required alan eksik: {field}"));
            }
        }
    }

    if schema_object.get("additionalProperties") == Some(&Value::Bool(false)) {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path}: additionalProperties için object değer gerekli"))?;
        let properties = schema_object
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{path}: additionalProperties=false için properties gerekli"))?;
        if let Some(unknown) = object.keys().find(|key| !properties.contains_key(*key)) {
            return Err(format!("{path}: bilinmeyen alan: {unknown}"));
        }
    }

    if let Some(properties) = schema_object.get("properties").and_then(Value::as_object) {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path}: properties için object değer gerekli"))?;
        for (field, child_schema) in properties {
            if let Some(child_value) = object.get(field) {
                validate_schema_instance(child_schema, child_value, &format!("{path}.{field}"))?;
            }
        }
    }

    if let Some(item_schema) = schema_object.get("items") {
        let array = value
            .as_array()
            .ok_or_else(|| format!("{path}: items için array değer gerekli"))?;
        for (index, item) in array.iter().enumerate() {
            validate_schema_instance(item_schema, item, &format!("{path}[{index}]"))?;
        }
    }

    Ok(())
}

fn assert_schema_instance(path: &Path, schema: &Value, fixture: &Value) {
    if let Err(error) = validate_schema_instance(schema, fixture, "$fixture") {
        panic!(
            "{} JSON Schema doğrulaması başarısız: {error}",
            path.display()
        );
    }
}

fn assert_fixture_metadata(path: &Path, fixture: &GoldenFixture) {
    assert_eq!(
        fixture.schema_version,
        GOLDEN_SCHEMA_VERSION,
        "{} schemaVersion desteklenmiyor",
        path.display()
    );
    assert!(!fixture.id.trim().is_empty(), "{} id boş", path.display());
    assert!(
        !fixture.description.trim().is_empty(),
        "{} description boş",
        path.display()
    );
    assert!(
        !fixture.parameters_version.trim().is_empty(),
        "{} parametersVersion boş",
        path.display()
    );
    let expected_parameters_prefix = format!("{}-", fixture.legal_year);
    assert!(
        fixture
            .parameters_version
            .starts_with(&expected_parameters_prefix),
        "{} parametersVersion legalYear ile başlamalı",
        path.display()
    );

    let source = &fixture.source;
    assert_eq!(
        source.source_type,
        EXPECTED_SOURCE_TYPE,
        "{} production çıktısını bağımsız kaynak gibi işaretleyemez",
        path.display()
    );
    for (field, value) in [
        ("referenceId", source.reference_id.as_str()),
        ("verifiedBy", source.verified_by.as_str()),
        ("notes", source.notes.as_str()),
    ] {
        assert!(
            !value.trim().is_empty(),
            "{} source.{} boş",
            path.display(),
            field
        );
    }
    NaiveDate::parse_from_str(&source.verified_at, "%Y-%m-%d").unwrap_or_else(|error| {
        panic!(
            "{} source.verifiedAt YYYY-MM-DD olmalı: {error}",
            path.display()
        )
    });
    match source.verification_status.as_str() {
        "verified" => assert!(
            VERIFIED_EVIDENCE_IDS.contains(&fixture.id.as_str()) && source.evidence.is_some(),
            "{} verified işaretli fixture evidence listesinde ve evidence yolunda olmalı",
            path.display()
        ),
        "pendingEvidence" => assert!(
            !VERIFIED_EVIDENCE_IDS.contains(&fixture.id.as_str()) && source.evidence.is_none(),
            "{} pendingEvidence fixture evidence listesinde veya evidence yolunda olamaz",
            path.display()
        ),
        other => panic!(
            "{} source.verificationStatus tanımsız: {other}",
            path.display()
        ),
    }
    assert_evidence(path, fixture);

    assert_eq!(
        fixture.expected.assertion_profile,
        EXPECTED_ASSERTION_PROFILE,
        "{} yalnız FULL_FINANCIAL golden profiliyle kabul edilir",
        path.display()
    );
    assert!(
        fixture.expected.result.is_object(),
        "{} expected.result object olmalı",
        path.display()
    );
    for field in required_result_fields() {
        assert!(
            fixture.expected.result.get(*field).is_some(),
            "{} expected.result.{} eksik",
            path.display(),
            field
        );
    }

    let year_dir = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    assert_eq!(
        year_dir,
        fixture.legal_year.to_string(),
        "{} fixture dizini legalYear ile eşleşmiyor",
        path.display()
    );
    assert!(
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.starts_with(&fixture.id)),
        "{} dosya adı fixture id ile başlamalı",
        path.display()
    );

    let period = period_for_request(&fixture.request);
    assert_eq!(
        period.taxYear,
        fixture.legal_year,
        "{} period.taxYear legalYear ile eşleşmiyor",
        path.display()
    );
    assert_eq!(
        fixture.request.periodId,
        period.id,
        "{} request.periodId dataset period id ile eşleşmiyor",
        path.display()
    );
    assert!(
        fixture
            .request
            .dataset
            .annualPayrollParameters
            .iter()
            .any(|parameters| parameters.year == fixture.legal_year),
        "{} legalYear için yıllık parametre seti eksik",
        path.display()
    );
    assert!(
        fixture.request.calculatedAt.contains('T'),
        "{} calculatedAt deterministik ISO timestamp olmalı",
        path.display()
    );
}

#[test]
fn golden_schema_is_present_and_versioned() {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/schema.json");
    let payload = fs::read_to_string(&schema_path)
        .unwrap_or_else(|error| panic!("{} okunamadı: {error}", schema_path.display()));
    let schema: Value = serde_json::from_str(&payload).unwrap_or_else(|error| {
        panic!(
            "{} JSON schema parse edilemedi: {error}",
            schema_path.display()
        )
    });

    assert_eq!(
        schema.get("$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "{} draft 2020-12 olmalı",
        schema_path.display()
    );
    assert_eq!(
        schema
            .get("properties")
            .and_then(|properties| properties.get("schemaVersion"))
            .and_then(|schema_version| schema_version.get("const"))
            .and_then(Value::as_u64),
        Some(GOLDEN_SCHEMA_VERSION as u64),
        "{} schemaVersion v1 olmalı",
        schema_path.display()
    );
}

#[test]
fn golden_schema_rejects_constraint_violations() {
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/schema.json");
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(&schema_path)
            .unwrap_or_else(|error| panic!("{} okunamadı: {error}", schema_path.display())),
    )
    .expect("golden schema parse edilmeli");
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/2026/G001-normal-full-month.json");
    let fixture: Value = serde_json::from_str(
        &fs::read_to_string(&fixture_path)
            .unwrap_or_else(|error| panic!("{} okunamadı: {error}", fixture_path.display())),
    )
    .expect("golden fixture parse edilmeli");

    let mut invalid_enum = fixture.clone();
    invalid_enum["source"]["verificationStatus"] = Value::String("unverified".into());
    assert!(validate_schema_instance(&schema, &invalid_enum, "$fixture").is_err());

    let mut unknown_property = fixture.clone();
    unknown_property["unexpected"] = Value::Bool(true);
    assert!(validate_schema_instance(&schema, &unknown_property, "$fixture").is_err());

    let mut missing_required = fixture;
    missing_required
        .as_object_mut()
        .expect("fixture object")
        .remove("expected");
    assert!(validate_schema_instance(&schema, &missing_required, "$fixture").is_err());
}

#[test]
fn golden_payroll_corpus_is_independently_declared_and_exactly_replayed() {
    let paths = golden_fixture_paths();
    assert!(
        paths.len() >= MINIMUM_GOLDEN_CASES,
        "Golden corpus en az {} fixture içermeli; bulunan: {}",
        MINIMUM_GOLDEN_CASES,
        paths.len()
    );

    let mut ids = HashSet::new();
    let mut evidence_count = 0;
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/schema.json");
    let schema_payload = fs::read_to_string(&schema_path)
        .unwrap_or_else(|error| panic!("{} okunamadı: {error}", schema_path.display()));
    let schema: Value = serde_json::from_str(&schema_payload).unwrap_or_else(|error| {
        panic!(
            "{} JSON schema parse edilemedi: {error}",
            schema_path.display()
        )
    });
    for path in paths {
        let payload = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} okunamadı: {error}", path.display()));
        let raw_fixture: Value = serde_json::from_str(&payload)
            .unwrap_or_else(|error| panic!("{} JSON parse edilemedi: {error}", path.display()));
        assert_schema_instance(&path, &schema, &raw_fixture);
        let fixture: GoldenFixture = serde_json::from_value(raw_fixture).unwrap_or_else(|error| {
            panic!("{} typed model parse edilemedi: {error}", path.display())
        });
        assert_fixture_metadata(&path, &fixture);
        assert!(
            ids.insert(fixture.id.clone()),
            "{} duplicate golden fixture id: {}",
            path.display(),
            fixture.id
        );
        if fixture.source.evidence.is_some() {
            evidence_count += 1;
        }

        let actual = calculate_payroll_checked(&fixture.request).unwrap_or_else(|error| {
            panic!(
                "{} ({}) calculate_payroll_checked ile hesaplanamadı: {error}",
                path.display(),
                fixture.id
            )
        });
        let actual_json = serde_json::to_value(&actual).unwrap_or_else(|error| {
            panic!(
                "{} actual result serialize edilemedi: {error}",
                path.display()
            )
        });
        assert_eq!(
            actual_json,
            fixture.expected.result,
            "{} ({}) FULL_FINANCIAL golden sonucu exact eşleşmiyor",
            path.display(),
            fixture.id
        );
    }

    assert_eq!(
        evidence_count,
        VERIFIED_EVIDENCE_IDS.len(),
        "kritik golden evidence kapsamı beklenen 15 fixture ile eşleşmeli"
    );
}
