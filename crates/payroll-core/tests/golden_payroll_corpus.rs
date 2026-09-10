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
    notes: String,
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
fn golden_payroll_corpus_is_independently_declared_and_exactly_replayed() {
    let paths = golden_fixture_paths();
    assert!(
        paths.len() >= MINIMUM_GOLDEN_CASES,
        "Golden corpus en az {} fixture içermeli; bulunan: {}",
        MINIMUM_GOLDEN_CASES,
        paths.len()
    );

    let mut ids = HashSet::new();
    for path in paths {
        let payload = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} okunamadı: {error}", path.display()));
        let fixture: GoldenFixture = serde_json::from_str(&payload).unwrap_or_else(|error| {
            panic!("{} schema/model parse edilemedi: {error}", path.display())
        });
        assert_fixture_metadata(&path, &fixture);
        assert!(
            ids.insert(fixture.id.clone()),
            "{} duplicate golden fixture id: {}",
            path.display(),
            fixture.id
        );

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
}
