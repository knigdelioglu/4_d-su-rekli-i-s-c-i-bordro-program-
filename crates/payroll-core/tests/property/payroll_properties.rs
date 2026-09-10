use super::{decimal_from_cents, simple_normal_request, SimpleNormalParameters};
use crate::reference::normal_payroll_oracle::{self, NormalPayrollInput};
use payroll_core::{calculate_payroll_checked, BordroStatus};
use proptest::prelude::*;
use rust_decimal::Decimal;

fn simple_normal_parameters() -> impl Strategy<Value = SimpleNormalParameters> {
    (
        150_000i64..=800_000i64,
        0i64..=60_000i64,
        0i64..=60_000i64,
        0i64..=60_000i64,
        90_000i64..=120_000i64,
        100i64..=1_200i64,
        0i64..=2_000i64,
        0i64..=500i64,
        0i64..=3_000i64,
        0i64..=500i64,
        0i64..=1_500i64,
    )
        .prop_map(
            |(
                daily_base,
                daily_meal,
                daily_meal_sgk_capacity,
                daily_meal_gv_capacity,
                daily_minimum,
                pek_multiplier,
                worker_sgk_percent,
                worker_unemployment_percent,
                employer_sgk_percent,
                employer_unemployment_percent,
                stamp_rate_binde,
            )| SimpleNormalParameters {
                daily_base: decimal_from_cents(daily_base),
                daily_meal: decimal_from_cents(daily_meal),
                daily_meal_sgk_capacity: decimal_from_cents(daily_meal_sgk_capacity),
                daily_meal_gv_capacity: decimal_from_cents(daily_meal_gv_capacity),
                daily_minimum: decimal_from_cents(daily_minimum),
                pek_multiplier: decimal_from_cents(pek_multiplier),
                worker_sgk_percent: decimal_from_cents(worker_sgk_percent),
                worker_unemployment_percent: decimal_from_cents(worker_unemployment_percent),
                employer_sgk_percent: decimal_from_cents(employer_sgk_percent),
                employer_unemployment_percent: decimal_from_cents(employer_unemployment_percent),
                stamp_rate_binde: decimal_from_cents(stamp_rate_binde),
            },
        )
}

proptest! {
    #![proptest_config(crate::property::proptest_config(128))]

    #[test]
    fn generated_normal_payroll_matches_independent_oracle(
        parameters in simple_normal_parameters(),
    ) {
        let request = simple_normal_request(parameters);
        let result = calculate_payroll_checked(&request)
            .unwrap_or_else(|error| panic!("generated valid normal request rejected: {error}"));
        let brackets = &request.dataset.annualPayrollParameters[0].gelirVergisiDilimleri;
        let expected = normal_payroll_oracle::calculate(NormalPayrollInput {
            hakedis_days: 31,
            worked_days: 31,
            statutory_days: 30,
            daily_base: parameters.daily_base,
            daily_meal: parameters.daily_meal,
            daily_meal_sgk_capacity: parameters.daily_meal_sgk_capacity,
            daily_meal_gv_capacity: parameters.daily_meal_gv_capacity,
            daily_minimum: parameters.daily_minimum,
            pek_multiplier: parameters.pek_multiplier,
            worker_sgk_percent: parameters.worker_sgk_percent,
            worker_unemployment_percent: parameters.worker_unemployment_percent,
            employer_sgk_percent: parameters.employer_sgk_percent,
            employer_unemployment_percent: parameters.employer_unemployment_percent,
            stamp_rate_binde: parameters.stamp_rate_binde,
            tax_brackets: brackets,
        });

        prop_assert_eq!(result.status, BordroStatus::CALCULATED);
        prop_assert_eq!(result.gelirToplam, expected.income_total);
        prop_assert_eq!(
            result.netOdeme,
            (result.gelirToplam - result.kesintiToplam).round_dp(2),
        );
        prop_assert!(result.kesintiToplam >= Decimal::ZERO);
        prop_assert_eq!(result.gelirler.tabanBrutAylik, Some(expected.base_income));
        prop_assert_eq!(result.gelirler.yemek, Some(expected.meal_income));

        let pek = result.pekDetay.as_ref().expect("normal PEK snapshot");
        prop_assert_eq!(pek.hesaplananPek, expected.pek.ham_pek);
        prop_assert_eq!(pek.hamPek, expected.pek.ham_pek);
        prop_assert_eq!(pek.primMatrahi, expected.pek.prim_matrahi);
        prop_assert_eq!(pek.pekAltSinir, expected.pek.lower_bound);
        prop_assert_eq!(pek.pekUstSinir, expected.pek.upper_bound);
        prop_assert_eq!(pek.altSinirTamamlamaFarki, expected.pek.lower_completion);
        prop_assert_eq!(pek.finalPek, expected.pek.final_pek);
        prop_assert!(pek.finalPek >= Decimal::ZERO);
        prop_assert!(pek.finalPek <= pek.pekUstSinir);
        prop_assert_eq!(pek.devredenPekKullanilan, Decimal::ZERO);
        prop_assert_eq!(pek.devredenPekAşanTutar, Decimal::ZERO);
        prop_assert_eq!(pek.isverenSgkPrimi, Some(expected.contributions.employer_sgk));
        prop_assert_eq!(pek.isverenIssizlikPrimi, Some(expected.contributions.employer_unemployment));
        prop_assert_eq!(
            pek.pekAltSinirTamamlamaIsverenPrimi,
            Some(expected.contributions.employer_lower_completion),
        );
        prop_assert_eq!(pek.isverenPrimToplami, Some(expected.contributions.employer_total));

        let statutory = result
            .statutorySnapshot
            .as_ref()
            .expect("normal statutory snapshot");
        prop_assert_eq!(statutory.sgkPrimGunSayisi, 30);
        prop_assert_eq!(statutory.pekAltSinir, expected.pek.lower_bound);
        prop_assert_eq!(statutory.pekUstSinir, expected.pek.upper_bound);
        prop_assert_eq!(
            statutory.sgkYemekIstisnasiToplam,
            expected.statutory_meal_sgk_capacity,
        );
        prop_assert_eq!(
            statutory.gvYemekIstisnasiToplam,
            expected.statutory_meal_gv_capacity,
        );

        let deductions = &result.kesintiler;
        prop_assert_eq!(deductions.isciSgkPrimi, Some(expected.contributions.worker_sgk));
        prop_assert_eq!(
            deductions.isciIssizlikPrimi,
            Some(expected.contributions.worker_unemployment),
        );
        prop_assert_eq!(deductions.gelirVergisi, Some(expected.gv_exemption.withheld));
        prop_assert_eq!(deductions.damgaVergisi, Some(expected.stamp.withheld));
        prop_assert!(deductions.isciSgkPrimi.unwrap_or(Decimal::ZERO) >= Decimal::ZERO);
        prop_assert!(deductions.isciIssizlikPrimi.unwrap_or(Decimal::ZERO) >= Decimal::ZERO);
        prop_assert!(deductions.gelirVergisi.unwrap_or(Decimal::ZERO) >= Decimal::ZERO);
        prop_assert!(deductions.damgaVergisi.unwrap_or(Decimal::ZERO) >= Decimal::ZERO);
        prop_assert_eq!(
            result.kesintiToplam,
            expected.deduction_total,
        );
        prop_assert_eq!(result.netOdeme, expected.net_payment);

        let gv = result.gvDetay.as_ref().expect("normal GV snapshot");
        prop_assert_eq!(gv.cariGvMatrahi, expected.gv_base);
        prop_assert!(gv.cariGvMatrahi >= Decimal::ZERO);
        prop_assert_eq!(gv.yeniKumulatifGvMatrahi, expected.gv_base);
        prop_assert_eq!(gv.brutGelirVergisi, expected.gross_gv);
        prop_assert!(gv.brutGelirVergisi >= Decimal::ZERO);
        prop_assert_eq!(gv.oncekiKumulatifGvMatrahi, Decimal::ZERO);
        prop_assert_eq!(gv.asgariUcretGvMatrahi, expected.monthly_asgari_gv_base);
        prop_assert_eq!(
            gv.asgariUcretGvIstisnasi,
            expected.gv_exemption.monthly_entitlement,
        );
        prop_assert_eq!(
            gv.ayniAyOncekiKullanilanGvIstisnasi,
            expected.gv_exemption.used_before,
        );
        prop_assert_eq!(
            gv.tahakkukOncesiKalanGvIstisnasi,
            expected.gv_exemption.remaining_before,
        );
        prop_assert_eq!(gv.uygulananGvIstisnasi, expected.gv_exemption.applied_current);
        prop_assert_eq!(
            gv.tahakkukSonrasiKalanGvIstisnasi,
            expected.gv_exemption.remaining_after,
        );
        prop_assert_eq!(gv.kesilenGelirVergisi, expected.gv_exemption.withheld);
        prop_assert!(gv.uygulananGvIstisnasi <= gv.asgariUcretGvIstisnasi);
        prop_assert!(gv.kesilenGelirVergisi >= Decimal::ZERO);

        let stamp = result.damgaDetay.as_ref().expect("normal stamp snapshot");
        prop_assert_eq!(stamp.brutDamgaVergisi, expected.stamp.gross);
        prop_assert_eq!(stamp.aylikDamgaIstisnaHakki, expected.stamp.monthly_entitlement);
        prop_assert_eq!(
            stamp.ayniAyOncekiKullanilanDamgaIstisnasi,
            expected.stamp.used_before,
        );
        prop_assert_eq!(stamp.uygulananDamgaIstisnasi, expected.stamp.applied);
        prop_assert_eq!(stamp.kalanDamgaIstisnasi, expected.stamp.remaining_after);
        prop_assert_eq!(stamp.kesilenDamgaVergisi, expected.stamp.withheld);
        prop_assert!(stamp.kesilenDamgaVergisi >= Decimal::ZERO);
    }
}
