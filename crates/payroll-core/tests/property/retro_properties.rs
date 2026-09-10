use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, AccrualType, AnnualPayrollParameters, BordroDonemi, CompensationRevision,
    CompensationRevisionOverride, CompensationRevisionReason, CompensationRevisionScope,
    CompensationRevisionStatus, DonemselKurumDegerleri, PayrollAccrualInput,
    PayrollCalculationRequest, PayrollDatasetSnapshot, PersonelPuantaj, RetroEntitlementEngine,
    RetroParameterKey,
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;

use super::{decimal_from_cents, proptest_config};

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "property-retro-period".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-02-15".into(),
        bitisTarihi: "2026-03-14".into(),
        donemAdi: "Property retro period".into(),
        taxYear: 2026,
        taxMonth: 3,
    }
}

fn personnel() -> payroll_core::Personel {
    serde_json::from_value(json!({
        "id": "property-retro-person",
        "tcNo": "10000000000",
        "ad": "Property",
        "soyad": "Retro",
        "grup": "1. Grup",
        "sgkSicilNo": "",
        "iban": "",
        "hizmetYili": 0
    }))
    .expect("property retro personnel fixture")
}

fn attendance(period: &BordroDonemi) -> PersonelPuantaj {
    let mut days = HashMap::new();
    let mut date = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    while date <= end {
        days.insert(date.to_string(), "Ç".to_owned());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: "property-retro-attendance".into(),
        personelId: "property-retro-person".into(),
        donemId: period.id.clone(),
        gunler: days,
    }
}

fn dataset_with_original_payroll(daily_wage: Decimal) -> PayrollDatasetSnapshot {
    let period = period();
    let mut settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        gunlukTabanUcret: daily_wage,
        gunlukAsgariUcret: Some(Decimal::from(1101)),
        pekTavanKatsayisi: Some(Decimal::from(9)),
        ..Default::default()
    };
    if let Some(groups) = settings.isPrimiGruplari.as_mut() {
        for group in groups {
            group.oran = Decimal::ZERO;
        }
    }

    let mut dataset = PayrollDatasetSnapshot {
        personnel: vec![personnel()],
        periods: vec![period.clone()],
        institutionSettings: HashMap::from([(period.id.clone(), settings)]),
        attendances: vec![attendance(&period)],
        annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
        ..Default::default()
    };
    let original = calculate_payroll(&PayrollCalculationRequest {
        personnelId: "property-retro-person".into(),
        periodId: period.id,
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "property-retro-normal".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-03-10".into(),
            sequence: 0,
            grossAmount: None,
            description: Some("Property retro normal".into()),
        }),
        dataset: dataset.clone(),
    })
    .expect("property retro normal payroll");
    dataset.payrolls.push(original);
    dataset
}

fn revision() -> CompensationRevision {
    CompensationRevision {
        id: "property-retro-revision".into(),
        reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
        title: "Property retro revision".into(),
        effectiveFrom: "2026-02-15".into(),
        effectiveTo: None,
        decisionDate: Some("2026-06-10".into()),
        signedAt: Some("2026-06-10".into()),
        description: Some("Property retro revision".into()),
        status: CompensationRevisionStatus::DRAFT,
        scope: CompensationRevisionScope::SELECTED_PERSONNEL,
        personnelIds: vec!["property-retro-person".into()],
        personnelGroup: None,
        createdAt: Some("2026-06-10T00:00:00Z".into()),
        updatedAt: None,
    }
}

proptest! {
    #![proptest_config(proptest_config(96))]

    #[test]
    fn generated_retro_replay_is_deterministic_and_conservative(
        original_cents in 100_000i64..=300_000i64,
        delta_cents in -50_000i64..=50_000i64,
    ) {
        prop_assume!(delta_cents != 0);
        let target_cents = original_cents + delta_cents;
        prop_assume!(target_cents > 0);

        let original_wage = decimal_from_cents(original_cents);
        let target_wage = decimal_from_cents(target_cents);
        let request = payroll_core::RetroCalculationRequest {
            batchId: "property-retro-batch".into(),
            revision: revision(),
            overrides: vec![CompensationRevisionOverride {
                id: "property-retro-override".into(),
                revisionId: "property-retro-revision".into(),
                parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
                value: target_wage,
                personnelId: None,
            }],
            personnelId: "property-retro-person".into(),
            paymentDate: "2026-06-20".into(),
            calculatedAt: "2026-06-20T00:00:00Z".into(),
            description: Some("Property retro".into()),
            dataset: dataset_with_original_payroll(original_wage),
        };

        let first = RetroEntitlementEngine::calculate(&request)
            .unwrap_or_else(|error| panic!("generated retro request rejected: {error}"));
        let second = RetroEntitlementEngine::calculate(&request)
            .unwrap_or_else(|error| panic!("replayed retro request rejected: {error}"));

        prop_assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap(),
        );
        prop_assert!(!first.allocations.is_empty());
        prop_assert_eq!(
            first.allocations.iter().map(|allocation| allocation.deltaAmount).sum::<Decimal>(),
            first.batch.totalGrossDelta,
        );
        prop_assert_eq!(&first.batch.id, "property-retro-batch");
        prop_assert_eq!(&first.batch.revisionId, "property-retro-revision");
        prop_assert_eq!(&first.batch.personnelId, "property-retro-person");
        for allocation in &first.allocations {
            prop_assert_eq!(&allocation.batchId, &first.batch.id);
            prop_assert_eq!(&allocation.personnelId, "property-retro-person");
            prop_assert_eq!(
                allocation.deltaAmount,
                allocation.targetAmount - allocation.originalRecognizedAmount,
            );
        }
        if target_wage > original_wage {
            prop_assert!(first.batch.totalGrossDelta > Decimal::ZERO);
        } else {
            prop_assert!(first.batch.totalGrossDelta < Decimal::ZERO);
        }
    }
}
