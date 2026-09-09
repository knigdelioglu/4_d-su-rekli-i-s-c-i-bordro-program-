#[path = "support/independent_audit.rs"]
mod support;
use chrono::NaiveDate;
use payroll_core::*;
use rust_decimal_macros::dec;

fn with_full_year_periods(
    mut req: PayrollCalculationRequest,
    year: i32,
) -> PayrollCalculationRequest {
    for month in 1..=12 {
        let p_req = support::dated_request(year, month);
        let period = p_req.dataset.periods[0].clone();
        if !req.dataset.periods.iter().any(|p| p.id == period.id) {
            req.dataset.periods.push(period.clone());
        }
        if let Some(settings) = p_req.dataset.institutionSettings.get(&period.id) {
            req.dataset
                .institutionSettings
                .entry(period.id)
                .or_insert_with(|| settings.clone());
        }
    }
    req
}

#[test]
fn audit_multi_accrual_same_month_ordering_and_cumulative_gv() {
    let mut req_normal = support::dated_request(2026, 7);
    req_normal = with_full_year_periods(req_normal, 2026);
    req_normal.dataset.annualPayrollParameters = vec![AnnualPayrollParameters::default_for_2026()];

    // 1. First payment event: TEDIYE on 2026-07-05
    let mut req_tediye = req_normal.clone();
    req_tediye.accrual = Some(PayrollAccrualInput {
        accrualId: "tediye-01".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-07-05".into(),
        sequence: 1,
        grossAmount: Some(dec!(20000)),
        description: Some("Yaz Tediyesi".into()),
    });
    let tediye_payroll = calculate_payroll_checked(&req_tediye).expect("Tediye should calculate");
    assert_eq!(tediye_payroll.status, BordroStatus::CALCULATED);
    let tediye_gv = tediye_payroll.gvDetay.as_ref().unwrap();
    let tediye_exemption = tediye_gv.uygulananGvIstisnasi;
    assert!(tediye_exemption > dec!(0));
    assert!(tediye_exemption <= dec!(4537.75));

    // 2. Second payment event: NORMAL salary on 2026-07-14
    let mut req_normal_2 = req_normal.clone();
    req_normal_2.accrual = Some(PayrollAccrualInput {
        accrualId: "normal-01".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-07-14".into(),
        sequence: 1,
        grossAmount: None,
        description: Some("Normal Maaş".into()),
    });
    // Add tediye to dataset payrolls
    req_normal_2.dataset.payrolls.push(tediye_payroll.clone());

    let normal_payroll = calculate_payroll_checked(&req_normal_2).expect("Normal should calculate");
    let normal_gv = normal_payroll.gvDetay.as_ref().unwrap();

    // Normal's previous cumulative GV must match tediye's new cumulative GV!
    let normal_prev_cum = normal_gv.yeniKumulatifGvMatrahi - normal_gv.cariGvMatrahi;
    assert_eq!(normal_prev_cum, tediye_gv.yeniKumulatifGvMatrahi);

    // Exemption remainder: in July 2026, cumulative minimum wage enters the 20% bracket,
    // so total monthly minimum wage exemption is exactly 4,537.75 TL!
    let normal_exemption = normal_gv.uygulananGvIstisnasi;
    assert_eq!(
        (tediye_exemption + normal_exemption).round_dp(2),
        dec!(4537.75)
    );

    // 3. Third payment event: TIS_IKRAMIYE on 2026-07-20
    let mut req_tis = req_normal.clone();
    req_tis.accrual = Some(PayrollAccrualInput {
        accrualId: "tis-01".into(),
        accrualType: AccrualType::TIS_IKRAMIYE,
        paymentDate: "2026-07-20".into(),
        sequence: 1,
        grossAmount: Some(dec!(15000)),
        description: Some("TİS İkramiyesi".into()),
    });
    req_tis.dataset.payrolls.push(tediye_payroll.clone());
    req_tis.dataset.payrolls.push(normal_payroll.clone());

    let tis_payroll = calculate_payroll_checked(&req_tis).expect("TİS should calculate");
    let tis_gv = tis_payroll.gvDetay.as_ref().unwrap();

    // TİS previous cumulative GV must match normal's new cumulative GV!
    let tis_prev_cum = tis_gv.yeniKumulatifGvMatrahi - tis_gv.cariGvMatrahi;
    assert_eq!(tis_prev_cum, normal_gv.yeniKumulatifGvMatrahi);

    // Exemption for 3rd event must be exactly 0 because monthly quota is exhausted!
    assert_eq!(tis_gv.uygulananGvIstisnasi, dec!(0));
}

#[test]
fn audit_multi_accrual_stale_detection_in_get_period_notices() {
    let mut req = support::dated_request(2026, 7);
    req = with_full_year_periods(req, 2026);
    let person_id = req.dataset.personnel[0].id.clone();
    let period_id = req.periodId.clone();

    // Accrual 1: CALCULATED
    let mut p1 = calculate_payroll_checked(&req).unwrap();
    p1.accrualId = "normal".into();
    p1.status = BordroStatus::CALCULATED;

    // Accrual 2: STALE
    let mut p2 = p1.clone();
    p2.id = "p-extra".into();
    p2.accrualId = "supplemental".into();
    p2.accrualType = AccrualType::SUPPLEMENTAL;
    p2.status = BordroStatus::STALE;

    // Put p1 first and p2 second
    req.dataset.payrolls = vec![p1.clone(), p2.clone()];

    let notices =
        get_period_notices(&req.dataset, &period_id).expect("get_period_notices should succeed");
    let has_stale_notice = notices
        .iter()
        .any(|n| n.code == "STALE_PAYROLL" && n.personnel_id.as_deref() == Some(&person_id));
    assert!(
        has_stale_notice,
        "STALE_PAYROLL notice MUST be generated even if the first accrual is CALCULATED"
    );

    // When p2 becomes CALCULATED, STALE notice must disappear
    req.dataset.payrolls[1].status = BordroStatus::CALCULATED;
    let notices_clean = get_period_notices(&req.dataset, &period_id).unwrap();
    assert!(!notices_clean.iter().any(|n| n.code == "STALE_PAYROLL"));
}

#[test]
fn audit_year_transition_with_pek_carry_and_gv_reset() {
    // 1. December 2026: Large bonus exceeding PEK ceiling
    let mut req_dec = support::dated_request(2026, 12);
    req_dec = with_full_year_periods(req_dec, 2026);
    req_dec.dataset.annualPayrollParameters = vec![
        AnnualPayrollParameters::default_for_2026(),
        AnnualPayrollParameters {
            year: 2027,
            gelirVergisiDilimleri: AnnualPayrollParameters::default_for_2026()
                .gelirVergisiDilimleri,
            ..AnnualPayrollParameters::default_for_2026()
        },
    ];
    let mut dec_settings = req_dec
        .dataset
        .institutionSettings
        .get(&req_dec.periodId)
        .unwrap()
        .clone();
    dec_settings.gunlukTabanUcret = dec!(3000);
    req_dec
        .dataset
        .institutionSettings
        .insert(req_dec.periodId.clone(), dec_settings);

    // Supplementary bonus in December: 350,000 TL non-wage
    let mut req_dec_bonus = req_dec.clone();
    req_dec_bonus.accrual = Some(PayrollAccrualInput {
        accrualId: "dec-bonus".into(),
        accrualType: AccrualType::SUPPLEMENTAL,
        paymentDate: "2026-12-25".into(),
        sequence: 1,
        grossAmount: Some(dec!(350000)),
        description: Some("Yıl Sonu Prim".into()),
    });
    let dec_bonus_payroll =
        calculate_payroll_checked(&req_dec_bonus).expect("Dec bonus should calculate");
    assert_eq!(dec_bonus_payroll.status, BordroStatus::CALCULATED);

    // Verify outgoing carry was produced with kalanAySayisi: 2
    let outgoing = dec_bonus_payroll
        .sonrakiDevredenPek
        .as_ref()
        .expect("Should have outgoing carry");
    assert!(!outgoing.is_empty());
    assert!(outgoing[0].tutar > dec!(0));
    assert_eq!(outgoing[0].kalanAySayisi, 2);
    let carry_amount = outgoing[0].tutar;

    // 2. January 2027: New calendar & tax year
    let mut req_jan = support::dated_request(2027, 1);
    req_jan = with_full_year_periods(req_jan, 2027);
    req_jan = with_full_year_periods(req_jan, 2026);
    req_jan.dataset.annualPayrollParameters = req_dec.dataset.annualPayrollParameters.clone();
    let mut dec_stored = dec_bonus_payroll.clone();
    dec_stored.donemId = req_dec.periodId.clone();
    req_jan.dataset.payrolls.push(dec_stored);

    // Calculate January 2027
    let jan_payroll = calculate_payroll_checked(&req_jan).expect("Jan 2027 should calculate");
    assert_eq!(jan_payroll.status, BordroStatus::CALCULATED);

    // Cumulative GV MUST reset to 0 in January!
    let jan_gv = jan_payroll.gvDetay.as_ref().unwrap();
    let jan_prev_cum = jan_gv.yeniKumulatifGvMatrahi - jan_gv.cariGvMatrahi;
    assert_eq!(
        jan_prev_cum,
        dec!(0),
        "Cumulative GV must reset to 0 at January 1"
    );

    // PEK carry from December MUST carry over into January!
    let incoming_pek = jan_payroll
        .devredenPekGelen
        .as_ref()
        .expect("Jan must have incoming PEK");
    assert!(!incoming_pek.is_empty());
    assert_eq!(incoming_pek[0].tutar, carry_amount);
    // kalanAySayisi ages by 1 month across year boundary (now 1)
    let jan_outgoing = jan_payroll.sonrakiDevredenPek.as_ref();
    if let Some(jan_out) = jan_outgoing {
        if !jan_out.is_empty() {
            assert_eq!(
                jan_out[0].kalanAySayisi, 1,
                "Remaining carry in Jan must have kalanAySayisi: 1"
            );
        }
    }
}

#[test]
fn audit_pek_carry_aging_and_exact_expiration() {
    let mut req_oct = support::dated_request(2026, 10);
    req_oct = with_full_year_periods(req_oct, 2026);
    req_oct = with_full_year_periods(req_oct, 2027);
    req_oct.dataset.annualPayrollParameters = vec![
        AnnualPayrollParameters::default_for_2026(),
        AnnualPayrollParameters {
            year: 2027,
            ..AnnualPayrollParameters::default_for_2026()
        },
    ];
    // High bonus in October exceeding ceiling (PEK ceiling ~297,270 TL, bonus 500,000 TL)
    let mut req_oct_bonus = req_oct.clone();
    req_oct_bonus.accrual = Some(PayrollAccrualInput {
        accrualId: "oct-bonus".into(),
        accrualType: AccrualType::SUPPLEMENTAL,
        paymentDate: "2026-10-20".into(),
        sequence: 1,
        grossAmount: Some(dec!(500000)),
        description: Some("Ekim Primi".into()),
    });
    let oct_payroll = calculate_payroll_checked(&req_oct_bonus).unwrap();
    let oct_out = oct_payroll.sonrakiDevredenPek.as_ref().unwrap();
    assert_eq!(oct_out[0].kalanAySayisi, 2);

    // In November: set high base wage so November's ceiling is saturated and carry must continue to December
    let mut req_nov = support::dated_request(2026, 11);
    req_nov = with_full_year_periods(req_nov, 2026);
    req_nov = with_full_year_periods(req_nov, 2027);
    req_nov.dataset.annualPayrollParameters = req_oct.dataset.annualPayrollParameters.clone();
    let mut nov_settings = req_nov
        .dataset
        .institutionSettings
        .get(&req_nov.periodId)
        .unwrap()
        .clone();
    nov_settings.gunlukTabanUcret = dec!(9800); // 9800 * 30 = 294,000 TL (near ceiling of 297,270)
    req_nov
        .dataset
        .institutionSettings
        .insert(req_nov.periodId.clone(), nov_settings);

    let mut oct_stored = oct_payroll.clone();
    oct_stored.donemId = req_oct.periodId.clone();
    req_nov.dataset.payrolls.push(oct_stored.clone());

    let nov_payroll = calculate_payroll_checked(&req_nov).unwrap();
    let nov_in = nov_payroll.devredenPekGelen.as_ref().unwrap();
    assert_eq!(nov_in[0].kalanAySayisi, 2); // incoming had 2
    let nov_out = nov_payroll.sonrakiDevredenPek.as_ref().unwrap();
    assert!(
        !nov_out.is_empty(),
        "Carry must survive November because ceiling was saturated"
    );
    assert_eq!(
        nov_out[0].kalanAySayisi, 1,
        "After 1 month, outgoing carry has kalanAySayisi 1"
    );

    // December (2 months elapsed, last eligible month):
    let mut req_dec = support::dated_request(2026, 12);
    req_dec = with_full_year_periods(req_dec, 2026);
    req_dec = with_full_year_periods(req_dec, 2027);
    req_dec.dataset.annualPayrollParameters = req_oct.dataset.annualPayrollParameters.clone();
    let mut dec_settings = req_dec
        .dataset
        .institutionSettings
        .get(&req_dec.periodId)
        .unwrap()
        .clone();
    dec_settings.gunlukTabanUcret = dec!(9800);
    req_dec
        .dataset
        .institutionSettings
        .insert(req_dec.periodId.clone(), dec_settings);

    let mut nov_stored = nov_payroll.clone();
    nov_stored.donemId = req_nov.periodId.clone();
    req_dec.dataset.payrolls.push(oct_stored);
    req_dec.dataset.payrolls.push(nov_stored);

    let dec_payroll = calculate_payroll_checked(&req_dec).unwrap();
    let dec_out = dec_payroll.sonrakiDevredenPek.as_ref();
    // After December, 2 months have fully elapsed. Carry from October must NOT carry to January!
    let carried_to_jan =
        dec_out.map_or(0, |list| list.iter().filter(|i| i.tutar > dec!(0)).count());
    assert_eq!(
        carried_to_jan, 0,
        "After 2 months elapsed, PEK carry from October must expire and not carry to January"
    );
}

#[test]
fn audit_sick_leave_exact_quota_and_split_episodes() {
    let mut records = Vec::new();
    let dates = [
        ("2026-02-01", "2026-02-03"), // Ep 1: 3 days (Feb 1, 2 paid; Feb 3 unpaid)
        ("2026-03-05", "2026-03-06"), // Ep 2: 2 days (Mar 5, 6 paid)
        ("2026-04-10", "2026-04-11"), // Ep 3: 2 days (Apr 10, 11 paid)
        ("2026-05-12", "2026-05-13"), // Ep 4: 2 days (May 12, 13 paid)
        ("2026-06-01", "2026-06-02"), // Ep 5: 2 days (Jun 1, 2 paid) -> Quota 5/5 complete!
        ("2026-07-04", "2026-07-05"), // Ep 6: 2 days (Jul 4, 5 UNPAID - quota exceeded!)
    ];
    for (start, end) in dates {
        records.push(SickLeaveRecord {
            id: format!("sick-{start}"),
            personnelId: "audit".into(),
            startDate: start.into(),
            endDate: end.into(),
            createdAt: None,
            updatedAt: None,
        });
    }

    // Period covering Episode 1 (February 2026)
    let period_feb = BordroDonemi {
        id: "p-feb".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Feb 2026".into(),
        taxYear: 2026,
        taxMonth: 2,
    };
    let paid_feb =
        payroll_engine::calculate_paid_sick_dates_from_records(&records, &period_feb).unwrap();
    assert_eq!(paid_feb.len(), 2);
    assert!(paid_feb.contains(&NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()));
    assert!(paid_feb.contains(&NaiveDate::from_ymd_opt(2026, 2, 2).unwrap()));
    assert!(
        !paid_feb.contains(&NaiveDate::from_ymd_opt(2026, 2, 3).unwrap()),
        "3rd day of episode is unpaid by employer"
    );

    // Period covering Episode 5 (June 2026) -> 5th episode, MUST be paid
    let period_jun = BordroDonemi {
        id: "p-jun".into(),
        yil: 2026,
        ay: 6,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Jun 2026".into(),
        taxYear: 2026,
        taxMonth: 6,
    };
    let paid_jun =
        payroll_engine::calculate_paid_sick_dates_from_records(&records, &period_jun).unwrap();
    assert_eq!(paid_jun.len(), 2);
    assert!(paid_jun.contains(&NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()));
    assert!(paid_jun.contains(&NaiveDate::from_ymd_opt(2026, 6, 2).unwrap()));

    // Period covering Episode 6 (July 2026) -> 6th episode, MUST BE ZERO paid days!
    let period_jul = BordroDonemi {
        id: "p-jul".into(),
        yil: 2026,
        ay: 7,
        baslangicTarihi: "2026-06-15".into(),
        bitisTarihi: "2026-07-14".into(),
        donemAdi: "Jul 2026".into(),
        taxYear: 2026,
        taxMonth: 7,
    };
    let paid_jul =
        payroll_engine::calculate_paid_sick_dates_from_records(&records, &period_jul).unwrap();
    assert!(
        paid_jul.is_empty(),
        "6th episode must have 0 employer-paid sick days"
    );
}

#[test]
fn audit_lower_bound_completion_with_earlier_same_month_event() {
    let mut req = support::dated_request(2026, 7);
    req = with_full_year_periods(req, 2026);
    let period_id = req.periodId.clone();
    let mut settings = req
        .dataset
        .institutionSettings
        .get(&period_id)
        .unwrap()
        .clone();
    settings.gunlukTabanUcret = dec!(500); // 500 TL/day
    req.dataset
        .institutionSettings
        .insert(period_id.clone(), settings);

    // Scenario A: Single NORMAL accrual
    // Prim days = 30. Ham PEK = 15,500 TL (15,000 wage + meal subject to sgk). Alt sınır = 33,030 TL.
    // Difference = 33,030 - 15,500 = 17,530 TL paid by employer!
    let normal_a = calculate_payroll_checked(&req).unwrap();
    let pek_a = normal_a.pekDetay.unwrap();
    assert_eq!(pek_a.altSinirTamamlamaFarki, dec!(17530));
    assert_eq!(pek_a.finalPek, dec!(33030));
    assert_eq!(pek_a.primMatrahi, dec!(15500));

    // Scenario B: Earlier TEDIYE payment in same month (e.g. 25,000 TL)
    let mut req_tediye = req.clone();
    req_tediye.accrual = Some(PayrollAccrualInput {
        accrualId: "tediye-early".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-07-05".into(),
        sequence: 1,
        grossAmount: Some(dec!(25000)),
        description: Some("Erken Tediye".into()),
    });
    let tediye_b = calculate_payroll_checked(&req_tediye).unwrap();

    let mut req_normal_b = req.clone();
    req_normal_b.accrual = Some(PayrollAccrualInput {
        accrualId: "normal-b".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-07-14".into(),
        sequence: 1,
        grossAmount: None,
        description: Some("Maaş".into()),
    });
    req_normal_b.dataset.payrolls.push(tediye_b);
    let normal_b = calculate_payroll_checked(&req_normal_b).unwrap();
    let pek_b = normal_b.pekDetay.unwrap();

    // Earlier tediye had 25,000 PEK.
    // Remaining floor for normal = 33,030 - 25,000 = 8,030 TL.
    // Normal ham PEK is 15,500 TL, which exceeds 8,030 TL!
    // Therefore altSinirTamamlamaFarki in normal_b must be 0 TL!
    assert_eq!(
        pek_b.altSinirTamamlamaFarki,
        dec!(0),
        "Earlier payment in same month satisfies employer floor"
    );
}
