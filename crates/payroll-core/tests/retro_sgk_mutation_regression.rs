use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_payroll, AccrualType, AnnualPayrollParameters, BordroDonemi, BordroKaydi,
    CompensationRevision, CompensationRevisionOverride, CompensationRevisionReason,
    CompensationRevisionScope, CompensationRevisionStatus, DonemselKurumDegerleri,
    PayrollAccrualInput, PayrollCalculationRequest, PayrollDatasetSnapshot, Personel,
    PersonelPuantaj, RetroEntitlementEngine, RetroParameterKey,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-02".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-02-15".into(),
        bitisTarihi: "2026-03-14".into(),
        donemAdi: "2026-02".into(),
        taxYear: 2026,
        taxMonth: 3,
    }
}

fn person() -> Personel {
    Personel {
        id: "p1".into(),
        tcNo: "1".into(),
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
    }
}

fn settings(daily_wage: Decimal) -> DonemselKurumDegerleri {
    let mut value = DonemselKurumDegerleri {
        donemId: "2026-02".into(),
        gunlukTabanUcret: daily_wage,
        gunlukYemek: Decimal::ZERO,
        birlestirilmisSosyalYardim: Decimal::ZERO,
        gunlukVasitaYol: Decimal::ZERO,
        giyimYardimi: Decimal::ZERO,
        hizmetZammiBirimi: Decimal::ZERO,
        geceCalismaPrimiYuzde: Some(Decimal::ZERO),
        geceCalismaTatiliPrimiYuzde: Some(Decimal::ZERO),
        ekOdeme: Some(Decimal::ZERO),
        digerGelirVarsayilan: Some(Decimal::ZERO),
        gunlukAsgariUcret: Some(dec!(100)),
        pekTavanKatsayisi: Some(dec!(9)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    };
    for group in value.isPrimiGruplari.as_mut().expect("default groups") {
        group.oran = Decimal::ZERO;
    }
    value
}

fn attendance(period: &BordroDonemi) -> PersonelPuantaj {
    let mut gunler = HashMap::new();
    let mut date = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d").unwrap();
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    PersonelPuantaj {
        id: "attendance-p1-2026-02".into(),
        personelId: "p1".into(),
        donemId: period.id.clone(),
        gunler,
    }
}

fn dataset(daily_wage: Decimal) -> PayrollDatasetSnapshot {
    let source_period = period();
    PayrollDatasetSnapshot {
        personnel: vec![person()],
        periods: vec![source_period.clone()],
        institutionSettings: [(source_period.id.clone(), settings(daily_wage))]
            .into_iter()
            .collect(),
        attendances: vec![attendance(&source_period)],
        annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
        ..PayrollDatasetSnapshot::default()
    }
}

fn normal_payroll(dataset: &PayrollDatasetSnapshot) -> BordroKaydi {
    calculate_payroll(&PayrollCalculationRequest {
        personnelId: "p1".into(),
        periodId: "2026-02".into(),
        calculatedAt: "2026-03-10T00:00:00Z".into(),
        manualIncome: None,
        accrual: Some(PayrollAccrualInput {
            accrualId: "normal-p1-2026-02".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-03-10".into(),
            sequence: 0,
            grossAmount: None,
            description: Some("Normal maaş".into()),
        }),
        dataset: dataset.clone(),
    })
    .expect("normal payroll fixture should calculate")
}

fn retro_result(
    original_daily_wage: Decimal,
    target_daily_wage: Decimal,
    id: &str,
) -> payroll_core::RetroCalculationResult {
    let mut source = dataset(original_daily_wage);
    source.payrolls.push(normal_payroll(&source));
    let revision_id = format!("revision-{id}");
    RetroEntitlementEngine::calculate(&payroll_core::RetroCalculationRequest {
        batchId: format!("batch-{id}"),
        revision: CompensationRevision {
            id: revision_id.clone(),
            reason: CompensationRevisionReason::COLLECTIVE_AGREEMENT,
            title: "SGK mutation regression".into(),
            effectiveFrom: "2026-02-15".into(),
            effectiveTo: None,
            decisionDate: Some("2026-06-10".into()),
            signedAt: Some("2026-06-10".into()),
            description: None,
            status: CompensationRevisionStatus::DRAFT,
            scope: CompensationRevisionScope::SELECTED_PERSONNEL,
            personnelIds: vec!["p1".into()],
            personnelGroup: None,
            createdAt: Some("2026-06-10T00:00:00Z".into()),
            updatedAt: None,
        },
        overrides: vec![CompensationRevisionOverride {
            id: format!("override-{id}"),
            revisionId: revision_id,
            parameter: RetroParameterKey::GUNLUK_TABAN_UCRET,
            value: target_daily_wage,
            personnelId: None,
        }],
        personnelId: "p1".into(),
        paymentDate: "2026-06-20".into(),
        calculatedAt: "2026-06-20T00:00:00Z".into(),
        description: None,
        dataset: source,
    })
    .expect("retro SGK fixture should calculate")
}

#[test]
fn employer_premiums_follow_canonical_pek_when_no_lower_bound_completion_exists() {
    let result = retro_result(dec!(120), dec!(140), "no-lower-bound");
    let allocation = result
        .allocations
        .iter()
        .find(|allocation| allocation.earningCode == payroll_core::RetroEarningCode::BASE_WAGE)
        .expect("base wage allocation");

    assert_eq!(allocation.retroPekDelta, dec!(560));
    assert_eq!(allocation.workerSgkDelta, dec!(78.40));
    assert_eq!(allocation.workerUnemploymentDelta, dec!(5.60));
    assert_eq!(allocation.employerSgkDelta, dec!(121.80));
    assert_eq!(allocation.employerUnemploymentDelta, dec!(11.20));
    assert_eq!(allocation.originalEmployerLowerBound, Decimal::ZERO);
    assert_eq!(allocation.targetEmployerLowerBound, Decimal::ZERO);
    assert_eq!(allocation.employerLowerBoundDelta, Decimal::ZERO);
    assert_eq!(allocation.employerLowerBoundPremiumDelta, Decimal::ZERO);
}

#[test]
fn increasing_worker_pek_reduces_lower_bound_completion_by_exact_worker_rates() {
    let result = retro_result(dec!(20), dec!(25), "lower-bound-down");
    let allocation = &result.allocations[0];

    assert_eq!(allocation.retroPekDelta, dec!(140));
    assert_eq!(allocation.workerSgkDelta, dec!(19.60));
    assert_eq!(allocation.workerUnemploymentDelta, dec!(1.40));
    assert_eq!(allocation.employerSgkDelta, Decimal::ZERO);
    assert_eq!(allocation.employerUnemploymentDelta, Decimal::ZERO);
    assert_eq!(allocation.originalEmployerLowerBound, dec!(2440));
    assert_eq!(allocation.targetEmployerLowerBound, dec!(2300));
    assert_eq!(allocation.employerLowerBoundDelta, dec!(-140));
    assert_eq!(allocation.employerLowerBoundPremiumDelta, dec!(-21.00));
}

#[test]
fn decreasing_worker_pek_increases_lower_bound_completion_by_exact_worker_rates() {
    let result = retro_result(dec!(25), dec!(20), "lower-bound-up");
    let allocation = &result.allocations[0];

    assert_eq!(allocation.retroPekDelta, dec!(-140));
    assert_eq!(allocation.workerSgkDelta, dec!(-19.60));
    assert_eq!(allocation.workerUnemploymentDelta, dec!(-1.40));
    assert_eq!(allocation.employerSgkDelta, Decimal::ZERO);
    assert_eq!(allocation.employerUnemploymentDelta, Decimal::ZERO);
    assert_eq!(allocation.originalEmployerLowerBound, dec!(2300));
    assert_eq!(allocation.targetEmployerLowerBound, dec!(2440));
    assert_eq!(allocation.employerLowerBoundDelta, dec!(140));
    assert_eq!(allocation.employerLowerBoundPremiumDelta, dec!(21.00));
}
