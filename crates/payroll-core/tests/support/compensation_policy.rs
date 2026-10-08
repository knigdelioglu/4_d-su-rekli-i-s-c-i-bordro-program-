// Shared by many integration-test binaries; each test uses a different subset.
#![allow(dead_code)]

use payroll_core::{
    BordroDonemi, CompensationBoundaryScope, CompensationPolicyGroup, CompensationPolicyState,
    CompensationPolicyVersion, DonemselKurumDegerleri, PayrollDatasetSnapshot, RoadRoundingMode,
};

pub fn state_from_settings(settings: &DonemselKurumDegerleri) -> CompensationPolicyState {
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

pub fn baseline_version(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> CompensationPolicyVersion {
    baseline_version_from_state(period, state_from_settings(settings))
}

pub fn baseline_version_from_state(
    period: &BordroDonemi,
    state: CompensationPolicyState,
) -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: format!("test-policy-{}", period.id),
        effectiveFrom: period.baslangicTarihi.clone(),
        state,
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-01-01T00:00:00Z".into(),
        source: "TEST_FIXTURE".into(),
        reason: "Explicit policy baseline for payroll fixture".into(),
        supersedesVersionId: None,
    }
}

pub fn replace_period_baseline(
    dataset: &mut PayrollDatasetSnapshot,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) {
    replace_period_baseline_from_state(dataset, period, state_from_settings(settings));
}

pub fn replace_period_baseline_from_state(
    dataset: &mut PayrollDatasetSnapshot,
    period: &BordroDonemi,
    state: CompensationPolicyState,
) {
    dataset
        .compensationPolicyVersions
        .retain(|version| version.effectiveFrom != period.baslangicTarihi);
    dataset
        .compensationPolicyVersions
        .push(baseline_version_from_state(period, state));
    dataset.compensationPolicyVersions.sort_by(|left, right| {
        left.effectiveFrom
            .cmp(&right.effectiveFrom)
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn seed_zero_tax_openings(dataset: &mut PayrollDatasetSnapshot) {
    for personnel in &dataset.personnel {
        let mut tax_years = dataset
            .periods
            .iter()
            .map(|period| period.taxYear)
            .collect::<Vec<_>>();
        tax_years.sort_unstable();
        tax_years.dedup();
        for year in tax_years {
            if dataset
                .taxOpenings
                .iter()
                .any(|opening| opening.personnelId == personnel.id && opening.year == year)
            {
                continue;
            }
            let has_legacy_opening = [
                (
                    personnel.devirKumulatifGvMatrahi,
                    personnel.devirKumulatifGvMatrahiYili,
                ),
                (
                    personnel.devirKumulatifAsgariGvMatrahi,
                    personnel.devirKumulatifAsgariGvMatrahiYili,
                ),
            ]
            .into_iter()
            .any(|(amount, opening_year)| {
                amount.is_some_and(|value| value > rust_decimal::Decimal::ZERO)
                    && opening_year == Some(year)
            });
            if has_legacy_opening {
                continue;
            }
            let Some(period) = dataset
                .periods
                .iter()
                .filter(|period| period.taxYear == year)
                .min_by(|left, right| {
                    left.taxMonth
                        .cmp(&right.taxMonth)
                        .then_with(|| left.baslangicTarihi.cmp(&right.baslangicTarihi))
                })
            else {
                continue;
            };
            dataset.taxOpenings.push(payroll_core::PersonelTaxOpening {
                id: format!("test-opening-{}-{year}", personnel.id),
                personnelId: personnel.id.clone(),
                year,
                gvCumulativeOpening: Some(rust_decimal::Decimal::ZERO),
                effectiveFromPeriodId: Some(period.id.clone()),
                asgariGvCumulativeOpening: Some(rust_decimal::Decimal::ZERO),
                asgariGvEffectiveFromPeriodId: Some(period.id.clone()),
                createdAt: None,
                updatedAt: None,
            });
        }
    }
}
