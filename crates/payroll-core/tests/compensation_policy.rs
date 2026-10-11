use payroll_core::{
    find_road_rounding_issues, order_compensation_policy_versions, resolve_compensation_segments,
    validate_compensation_policy_state, validate_compensation_policy_timeline,
    CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
    CompensationPolicyVersion, PayrollDatasetSnapshot, ResolvedCompensationSegment,
    RoadRoundingMode,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::{json, Value};

fn state(base: Decimal) -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: base,
        gunlukYemek: dec!(10.12345),
        birlestirilmisSosyalYardim: dec!(20),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiYuzde: None,
        isPrimiGruplari: vec![CompensationPolicyGroup {
            id: "g1".into(),
            ad: "1. Grup".into(),
            oran: dec!(9.125),
            aktif: true,
        }],
        geceCalismaPrimiYuzde: dec!(0),
        geceCalismaTatiliPrimiYuzde: dec!(0),
        ekOdeme: None,
        digerGelirVarsayilan: Some(dec!(0)),
    }
}

fn version(id: &str, date: &str, base: Decimal, parent: Option<&str>) -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: id.into(),
        effectiveFrom: date.into(),
        state: state(base),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-01-01T00:00:00Z".into(),
        source: "ADMINISTRATIVE_DECISION".into(),
        reason: format!("reason-{id}"),
        supersedesVersionId: parent.map(str::to_owned),
    }
}

fn resolve(
    start: &str,
    end: &str,
    versions: &[CompensationPolicyVersion],
) -> Vec<ResolvedCompensationSegment> {
    resolve_compensation_segments(start, end, versions).unwrap()
}

#[test]
fn inclusive_15_14_segments_include_start_and_end_effective_dates() {
    let versions = vec![
        version("base", "2026-01-15", dec!(100), None),
        version("middle", "2026-01-16", dec!(110), None),
        version("end", "2026-02-14", dec!(120), None),
    ];
    let segments = resolve("2026-01-15", "2026-02-14", &versions);
    assert_eq!(segments.len(), 3);
    assert_eq!(
        (segments[0].startDate.as_str(), segments[0].endDate.as_str()),
        ("2026-01-15", "2026-01-15")
    );
    assert_eq!(
        (segments[1].startDate.as_str(), segments[1].endDate.as_str()),
        ("2026-01-16", "2026-02-13")
    );
    assert_eq!(
        (segments[2].startDate.as_str(), segments[2].endDate.as_str()),
        ("2026-02-14", "2026-02-14")
    );
    assert_eq!(segments[1].state.gunlukTabanUcret, dec!(110));
}

#[test]
fn calendar_month_edges_cover_28_29_30_and_31_inclusive_days() {
    for (start, end, days) in [
        ("2025-02-15", "2025-03-14", 28),
        ("2024-02-15", "2024-03-14", 29),
        ("2025-04-15", "2025-05-14", 30),
        ("2025-01-15", "2025-02-14", 31),
    ] {
        let segments = resolve(start, end, &[version("old", "2020-01-01", dec!(100), None)]);
        assert_eq!(segments.len(), 1);
        assert_eq!(
            (segments[0].startDate.as_str(), segments[0].endDate.as_str()),
            (start, end)
        );
        let start_date = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        let end_date = chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
        assert_eq!((end_date - start_date).num_days() + 1, days);
    }
}

#[test]
fn long_gap_is_valid_and_missing_baseline_is_a_typed_error() {
    assert_eq!(
        resolve(
            "2026-03-15",
            "2026-04-14",
            &[version("old", "2020-01-01", dec!(100), None)]
        )
        .len(),
        1
    );
    let error = resolve_compensation_segments("2026-03-15", "2026-04-14", &[]).unwrap_err();
    assert!(matches!(error, payroll_core::DomainError::NotFound(_)));
    assert!(error
        .to_string()
        .contains("Bu tarih için ücret politikası tanımlı değil"));
}

#[test]
fn chained_corrections_resolve_to_the_sole_active_leaf() {
    let versions = vec![
        version("base", "2026-01-15", dec!(100), None),
        version("fix-1", "2026-01-15", dec!(110), Some("base")),
        version("fix-2", "2026-01-15", dec!(120), Some("fix-1")),
    ];
    let segments = resolve("2026-01-15", "2026-02-14", &versions);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].versionId, "fix-2");
    assert_eq!(segments[0].state.gunlukTabanUcret, dec!(120));
}

#[test]
fn one_in_period_change_produces_two_complete_segments() {
    let versions = vec![
        version("base", "2025-01-01", dec!(100), None),
        version("change", "2026-02-01", dec!(200), None),
    ];
    let segments = resolve("2026-01-15", "2026-02-14", &versions);
    assert_eq!(segments.len(), 2);
    assert_eq!(
        (segments[0].startDate.as_str(), segments[0].endDate.as_str()),
        ("2026-01-15", "2026-01-31")
    );
    assert_eq!(
        (segments[1].startDate.as_str(), segments[1].endDate.as_str()),
        ("2026-02-01", "2026-02-14")
    );
    assert_eq!(segments[0].state.gunlukTabanUcret, dec!(100));
    assert_eq!(segments[1].state.gunlukTabanUcret, dec!(200));
}

#[test]
fn invalid_dates_reverse_ranges_and_input_order_fail_closed() {
    for date in [
        "2025-02-29",
        "2026-2-01",
        "2026-02-01T00:00:00Z",
        "2026-02-30",
    ] {
        assert!(
            resolve_compensation_segments(date, "2026-03-01", &[]).is_err(),
            "{date}"
        );
    }
    assert!(resolve_compensation_segments("2026-03-02", "2026-03-01", &[]).is_err());
    assert!(resolve_compensation_segments(
        "2026-01-15",
        "2026-02-14",
        &[version("bad-version-date", "2025-02-29", dec!(100), None)]
    )
    .is_err());
    let unsorted = vec![
        version("later", "2026-02-01", dec!(100), None),
        version("earlier", "2026-01-01", dec!(100), None),
    ];
    assert!(resolve_compensation_segments("2026-01-15", "2026-02-14", &unsorted).is_err());
    let backwards_correction = vec![
        version("fix", "2026-01-15", dec!(110), Some("base")),
        version("base", "2026-01-15", dec!(100), None),
    ];
    assert!(
        resolve_compensation_segments("2026-01-15", "2026-02-14", &backwards_correction).is_err()
    );
}

#[test]
fn duplicate_date_identity_and_supersession_graph_errors_are_rejected() {
    assert!(order_compensation_policy_versions(&[
        version("a", "2026-01-15", dec!(100), None),
        version("b", "2026-01-15", dec!(110), None),
    ])
    .is_err());
    assert!(order_compensation_policy_versions(&[
        version("same", "2026-01-15", dec!(100), None),
        version("same", "2026-01-15", dec!(110), None),
    ])
    .is_err());
    assert!(order_compensation_policy_versions(&[version(
        "a",
        "2026-01-15",
        dec!(100),
        Some("absent")
    )])
    .is_err());
    assert!(order_compensation_policy_versions(&[version(
        "a",
        "2026-01-15",
        dec!(100),
        Some("a")
    )])
    .is_err());
    assert!(order_compensation_policy_versions(&[
        version("a", "2026-01-15", dec!(100), Some("b")),
        version("b", "2026-01-15", dec!(100), Some("a")),
    ])
    .is_err());
    assert!(order_compensation_policy_versions(&[
        version("base", "2026-01-15", dec!(100), None),
        version("fix", "2026-02-15", dec!(110), Some("base")),
    ])
    .is_err());
    assert!(order_compensation_policy_versions(&[
        version("base", "2026-01-15", dec!(100), None),
        version("fix-a", "2026-01-15", dec!(110), Some("base")),
        version("fix-b", "2026-01-15", dec!(120), Some("base")),
    ])
    .is_err());
}

#[test]
fn state_validation_rejects_bad_values_and_ambiguous_group_tokens() {
    assert!(validate_compensation_policy_state(&state(dec!(0))).is_err());
    let mut invalid = state(dec!(100));
    invalid.gunlukYemek = dec!(-1);
    assert!(validate_compensation_policy_state(&invalid).is_err());
    invalid = state(dec!(100));
    invalid.geceCalismaPrimiYuzde = dec!(101);
    assert!(validate_compensation_policy_state(&invalid).is_err());
    invalid = state(dec!(100));
    invalid.isPrimiGruplari.push(CompensationPolicyGroup {
        id: "other".into(),
        ad: "g1".into(),
        oran: dec!(10),
        aktif: true,
    });
    assert!(validate_compensation_policy_state(&invalid).is_err());
}

#[test]
fn same_group_id_and_name_is_valid_but_cross_group_active_tokens_are_ambiguous() {
    let mut same_token = state(dec!(100));
    same_token.isPrimiGruplari[0].id = "G1".into();
    same_token.isPrimiGruplari[0].ad = "G1".into();
    validate_compensation_policy_state(&same_token).unwrap();

    let mut same_date = version("same-token", "2026-01-15", dec!(100), None);
    same_date.state = same_token.clone();
    let segments = resolve("2026-01-15", "2026-02-14", &[same_date]);
    assert_eq!(segments[0].state.isPrimiGruplari[0].id, "G1");
    assert_eq!(segments[0].state.isPrimiGruplari[0].ad, "G1");

    // Inactive groups do not participate in personnel matching. Their
    // historical tokens may overlap with an active group as before.
    let mut with_inactive = same_token.clone();
    with_inactive.isPrimiGruplari.push(CompensationPolicyGroup {
        id: "G1".into(),
        ad: "G1".into(),
        oran: dec!(10),
        aktif: false,
    });
    validate_compensation_policy_state(&with_inactive).unwrap();

    let mut ambiguous = same_token;
    ambiguous.isPrimiGruplari.push(CompensationPolicyGroup {
        id: "G2".into(),
        ad: "G1".into(),
        oran: dec!(10),
        aktif: true,
    });
    assert!(validate_compensation_policy_state(&ambiguous).is_err());
}

#[test]
fn state_json_requires_nullable_keys_and_round_trips_exact_decimal_strings() {
    let original = state(dec!(123.456789));
    let encoded = serde_json::to_value(&original).unwrap();
    assert_eq!(encoded["gunlukTabanUcret"], "123.456789");
    assert_eq!(encoded["gunlukYemek"], "10.12345");
    assert_eq!(encoded["isPrimiYuzde"], Value::Null);
    assert_eq!(encoded["ekOdeme"], Value::Null);
    assert_eq!(
        serde_json::from_value::<CompensationPolicyState>(encoded.clone()).unwrap(),
        original
    );
    let mut missing_nullable = encoded.clone();
    missing_nullable.as_object_mut().unwrap().remove("ekOdeme");
    assert!(serde_json::from_value::<CompensationPolicyState>(missing_nullable).is_err());
    let mut missing_required = encoded.clone();
    missing_required
        .as_object_mut()
        .unwrap()
        .remove("geceCalismaPrimiYuzde");
    assert!(serde_json::from_value::<CompensationPolicyState>(missing_required).is_err());
    let mut numeric = encoded.clone();
    numeric["gunlukTabanUcret"] = json!(123.456789);
    assert!(serde_json::from_value::<CompensationPolicyState>(numeric).is_err());
    let mut invalid_decimal = encoded.clone();
    invalid_decimal["gunlukTabanUcret"] = json!("NaN");
    assert!(serde_json::from_value::<CompensationPolicyState>(invalid_decimal).is_err());
    let mut explicit_zero = encoded;
    explicit_zero["ekOdeme"] = json!("0");
    explicit_zero["isPrimiYuzde"] = json!("0");
    let explicit_zero: CompensationPolicyState = serde_json::from_value(explicit_zero).unwrap();
    assert_eq!(explicit_zero.ekOdeme, Some(Decimal::ZERO));
    assert_eq!(explicit_zero.isPrimiYuzde, Some(Decimal::ZERO));
    assert_eq!(explicit_zero.digerGelirVarsayilan, Some(Decimal::ZERO));
}

#[test]
fn resolver_is_deterministic_from_only_dates_and_timeline() {
    let versions = [version("base", "2020-01-01", dec!(100), None)];
    assert_eq!(
        resolve("2026-01-15", "2026-02-14", &versions),
        resolve("2026-01-15", "2026-02-14", &versions)
    );
}

#[test]
fn legacy_dataset_json_without_policy_collection_loads_as_empty() {
    let mut legacy = serde_json::to_value(PayrollDatasetSnapshot::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("compensationPolicyVersions");
    let decoded: PayrollDatasetSnapshot = serde_json::from_value(legacy).unwrap();
    assert!(decoded.compensationPolicyVersions.is_empty());
}

#[test]
fn policy_boundary_flags_are_required_and_use_canonical_tokens() {
    let original = version("base", "2026-01-15", dec!(100), None);
    let encoded = serde_json::to_value(&original).unwrap();
    assert_eq!(encoded["boundaryScope"], "FULL_EARNING");
    assert_eq!(encoded["roadRoundingMode"], "PER_COMPENSATION_SLICE");
    assert_eq!(
        serde_json::from_value::<CompensationPolicyVersion>(encoded.clone()).unwrap(),
        original
    );

    for field in ["boundaryScope", "roadRoundingMode"] {
        let mut missing = encoded.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<CompensationPolicyVersion>(missing).is_err());
        let mut unknown = encoded.clone();
        unknown[field] = json!("UNKNOWN");
        assert!(serde_json::from_value::<CompensationPolicyVersion>(unknown).is_err());
    }
}

#[test]
fn road_only_projection_preserves_all_nonroad_state_and_accepts_clipped_baseline() {
    let mut baseline = version("base", "2026-01-01", dec!(100), None);
    let mut road_only = version("road", "2026-01-25", dec!(100), None);
    baseline.state.gunlukVasitaYol = dec!(20.005);
    road_only.state.gunlukVasitaYol = dec!(30.005);
    road_only.boundaryScope = CompensationBoundaryScope::RoadOnly;
    let segments = resolve(
        "2026-01-15",
        "2026-02-14",
        &[baseline.clone(), road_only.clone()],
    );
    assert_eq!(segments.len(), 2);
    assert_eq!(
        segments[1].boundaryScope,
        CompensationBoundaryScope::RoadOnly
    );
    assert_eq!(
        segments[1].state.gunlukTabanUcret,
        segments[0].state.gunlukTabanUcret
    );
    assert_eq!(segments[1].state.gunlukVasitaYol, dec!(30.005));

    let mut contradictory = road_only;
    contradictory.state.gunlukYemek += Decimal::ONE;
    assert!(
        resolve_compensation_segments("2026-01-15", "2026-02-14", &[baseline, contradictory])
            .is_err()
    );

    // This is the only row the scoped projection has to load. No earlier
    // FULL_EARNING row is needed to establish the period's natural baseline.
    let mut clipped_baseline = version("clipped-road", "2026-01-01", dec!(100), None);
    clipped_baseline.boundaryScope = CompensationBoundaryScope::RoadOnly;
    let projected = resolve("2026-01-15", "2026-02-14", &[clipped_baseline]);
    assert_eq!(projected.len(), 1);
    assert_eq!(
        projected[0].boundaryScope,
        CompensationBoundaryScope::RoadOnly
    );
}

#[test]
fn road_modes_must_match_active_projection_but_superseded_flags_are_inert() {
    let baseline = version("base", "2026-01-15", dec!(100), None);
    let mut changed = version("changed", "2026-02-01", dec!(110), None);
    changed.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    let error =
        resolve_compensation_segments("2026-01-15", "2026-02-14", &[baseline.clone(), changed])
            .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("2026-02-01"));
    assert!(message.contains("base"));
    assert!(message.contains("changed"));
    assert!(message.contains("2026-01-15–2026-02-14"));
    let issues = find_road_rounding_issues(
        "2026-01-15",
        "2026-02-14",
        &[baseline.clone(), {
            let mut active = version("changed", "2026-02-01", dec!(110), None);
            active.roadRoundingMode = RoadRoundingMode::PeriodTotal;
            active
        }],
    )
    .unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].expected_version_id, "base");
    assert_eq!(issues[0].actual_version_id, "changed");
    assert_eq!(issues[0].effective_from, "2026-02-01");
    assert_eq!(issues[0].period_start_inclusive, "2026-01-15");
    assert_eq!(issues[0].period_end_inclusive, "2026-02-14");

    let old = version("old", "2026-01-15", dec!(100), None);
    let mut active_fix = version("fix", "2026-01-15", dec!(100), Some("old"));
    active_fix.boundaryScope = CompensationBoundaryScope::RoadOnly;
    let next = version("next", "2026-02-01", dec!(110), None);
    let selected = resolve(
        "2026-01-15",
        "2026-02-14",
        &[old.clone(), active_fix.clone(), next.clone()],
    );
    assert_eq!(selected[0].versionId, "fix");

    let mut differing_ancestor = old;
    differing_ancestor.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    assert!(validate_compensation_policy_timeline(&[differing_ancestor, active_fix, next]).is_ok());
}

#[test]
fn rounding_mode_change_on_actual_service_period_start_is_valid() {
    let mut previous = version("previous", "2026-01-01", dec!(100), None);
    previous.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    let mut at_boundary = version("at-boundary", "2026-01-15", dec!(100), None);
    at_boundary.roadRoundingMode = RoadRoundingMode::PerCompensationSlice;

    let issues =
        find_road_rounding_issues("2026-01-15", "2026-02-14", &[previous, at_boundary.clone()])
            .unwrap();
    assert!(issues.is_empty());
    let segments = resolve(
        "2026-01-15",
        "2026-02-14",
        &[version("old", "2026-01-01", dec!(100), None), at_boundary],
    );
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].versionId, "at-boundary");
}

#[test]
fn service_period_raise_and_road_rounding_transition_stay_consistent_across_new_year() {
    let mut baseline = version("base-2027", "2027-12-15", dec!(100), None);
    baseline.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    baseline.state.gunlukVasitaYol = dec!(136.44);

    let mut february_raise = version("raise-2028-02-01", "2028-01-01", dec!(106.25), None);
    february_raise.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    february_raise.state.gunlukVasitaYol = dec!(136.44);

    let mut next_service_period = version("rounding-2028-01-15", "2028-01-15", dec!(106.25), None);
    next_service_period.roadRoundingMode = RoadRoundingMode::PerCompensationSlice;
    next_service_period.state.gunlukVasitaYol = dec!(136.44);

    let december = resolve(
        "2027-12-15",
        "2028-01-14",
        &[baseline.clone(), february_raise.clone()],
    );
    assert_eq!(december.len(), 2);
    assert_eq!(december[0].roadRoundingMode, RoadRoundingMode::PeriodTotal);
    assert_eq!(december[1].roadRoundingMode, RoadRoundingMode::PeriodTotal);
    assert_eq!(december[1].state.gunlukTabanUcret, dec!(106.25));
    assert_eq!(december[1].state.gunlukVasitaYol, dec!(136.44));

    let january = resolve(
        "2028-01-15",
        "2028-02-14",
        &[baseline, february_raise, next_service_period],
    );
    assert_eq!(january.len(), 1);
    assert_eq!(
        january[0].roadRoundingMode,
        RoadRoundingMode::PerCompensationSlice
    );
}

#[test]
fn same_date_correction_can_restore_the_service_period_rounding_mode() {
    let mut baseline = version("base", "2027-09-15", dec!(100), None);
    baseline.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    baseline.state.gunlukVasitaYol = dec!(136.44);

    let mut wrong = version("wrong-rounding", "2027-10-01", dec!(106.25), None);
    wrong.roadRoundingMode = RoadRoundingMode::PerCompensationSlice;
    wrong.state.gunlukVasitaYol = dec!(136.44);

    let mut correction = version(
        "corrected-rounding",
        "2027-10-01",
        dec!(106.25),
        Some("wrong-rounding"),
    );
    correction.roadRoundingMode = RoadRoundingMode::PeriodTotal;
    correction.state.gunlukVasitaYol = dec!(136.44);

    let resolved = resolve("2027-09-15", "2027-10-14", &[baseline, wrong, correction]);
    assert_eq!(resolved.len(), 2);
    assert!(resolved
        .iter()
        .all(|segment| segment.roadRoundingMode == RoadRoundingMode::PeriodTotal));
    assert_eq!(resolved[1].versionId, "corrected-rounding");
    assert_eq!(resolved[1].state.gunlukVasitaYol, dec!(136.44));
}
