use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::BTreeMap;

fn d(v: &Value) -> Decimal {
    v.as_str().unwrap_or("0").parse().unwrap()
}
fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
}
fn pct(base: Decimal, rate: Decimal) -> Decimal {
    money(base * rate / Decimal::from(100))
}
fn progressive(base: Decimal, previous: Decimal, brackets: &[(Decimal, Decimal)]) -> Decimal {
    let tax = |x: Decimal| {
        let mut low = Decimal::ZERO;
        let mut out = Decimal::ZERO;
        for (limit, rate) in brackets {
            let end = x.min(*limit);
            if end > low {
                out += (end - low) * rate;
            }
            if x <= *limit {
                return out;
            }
            low = *limit;
        }
        out + (x - low).max(Decimal::ZERO) * brackets.last().map(|x| x.1).unwrap_or(Decimal::ZERO)
    };
    money(tax(previous + base) - tax(previous))
}

fn date(value: &Value) -> NaiveDate {
    NaiveDate::parse_from_str(value.as_str().unwrap(), "%Y-%m-%d").unwrap()
}

#[test]
fn uat_november_december_rows_reconcile_to_independent_oracle() {
    let root: Value =
        serde_json::from_str(include_str!("fixtures/uat_2027_financial_cohort.json")).unwrap();
    let rows = root["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 40);
    let mut totals = [Decimal::ZERO; 10];
    let mut row_counts = BTreeMap::<String, usize>::new();
    let mut base_wage_totals = BTreeMap::<String, Decimal>::new();
    let mut direct_deduction_totals = BTreeMap::<String, BTreeMap<&str, Decimal>>::new();
    for row in rows {
        let e = &row["earnings"];
        let p = &row["parameters"];
        let t = &row["tax"];
        let o = &row["observed"];
        let personal = &row["deductionPolicy"]["personal"];
        let period = row["period"].as_str().unwrap();
        assert_eq!(
            d(&personal["sabitBesTutar"]),
            d(&row["deductionPolicy"]["personalBesFixed"]),
            "{period} fixed BES input"
        );
        let period_input = &root["periodInputs"][period];
        let service_start = date(&period_input["serviceStart"]);
        let service_end = date(&period_input["serviceEnd"]);
        let payment_date = date(&period_input["paymentDate"]);
        let service_days = service_end.signed_duration_since(service_start).num_days() + 1;
        assert_eq!(row["taxYear"].as_i64(), Some(payment_date.year() as i64));
        assert_eq!(row["taxMonth"].as_i64(), Some(payment_date.month() as i64));
        assert_eq!(row["taxYear"].as_i64(), period_input["taxYear"].as_i64());
        assert_eq!(row["taxMonth"].as_i64(), period_input["taxMonth"].as_i64());

        let daily_base_wage = period_input["wageSlices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|slice| {
                let slice_start = date(&slice["startDate"]);
                let slice_end = date(&slice["endDate"]);
                assert!(slice_start >= service_start && slice_end <= service_end);
                let calendar_days = slice_end.signed_duration_since(slice_start).num_days() + 1;
                let attendance = &slice["attendanceCodeCounts"];
                let all_code_days = attendance
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|count| count.as_i64().unwrap())
                    .sum::<i64>();
                assert_eq!(all_code_days, calendar_days);
                let earning_days = ["Ç", "T", "G", "İ", "GÇ", "GÇT"]
                    .iter()
                    .map(|code| attendance[*code].as_i64().unwrap())
                    .sum::<i64>();
                money(d(&slice["dailyBaseWage"]) * Decimal::from(earning_days))
            })
            .sum::<Decimal>();
        assert_eq!(service_days, 30 + (period == "2027-12") as i64);
        assert_eq!(
            daily_base_wage,
            d(&e["tabanBrutAylik"]),
            "{period} daily base wage"
        );
        *row_counts.entry(period.to_string()).or_default() += 1;
        *base_wage_totals.entry(period.to_string()).or_default() += daily_base_wage;
        for (input, output) in [
            ("icraTutar", "garnishment"),
            ("nafakaTutar", "alimony"),
            ("kisiBorcuTutar", "personDebt"),
            ("dogumAskerlikBorclanmasiTutar", "birthMilitary"),
            ("hayatSaglikSigortasiTutar", "lifeHealth"),
            ("digerKesintiTutar", "other"),
        ] {
            let amount = d(&personal[input]);
            assert_eq!(amount, d(&o[output]), "{period} {output}");
            *direct_deduction_totals
                .entry(period.to_string())
                .or_default()
                .entry(output)
                .or_default() += amount;
        }
        let gross = [
            "tabanBrutAylik",
            "giyimYardimi",
            "hizmetZammi",
            "isPrimi",
            "birlestirilmisSosyalYardim",
            "vasitaYol",
            "yemek",
            "ekOdeme",
            "tediye",
            "tisIkramiyesi",
            "digerGelir",
        ]
        .iter()
        .map(|k| d(&e[*k]))
        .sum::<Decimal>();
        let meal_sgk = d(&p["mealSgk"]) * Decimal::from(row["mealDays"].as_i64().unwrap());
        let meal_gv = d(&p["mealGv"]) * Decimal::from(row["mealDays"].as_i64().unwrap());
        let monthly_min = d(&p["dailyMin"]) * Decimal::from(30);
        let pek = (gross - meal_sgk)
            .max(Decimal::ZERO)
            .max(monthly_min)
            .min(monthly_min * d(&p["cap"]));
        let sgk = pct(pek, d(&p["sgkWorker"]));
        let unemp = pct(pek, d(&p["unempWorker"]));
        let brackets = t["brackets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| (d(&b["limit"]), d(&b["oran"])))
            .collect::<Vec<_>>();
        let union = if row["deductionPolicy"]["sendikaMember"]
            .as_bool()
            .unwrap_or(false)
        {
            let personal = d(&row["deductionPolicy"]["personalUnionFixed"]);
            if personal > Decimal::ZERO {
                personal
            } else {
                d(&row["deductionPolicy"]["unionFixed"])
            }
        } else {
            Decimal::ZERO
        };
        let gv = &row["gvInputs"];
        let insurance_candidate =
            d(&gv["lifePremium"]) / Decimal::from(2) + d(&gv["healthPremium"]);
        let insurance_wage_base = (gross - meal_gv).max(Decimal::ZERO);
        assert_eq!(insurance_wage_base, d(&t["insuranceWageBase"]));
        let insurance_cap =
            money(insurance_wage_base * d(&p["insurancePercentCap"]) / Decimal::from(100));
        let insurance = insurance_candidate
            .min(insurance_cap)
            .min(d(&gv["annualCap"]) - d(&gv["priorInsurance"]))
            .max(Decimal::ZERO);
        let tax_base = gross - sgk - unemp - meal_gv - union - insurance - d(&gv["birthMilitary"]);
        assert_eq!(tax_base, d(&o["gvBase"]), "{period} GV base");
        // independent base is gross less worker premiums, statutory meal exclusion and union dues
        let gross_tax = progressive(tax_base, d(&t["previousCumulative"]), &brackets);
        let min_tax = progressive(
            d(&t["monthlyMinimumBase"]),
            d(&t["previousMinimumCumulative"]),
            &brackets,
        );
        let stamp = money((gross - meal_sgk) * d(&p["stampRatePermille"]) / Decimal::from(1000));
        let stamp_ex = money(monthly_min * d(&p["stampRatePermille"]) / Decimal::from(1000));
        let union = if row["deductionPolicy"]["sendikaMember"]
            .as_bool()
            .unwrap_or(false)
        {
            let personal = d(&row["deductionPolicy"]["personalUnionFixed"]);
            if personal > Decimal::ZERO {
                personal
            } else {
                d(&row["deductionPolicy"]["unionFixed"])
            }
        } else {
            Decimal::ZERO
        };
        let personal = row["deductionPolicy"]["personal"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| k.as_str() != "sabitBesTutar")
            .map(|(_, v)| d(v))
            .sum::<Decimal>();
        let bes = if row["deductionPolicy"]["besMember"]
            .as_bool()
            .unwrap_or(false)
        {
            let fixed = d(&row["deductionPolicy"]["personalBesFixed"]);
            if fixed > Decimal::ZERO {
                fixed
            } else {
                let rate = d(&row["deductionPolicy"]["personalBesPercent"]);
                (pek * (if rate > Decimal::ZERO {
                    rate
                } else {
                    d(&row["deductionPolicy"]["besPercent"])
                })) / Decimal::from(100)
            }
            .trunc()
        } else {
            Decimal::ZERO
        };
        let tax_withheld = money(gross_tax - min_tax).max(Decimal::ZERO);
        let stamp_withheld = (stamp - stamp_ex).max(Decimal::ZERO);
        let total = money(sgk + unemp + tax_withheld + stamp_withheld + union + bes + personal);
        let net = money(gross - total);
        let employer_sgk = pct(pek, d(&p["sgkEmployer"]));
        let employer_unemp = pct(pek, d(&p["unempEmployer"]));
        for (a, k) in [
            (gross, "gross"),
            (pek, "pek"),
            (sgk, "workerSgk"),
            (unemp, "workerUnemp"),
            (gross_tax, "grossTax"),
            (min_tax, "minTaxExemption"),
            (tax_withheld, "taxWithheld"),
            (stamp, "grossStamp"),
            (stamp_withheld, "stampWithheld"),
            (union, "union"),
            (bes, "bes"),
            (total, "totalDeductions"),
            (net, "net"),
            (employer_sgk, "employerSgk"),
            (employer_unemp, "employerUnemp"),
        ] {
            assert_eq!(a, d(&o[k]), "{} {}", row["period"], k);
        }
        totals[0] += gross;
        totals[1] += net;
    }
    assert_eq!(totals[0], Decimal::from_str_exact("4711226.10").unwrap());
    assert_eq!(totals[1], Decimal::from_str_exact("3213897.87").unwrap());
    assert_eq!(row_counts.get("2027-11"), Some(&20));
    assert_eq!(row_counts.get("2027-12"), Some(&20));
    assert_eq!(
        base_wage_totals["2027-11"],
        Decimal::from_str_exact("1871676.20").unwrap()
    );
    assert_eq!(
        base_wage_totals["2027-12"],
        Decimal::from_str_exact("1934065.40").unwrap()
    );
    for period in ["2027-11", "2027-12"] {
        for (category, expected) in [
            ("garnishment", "3150.00"),
            ("alimony", "3200.00"),
            ("personDebt", "350.00"),
            ("birthMilitary", "250.00"),
            ("lifeHealth", "500.00"),
            ("other", "100.00"),
        ] {
            assert_eq!(
                direct_deduction_totals[period][category],
                Decimal::from_str_exact(expected).unwrap(),
                "{period} {category} total"
            );
        }
    }
}
