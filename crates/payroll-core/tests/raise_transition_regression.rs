use chrono::{Duration, NaiveDate};
use payroll_core::*;
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period(year: i32, month: u32) -> BordroDonemi {
    let start = NaiveDate::from_ymd_opt(year, month, 15).unwrap();
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    BordroDonemi {
        id: format!("{year}-{month:02}"),
        yil: year,
        ay: month as i32,
        baslangicTarihi: start.to_string(),
        bitisTarihi: NaiveDate::from_ymd_opt(next_year, next_month, 14)
            .unwrap()
            .to_string(),
        donemAdi: format!("{year}-{month:02}"),
        taxYear: year,
        taxMonth: month as i32,
    }
}

fn attendance(person_id: &str, period: &BordroDonemi) -> PersonelPuantaj {
    let mut days = HashMap::new();
    let mut date = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    while date <= end {
        days.insert(date.to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: format!("{person_id}_{}", period.id),
        personelId: person_id.into(),
        donemId: period.id.clone(),
        gunler: days,
    }
}

fn policy_state(settings: &DonemselKurumDegerleri) -> CompensationPolicyState {
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
    }
}

fn set_policy_version(
    request: &mut PayrollCalculationRequest,
    effective_from: &str,
    state: CompensationPolicyState,
) {
    if let Some(version) = request
        .dataset
        .compensationPolicyVersions
        .iter_mut()
        .find(|version| version.effectiveFrom == effective_from)
    {
        version.state = state;
        return;
    }
    request
        .dataset
        .compensationPolicyVersions
        .push(CompensationPolicyVersion {
            id: format!("test-policy-{effective_from}"),
            effectiveFrom: effective_from.into(),
            state,
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Explicit timeline fixture".into(),
            supersedesVersionId: None,
        });
    request
        .dataset
        .compensationPolicyVersions
        .sort_by(|left, right| {
            left.effectiveFrom
                .cmp(&right.effectiveFrom)
                .then_with(|| left.id.cmp(&right.id))
        });
}

fn update_policy_state(
    request: &mut PayrollCalculationRequest,
    effective_from: &str,
    update: impl FnOnce(&mut CompensationPolicyState),
) {
    let mut state = request
        .dataset
        .compensationPolicyVersions
        .iter()
        .find(|version| version.effectiveFrom == effective_from)
        .or_else(|| {
            request
                .dataset
                .compensationPolicyVersions
                .iter()
                .filter(|version| version.effectiveFrom.as_str() < effective_from)
                .max_by(|left, right| left.effectiveFrom.cmp(&right.effectiveFrom))
        })
        .expect("policy version at or before effective date")
        .state
        .clone();
    update(&mut state);
    set_policy_version(request, effective_from, state);
}

fn request(year: i32, month: u32) -> PayrollCalculationRequest {
    // Reuse a valid structural DTO; all tested compensation inputs and
    // expected amounts below are set independently of the golden result.
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("golden/2026/G020-social-aid.json")).unwrap();
    let mut request: PayrollCalculationRequest =
        serde_json::from_value(fixture["request"].clone()).unwrap();
    let source_settings: HashMap<String, DonemselKurumDegerleri> =
        serde_json::from_value(fixture["request"]["dataset"]["institutionSettings"].clone())
            .unwrap();
    let current = period(year, month);
    let previous = if month == 1 {
        period(year - 1, 12)
    } else {
        period(year, month - 1)
    };
    let mut settings = source_settings.values().next().unwrap().clone();
    settings.donemId = current.id.clone();
    settings.gunlukTabanUcret = dec!(110);
    settings.gunlukYemek = dec!(11);
    settings.birlestirilmisSosyalYardim = dec!(1100);
    settings.giyimYardimi = dec!(110);
    settings.hizmetZammiBirimi = dec!(1.1);
    let mut old = settings.clone();
    old.donemId = previous.id.clone();
    old.gunlukTabanUcret = dec!(100);
    old.gunlukYemek = dec!(10);
    old.birlestirilmisSosyalYardim = dec!(1000);
    old.giyimYardimi = dec!(100);
    old.hizmetZammiBirimi = dec!(1);
    request.periodId = current.id.clone();
    let accrual = request.accrual.as_mut().unwrap();
    accrual.accrualId = format!("{}_{}", request.personnelId, current.id);
    accrual.paymentDate = format!("{year}-{month:02}-25");
    request.dataset.attendances = vec![attendance(&request.personnelId, &current)];
    request.dataset.institutionSettings = HashMap::from([
        (
            previous.id.clone(),
            PayrollPeriodInstitutionSettings::from(&old),
        ),
        (
            current.id.clone(),
            PayrollPeriodInstitutionSettings::from(&settings),
        ),
    ]);
    request.dataset.periods = vec![previous.clone(), current];
    request.dataset.taxOpenings = vec![PersonelTaxOpening {
        id: format!("test-opening-{}-{year}", request.personnelId),
        personnelId: request.personnelId.clone(),
        year,
        gvCumulativeOpening: Some(Decimal::ZERO),
        effectiveFromPeriodId: Some(request.periodId.clone()),
        asgariGvCumulativeOpening: Some(Decimal::ZERO),
        asgariGvEffectiveFromPeriodId: Some(request.periodId.clone()),
        createdAt: None,
        updatedAt: None,
    }];
    let raise_date = if month == 12 {
        format!("{}-01-01", year + 1)
    } else {
        format!("{year}-{:02}-01", month + 1)
    };
    request.dataset.compensationPolicyVersions = vec![
        CompensationPolicyVersion {
            id: format!("test-policy-{}", previous.baslangicTarihi),
            effectiveFrom: previous.baslangicTarihi.clone(),
            state: policy_state(&old),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Explicit baseline".into(),
            supersedesVersionId: None,
        },
        CompensationPolicyVersion {
            id: format!("test-policy-{raise_date}"),
            effectiveFrom: raise_date,
            state: policy_state(&settings),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "Explicit dated increase".into(),
            supersedesVersionId: None,
        },
    ];
    for tax_month in 1..month {
        let historical = period(year, tax_month);
        if request
            .dataset
            .periods
            .iter()
            .any(|existing| existing.id == historical.id)
        {
            continue;
        }
        let mut historical_settings = old.clone();
        historical_settings.donemId = historical.id.clone();
        request.dataset.institutionSettings.insert(
            historical.id.clone(),
            PayrollPeriodInstitutionSettings::from(&historical_settings),
        );
        request.dataset.periods.push(historical);
    }
    request.dataset.annualPayrollParameters[0].year = year;
    request
}

fn advance(request: &mut PayrollCalculationRequest) {
    let previous_period = request
        .dataset
        .periods
        .iter()
        .find(|p| p.id == request.periodId)
        .unwrap()
        .clone();
    let payroll = calculate_payroll(request).unwrap();
    request.dataset.payrolls.push(payroll);
    request.dataset.taxOpenings.clear();
    let month = previous_period.ay as u32;
    let next = if month == 12 {
        period(previous_period.yil + 1, 1)
    } else {
        period(previous_period.yil, month + 1)
    };
    let mut settings = request.dataset.institutionSettings[&previous_period.id].clone();
    settings.donemId = next.id.clone();
    request
        .dataset
        .institutionSettings
        .insert(next.id.clone(), settings);
    request
        .dataset
        .attendances
        .push(attendance(&request.personnelId, &next));
    request.periodId = next.id.clone();
    let accrual = request.accrual.as_mut().unwrap();
    accrual.accrualId = format!("{}_{}", request.personnelId, next.id);
    accrual.paymentDate = format!("{}-25", next.id);
    if !request
        .dataset
        .annualPayrollParameters
        .iter()
        .any(|annual| annual.year == next.taxYear)
    {
        let mut annual = AnnualPayrollParameters::default_for_2026();
        annual.year = next.taxYear;
        request.dataset.annualPayrollParameters.push(annual);
    }
    request.dataset.periods.push(next);
}

#[test]
fn all_four_incomes_split_at_first_of_month_for_28_29_30_31_day_periods_and_new_year() {
    for (year, month, before_days, expected_social, expected_clothing) in [
        (2026, 1, 17, dec!(1045.16), dec!(104.52)),
        (2026, 2, 14, dec!(1050), dec!(105)),
        (2024, 2, 15, dec!(1048.28), dec!(104.83)),
        (2026, 4, 16, dec!(1046.67), dec!(104.67)),
        (2025, 12, 17, dec!(1045.16), dec!(104.52)),
    ] {
        let request = request(year, month);
        let payroll = calculate_payroll(&request).unwrap();
        assert_eq!(
            payroll.gelirler.tabanBrutAylik,
            Some(Decimal::from(before_days * 100 + 14 * 110))
        );
        assert_eq!(
            payroll.gelirler.yemek,
            Some(Decimal::from(before_days * 10 + 14 * 11))
        );
        assert_eq!(
            payroll.gelirler.birlestirilmisSosyalYardim,
            Some(expected_social)
        );
        assert_eq!(payroll.gelirler.giyimYardimi, Some(expected_clothing));
        assert_eq!(
            payroll.statutorySnapshot.unwrap().sgkPrimGunSayisi,
            30,
            "SGK normalization is separate from earned calendar days"
        );
    }
}

#[test]
fn union_dues_use_the_new_daily_wage_without_weighting_paid_days() {
    for (year, month) in [(2026, 2), (2024, 2), (2026, 4), (2026, 8), (2025, 12)] {
        let mut request = request(year, month);
        request
            .dataset
            .institutionSettings
            .values_mut()
            .for_each(|settings| {
                settings.sendikaAidatiYuzde = Some(dec!(65));
                settings.sabitSendikaAidati = None;
            });
        let baseline_effective_from = request
            .dataset
            .compensationPolicyVersions
            .iter()
            .min_by(|left, right| left.effectiveFrom.cmp(&right.effectiveFrom))
            .unwrap()
            .effectiveFrom
            .clone();
        update_policy_state(&mut request, &baseline_effective_from, |state| {
            state.gunlukTabanUcret = dec!(2443.28);
        });
        let raise_date = request
            .dataset
            .compensationPolicyVersions
            .iter()
            .max_by(|left, right| left.effectiveFrom.cmp(&right.effectiveFrom))
            .unwrap()
            .effectiveFrom
            .clone();
        update_policy_state(&mut request, &raise_date, |state| {
            state.gunlukTabanUcret = dec!(2616.26);
        });
        request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
            sendikaUyesi: Some(true),
            ..Default::default()
        });
        let payroll = calculate_payroll(&request).unwrap();
        assert_eq!(payroll.kesintiler.sendikaAidati, Some(dec!(1700.57)));
        if month == 8 {
            assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(78163.40)));
        }

        // Even without paid days after the raise, membership dues use the
        // current daily rate rather than an attendance-weighted salary rate.
        let cutoff = format!("{}-01", &request.dataset.periods[1].bitisTarihi[..7]);
        for (date, code) in &mut request.dataset.attendances[0].gunler {
            if date >= &cutoff {
                *code = "R".into();
            }
        }
        assert_eq!(
            calculate_payroll(&request)
                .unwrap()
                .kesintiler
                .sendikaAidati,
            Some(dec!(1700.57))
        );

        request.dataset.personnel[0]
            .kesintiler
            .as_mut()
            .unwrap()
            .sabitSendikaAidati = Some(dec!(123));
        assert_eq!(
            calculate_payroll(&request)
                .unwrap()
                .kesintiler
                .sendikaAidati,
            Some(dec!(123))
        );
        request.dataset.personnel[0]
            .kesintiler
            .as_mut()
            .unwrap()
            .sendikaUyesi = Some(false);
        assert_eq!(
            calculate_payroll(&request)
                .unwrap()
                .kesintiler
                .sendikaAidati,
            Some(dec!(0))
        );
    }
}

#[test]
fn automatic_raised_rates_continue_into_two_copied_following_periods() {
    let mut request = request(2026, 1);
    advance(&mut request);
    let payroll = calculate_payroll_checked(&request).unwrap();
    assert_eq!(payroll.gelirler.yemek, Some(dec!(308))); // 28 × 11
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1100))
    );
    assert_eq!(payroll.gelirler.giyimYardimi, Some(dec!(110)));
    advance(&mut request);
    let payroll = calculate_payroll_checked(&request).unwrap();
    assert_eq!(payroll.gelirler.yemek, Some(dec!(341))); // 31 × 11
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1100))
    );
    assert_eq!(payroll.gelirler.giyimYardimi, Some(dec!(110)));
}

#[test]
fn consecutive_scheduled_raises_compound_the_effective_rates() {
    let mut request = request(2026, 1);
    advance(&mut request);
    let mut state = request
        .dataset
        .compensationPolicyVersions
        .iter()
        .find(|version| version.effectiveFrom == "2026-02-01")
        .unwrap()
        .state
        .clone();
    state.gunlukTabanUcret = dec!(121);
    state.gunlukYemek = dec!(12.1);
    state.birlestirilmisSosyalYardim = dec!(1210);
    state.giyimYardimi = dec!(121);
    set_policy_version(&mut request, "2026-03-01", state);
    let payroll = calculate_payroll_checked(&request).unwrap();
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(3234))); // 14 × 110 + 14 × 121
    assert_eq!(payroll.gelirler.yemek, Some(dec!(323.4))); // 14 × 11 + 14 × 12.1
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1155))
    );
    assert_eq!(payroll.gelirler.giyimYardimi, Some(dec!(115.5)));
}

#[test]
fn explicitly_entered_rates_take_precedence_without_being_raised_twice() {
    let mut request = request(2026, 1);
    let mut state = request
        .dataset
        .compensationPolicyVersions
        .iter()
        .find(|version| version.effectiveFrom == "2026-02-01")
        .unwrap()
        .state
        .clone();
    state.gunlukYemek = dec!(13);
    state.birlestirilmisSosyalYardim = dec!(1200);
    state.giyimYardimi = dec!(115);
    set_policy_version(&mut request, "2026-02-01", state);
    let payroll = calculate_payroll(&request).unwrap();
    assert_eq!(payroll.gelirler.yemek, Some(dec!(352)));
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1090.32))
    );
    assert_eq!(payroll.gelirler.giyimYardimi, Some(dec!(106.77)));
    advance(&mut request);
    let payroll = calculate_payroll_checked(&request).unwrap();
    assert_eq!(payroll.gelirler.yemek, Some(dec!(364)));
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1200))
    );
    assert_eq!(payroll.gelirler.giyimYardimi, Some(dec!(115)));
    let current_period = request
        .dataset
        .periods
        .iter()
        .find(|period| period.id == request.periodId)
        .unwrap()
        .clone();
    update_policy_state(&mut request, &current_period.baslangicTarihi, |state| {
        state.gunlukYemek = dec!(12);
    });
    assert_eq!(
        calculate_payroll(&request).unwrap().gelirler.yemek,
        Some(dec!(336))
    );
}

#[test]
fn only_worked_days_earn_meals_but_paid_sick_days_keep_their_correct_wage_rate() {
    let mut request = request(2026, 1);
    let attendance = &mut request.dataset.attendances[0];
    for code in attendance.gunler.values_mut() {
        *code = "T".into();
    }
    attendance.gunler.insert("2026-01-15".into(), "Ç".into());
    attendance.gunler.insert("2026-02-02".into(), "GÇ".into());
    attendance.gunler.insert("2026-01-31".into(), "R".into());
    attendance.gunler.insert("2026-02-01".into(), "R".into());
    request.dataset.sickLeaveRecords.push(SickLeaveRecord {
        id: "report".into(),
        personnelId: request.personnelId.clone(),
        startDate: "2026-01-31".into(),
        endDate: "2026-02-01".into(),
        createdAt: None,
        updatedAt: None,
    });
    let payroll = calculate_payroll(&request).unwrap();
    assert_eq!(payroll.gelirler.tabanBrutAylik, Some(dec!(3240)));
    assert_eq!(payroll.gelirler.yemek, Some(dec!(21)));
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(1045.16))
    );
}

#[test]
fn missing_previous_period_missing_settings_gaps_and_invalid_raise_months_fail_closed() {
    let original = request(2026, 1);
    let mut missing_baseline = original.clone();
    missing_baseline.dataset.compensationPolicyVersions.clear();
    let error = calculate_payroll(&missing_baseline)
        .expect_err("missing explicit policy baseline must fail closed");
    assert!(matches!(error, DomainError::NotFound(_)));

    assert!(calculate_payroll(&original).is_ok());
}

#[test]
fn october_2027_and_january_2028_single_and_batch_style_calculations_accept_valid_policy_timeline()
{
    // These service periods contain first-of-month earning changes, while the
    // road rounding mode stays constant. Both the single calculation path and
    // the batch controller's per-person engine call must accept that timeline.
    for (year, month, expected_boundary) in [(2027, 10, "2027-11-01"), (2028, 1, "2028-02-01")] {
        let mut source = request(year, month);
        for version in &mut source.dataset.compensationPolicyVersions {
            version.roadRoundingMode = RoadRoundingMode::PeriodTotal;
        }

        let single = calculate_payroll_with_compensation_policy_checked(&source)
            .expect("valid October 2027/January 2028 policy timeline should calculate");
        assert_eq!(single.donemId, source.periodId);
        let snapshot = single
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
            .expect("authoritative compensation snapshot");
        assert_eq!(snapshot.compensation.segments.len(), 2);
        assert_eq!(
            snapshot.compensation.segments[1].startDate,
            expected_boundary
        );
        assert!(snapshot
            .compensation
            .segments
            .iter()
            .all(|segment| { segment.roadRoundingMode == RoadRoundingMode::PeriodTotal }));

        let batch_style = vec![source.clone(), source.clone()]
            .into_iter()
            .map(|request| calculate_payroll_with_compensation_policy_checked(&request))
            .collect::<Result<Vec<_>>>();
        assert_eq!(batch_style.expect("batch per-person calculations").len(), 2);

        let mut conflicting = source;
        conflicting.dataset.compensationPolicyVersions[1].roadRoundingMode =
            RoadRoundingMode::PerCompensationSlice;
        let error = calculate_payroll_with_compensation_policy_checked(&conflicting)
            .expect_err("a real in-period rounding change must still be rejected");
        let detail = error.to_string();
        assert!(detail.contains(expected_boundary), "{detail}");
        assert!(detail.contains("test-policy-"), "{detail}");
    }
}

#[test]
fn isolated_uat_cohort_calculates_twenty_months_for_twenty_five_personnel() {
    let mut seed = request(2027, 2);
    let mut state = seed.dataset.compensationPolicyVersions[0].state.clone();
    state.gunlukTabanUcret = dec!(1000.00);
    state.gunlukVasitaYol = dec!(75.125);
    let mut versions = vec![CompensationPolicyVersion {
        id: "uat-baseline-2027-01-15".into(),
        effectiveFrom: "2027-01-15".into(),
        state: state.clone(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
        createdAt: "2026-10-11T00:00:00Z".into(),
        source: "SYNTHETIC_UAT".into(),
        reason: "Isolated synthetic cohort baseline".into(),
        supersedesVersionId: None,
    }];
    let mut actions = [
        ("2027-02-01", Some(dec!(7.08))),
        ("2027-09-01", Some(dec!(11.35))),
        ("2028-02-01", Some(dec!(6.25))),
        ("2028-09-01", Some(dec!(9.50))),
    ]
    .into_iter()
    .chain(
        [
            "2027-03-15",
            "2027-04-15",
            "2027-05-15",
            "2027-06-15",
            "2027-07-15",
            "2027-10-15",
            "2027-11-15",
            "2028-03-15",
            "2028-05-15",
            "2028-07-15",
        ]
        .map(|date| (date, None)),
    )
    .collect::<Vec<_>>();
    actions.sort_by_key(|(date, _)| *date);
    for (date, salary_percent) in actions {
        if let Some(percent) = salary_percent {
            state.gunlukTabanUcret = (state.gunlukTabanUcret
                * (Decimal::ONE + percent / dec!(100)))
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
            let version_id = format!("uat-salary-{date}");
            versions.push(CompensationPolicyVersion {
                id: version_id.clone(),
                effectiveFrom: date.into(),
                state: state.clone(),
                boundaryScope: CompensationBoundaryScope::FullEarning,
                roadRoundingMode: RoadRoundingMode::PeriodTotal,
                createdAt: "2026-10-11T00:00:00Z".into(),
                source: "SYNTHETIC_UAT".into(),
                reason: format!("Synthetic salary increase {percent}%"),
                supersedesVersionId: None,
            });
            if date == "2027-02-01" {
                state.gunlukTabanUcret += dec!(1.00);
                versions.push(CompensationPolicyVersion {
                    id: "uat-salary-2027-02-01-correction".into(),
                    effectiveFrom: date.into(),
                    state: state.clone(),
                    boundaryScope: CompensationBoundaryScope::FullEarning,
                    roadRoundingMode: RoadRoundingMode::PeriodTotal,
                    createdAt: "2026-10-11T00:00:01Z".into(),
                    source: "SYNTHETIC_UAT".into(),
                    reason: "Synthetic same-date correction".into(),
                    supersedesVersionId: Some(version_id),
                });
            }
        } else {
            state.gunlukVasitaYol += dec!(0.125);
            versions.push(CompensationPolicyVersion {
                id: format!("uat-road-{date}"),
                effectiveFrom: date.into(),
                state: state.clone(),
                boundaryScope: CompensationBoundaryScope::RoadOnly,
                roadRoundingMode: RoadRoundingMode::PeriodTotal,
                createdAt: "2026-10-11T00:00:00Z".into(),
                source: "SYNTHETIC_UAT".into(),
                reason: "Synthetic road-only increase".into(),
                supersedesVersionId: None,
            });
        }
    }
    validate_compensation_policy_timeline(&versions).expect("synthetic timeline is valid");

    let old_person_id = seed.personnelId.clone();
    seed.dataset.compensationPolicyVersions = versions;
    let mut calculated_count = 0;
    for personnel_index in 0..25 {
        let personnel_id = format!("uat-person-{personnel_index:02}");
        let mut request = seed.clone();
        request.personnelId = personnel_id.clone();
        for person in &mut request.dataset.personnel {
            if person.id == old_person_id {
                person.id = personnel_id.clone();
                person.tcNo = format!("100000000{personnel_index:02}");
            }
        }
        for attendance in &mut request.dataset.attendances {
            if attendance.personelId == old_person_id {
                attendance.personelId = personnel_id.clone();
                attendance.id = format!("{personnel_id}_{}", attendance.donemId);
            }
        }
        for record in &mut request.dataset.sickLeaveRecords {
            if record.personnelId == old_person_id {
                record.personnelId = personnel_id.clone();
            }
        }
        for opening in &mut request.dataset.taxOpenings {
            if opening.personnelId == old_person_id {
                opening.personnelId = personnel_id.clone();
                opening.id = format!("{personnel_id}_{}", opening.year);
            }
        }
        for payroll in &mut request.dataset.payrolls {
            if payroll.personelId == old_person_id {
                payroll.personelId = personnel_id.clone();
            }
        }
        if let Some(accrual) = request.accrual.as_mut() {
            accrual.accrualId = format!("{personnel_id}_{}", request.periodId);
        }

        for _ in 0..20 {
            let payroll = calculate_payroll_with_compensation_policy_checked(&request)
                .unwrap_or_else(|error| {
                    panic!("{} {} failed: {error}", personnel_id, request.periodId)
                });
            if request.periodId == "2027-10" || request.periodId == "2028-01" {
                assert_eq!(payroll.personelId, personnel_id);
                assert!(payroll
                    .calculationMetadata
                    .as_ref()
                    .and_then(|metadata| metadata.snapshot.as_ref())
                    .is_some_and(|snapshot| snapshot.compensation.segments.iter().all(
                        |segment| { segment.roadRoundingMode == RoadRoundingMode::PeriodTotal }
                    )));
            }
            request.dataset.payrolls.push(payroll);
            calculated_count += 1;

            let current = request
                .dataset
                .periods
                .iter()
                .find(|period| period.id == request.periodId)
                .expect("current synthetic period")
                .clone();
            let month = current.ay as u32;
            let next = if month == 12 {
                period(current.yil + 1, 1)
            } else {
                period(current.yil, month + 1)
            };
            let mut settings = request.dataset.institutionSettings[&current.id].clone();
            settings.donemId = next.id.clone();
            request
                .dataset
                .institutionSettings
                .insert(next.id.clone(), settings);
            request
                .dataset
                .attendances
                .push(attendance(&personnel_id, &next));
            request.dataset.periods.push(next.clone());
            request.dataset.taxOpenings.clear();
            if !request
                .dataset
                .annualPayrollParameters
                .iter()
                .any(|annual| annual.year == next.taxYear)
            {
                let mut annual = AnnualPayrollParameters::default_for_2026();
                annual.year = next.taxYear;
                request.dataset.annualPayrollParameters.push(annual);
            }
            request.periodId = next.id.clone();
            if let Some(accrual) = request.accrual.as_mut() {
                accrual.accrualId = format!("{personnel_id}_{}", next.id);
                accrual.paymentDate = format!("{}-25", next.id);
            }
        }
    }
    assert_eq!(calculated_count, 25 * 20);
}

#[test]
fn pre_raise_slice_accepts_a_group_first_defined_in_the_current_period() {
    let mut request = request(2026, 8);
    request.dataset.personnel[0].grup = "UX Deneme Grubu".into();
    let group = CompensationPolicyGroup {
        id: "ux-test".into(),
        ad: "UX Deneme Grubu".into(),
        oran: dec!(5),
        aktif: true,
    };
    for version in &mut request.dataset.compensationPolicyVersions {
        version.state.isPrimiGruplari = vec![group.clone()];
    }

    let payroll = calculate_payroll(&request)
        .expect("a newly configured current-period group must cover the pre-raise slice");
    assert_eq!(payroll.isPrimiDetay.unwrap().grupAd, "UX Deneme Grubu");
}

#[test]
fn indexed_social_aid_carries_forward_at_cent_precision() {
    let mut request = request(2026, 1);
    let mut previous_state = request.dataset.compensationPolicyVersions[0].state.clone();
    previous_state.birlestirilmisSosyalYardim = dec!(5089.70);
    let mut current_state = request.dataset.compensationPolicyVersions[1].state.clone();
    current_state.birlestirilmisSosyalYardim = dec!(5089.70);
    current_state.gunlukTabanUcret = dec!(107.08);
    set_policy_version(&mut request, "2025-12-15", previous_state);
    set_policy_version(&mut request, "2026-02-01", current_state);

    advance(&mut request);
    let mut indexed_state = request
        .dataset
        .compensationPolicyVersions
        .iter()
        .find(|version| version.effectiveFrom == "2026-02-01")
        .unwrap()
        .state
        .clone();
    indexed_state.birlestirilmisSosyalYardim = dec!(5450.05);
    set_policy_version(&mut request, "2026-02-01", indexed_state);
    let payroll = calculate_payroll(&request).expect("following period after indexed raise");
    assert_eq!(
        payroll.gelirler.birlestirilmisSosyalYardim,
        Some(dec!(5450.05))
    );
}

fn retro_request(
    request: &PayrollCalculationRequest,
    parameter: RetroParameterKey,
    value: Decimal,
) -> RetroCalculationRequest {
    let mut dataset = request.dataset.clone();
    dataset.payrolls.push(calculate_payroll(request).unwrap());
    let period = dataset
        .periods
        .iter()
        .find(|p| p.id == request.periodId)
        .unwrap();
    RetroCalculationRequest {
        batchId: "batch".into(),
        personnelId: request.personnelId.clone(),
        paymentDate: period.bitisTarihi.clone(),
        calculatedAt: request.calculatedAt.clone(),
        description: None,
        revision: CompensationRevision {
            id: "revision".into(),
            reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
            title: "Raise replay".into(),
            effectiveFrom: period.baslangicTarihi.clone(),
            effectiveTo: Some(period.bitisTarihi.clone()),
            decisionDate: None,
            signedAt: None,
            description: None,
            status: CompensationRevisionStatus::DRAFT,
            scope: CompensationRevisionScope::ALL_PERSONNEL,
            personnelIds: vec![],
            personnelGroup: None,
            createdAt: None,
            updatedAt: None,
        },
        overrides: vec![CompensationRevisionOverride {
            id: "override".into(),
            revisionId: "revision".into(),
            parameter,
            value,
            personnelId: None,
        }],
        dataset,
    }
}

#[test]
fn retro_replay_does_not_create_allowance_differences_from_segment_rounding() {
    let mut request = request(2026, 1);
    // 54.84419... + 49.68194... rounds to 104.53 as a total,
    // but rounding the two segments separately incorrectly yields 104.52.
    let mut old_settings = request.dataset.compensationPolicyVersions[0].state.clone();
    old_settings.giyimYardimi = dec!(100.01);
    let mut current_settings = request.dataset.compensationPolicyVersions[1].state.clone();
    current_settings.giyimYardimi = dec!(110.01);
    let old_period = request
        .dataset
        .periods
        .iter()
        .find(|period| period.id == "2025-12")
        .unwrap()
        .clone();
    set_policy_version(&mut request, &old_period.baslangicTarihi, old_settings);
    set_policy_version(&mut request, "2026-02-01", current_settings);
    let retro = retro_request(&request, RetroParameterKey::GUNLUK_VASITA_YOL, dec!(0));
    let result = RetroEntitlementEngine::calculate(&retro).unwrap();
    assert_eq!(
        calculate_payroll(&request).unwrap().gelirler.giyimYardimi,
        Some(dec!(104.53))
    );
    assert!(
        result.allocations.iter().all(|a| !matches!(
            a.earningCode,
            RetroEarningCode::CLOTHING
                | RetroEarningCode::SOCIAL_AID
                | RetroEarningCode::MEAL
                | RetroEarningCode::SERVICE_INCREMENT
        )),
        "unexpected rounding delta: {:?}",
        result.allocations
    );
}

#[test]
fn retro_absolute_base_wage_revision_does_not_implicitly_reindex_other_income() {
    let request = request(2026, 1);
    let result = RetroEntitlementEngine::calculate(&retro_request(
        &request,
        RetroParameterKey::GUNLUK_TABAN_UCRET,
        dec!(120),
    ))
    .unwrap();
    for code in [
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::MEAL,
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::CLOTHING,
    ] {
        assert!(
            result.allocations.iter().all(|a| a.earningCode != code),
            "unexpected {code:?} delta: {:?}",
            result.allocations
        );
    }
    assert_eq!(
        result
            .allocations
            .iter()
            .find(|a| a.earningCode == RetroEarningCode::BASE_WAGE)
            .unwrap()
            .deltaAmount,
        dec!(480)
    );
}

#[test]
fn retro_replay_after_transition_uses_full_raised_allowances() {
    let mut request = request(2026, 1);
    advance(&mut request);
    let result = RetroEntitlementEngine::calculate(&retro_request(
        &request,
        RetroParameterKey::GUNLUK_VASITA_YOL,
        dec!(1),
    ))
    .unwrap();
    for code in [
        RetroEarningCode::SERVICE_INCREMENT,
        RetroEarningCode::MEAL,
        RetroEarningCode::SOCIAL_AID,
        RetroEarningCode::CLOTHING,
    ] {
        assert!(
            result.allocations.iter().all(|a| a.earningCode != code),
            "unexpected {code:?} delta: {:?}",
            result.allocations
        );
    }
}

#[test]
fn retro_meal_tax_treatment_uses_the_source_period_meal_policy() {
    for selected in [true, false] {
        let mut request = request(2026, 1);
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .yemekTamGvIstisnaDamgaTabi = selected;
        let retro = retro_request(&request, RetroParameterKey::GUNLUK_YEMEK, dec!(350));
        let result = RetroEntitlementEngine::calculate(&retro).unwrap();
        let meal = result
            .allocations
            .iter()
            .find(|allocation| allocation.earningCode == RetroEarningCode::MEAL)
            .unwrap();
        assert_eq!(
            meal.incomeTaxTreatment,
            if selected {
                RetroTaxTreatment::EXEMPT
            } else {
                RetroTaxTreatment::TAXABLE
            }
        );
        assert_eq!(meal.stampTaxTreatment, RetroTaxTreatment::TAXABLE);
        let mut dataset = retro.dataset.clone();
        dataset.retroBatches.push(result.batch.clone());
        dataset.retroAllocations.extend(result.allocations.clone());
        payroll_core::retro::retro_payment_income(&dataset, &result.batch.id).unwrap();
    }
}

#[test]
fn rate_history_changes_protect_cross_year_payrolls_and_retro_ledgers_without_source_payroll() {
    let mut request = request(2025, 12);
    advance(&mut request);
    let later = calculate_payroll_checked(&request).unwrap();
    let mut retro = retro_request(&request, RetroParameterKey::GUNLUK_VASITA_YOL, dec!(1));
    let result = RetroEntitlementEngine::calculate(&retro).unwrap();
    retro.dataset.payrolls.last_mut().unwrap().status = BordroStatus::FINALIZED;
    let mut batch = result.batch;
    batch.status = CompensationRevisionStatus::FINALIZED;
    retro.dataset.retroBatches.push(batch);
    retro.dataset.retroAllocations.extend(result.allocations);
    assert_eq!(later.gelirler.birlestirilmisSosyalYardim, Some(dec!(1100)));
    assert!(retro
        .dataset
        .payrolls
        .iter()
        .all(|p| p.donemId != "2025-11"));
    let impact = evaluate_payroll_invalidation(
        &retro.dataset,
        &PayrollMutation::Period {
            periodId: "2025-11".into(),
        },
    )
    .unwrap();
    // The explicit policy timeline means an older copied period setting does
    // not determine the later compensation snapshot.
    assert!(!impact
        .blockedByFinalized
        .iter()
        .any(|p| p.periodId == "2026-01"));
    assert!(!impact
        .blockedByFinalizedRetroBatches
        .iter()
        .any(|id| id == "batch"));
    let impact = evaluate_payroll_invalidation(
        &retro.dataset,
        &PayrollMutation::PeriodInsert {
            startDate: "2025-11-15".into(),
            taxYear: 2025,
            taxMonth: 11,
        },
    )
    .unwrap();
    assert!(!impact
        .blockedByFinalized
        .iter()
        .any(|p| p.periodId == "2026-01"));
    assert!(!impact
        .blockedByFinalizedRetroBatches
        .iter()
        .any(|id| id == "batch"));
    let impact = evaluate_payroll_invalidation(
        &retro.dataset,
        &PayrollMutation::Period {
            periodId: "2026-01".into(),
        },
    )
    .unwrap();
    assert!(impact
        .blockedByFinalized
        .iter()
        .any(|payroll| payroll.periodId == "2026-01"));
    assert!(impact
        .blockedByFinalizedRetroBatches
        .iter()
        .any(|id| id == "batch"));
    // Mutable records are invalidated by the same dependency rule.
    retro.dataset.payrolls.last_mut().unwrap().status = BordroStatus::CALCULATED;
    retro.dataset.retroBatches.last_mut().unwrap().status = CompensationRevisionStatus::CALCULATED;
    let impact = evaluate_payroll_invalidation(
        &retro.dataset,
        &PayrollMutation::Period {
            periodId: "2026-01".into(),
        },
    )
    .unwrap();
    assert!(impact
        .affectedPayrolls
        .iter()
        .any(|p| p.periodId == "2026-01"));
    assert!(impact.affectedRetroBatches.iter().any(|id| id == "batch"));
}
