use super::{decimal_from_cents, simple_normal_request};
use crate::reference::pek_oracle::{self, NormalPekInput};
use payroll_core::{
    calculate_prime_esas_kazanc, DevredenPekKaydi, DonemselKurumDegerleri, GelirKalemleri,
    PuantajOzeti,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

proptest! {
    #![proptest_config(crate::property::proptest_config(128))]

    #[test]
    fn no_carry_pek_matches_independent_oracle(
        income_cents in 0i64..=600_000_000i64,
        statutory_days in 0i32..=30i32,
        daily_minimum_cents in 90_000i64..=120_000i64,
        multiplier_cents in 100i64..=1_200i64,
    ) {
        let income = decimal_from_cents(income_cents);
        let daily_minimum = decimal_from_cents(daily_minimum_cents);
        let multiplier = decimal_from_cents(multiplier_cents);
        let settings = DonemselKurumDegerleri {
            donemId: "property-pek".into(),
            gunlukAsgariUcret: Some(daily_minimum),
            pekTavanKatsayisi: Some(multiplier),
            ..Default::default()
        };
        let summary = PuantajOzeti {
            c: statutory_days,
            ..Default::default()
        };
        let income_items = GelirKalemleri {
            tabanBrutAylik: Some(income),
            yemek: Some(Decimal::ZERO),
            ..Default::default()
        };
        let (actual, next_carry) = calculate_prime_esas_kazanc(
            &income_items,
            Some(&summary),
            Some(&settings),
            &[],
        );
        let expected = pek_oracle::normal(NormalPekInput {
            gross_income: income,
            sgk_meal_exemption: Decimal::ZERO,
            statutory_days,
            daily_minimum,
            pek_multiplier: multiplier,
        });

        prop_assert!(next_carry.is_empty());
        prop_assert_eq!(actual.hesaplananPek, expected.ham_pek);
        prop_assert_eq!(actual.hamPek, expected.ham_pek);
        prop_assert_eq!(actual.primMatrahi, expected.prim_matrahi);
        prop_assert_eq!(actual.pekAltSinir, expected.lower_bound);
        prop_assert_eq!(actual.pekUstSinir, expected.upper_bound);
        prop_assert_eq!(actual.altSinirTamamlamaFarki, expected.lower_completion);
        prop_assert_eq!(actual.finalPek, expected.final_pek);
    }

    #[test]
    fn incoming_carry_is_conserved_and_never_exceeds_pek_capacity(
        wage_cents in 0i64..=500_000_000i64,
        non_wage_cents in 0i64..=500_000_000i64,
        carry_cents in 1i64..=500_000_000i64,
        lifetime in 1i32..=3i32,
        multiplier_cents in 100i64..=900i64,
    ) {
        let wage = decimal_from_cents(wage_cents);
        let non_wage = decimal_from_cents(non_wage_cents);
        let carry_amount = decimal_from_cents(carry_cents);
        let settings = DonemselKurumDegerleri {
            donemId: "property-carry".into(),
            gunlukAsgariUcret: Some(Decimal::from(1101)),
            pekTavanKatsayisi: Some(decimal_from_cents(multiplier_cents)),
            ..Default::default()
        };
        let summary = PuantajOzeti { c: 30, ..Default::default() };
        let income = GelirKalemleri {
            tabanBrutAylik: Some(wage),
            isPrimi: Some(non_wage),
            ..Default::default()
        };
        let incoming = [DevredenPekKaydi {
            tutar: carry_amount,
            kalanAySayisi: lifetime,
            kaynakDonemId: Some("carry-source".into()),
        }];
        let (actual, next_carry) = calculate_prime_esas_kazanc(
            &income,
            Some(&summary),
            Some(&settings),
            &incoming,
        );

        let consumed = actual.devredenPekKullanilan;
        let residual_from_input = next_carry
            .iter()
            .filter(|item| item.kaynakDonemId.as_deref() == Some("carry-source"))
            .map(|item| item.tutar)
            .sum::<Decimal>();
        let expired = carry_amount - consumed - residual_from_input;
        prop_assert!(consumed >= Decimal::ZERO);
        prop_assert!(consumed <= carry_amount);
        prop_assert!(residual_from_input >= Decimal::ZERO);
        prop_assert!(expired >= Decimal::ZERO);
        prop_assert_eq!(carry_amount, consumed + residual_from_input + expired);
        prop_assert!(actual.primMatrahi <= actual.pekUstSinir);
        prop_assert_eq!(
            actual.primMatrahi,
            (actual.hamPek + consumed).min(actual.pekUstSinir),
        );
        prop_assert!(
            next_carry
                .iter()
                .all(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
        );
        for item in next_carry.iter().filter(|item| {
            item.kaynakDonemId.as_deref() == Some("carry-source")
        }) {
            prop_assert_eq!(item.kalanAySayisi, lifetime - 1);
        }
        for item in next_carry.iter().filter(|item| item.kaynakDonemId.is_none()) {
            prop_assert_eq!(item.kalanAySayisi, 2);
        }
    }
}

#[test]
fn property_request_builder_keeps_the_full_normal_fixture_valid() {
    let request = simple_normal_request(Default::default());
    payroll_core::calculate_payroll_checked(&request).expect("baseline property request");
}
