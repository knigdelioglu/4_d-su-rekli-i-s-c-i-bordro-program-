//! Differential fixtures for the legacy calculator versus the real
//! compensation-policy calculation entry point. This module is test-only.

use super::compensation::{
    calculate_policy_meal_exemptions, policy_date, resolve_compensation_timeline,
};
use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use std::ops::{Deref, DerefMut};

    /// The oracle's raw source remains test-local and outside the active DTO.
    #[derive(Clone)]
    struct LegacyShadowDataset {
        snapshot: PayrollDatasetSnapshot,
        institutionSettings: HashMap<String, LegacyInstitutionSettings>,
        zamAylari: Vec<i32>,
    }

    impl Deref for LegacyShadowDataset {
        type Target = PayrollDatasetSnapshot;

        fn deref(&self) -> &Self::Target {
            &self.snapshot
        }
    }

    impl DerefMut for LegacyShadowDataset {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.snapshot
        }
    }

    #[derive(Clone)]
    struct LegacyShadowRequest {
        personnelId: String,
        periodId: String,
        calculatedAt: String,
        manualIncome: Option<ManualPayrollIncomeInput>,
        accrual: Option<PayrollAccrualInput>,
        dataset: LegacyShadowDataset,
    }

    impl LegacyShadowRequest {
        fn active_request(&self) -> PayrollCalculationRequest {
            let mut dataset = self.dataset.snapshot.clone();
            dataset.institutionSettings = self
                .dataset
                .institutionSettings
                .iter()
                .map(|(period_id, settings)| {
                    (
                        period_id.clone(),
                        PayrollPeriodInstitutionSettings::from(settings),
                    )
                })
                .collect();
            PayrollCalculationRequest {
                personnelId: self.personnelId.clone(),
                periodId: self.periodId.clone(),
                calculatedAt: self.calculatedAt.clone(),
                manualIncome: self.manualIncome.clone(),
                accrual: self.accrual.clone(),
                dataset,
            }
        }

        fn legacy_input(&self) -> LegacyBackupPayrollInput {
            LegacyBackupPayrollInput {
                institutionSettings: self.dataset.institutionSettings.clone(),
                raiseMonths: self.dataset.zamAylari.clone(),
            }
        }
    }

    trait PolicyFixture {
        fn policy_request(&self) -> PayrollCalculationRequest;
    }

    impl PolicyFixture for PayrollCalculationRequest {
        fn policy_request(&self) -> PayrollCalculationRequest {
            self.clone()
        }
    }

    impl PolicyFixture for LegacyShadowRequest {
        fn policy_request(&self) -> PayrollCalculationRequest {
            self.active_request()
        }
    }

    fn calculate_payroll_with_policy(request: &impl PolicyFixture) -> Result<BordroKaydi> {
        super::calculate_payroll_with_compensation_policy(&request.policy_request())
    }

    fn calculate_legacy_payroll(request: &LegacyShadowRequest) -> Result<BordroKaydi> {
        let active_request = request.active_request();
        super::super::calculate_legacy_backup_payroll_checked(
            &active_request,
            &request.legacy_input(),
        )?
        .ok_or_else(|| DomainError::InvalidData("Legacy oracle fixture was not replayable.".into()))
    }

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
            baslangicTarihi: start.format("%Y-%m-%d").to_string(),
            bitisTarihi: NaiveDate::from_ymd_opt(next_year, next_month, 14)
                .unwrap()
                .format("%Y-%m-%d")
                .to_string(),
            donemAdi: format!("{year}-{month:02}"),
            taxYear: year,
            taxMonth: month as i32,
        }
    }

    fn attendance(person_id: &str, period: &BordroDonemi, meal_dates: &[&str]) -> PersonelPuantaj {
        let mut days = HashMap::new();
        let mut date = policy_date(&period.baslangicTarihi).unwrap();
        let end = policy_date(&period.bitisTarihi).unwrap();
        while date <= end {
            let date_text = date.format("%Y-%m-%d").to_string();
            let code = if meal_dates.contains(&date_text.as_str()) {
                "Ç"
            } else {
                "G"
            };
            days.insert(date_text, code.into());
            date += Duration::days(1);
        }
        PersonelPuantaj {
            id: format!("{person_id}_{}", period.id),
            personelId: person_id.into(),
            donemId: period.id.clone(),
            gunler: days,
        }
    }

    fn patterned_attendance(person_id: &str, period: &BordroDonemi) -> PersonelPuantaj {
        let mut attendance = attendance(person_id, period, &[]);
        let mut dates = attendance.gunler.keys().cloned().collect::<Vec<_>>();
        dates.sort();
        let codes = ["Ç", "T", "G", "İ", "GÇ", "GÇT"];
        for (index, date) in dates.into_iter().enumerate() {
            attendance
                .gunler
                .insert(date, codes[index % codes.len()].into());
        }
        attendance
    }

    fn all_c_attendance(person_id: &str, period: &BordroDonemi) -> PersonelPuantaj {
        let mut dates = Vec::new();
        let mut date = policy_date(&period.baslangicTarihi).unwrap();
        let end = policy_date(&period.bitisTarihi).unwrap();
        while date <= end {
            dates.push(date.format("%Y-%m-%d").to_string());
            date += Duration::days(1);
        }
        let refs = dates.iter().map(String::as_str).collect::<Vec<_>>();
        attendance(person_id, period, &refs)
    }

    fn fractional_raise_request() -> LegacyShadowRequest {
        let mut request = legacy_raise_request();
        let period = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == request.periodId)
            .unwrap()
            .clone();
        request.dataset.attendances = vec![patterned_attendance(&request.personnelId, &period)];
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.gunlukTabanUcret = dec!(100.005);
            settings.gunlukYemek = dec!(10.005);
            settings.gunlukVasitaYol = dec!(20.005);
            settings.giyimYardimi = Decimal::ZERO;
            settings.hizmetZammiBirimi = Decimal::ZERO;
            settings.isPrimiGruplari.as_mut().unwrap()[0].oran = dec!(10.005);
            settings.geceCalismaPrimiYuzde = Some(dec!(3.005));
            settings.geceCalismaTatiliPrimiYuzde = Some(dec!(4.005));
            settings.ekOdeme = None;
            settings.digerGelirVarsayilan = None;
        }
        request
    }

    fn legacy_template_request() -> LegacyShadowRequest {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/golden/2026/G020-social-aid.json"))
                .unwrap();
        let request: PayrollCalculationRequest =
            serde_json::from_value(fixture["request"].clone()).unwrap();
        let institutionSettings =
            serde_json::from_value(fixture["request"]["dataset"]["institutionSettings"].clone())
                .unwrap();
        let zamAylari = serde_json::from_value(fixture["request"]["dataset"]["zamAylari"].clone())
            .unwrap_or_default();
        LegacyShadowRequest {
            personnelId: request.personnelId,
            periodId: request.periodId,
            calculatedAt: request.calculatedAt,
            manualIncome: request.manualIncome,
            accrual: request.accrual,
            dataset: LegacyShadowDataset {
                snapshot: request.dataset,
                institutionSettings,
                zamAylari,
            },
        }
    }

    #[test]
    fn current_normal_snapshot_replays_its_full_financial_projection() {
        let request = n_segment_policy_request();
        let calculated = calculate_payroll_with_policy(&request).unwrap();
        let period = request.dataset.periods.first().unwrap();

        let disposition = crate::validate_payroll_calculation_metadata_with_context(
            &calculated,
            period,
            &request.dataset.payrolls,
            &request.dataset.periods,
        )
        .expect("a current-engine calculation must replay from its saved snapshot");

        assert_eq!(
            disposition,
            crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
        );
    }

    #[test]
    fn current_normal_snapshot_rejects_named_financial_projection_forgery() {
        let request = n_segment_policy_request();
        let calculated = calculate_payroll_with_policy(&request).unwrap();
        let period = request.dataset.periods.first().unwrap();

        macro_rules! forged {
            ($field:literal, $edit:expr) => {{
                let mut value = calculated.clone();
                ($edit)(&mut value);
                let error = crate::validate_payroll_calculation_metadata_with_context(
                    &value,
                    period,
                    &request.dataset.payrolls,
                    &request.dataset.periods,
                )
                .expect_err("forged current financial output must fail replay");
                assert!(
                    error.to_string().contains($field),
                    "expected named mismatch {} but received {}",
                    $field,
                    error
                );
            }};
        }

        forged!("gelirler.tabanBrutAylik", |value: &mut BordroKaydi| {
            value.gelirler.tabanBrutAylik = value
                .gelirler
                .tabanBrutAylik
                .map(|amount| amount + Decimal::ONE);
            value.gelirToplam += Decimal::ONE;
        });
        forged!("netOdeme", |value: &mut BordroKaydi| {
            value.netOdeme += Decimal::ONE;
        });
        forged!("kesintiler.isciSgkPrimi", |value: &mut BordroKaydi| {
            value.kesintiler.isciSgkPrimi = value
                .kesintiler
                .isciSgkPrimi
                .map(|amount| amount + Decimal::ONE);
        });
        forged!("pekDetay.primMatrahi", |value: &mut BordroKaydi| {
            value.pekDetay.as_mut().unwrap().primMatrahi += Decimal::ONE;
        });
        forged!("pekDetay.isverenSgkPrimi", |value: &mut BordroKaydi| {
            value.pekDetay.as_mut().unwrap().isverenSgkPrimi = value
                .pekDetay
                .as_ref()
                .unwrap()
                .isverenSgkPrimi
                .map(|amount| amount + Decimal::ONE);
        });
        forged!("gvDetay.brutGelirVergisi", |value: &mut BordroKaydi| {
            value.gvDetay.as_mut().unwrap().brutGelirVergisi += Decimal::ONE;
        });
        forged!("damgaDetay.brutDamgaVergisi", |value: &mut BordroKaydi| {
            value.damgaDetay.as_mut().unwrap().brutDamgaVergisi += Decimal::ONE;
        });
        forged!("isPrimiDetay.tutar", |value: &mut BordroKaydi| {
            value.isPrimiDetay.as_mut().unwrap().tutar += Decimal::ONE;
        });
        forged!("sonrakiDevredenPek", |value: &mut BordroKaydi| {
            value.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
                tutar: Decimal::ONE,
                kalanAySayisi: 1,
                kaynakDonemId: Some(value.donemId.clone()),
            }]);
        });
    }

    fn explicit_zero_tax_opening(request: &mut PayrollCalculationRequest, period_id: &str) {
        let year = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == period_id)
            .map(|period| period.taxYear)
            .unwrap_or(2026);
        request.dataset.taxOpenings = vec![PersonelTaxOpening {
            id: format!("{}-opening", request.personnelId),
            personnelId: request.personnelId.clone(),
            year,
            gvCumulativeOpening: Some(Decimal::ZERO),
            effectiveFromPeriodId: Some(period_id.into()),
            asgariGvCumulativeOpening: Some(Decimal::ZERO),
            asgariGvEffectiveFromPeriodId: Some(period_id.into()),
            createdAt: None,
            updatedAt: None,
        }];
    }

    fn explicit_zero_tax_opening_shadow(request: &mut LegacyShadowRequest, period_id: &str) {
        let mut active_request = request.active_request();
        explicit_zero_tax_opening(&mut active_request, period_id);
        request.dataset.snapshot.taxOpenings = active_request.dataset.taxOpenings;
    }

    fn legacy_raise_request() -> LegacyShadowRequest {
        legacy_raise_request_for(2026, 1)
    }

    fn legacy_raise_request_for(year: i32, month: u32) -> LegacyShadowRequest {
        let mut request = legacy_template_request();
        let current = period(year, month);
        let previous = if month == 1 {
            period(year - 1, 12)
        } else {
            period(year, month - 1)
        };
        let mut current_settings = request
            .dataset
            .institutionSettings
            .values()
            .next()
            .unwrap()
            .clone();
        current_settings.donemId = current.id.clone();
        current_settings.gunlukTabanUcret = dec!(110);
        current_settings.gunlukYemek = dec!(12);
        current_settings.birlestirilmisSosyalYardim = dec!(110);
        current_settings.gunlukVasitaYol = dec!(22);
        current_settings.giyimYardimi = dec!(11);
        current_settings.hizmetZammiBirimi = dec!(2);
        current_settings.isPrimiGruplari.as_mut().unwrap()[0].oran = dec!(20);
        current_settings.geceCalismaPrimiYuzde = Some(dec!(5));
        current_settings.geceCalismaTatiliPrimiYuzde = Some(dec!(7));
        current_settings.ekOdeme = Some(dec!(20));
        current_settings.digerGelirVarsayilan = None;
        current_settings.transportRateChanges = None;

        let mut previous_settings = current_settings.clone();
        previous_settings.donemId = previous.id.clone();
        previous_settings.gunlukTabanUcret = dec!(100);
        previous_settings.gunlukYemek = dec!(10);
        previous_settings.birlestirilmisSosyalYardim = dec!(100);
        previous_settings.gunlukVasitaYol = dec!(20);
        previous_settings.giyimYardimi = dec!(10);
        previous_settings.hizmetZammiBirimi = dec!(1);
        previous_settings.isPrimiGruplari.as_mut().unwrap()[0].oran = dec!(10);
        previous_settings.geceCalismaPrimiYuzde = Some(dec!(3));
        previous_settings.geceCalismaTatiliPrimiYuzde = Some(dec!(4));
        previous_settings.ekOdeme = Some(dec!(10));

        request.periodId = current.id.clone();
        request.calculatedAt = "2026-02-20T12:00:00Z".into();
        let accrual = request.accrual.as_mut().unwrap();
        accrual.accrualId = format!("{}-{}", request.personnelId, current.id);
        accrual.paymentDate = format!("{year}-{month:02}-25");
        request.dataset.periods = vec![previous.clone(), current.clone()];
        request.dataset.attendances = vec![all_c_attendance(&request.personnelId, &current)];
        request.dataset.institutionSettings = HashMap::from([
            (previous.id.clone(), previous_settings),
            (current.id.clone(), current_settings),
        ]);
        request.dataset.zamAylari = vec![if month == 12 { 1 } else { month as i32 + 1 }];
        request.dataset.payrolls.clear();
        request.dataset.compensationPolicyVersions.clear();
        request.dataset.annualPayrollParameters[0].year = year;
        explicit_zero_tax_opening_shadow(&mut request, &current.id);
        request.manualIncome = Some(ManualPayrollIncomeInput {
            tediye: Some(Decimal::ZERO),
            tisIkramiyesi: None,
        });
        request
    }

    fn state_from_legacy_settings(settings: &DonemselKurumDegerleri) -> CompensationPolicyState {
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

    fn policy_version(
        id: &str,
        effective_from: &str,
        state: CompensationPolicyState,
        supersedes: Option<&str>,
        boundary_scope: CompensationBoundaryScope,
        road_rounding_mode: RoadRoundingMode,
    ) -> CompensationPolicyVersion {
        CompensationPolicyVersion {
            id: id.into(),
            effectiveFrom: effective_from.into(),
            state,
            boundaryScope: boundary_scope,
            roadRoundingMode: road_rounding_mode,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TASK4_TEST_FIXTURE".into(),
            reason: "Explicit fixture mapping".into(),
            supersedesVersionId: supersedes.map(str::to_owned),
        }
    }

    /// Test-only fixture adapter: it turns the legacy oracle's effective
    /// compensation values into full-state policy rows with explicit flags.
    /// Dated road amounts stay in those same monetary state rows.
    fn materialize_legacy_raise_policy(request: &mut LegacyShadowRequest) {
        let road_rounding = if request
            .dataset
            .institutionSettings
            .get(&request.periodId)
            .and_then(|settings| settings.transportRateChanges.as_ref())
            .is_some()
        {
            RoadRoundingMode::PeriodTotal
        } else {
            RoadRoundingMode::PerCompensationSlice
        };
        materialize_legacy_raise_policy_with_mode(request, road_rounding).unwrap();
    }

    fn materialize_legacy_raise_policy_with_mode(
        request: &mut LegacyShadowRequest,
        road_rounding: RoadRoundingMode,
    ) -> Result<()> {
        let period = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == request.periodId)
            .ok_or_else(|| DomainError::NotFound("Fixture dönemi bulunamadı.".into()))?
            .clone();
        let transition = crate::raise_transition::resolve_raise_transition(
            &request.dataset.snapshot,
            &request.dataset.institutionSettings,
            &request.dataset.zamAylari,
            &period,
        )?;
        let mut previous = transition
            .previous
            .ok_or_else(|| DomainError::InvalidData("Fixture raise eski ayarları eksik.".into()))?;
        let cutoff = transition
            .cutoff
            .ok_or_else(|| DomainError::InvalidData("Fixture raise kesim tarihi eksik.".into()))?;
        if let Some(current_groups) = transition.current.isPrimiGruplari.as_ref() {
            let personnel_group = &request.dataset.personnel[0].grup;
            let previous_groups = previous.isPrimiGruplari.get_or_insert_with(Vec::new);
            for group in current_groups {
                if group.ad == *personnel_group || group.id == *personnel_group {
                    if !previous_groups.iter().any(|existing| {
                        existing.ad == *personnel_group || existing.id == *personnel_group
                    }) {
                        previous_groups.push(group.clone());
                    }
                    break;
                }
            }
        }

        let period_start = policy_date(&period.baslangicTarihi).unwrap();
        let period_end = policy_date(&period.bitisTarihi).unwrap();
        let mut boundaries = std::collections::BTreeSet::from([period_start]);
        if cutoff >= period_start && cutoff <= period_end {
            boundaries.insert(cutoff);
        }

        let current_settings = request
            .dataset
            .institutionSettings
            .get(&request.periodId)
            .ok_or_else(|| DomainError::InvalidData("Fixture cari dönem ayarları eksik.".into()))?;
        let road_changes = current_settings
            .transportRateChanges
            .as_deref()
            .unwrap_or_default();
        for change in road_changes {
            let effective_date = policy_date(&change.effectiveFrom)?;
            if effective_date < period_start || effective_date > period_end {
                return Err(DomainError::ValidationError(
                    "Fixture dated road boundary dönem dışında.".into(),
                ));
            }
            boundaries.insert(effective_date);
        }
        let period_road_baseline = current_settings.gunlukVasitaYol;

        let mut versions = Vec::with_capacity(boundaries.len());
        for (index, effective_date) in boundaries.into_iter().enumerate() {
            let legacy_state = if effective_date >= cutoff {
                &transition.current
            } else {
                &previous
            };
            let mut state = state_from_legacy_settings(legacy_state);
            if current_settings.transportRateChanges.is_some() {
                let effective_from = effective_date.format("%Y-%m-%d").to_string();
                state.gunlukVasitaYol = road_changes
                    .iter()
                    .rev()
                    .find(|change| change.effectiveFrom.as_str() <= effective_from.as_str())
                    .map_or(period_road_baseline, |change| change.gunlukVasitaYol);
            }
            let id = format!("fixture-{index}");
            let boundary_scope = if effective_date == period_start || effective_date == cutoff {
                CompensationBoundaryScope::FullEarning
            } else {
                CompensationBoundaryScope::RoadOnly
            };
            versions.push(policy_version(
                &id,
                &effective_date.format("%Y-%m-%d").to_string(),
                state,
                None,
                boundary_scope,
                road_rounding,
            ));
        }
        request.dataset.compensationPolicyVersions = versions;
        Ok(())
    }

    fn assert_full_record_equal(legacy: &BordroKaydi, shadow: &BordroKaydi) {
        assert!(legacy.calculationMetadata.is_none());
        if shadow.accrualType == AccrualType::NORMAL {
            let metadata = shadow
                .calculationMetadata
                .as_ref()
                .expect("the real compensation entry point must persist calculation metadata");
            assert!(metadata.snapshot.is_some());
        } else {
            assert!(shadow.calculationMetadata.is_none());
        }
        let mut legacy_value = serde_json::to_value(legacy).unwrap();
        let mut policy_value = serde_json::to_value(shadow).unwrap();
        legacy_value
            .as_object_mut()
            .unwrap()
            .remove("calculationMetadata");
        policy_value
            .as_object_mut()
            .unwrap()
            .remove("calculationMetadata");
        assert_eq!(
            legacy_value,
            policy_value,
            "all financial fields in the serialized payroll record must match when timestamps are fixed"
        );
    }

    fn n_segment_policy_request() -> PayrollCalculationRequest {
        let mut legacy_request = legacy_template_request();
        let period = legacy_request.dataset.periods[0].clone();
        let mut settings = legacy_request
            .dataset
            .institutionSettings
            .values()
            .next()
            .unwrap()
            .clone();
        settings.donemId = period.id.clone();
        settings.gunlukTabanUcret = dec!(102);
        settings.gunlukYemek = dec!(10.005);
        settings.birlestirilmisSosyalYardim = Decimal::ZERO;
        settings.gunlukVasitaYol = Decimal::ZERO;
        settings.giyimYardimi = Decimal::ZERO;
        settings.hizmetZammiBirimi = Decimal::ZERO;
        settings.isPrimiGruplari.as_mut().unwrap()[0].oran = Decimal::ZERO;
        settings.geceCalismaPrimiYuzde = Some(Decimal::ZERO);
        settings.geceCalismaTatiliPrimiYuzde = Some(Decimal::ZERO);
        settings.ekOdeme = Some(Decimal::ZERO);
        settings.digerGelirVarsayilan = None;
        settings.yemekTamGvIstisnaDamgaTabi = false;
        settings.gunlukAsgariUcret = Some(dec!(100));
        settings.pekTavanKatsayisi = Some(dec!(9));
        settings.gunlukYemekIstisnasiSGK = Some(dec!(1000));
        settings.gunlukYemekIstisnasiGV = Some(dec!(1000));
        settings.statutoryParameterSnapshot = None;
        settings.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
            effectiveFrom: "2026-01-16".into(),
            gunlukAsgariUcret: Some(dec!(100)),
            pekTavanKatsayisi: Some(dec!(9)),
            gunlukYemekIstisnasiSGK: Some(dec!(1001)),
            gunlukYemekIstisnasiGV: Some(dec!(1001)),
        }]);
        legacy_request.dataset.institutionSettings =
            HashMap::from([(period.id.clone(), settings.clone())]);
        let mut request = legacy_request.active_request();
        request.dataset.periods = vec![period.clone()];
        request.dataset.attendances = vec![attendance(
            &request.personnelId,
            &period,
            &["2026-01-15", "2026-01-16"],
        )];
        request.dataset.personnel[0].hizmetYili = 3;
        request.dataset.payrolls.clear();
        let mut baseline = state_from_legacy_settings(&settings);
        baseline.gunlukTabanUcret = dec!(100);
        baseline.birlestirilmisSosyalYardim = dec!(100);
        baseline.giyimYardimi = dec!(10);
        baseline.hizmetZammiBirimi = dec!(1);
        let mut second = baseline.clone();
        second.gunlukTabanUcret = dec!(101);
        second.birlestirilmisSosyalYardim = dec!(110);
        second.giyimYardimi = dec!(20);
        second.hizmetZammiBirimi = dec!(1.5);
        let mut third = baseline.clone();
        third.gunlukTabanUcret = dec!(102);
        third.birlestirilmisSosyalYardim = dec!(150);
        third.giyimYardimi = dec!(30);
        third.hizmetZammiBirimi = dec!(2);
        request.dataset.compensationPolicyVersions = vec![
            policy_version(
                "n-1",
                "2026-01-15",
                second.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "n-2",
                "2026-01-16",
                second,
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "n-3",
                "2026-02-01",
                third,
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
        ];
        request.dataset.compensationPolicyVersions[0].state = baseline;
        request.dataset.annualPayrollParameters[0].year = 2026;
        request.calculatedAt = "2026-02-20T12:00:00Z".into();
        explicit_zero_tax_opening(&mut request, &period.id);
        request
    }

    #[test]
    fn one_raise_uses_explicit_policy_and_matches_the_full_legacy_record() {
        let mut request = legacy_raise_request();
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.sendikaAidatiYuzde = Some(dec!(65));
            settings.sabitSendikaAidati = None;
        }
        let deductions = request.dataset.personnel[0]
            .kesintiler
            .get_or_insert_with(PersonelKesintileri::default);
        deductions.sendikaUyesi = Some(true);
        deductions.sabitSendikaAidati = None;
        materialize_legacy_raise_policy(&mut request);

        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.tediye, Some(Decimal::ZERO));
        assert_eq!(legacy.gelirler.tisIkramiyesi, None);
        assert_eq!(legacy.gelirler.digerGelir, None);
        assert_eq!(legacy.gelirler.ekOdeme, Some(dec!(20)));
        assert_eq!(legacy.kesintiler.sendikaAidati, Some(dec!(71.50)));
    }

    #[test]
    fn union_and_bes_personal_fixed_institution_fixed_default_and_membership_precedence_match() {
        for (member, personal_fixed, institution_fixed, expected) in [
            (true, Some(dec!(123)), Some(dec!(456)), dec!(123)),
            (true, Some(Decimal::ZERO), Some(dec!(456)), dec!(456)),
            (true, None, None, dec!(71.50)),
            (false, Some(dec!(123)), Some(dec!(456)), Decimal::ZERO),
        ] {
            let mut request = legacy_raise_request();
            for settings in request.dataset.institutionSettings.values_mut() {
                settings.sendikaAidatiYuzde = Some(dec!(65));
                settings.sabitSendikaAidati = institution_fixed;
            }
            request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
                sendikaUyesi: Some(member),
                sabitSendikaAidati: personal_fixed,
                ..Default::default()
            });
            materialize_legacy_raise_policy(&mut request);
            let legacy = calculate_legacy_payroll(&request).unwrap();
            let shadow = calculate_payroll_with_policy(&request).unwrap();
            assert_full_record_equal(&legacy, &shadow);
            assert_eq!(legacy.kesintiler.sendikaAidati, Some(expected));
            if member && personal_fixed.is_none() && institution_fixed.is_none() {
                let cutoff = request
                    .dataset
                    .periods
                    .iter()
                    .find(|period| period.id == request.periodId)
                    .unwrap()
                    .bitisTarihi[..7]
                    .to_owned();
                let cutoff = format!("{cutoff}-01");
                let mut no_paid_days_after_cutoff = request.clone();
                for (date, code) in &mut no_paid_days_after_cutoff.dataset.attendances[0].gunler {
                    if date >= &cutoff {
                        *code = "R".into();
                    }
                }
                let legacy = calculate_legacy_payroll(&no_paid_days_after_cutoff).unwrap();
                let shadow = calculate_payroll_with_policy(&no_paid_days_after_cutoff).unwrap();
                assert_full_record_equal(&legacy, &shadow);
                assert_eq!(legacy.kesintiler.sendikaAidati, Some(dec!(71.50)));
            }
        }

        for (member, personal_fixed, institution_fixed, personal_rate, expected_fixed) in [
            (true, Some(dec!(80)), Some(dec!(100)), None, Some(dec!(80))),
            (
                true,
                Some(Decimal::ZERO),
                Some(dec!(100)),
                None,
                Some(dec!(100)),
            ),
            (true, None, None, Some(dec!(5)), None),
            (
                false,
                Some(dec!(80)),
                Some(dec!(100)),
                Some(dec!(5)),
                Some(Decimal::ZERO),
            ),
        ] {
            let mut request = legacy_raise_request();
            for settings in request.dataset.institutionSettings.values_mut() {
                settings.besOraniYuzde = Some(dec!(3));
                settings.sabitBesTutar = institution_fixed;
            }
            request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
                besUyesi: Some(member),
                sabitBesTutar: personal_fixed,
                oksOraniYuzde: personal_rate,
                ..Default::default()
            });
            materialize_legacy_raise_policy(&mut request);
            let legacy = calculate_legacy_payroll(&request).unwrap();
            let shadow = calculate_payroll_with_policy(&request).unwrap();
            assert_full_record_equal(&legacy, &shadow);
            if let Some(expected) = expected_fixed {
                assert_eq!(legacy.kesintiler.bes, Some(expected));
            } else {
                let pek = legacy.pekDetay.as_ref().unwrap().primMatrahi;
                assert_eq!(legacy.kesintiler.bes, Some((pek * dec!(0.05)).floor()));
            }
        }
    }

    #[test]
    fn manual_income_null_zero_and_decimal_rounding_match_full_records() {
        let cases = [
            (None, None, None),
            (Some(ManualPayrollIncomeInput::default()), None, None),
            (
                Some(ManualPayrollIncomeInput {
                    tediye: Some(Decimal::ZERO),
                    tisIkramiyesi: None,
                }),
                Some(Decimal::ZERO),
                None,
            ),
            (
                Some(ManualPayrollIncomeInput {
                    tediye: Some(dec!(12.345)),
                    tisIkramiyesi: Some(Decimal::ZERO),
                }),
                Some(dec!(12.35)),
                Some(Decimal::ZERO),
            ),
        ];
        for (manual, expected_tediye, expected_tis) in cases {
            let mut request = legacy_raise_request();
            request.manualIncome = manual;
            materialize_legacy_raise_policy(&mut request);
            let legacy = calculate_legacy_payroll(&request).unwrap();
            let shadow = calculate_payroll_with_policy(&request).unwrap();
            assert_full_record_equal(&legacy, &shadow);
            assert_eq!(legacy.gelirler.tediye, expected_tediye);
            assert_eq!(legacy.gelirler.tisIkramiyesi, expected_tis);
        }
    }

    #[test]
    fn premium_rounds_each_compensation_slice_and_merges_the_last_earning_group_detail() {
        let mut request = fractional_raise_request();
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.isPrimiYuzde = Some(dec!(99));
        }
        let previous_period_id = request.dataset.periods[0].id.clone();
        request
            .dataset
            .institutionSettings
            .get_mut(&previous_period_id)
            .unwrap()
            .isPrimiGruplari
            .as_mut()
            .unwrap()[0]
            .oran = dec!(10.005);
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .isPrimiGruplari
            .as_mut()
            .unwrap()[0]
            .oran = dec!(12.005);
        materialize_legacy_raise_policy(&mut request);
        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.isPrimi, Some(dec!(120.06)));
        let detail = legacy.isPrimiDetay.as_ref().unwrap();
        assert_eq!(detail.hakGunu, 11);
        assert_eq!(detail.oran, dec!(12.005));
        assert_eq!(detail.tutar, dec!(120.06));
    }

    #[test]
    fn automatic_raise_indexing_and_explicit_overrides_become_full_policy_states() {
        let mut automatic = legacy_raise_request();
        for settings in automatic.dataset.institutionSettings.values_mut() {
            settings.gunlukYemek = dec!(10);
            settings.birlestirilmisSosyalYardim = dec!(100);
            settings.giyimYardimi = dec!(10);
            settings.hizmetZammiBirimi = dec!(1);
        }
        materialize_legacy_raise_policy(&mut automatic);
        assert_eq!(automatic.dataset.compensationPolicyVersions.len(), 2);
        let indexed = &automatic.dataset.compensationPolicyVersions[1].state;
        assert_eq!(indexed.gunlukYemek, dec!(11));
        assert_eq!(indexed.birlestirilmisSosyalYardim, dec!(110));
        assert_eq!(indexed.giyimYardimi, dec!(11));
        assert_eq!(indexed.hizmetZammiBirimi, dec!(1.1));
        let legacy = calculate_legacy_payroll(&automatic).unwrap();
        let shadow = calculate_payroll_with_policy(&automatic).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.yemek, Some(dec!(324)));

        let mut explicit = legacy_raise_request();
        let current = explicit
            .dataset
            .institutionSettings
            .get_mut(&explicit.periodId)
            .unwrap();
        current.gunlukYemek = dec!(13);
        current.birlestirilmisSosyalYardim = dec!(130);
        current.giyimYardimi = dec!(15);
        current.hizmetZammiBirimi = dec!(3);
        materialize_legacy_raise_policy(&mut explicit);
        let overridden = &explicit.dataset.compensationPolicyVersions[1].state;
        assert_eq!(overridden.gunlukYemek, dec!(13));
        assert_eq!(overridden.birlestirilmisSosyalYardim, dec!(130));
        assert_eq!(overridden.giyimYardimi, dec!(15));
        assert_eq!(overridden.hizmetZammiBirimi, dec!(3));
        let legacy = calculate_legacy_payroll(&explicit).unwrap();
        let shadow = calculate_payroll_with_policy(&explicit).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.yemek, Some(dec!(352)));
    }

    #[test]
    fn consecutive_automatic_raises_compound_before_explicit_policy_calculation() {
        let mut request = legacy_raise_request();
        let current_id = request.periodId.clone();
        let december_id = request.dataset.periods[0].id.clone();
        let november = period(2025, 11);
        let mut november_settings = request
            .dataset
            .institutionSettings
            .get(&december_id)
            .unwrap()
            .clone();
        november_settings.donemId = november.id.clone();
        november_settings.gunlukTabanUcret = dec!(100);
        let december_settings = request
            .dataset
            .institutionSettings
            .get_mut(&december_id)
            .unwrap();
        december_settings.gunlukTabanUcret = dec!(110);
        let january_settings = request
            .dataset
            .institutionSettings
            .get_mut(&current_id)
            .unwrap();
        january_settings.gunlukTabanUcret = dec!(121);
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.gunlukYemek = dec!(10);
            settings.birlestirilmisSosyalYardim = dec!(100);
            settings.giyimYardimi = dec!(10);
            settings.hizmetZammiBirimi = dec!(1);
        }
        request
            .dataset
            .institutionSettings
            .insert(november.id.clone(), november_settings);
        request.dataset.periods.insert(0, november);
        request.dataset.zamAylari = vec![1, 2];
        materialize_legacy_raise_policy(&mut request);
        let before = &request.dataset.compensationPolicyVersions[0].state;
        let after = &request.dataset.compensationPolicyVersions[1].state;
        assert_eq!(before.gunlukYemek, dec!(11));
        assert_eq!(after.gunlukYemek, dec!(12.1));
        assert_eq!(before.birlestirilmisSosyalYardim, dec!(110));
        assert_eq!(after.birlestirilmisSosyalYardim, dec!(121));
        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.yemek, Some(dec!(356.4)));
    }

    #[test]
    fn cent_precision_auto_indexing_carries_forward_as_an_explicit_policy_state() {
        let mut request = legacy_raise_request();
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.birlestirilmisSosyalYardim = dec!(5089.70);
        }
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .gunlukTabanUcret = dec!(107.08);
        materialize_legacy_raise_policy(&mut request);
        assert_eq!(
            request.dataset.compensationPolicyVersions[1]
                .state
                .birlestirilmisSosyalYardim,
            dec!(5450.05076)
        );
        let legacy_first = calculate_legacy_payroll(&request).unwrap();
        let shadow_first = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy_first, &shadow_first);

        let next = period(2026, 2);
        let mut legacy_request = request.clone();
        let mut shadow_request = request;
        for (candidate, previous_payroll) in [
            (&mut legacy_request, legacy_first),
            (&mut shadow_request, shadow_first),
        ] {
            candidate.dataset.payrolls.push(previous_payroll);
            let mut settings = candidate
                .dataset
                .institutionSettings
                .get(&candidate.periodId)
                .unwrap()
                .clone();
            settings.donemId = next.id.clone();
            candidate
                .dataset
                .institutionSettings
                .insert(next.id.clone(), settings);
            candidate
                .dataset
                .attendances
                .push(all_c_attendance(&candidate.personnelId, &next));
            candidate.dataset.periods.push(next.clone());
            candidate.periodId = next.id.clone();
            candidate.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-{}", candidate.personnelId, next.id),
                accrualType: AccrualType::NORMAL,
                paymentDate: "2026-02-25".into(),
                sequence: 0,
                grossAmount: None,
                description: Some("Indexed policy carry-forward".into()),
            });
        }
        let legacy = calculate_legacy_payroll(&legacy_request).unwrap();
        let shadow = calculate_payroll_with_policy(&shadow_request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(
            legacy.gelirler.birlestirilmisSosyalYardim,
            Some(dec!(5450.05))
        );
    }

    #[test]
    fn one_daily_meal_override_does_not_coalesce_other_fractional_earnings() {
        let mut request = fractional_raise_request();
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .gunlukYemek = dec!(11.005);
        materialize_legacy_raise_policy(&mut request);
        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.gelirler.tabanBrutAylik, Some(dec!(3100.16)));
        assert_eq!(legacy.gelirler.yemek, Some(dec!(115.06)));
        assert_eq!(legacy.gelirler.vasitaYol, Some(dec!(220.06)));
        assert_eq!(legacy.gelirler.isPrimi, Some(dec!(110.06)));
        assert_eq!(legacy.gelirler.geceCalismasiUcreti, Some(dec!(15.03)));
        assert_eq!(legacy.gelirler.geceCalismasiTatiliUcreti, Some(dec!(20.03)));
    }

    #[test]
    fn n_segment_golden_intersects_statutory_dates_and_preserves_b_rnd_01() {
        let request = n_segment_policy_request();
        assert_eq!(request.dataset.periods.len(), 1);
        assert_eq!(request.dataset.institutionSettings.len(), 1);
        assert!(request.dataset.payrolls.is_empty());
        assert_eq!(request.dataset.compensationPolicyVersions.len(), 3);

        let result = calculate_payroll_with_policy(&request).unwrap();
        assert_eq!(result.gelirler.tabanBrutAylik, Some(dec!(3144)));
        assert_eq!(result.gelirler.tediye, None);
        assert_eq!(result.gelirler.tisIkramiyesi, None);
        assert_eq!(result.gelirler.ekOdeme, Some(Decimal::ZERO));
        assert_eq!(result.gelirler.digerGelir, None);
        assert_eq!(result.gelirler.yemek, Some(dec!(20.02)));
        assert_eq!(
            result.gelirler.birlestirilmisSosyalYardim,
            Some(dec!(127.74))
        );
        assert_eq!(result.gelirler.vasitaYol, Some(Decimal::ZERO));
        assert_eq!(result.gelirler.giyimYardimi, Some(dec!(24.19)));
        assert_eq!(result.gelirler.isPrimi, Some(Decimal::ZERO));
        assert_eq!(result.gelirler.geceCalismasiUcreti, Some(Decimal::ZERO));
        assert_eq!(
            result.gelirler.geceCalismasiTatiliUcreti,
            Some(Decimal::ZERO)
        );
        assert_eq!(result.gelirler.hizmetZammi, Some(dec!(5.13)));
        assert_eq!(result.gelirToplam, dec!(3321.08));
        assert_eq!(result.pekDetay.as_ref().unwrap().hamPek, dec!(3301.06));
        assert_eq!(result.pekDetay.as_ref().unwrap().primMatrahi, dec!(3301.06));
        assert_eq!(result.pekDetay.as_ref().unwrap().finalPek, dec!(3301.06));
        assert_eq!(result.pekDetay.as_ref().unwrap().pekAltSinir, dec!(3000));
        assert_eq!(result.pekDetay.as_ref().unwrap().pekUstSinir, dec!(27000));
        assert_eq!(
            result.pekDetay.as_ref().unwrap().yemekIstisnasiTutar,
            dec!(20.02)
        );
        assert_eq!(result.kesintiler.isciSgkPrimi, Some(dec!(462.15)));
        assert_eq!(result.kesintiler.isciIssizlikPrimi, Some(dec!(33.01)));
        assert_eq!(result.kesintiler.sendikaAidati, Some(Decimal::ZERO));
        assert_eq!(result.kesintiler.bes, Some(Decimal::ZERO));
        assert_eq!(result.kesintiler.gelirVergisi, Some(dec!(38.39)));
        assert_eq!(result.kesintiler.damgaVergisi, Some(dec!(2.29)));
        assert_eq!(result.kesintiToplam, dec!(535.84));
        assert_eq!(result.netOdeme, dec!(2785.24));
        assert_eq!(
            result.pekDetay.as_ref().unwrap().isverenSgkPrimi,
            Some(dec!(717.98))
        );
        assert_eq!(
            result.pekDetay.as_ref().unwrap().isverenIssizlikPrimi,
            Some(dec!(66.02))
        );
        assert_eq!(
            result.pekDetay.as_ref().unwrap().isverenPrimToplami,
            Some(dec!(784.00))
        );
        let gv = result.gvDetay.as_ref().unwrap();
        assert_eq!(gv.oncekiKumulatifGvMatrahi, Decimal::ZERO);
        assert_eq!(gv.cariGvMatrahi, dec!(2805.90));
        assert_eq!(gv.brutGelirVergisi, dec!(420.89));
        assert_eq!(gv.asgariUcretGvMatrahi, dec!(2550));
        assert_eq!(gv.asgariUcretGvIstisnasi, dec!(382.50));
        assert_eq!(gv.uygulananGvIstisnasi, dec!(382.50));
        assert_eq!(gv.kesilenGelirVergisi, dec!(38.39));
        let stamp = result.damgaDetay.as_ref().unwrap();
        assert_eq!(stamp.brutDamgaVergisi, dec!(25.06));
        assert_eq!(stamp.aylikDamgaIstisnaHakki, dec!(22.77));
        assert_eq!(stamp.uygulananDamgaIstisnasi, dec!(22.77));
        assert_eq!(stamp.kesilenDamgaVergisi, dec!(2.29));
        assert_eq!(result.devredenPekGelen, Some(Vec::new()));
        assert_eq!(result.sonrakiDevredenPek, Some(Vec::new()));

        let mut one_statutory_slice = result.statutorySnapshot.clone().unwrap();
        one_statutory_slice.segments = vec![ResolvedStatutorySegmentSnapshot {
            effectiveFrom: "2026-01-15".into(),
            effectiveTo: "2026-02-14".into(),
            sgkPrimGunSayisi: 30,
            fiiliYemekGunu: 2,
            gunlukAsgariUcret: dec!(100),
            pekTavanKatsayisi: dec!(9),
            gunlukYemekIstisnasiSGK: dec!(1000),
            gunlukYemekIstisnasiGV: dec!(1000),
        }];
        let resolved = crate::resolve_compensation_segments(
            "2026-01-15",
            "2026-02-14",
            &request.dataset.compensationPolicyVersions,
        )
        .unwrap();
        assert_eq!(
            resolved
                .iter()
                .map(|segment| (segment.startDate.as_str(), segment.endDate.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("2026-01-15", "2026-01-15"),
                ("2026-01-16", "2026-01-31"),
                ("2026-02-01", "2026-02-14"),
            ]
        );
        let one_compensation_version = policy_version(
            "constant-meal",
            "2026-01-15",
            resolved[0].state.clone(),
            None,
            CompensationBoundaryScope::FullEarning,
            RoadRoundingMode::PerCompensationSlice,
        );
        let one_compensation_versions = [one_compensation_version];
        let one_compensation_segments = crate::resolve_compensation_segments(
            "2026-01-15",
            "2026-02-14",
            &one_compensation_versions,
        )
        .unwrap();
        let one_compensation_timeline =
            resolve_compensation_timeline(&request.dataset.periods[0], one_compensation_segments)
                .unwrap();
        let statutory_boundary_only = calculate_policy_meal_exemptions(
            &request.dataset.attendances[0],
            &request.dataset.periods[0],
            result.statutorySnapshot.as_ref().unwrap(),
            &one_compensation_timeline,
        )
        .unwrap();
        assert_eq!(statutory_boundary_only.sgk, dec!(20.01));
        assert_eq!(statutory_boundary_only.gv, dec!(20.01));
        let same_statutory_slice = calculate_policy_meal_exemptions(
            &request.dataset.attendances[0],
            &request.dataset.periods[0],
            &one_statutory_slice,
            &one_compensation_timeline,
        )
        .unwrap();
        assert_eq!(same_statutory_slice.sgk, dec!(20.01));
        assert_eq!(same_statutory_slice.gv, dec!(20.01));
    }

    #[test]
    fn one_policy_baseline_calculates_without_legacy_compensation_history() {
        let mut request = n_segment_policy_request();
        let baseline = request.dataset.compensationPolicyVersions[0].clone();
        request.dataset.compensationPolicyVersions = vec![baseline];
        assert_eq!(request.dataset.periods.len(), 1);
        assert_eq!(request.dataset.institutionSettings.len(), 1);
        assert_eq!(request.dataset.compensationPolicyVersions.len(), 1);
        assert_eq!(request.dataset.taxOpenings.len(), 1);
        assert_eq!(
            request.dataset.taxOpenings[0].gvCumulativeOpening,
            Some(Decimal::ZERO)
        );
        assert_eq!(
            request.dataset.taxOpenings[0].asgariGvCumulativeOpening,
            Some(Decimal::ZERO)
        );

        let result = calculate_payroll_with_policy(&request).unwrap();
        assert_eq!(result.gelirler.tabanBrutAylik, Some(dec!(3100)));
        assert_eq!(result.gelirler.yemek, Some(dec!(20.01)));
        assert_eq!(
            result.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
            Decimal::ZERO
        );
        assert_eq!(result.oncekiKumulatifGvMatrahi, Some(Decimal::ZERO));
    }

    #[test]
    fn policy_resolver_failures_do_not_fall_back_to_legacy_settings() {
        let mut request = legacy_raise_request();
        materialize_legacy_raise_policy(&mut request);
        request.dataset.compensationPolicyVersions.clear();
        assert!(matches!(
            calculate_payroll_with_policy(&request),
            Err(DomainError::NotFound(_))
        ));

        let mut request = legacy_raise_request();
        materialize_legacy_raise_policy(&mut request);
        let corrected_state = request.dataset.compensationPolicyVersions[0].state.clone();
        request.dataset.compensationPolicyVersions = vec![
            policy_version(
                "baseline",
                "2026-01-15",
                corrected_state.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "correction",
                "2026-01-15",
                corrected_state,
                Some("baseline"),
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
        ];
        let corrected = crate::resolve_compensation_segments(
            "2026-01-15",
            "2026-02-14",
            &request.dataset.compensationPolicyVersions,
        )
        .unwrap();
        assert_eq!(corrected.len(), 1);
        assert_eq!(corrected[0].versionId, "correction");
        assert!(calculate_payroll_with_policy(&request).is_ok());

        request.dataset.compensationPolicyVersions.reverse();
        assert!(calculate_payroll_with_policy(&request).is_err());

        let mut invalid_state = n_segment_policy_request();
        invalid_state.dataset.compensationPolicyVersions[0]
            .state
            .gunlukTabanUcret = Decimal::ZERO;
        assert!(matches!(
            calculate_payroll_with_policy(&invalid_state),
            Err(DomainError::ValidationError(_))
        ));

        let mut invalid_date = n_segment_policy_request();
        invalid_date.dataset.compensationPolicyVersions[0].effectiveFrom = "2026-1-15".into();
        assert!(matches!(
            calculate_payroll_with_policy(&invalid_date),
            Err(DomainError::ValidationError(_))
        ));

        let mut out_of_order_dates = n_segment_policy_request();
        out_of_order_dates
            .dataset
            .compensationPolicyVersions
            .swap(0, 2);
        assert!(matches!(
            calculate_payroll_with_policy(&out_of_order_dates),
            Err(DomainError::InvalidData(_))
        ));
    }

    #[test]
    fn persisted_boundary_flags_control_rounding_and_fail_closed() {
        let request = n_segment_policy_request();
        let period = &request.dataset.periods[0];
        let state = request.dataset.compensationPolicyVersions[0].state.clone();
        let mut versions = vec![
            policy_version(
                "baseline",
                "2026-01-15",
                state.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "same-rate",
                "2026-01-16",
                state.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
        ];
        let segments = crate::resolve_compensation_segments(
            &period.baslangicTarihi,
            &period.bitisTarihi,
            &versions,
        )
        .unwrap();
        let full_timeline = resolve_compensation_timeline(period, segments.clone()).unwrap();
        assert_eq!(full_timeline.earning_slice_count().unwrap(), 2);

        versions[1].boundaryScope = CompensationBoundaryScope::RoadOnly;
        let road_only_segments = crate::resolve_compensation_segments(
            &period.baslangicTarihi,
            &period.bitisTarihi,
            &versions,
        )
        .unwrap();
        let road_only_timeline = resolve_compensation_timeline(period, road_only_segments).unwrap();
        assert_eq!(road_only_timeline.earning_slice_count().unwrap(), 1);

        let mut mismatched_mode = versions.clone();
        mismatched_mode[1].roadRoundingMode = RoadRoundingMode::PeriodTotal;
        assert!(matches!(
            crate::resolve_compensation_segments(
                &period.baslangicTarihi,
                &period.bitisTarihi,
                &mismatched_mode
            ),
            Err(DomainError::InvalidData(_))
        ));

        let mut contradictory = versions.clone();
        contradictory[1].state.gunlukTabanUcret += Decimal::ONE;
        assert!(matches!(
            crate::resolve_compensation_segments(
                &period.baslangicTarihi,
                &period.bitisTarihi,
                &contradictory
            ),
            Err(DomainError::InvalidData(_))
        ));

        let superseded_with_other_flags = vec![
            policy_version(
                "old-baseline",
                "2026-01-15",
                state.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PeriodTotal,
            ),
            policy_version(
                "baseline",
                "2026-01-15",
                state.clone(),
                Some("old-baseline"),
                CompensationBoundaryScope::RoadOnly,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "next",
                "2026-02-01",
                state.clone(),
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
        ];
        let superseded_segments = crate::resolve_compensation_segments(
            &period.baslangicTarihi,
            &period.bitisTarihi,
            &superseded_with_other_flags,
        )
        .unwrap();
        assert_eq!(superseded_segments[0].versionId, "baseline");
        assert_eq!(
            superseded_segments[0].boundaryScope,
            CompensationBoundaryScope::RoadOnly
        );
        assert_eq!(
            superseded_segments[0].roadRoundingMode,
            RoadRoundingMode::PerCompensationSlice
        );

        let preperiod_versions = [policy_version(
            "preperiod-baseline",
            "2026-01-01",
            state,
            None,
            CompensationBoundaryScope::RoadOnly,
            RoadRoundingMode::PerCompensationSlice,
        )];
        let preperiod_segments = crate::resolve_compensation_segments(
            &period.baslangicTarihi,
            &period.bitisTarihi,
            &preperiod_versions,
        )
        .unwrap();
        let preperiod_timeline = resolve_compensation_timeline(period, preperiod_segments).unwrap();
        assert_eq!(preperiod_timeline.earning_slice_count().unwrap(), 1);
        assert_eq!(
            preperiod_timeline.resolved_segments()[0].startDate,
            period.baslangicTarihi
        );
        assert_eq!(
            preperiod_timeline.resolved_segments()[0].endDate,
            period.bitisTarihi
        );

        let mut version_json = serde_json::to_value(&versions[0]).unwrap();
        version_json
            .as_object_mut()
            .unwrap()
            .remove("boundaryScope");
        assert!(serde_json::from_value::<CompensationPolicyVersion>(version_json).is_err());
        let mut version_json = serde_json::to_value(&versions[0]).unwrap();
        version_json["boundaryScope"] = serde_json::Value::String("FULL_EARNINGS".into());
        assert!(serde_json::from_value::<CompensationPolicyVersion>(version_json).is_err());
        let mut version_json = serde_json::to_value(&versions[0]).unwrap();
        version_json["roadRoundingMode"] = serde_json::Value::String("PER_POLICY_SLICE".into());
        assert!(serde_json::from_value::<CompensationPolicyVersion>(version_json).is_err());
    }

    #[test]
    fn full_record_parity_covers_28_29_30_and_31_day_service_periods() {
        for (year, month, expected_days) in
            [(2026, 2, 28), (2024, 2, 29), (2026, 4, 30), (2026, 1, 31)]
        {
            let mut request = legacy_raise_request_for(year, month);
            materialize_legacy_raise_policy(&mut request);
            assert_eq!(request.dataset.attendances[0].gunler.len(), expected_days);
            let legacy = calculate_legacy_payroll(&request).unwrap();
            let shadow = calculate_payroll_with_policy(&request).unwrap();
            assert_full_record_equal(&legacy, &shadow);
        }
    }

    #[test]
    fn year_crossing_service_period_uses_explicit_payment_tax_anchor() {
        let mut request = legacy_raise_request_for(2025, 12);
        let current_id = request.periodId.clone();
        let current = request
            .dataset
            .periods
            .iter_mut()
            .find(|period| period.id == current_id)
            .unwrap();
        current.taxYear = 2026;
        current.taxMonth = 1;
        request.accrual.as_mut().unwrap().paymentDate = "2026-01-25".into();
        request.dataset.annualPayrollParameters[0].year = 2026;
        explicit_zero_tax_opening_shadow(&mut request, &current_id);
        materialize_legacy_raise_policy(&mut request);

        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.paymentDate, "2026-01-25");
        assert_eq!(legacy.oncekiKumulatifGvMatrahi, Some(Decimal::ZERO));
    }

    #[test]
    fn paid_sick_days_and_all_attendance_labels_match_across_the_policy_cut() {
        let mut request = legacy_raise_request();
        let current = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == request.periodId)
            .unwrap()
            .clone();
        let mut attendance = patterned_attendance(&request.personnelId, &current);
        attendance.gunler.insert("2026-01-31".into(), "R".into());
        attendance.gunler.insert("2026-02-01".into(), "R".into());
        request.dataset.attendances = vec![attendance];
        request.dataset.sickLeaveRecords = vec![SickLeaveRecord {
            id: "paid-two-day-episode".into(),
            personnelId: request.personnelId.clone(),
            startDate: "2026-01-31".into(),
            endDate: "2026-02-01".into(),
            createdAt: None,
            updatedAt: None,
        }];
        materialize_legacy_raise_policy(&mut request);

        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert_eq!(legacy.odenenRaporluGun, Some(2));
        assert_eq!(legacy.gelirler.tabanBrutAylik, Some(dec!(3240)));
        assert!(legacy.puantajOzeti.gc > 0);
        assert!(legacy.puantajOzeti.gct > 0);
    }

    #[test]
    fn fractional_daily_rounding_respects_compensation_road_and_statutory_boundaries() {
        for (transport_changes, expected_road, expected_version_dates, road_rounding) in [
            (
                None,
                dec!(220.06),
                vec!["2026-01-15", "2026-02-01"],
                RoadRoundingMode::PerCompensationSlice,
            ),
            (
                Some(Vec::new()),
                dec!(220.06),
                vec!["2026-01-15", "2026-02-01"],
                RoadRoundingMode::PeriodTotal,
            ),
            (
                Some(vec![TransportRateChange {
                    effectiveFrom: "2026-01-25".into(),
                    gunlukVasitaYol: dec!(30.005),
                }]),
                dec!(300.06),
                vec!["2026-01-15", "2026-01-25", "2026-02-01"],
                RoadRoundingMode::PeriodTotal,
            ),
        ] {
            let mut request = fractional_raise_request();
            let current_period = request
                .dataset
                .periods
                .iter_mut()
                .find(|period| period.id == request.periodId)
                .unwrap();
            // The January service period closes in February. The dated
            // minimum-wage override becomes the legal reference for February.
            current_period.taxMonth = 2;
            request.accrual.as_mut().unwrap().paymentDate = "2026-02-25".into();
            request
                .dataset
                .institutionSettings
                .get_mut(&request.periodId)
                .unwrap()
                .transportRateChanges = transport_changes;
            request
                .dataset
                .institutionSettings
                .get_mut(&request.periodId)
                .unwrap()
                .statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
                effectiveFrom: "2026-01-23".into(),
                gunlukAsgariUcret: Some(dec!(101)),
                pekTavanKatsayisi: Some(dec!(9)),
                gunlukYemekIstisnasiSGK: Some(dec!(1000)),
                gunlukYemekIstisnasiGV: Some(dec!(1000)),
            }]);
            materialize_legacy_raise_policy_with_mode(&mut request, road_rounding).unwrap();
            assert_eq!(
                request
                    .dataset
                    .compensationPolicyVersions
                    .iter()
                    .map(|version| version.effectiveFrom.as_str())
                    .collect::<Vec<_>>(),
                expected_version_dates
            );
            let versions = &request.dataset.compensationPolicyVersions;
            assert_eq!(versions.len(), expected_version_dates.len());
            assert!(versions
                .iter()
                .all(|version| version.roadRoundingMode == road_rounding));
            if expected_version_dates.len() == 3 {
                assert_eq!(
                    versions[1].boundaryScope,
                    CompensationBoundaryScope::RoadOnly
                );
                assert_eq!(versions[1].state.gunlukVasitaYol, dec!(30.005));
                assert_eq!(
                    versions[0].boundaryScope,
                    CompensationBoundaryScope::FullEarning
                );
                assert_eq!(
                    versions[2].boundaryScope,
                    CompensationBoundaryScope::FullEarning
                );
            } else {
                assert!(
                    versions
                        .iter()
                        .all(|version| version.boundaryScope
                            == CompensationBoundaryScope::FullEarning)
                );
            }
            let persisted_policy_json =
                serde_json::to_string(versions).expect("final policy model should serialize");
            request.dataset.compensationPolicyVersions =
                serde_json::from_str(&persisted_policy_json)
                    .expect("persisted policy model should reload with both flags");
            let legacy = calculate_legacy_payroll(&request).unwrap();
            let shadow = calculate_payroll_with_policy(&request).unwrap();
            assert_full_record_equal(&legacy, &shadow);
            assert_eq!(legacy.gelirler.tabanBrutAylik, Some(dec!(3100.16)));
            assert_eq!(legacy.gelirler.yemek, Some(dec!(110.06)));
            assert_eq!(legacy.gelirler.vasitaYol, Some(expected_road));
            assert_eq!(legacy.gelirler.isPrimi, Some(dec!(110.06)));
            assert_eq!(legacy.gelirler.geceCalismasiUcreti, Some(dec!(15.03)));
            assert_eq!(legacy.gelirler.geceCalismasiTatiliUcreti, Some(dec!(20.03)));
        }
    }

    #[test]
    fn previous_normal_tis_event_and_next_normal_preserve_tax_and_carry_chains() {
        let mut request = legacy_raise_request();
        for settings in request.dataset.institutionSettings.values_mut() {
            settings.birlestirilmisSosyalYardim = dec!(300000);
        }
        materialize_legacy_raise_policy(&mut request);
        let first_legacy = calculate_legacy_payroll(&request).unwrap();
        let first_shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&first_legacy, &first_shadow);
        assert!(!first_legacy.sonrakiDevredenPek.as_ref().unwrap().is_empty());

        let mut legacy_tis_request = request.clone();
        let mut shadow_tis_request = request.clone();
        legacy_tis_request.dataset.payrolls = vec![first_legacy.clone()];
        shadow_tis_request.dataset.payrolls = vec![first_shadow.clone()];
        for candidate in [&mut legacy_tis_request, &mut shadow_tis_request] {
            candidate.manualIncome = None;
            candidate.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-tis", candidate.personnelId),
                accrualType: AccrualType::TIS_IKRAMIYE,
                paymentDate: "2026-01-25".into(),
                sequence: 1,
                grossAmount: Some(dec!(50000)),
                description: Some("Same-month TIS event".into()),
            });
        }
        let tis_legacy = calculate_legacy_payroll(&legacy_tis_request).unwrap();
        let tis_shadow = calculate_payroll_with_policy(&shadow_tis_request).unwrap();
        assert_full_record_equal(&tis_legacy, &tis_shadow);
        assert_eq!(tis_legacy.devredenPekGelen, tis_shadow.devredenPekGelen);
        assert_eq!(tis_legacy.sonrakiDevredenPek, tis_shadow.sonrakiDevredenPek);
        assert!(!tis_legacy.devredenPekGelen.as_ref().unwrap().is_empty());
        assert!(
            tis_legacy
                .pekDetay
                .as_ref()
                .unwrap()
                .aylikOncekiPekTuketimi
                .unwrap()
                > Decimal::ZERO
        );

        let next = period(2026, 2);
        for candidate in [&mut legacy_tis_request, &mut shadow_tis_request] {
            candidate.dataset.payrolls.push(tis_legacy.clone());
            candidate.dataset.institutionSettings.insert(
                next.id.clone(),
                DonemselKurumDegerleri {
                    donemId: next.id.clone(),
                    ..candidate.dataset.institutionSettings[&request.periodId].clone()
                },
            );
            candidate
                .dataset
                .attendances
                .push(all_c_attendance(&candidate.personnelId, &next));
            candidate.dataset.periods.push(next.clone());
            candidate.periodId = next.id.clone();
            candidate.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-{}", candidate.personnelId, next.id),
                accrualType: AccrualType::NORMAL,
                paymentDate: "2026-02-25".into(),
                sequence: 0,
                grossAmount: None,
                description: Some("Next month NORMAL".into()),
            });
            candidate.manualIncome = None;
        }
        let next_legacy = calculate_legacy_payroll(&legacy_tis_request).unwrap();
        let next_shadow = calculate_payroll_with_policy(&shadow_tis_request).unwrap();
        assert_full_record_equal(&next_legacy, &next_shadow);
        assert_eq!(
            next_legacy.oncekiKumulatifGvMatrahi,
            next_shadow.oncekiKumulatifGvMatrahi
        );
        assert_eq!(next_legacy.devredenPekGelen, next_shadow.devredenPekGelen);
        assert_eq!(
            next_legacy.sonrakiDevredenPek,
            next_shadow.sonrakiDevredenPek
        );
        assert!(!next_legacy.devredenPekGelen.as_ref().unwrap().is_empty());

        legacy_tis_request
            .dataset
            .payrolls
            .push(next_legacy.clone());
        shadow_tis_request
            .dataset
            .payrolls
            .push(next_shadow.clone());
        let following_periods = (3..=12)
            .map(|month| period(2026, month))
            .chain(std::iter::once(period(2027, 1)));
        for following_period in following_periods {
            for candidate in [&mut legacy_tis_request, &mut shadow_tis_request] {
                let mut settings =
                    candidate.dataset.institutionSettings[&candidate.periodId].clone();
                settings.donemId = following_period.id.clone();
                candidate
                    .dataset
                    .institutionSettings
                    .insert(following_period.id.clone(), settings);
                candidate
                    .dataset
                    .attendances
                    .push(all_c_attendance(&candidate.personnelId, &following_period));
                candidate.dataset.periods.push(following_period.clone());
                candidate.periodId = following_period.id.clone();
                if !candidate
                    .dataset
                    .annualPayrollParameters
                    .iter()
                    .any(|annual| annual.year == following_period.taxYear)
                {
                    let mut annual = AnnualPayrollParameters::default_for_2026();
                    annual.year = following_period.taxYear;
                    candidate.dataset.annualPayrollParameters.push(annual);
                }
                if !candidate
                    .dataset
                    .taxOpenings
                    .iter()
                    .any(|opening| opening.year == following_period.taxYear)
                {
                    let mut opening_request = candidate.clone();
                    explicit_zero_tax_opening_shadow(&mut opening_request, &following_period.id);
                    candidate
                        .dataset
                        .taxOpenings
                        .push(opening_request.dataset.taxOpenings.pop().unwrap());
                }
                candidate.accrual = Some(PayrollAccrualInput {
                    accrualId: format!("{}-{}", candidate.personnelId, following_period.id),
                    accrualType: AccrualType::NORMAL,
                    paymentDate: format!("{}-25", following_period.id),
                    sequence: 0,
                    grossAmount: None,
                    description: Some("Copied compensation policy NORMAL".into()),
                });
            }
            let following_legacy = calculate_legacy_payroll(&legacy_tis_request).unwrap();
            let following_shadow = calculate_payroll_with_policy(&shadow_tis_request).unwrap();
            assert_full_record_equal(&following_legacy, &following_shadow);
            assert_eq!(
                following_legacy.devredenPekGelen,
                following_shadow.devredenPekGelen
            );
            assert_eq!(
                following_legacy.sonrakiDevredenPek,
                following_shadow.sonrakiDevredenPek
            );
            assert_eq!(
                following_legacy.oncekiKumulatifGvMatrahi,
                following_shadow.oncekiKumulatifGvMatrahi
            );
            legacy_tis_request.dataset.payrolls.push(following_legacy);
            shadow_tis_request.dataset.payrolls.push(following_shadow);
        }
    }

    #[test]
    fn earlier_same_month_supplementary_event_precedes_normal_peK_and_meal_exemption() {
        let mut request = legacy_raise_request();
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .gunlukYemekIstisnasiSGK = Some(dec!(1000));
        request
            .dataset
            .institutionSettings
            .get_mut(&request.periodId)
            .unwrap()
            .gunlukYemekIstisnasiGV = Some(dec!(1000));
        materialize_legacy_raise_policy(&mut request);
        let mut legacy_tis_request = request.clone();
        let mut shadow_tis_request = request.clone();
        for candidate in [&mut legacy_tis_request, &mut shadow_tis_request] {
            candidate.manualIncome = None;
            candidate.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-earlier-tis", candidate.personnelId),
                accrualType: AccrualType::TIS_IKRAMIYE,
                paymentDate: "2026-01-25".into(),
                sequence: 0,
                grossAmount: Some(dec!(500)),
                description: Some("Earlier same-month TIS".into()),
            });
        }
        let legacy_tis = calculate_legacy_payroll(&legacy_tis_request).unwrap();
        let shadow_tis = calculate_payroll_with_policy(&shadow_tis_request).unwrap();
        assert_full_record_equal(&legacy_tis, &shadow_tis);

        let mut legacy_normal_request = request.clone();
        let mut shadow_normal_request = request;
        for (candidate, earlier) in [
            (&mut legacy_normal_request, legacy_tis.clone()),
            (&mut shadow_normal_request, shadow_tis.clone()),
        ] {
            candidate.dataset.payrolls.push(earlier);
            candidate.manualIncome = None;
            candidate.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-following-normal", candidate.personnelId),
                accrualType: AccrualType::NORMAL,
                paymentDate: "2026-01-25".into(),
                sequence: 1,
                grossAmount: None,
                description: Some("Following same-month NORMAL".into()),
            });
        }
        let legacy = calculate_legacy_payroll(&legacy_normal_request).unwrap();
        let shadow = calculate_payroll_with_policy(&shadow_normal_request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        let pek = legacy.pekDetay.as_ref().unwrap();
        assert!(pek.aylikOncekiPekTuketimi.unwrap() > Decimal::ZERO);
        assert!(pek.yemekIstisnasiTutar > Decimal::ZERO);
        assert!(
            legacy
                .gvDetay
                .as_ref()
                .unwrap()
                .ayniAyOncekiKullanilanGvIstisnasi
                > Decimal::ZERO
        );
    }

    #[test]
    fn current_normal_snapshot_replays_same_month_event_from_authoritative_saved_row() {
        let mut request = legacy_raise_request();
        materialize_legacy_raise_policy(&mut request);
        let period = request
            .dataset
            .periods
            .iter()
            .find(|period| period.id == request.periodId)
            .unwrap()
            .clone();

        let mut earlier_request = request.clone();
        earlier_request.manualIncome = None;
        earlier_request.accrual = Some(PayrollAccrualInput {
            accrualId: format!("{}-prior-tis", request.personnelId),
            accrualType: AccrualType::TIS_IKRAMIYE,
            paymentDate: "2026-01-20".into(),
            sequence: 0,
            grossAmount: Some(dec!(500)),
            description: Some("Earlier same-month TIS".into()),
        });
        let earlier = calculate_payroll_with_policy(&earlier_request).unwrap();

        let mut current_request = request;
        current_request.dataset.payrolls.push(earlier.clone());
        current_request.manualIncome = None;
        current_request.accrual = Some(PayrollAccrualInput {
            accrualId: format!("{}-following-normal", current_request.personnelId),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-01-25".into(),
            sequence: 1,
            grossAmount: None,
            description: Some("Following same-month NORMAL".into()),
        });
        let current = calculate_payroll_with_policy(&current_request).unwrap();

        assert_eq!(
            crate::validate_payroll_calculation_metadata_with_context(
                &current,
                &period,
                &current_request.dataset.payrolls,
                &current_request.dataset.periods,
            )
            .expect("same-month event should replay from its authoritative saved row"),
            crate::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
        );

        let error = crate::validate_payroll_calculation_metadata_with_context(
            &current,
            &period,
            &[],
            &current_request.dataset.periods,
        )
        .expect_err("replay must fail closed when the frozen same-month event source is absent");
        assert!(error
            .to_string()
            .contains("missing same-payment-month event"));

        let mut forged_source = earlier;
        forged_source.pekDetay.as_mut().unwrap().primMatrahi += Decimal::ONE;
        let error = crate::validate_payroll_calculation_metadata_with_context(
            &current,
            &period,
            &[forged_source],
            &current_request.dataset.periods,
        )
        .expect_err("same-month event values must still match the frozen snapshot");
        assert!(error.to_string().contains("pekPrimeBase"));
    }

    #[test]
    fn synthetic_2026_2027_acceptance_uses_sep_2026_baseline_without_wage_history() {
        // Future statutory/tax amounts are explicitly synthetic test inputs.
        let mut request = legacy_raise_request_for(2026, 9);
        let initial_settings = request.dataset.institutionSettings["2026-09"].clone();
        let baseline = state_from_legacy_settings(&initial_settings);
        let mut march_state = baseline.clone();
        march_state.gunlukTabanUcret += dec!(15);
        march_state.gunlukYemek += dec!(2);
        let mut september_state = march_state.clone();
        september_state.gunlukVasitaYol += dec!(3);
        let policy_versions = vec![
            policy_version(
                "acceptance-baseline-20260915",
                "2026-09-15",
                baseline,
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "acceptance-raise-20270301",
                "2027-03-01",
                march_state,
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
            policy_version(
                "acceptance-road-state-20270901",
                "2027-09-01",
                september_state,
                None,
                CompensationBoundaryScope::FullEarning,
                RoadRoundingMode::PerCompensationSlice,
            ),
        ];

        let mut periods = Vec::new();
        let mut settings_by_period = HashMap::new();
        let mut attendances = Vec::new();
        for (year, month) in (2026..=2027).flat_map(|year| {
            let months: Vec<u32> = if year == 2026 {
                (9..=12).collect()
            } else {
                (1..=8).collect()
            };
            months.into_iter().map(move |month| (year, month))
        }) {
            let item = period(year, month);
            let mut settings = initial_settings.clone();
            settings.donemId = item.id.clone();
            // The policy timeline is the sole wage authority. These period
            // settings retain statutory/deduction inputs only.
            settings.gunlukTabanUcret = Decimal::ZERO;
            settings.gunlukYemek = Decimal::ZERO;
            settings.birlestirilmisSosyalYardim = Decimal::ZERO;
            settings.gunlukVasitaYol = Decimal::ZERO;
            settings.giyimYardimi = Decimal::ZERO;
            settings.hizmetZammiBirimi = Decimal::ZERO;
            settings.isPrimiYuzde = Some(Decimal::ZERO);
            settings.isPrimiGruplari = Some(Vec::new());
            settings.geceCalismaPrimiYuzde = Some(Decimal::ZERO);
            settings.geceCalismaTatiliPrimiYuzde = Some(Decimal::ZERO);
            settings.ekOdeme = Some(Decimal::ZERO);
            settings.digerGelirVarsayilan = None;
            settings.transportRateChanges = None;
            settings.gunlukAsgariUcret = Some(if year == 2026 { dec!(1000) } else { dec!(1200) });
            settings.pekTavanKatsayisi = Some(dec!(7.5));
            settings.gunlukYemekIstisnasiSGK = Some(dec!(100));
            settings.gunlukYemekIstisnasiGV = Some(dec!(100));
            settings.statutoryParameterSnapshot = None;
            settings.statutoryParameterSegments = Some(Vec::new());
            periods.push(item.clone());
            attendances.push(all_c_attendance(&request.personnelId, &item));
            settings_by_period.insert(item.id.clone(), settings);
        }

        let mut annual_2026 = request.dataset.annualPayrollParameters[0].clone();
        annual_2026.year = 2026;
        let mut annual_2027 = annual_2026.clone();
        annual_2027.year = 2027;
        annual_2027.sigortaGvYillikBrutAsgariUcretTavani = Some(dec!(180000));

        request.periodId = "2026-09".into();
        request.dataset.periods = periods.clone();
        request.dataset.institutionSettings = settings_by_period;
        request.dataset.attendances = attendances;
        request.dataset.annualPayrollParameters = vec![annual_2026, annual_2027];
        request.dataset.compensationPolicyVersions = policy_versions;
        request.dataset.payrolls.clear();
        request.dataset.taxOpenings = vec![
            PersonelTaxOpening {
                id: "acceptance-opening-2026".into(),
                personnelId: request.personnelId.clone(),
                year: 2026,
                gvCumulativeOpening: Some(dec!(1234.567)),
                effectiveFromPeriodId: Some("2026-09".into()),
                asgariGvCumulativeOpening: Some(dec!(456.789)),
                asgariGvEffectiveFromPeriodId: Some("2026-09".into()),
                createdAt: None,
                updatedAt: None,
            },
            PersonelTaxOpening {
                id: "acceptance-opening-2027".into(),
                personnelId: request.personnelId.clone(),
                year: 2027,
                gvCumulativeOpening: Some(dec!(2345.678)),
                effectiveFromPeriodId: Some("2027-01".into()),
                asgariGvCumulativeOpening: Some(dec!(567.891)),
                asgariGvEffectiveFromPeriodId: Some("2027-01".into()),
                createdAt: None,
                updatedAt: None,
            },
        ];
        request.dataset.zamAylari.clear();

        let mut calculated = Vec::new();
        for item in &periods {
            request.periodId = item.id.clone();
            request.calculatedAt = format!("{}T12:00:00Z", item.bitisTarihi);
            request.accrual = Some(PayrollAccrualInput {
                accrualId: format!("{}-{}", request.personnelId, item.id),
                accrualType: AccrualType::NORMAL,
                paymentDate: format!("{}-25", item.id),
                sequence: 0,
                grossAmount: None,
                description: Some("Synthetic 2026-2027 policy acceptance".into()),
            });
            let payroll = calculate_payroll_with_policy(&request)
                .unwrap_or_else(|error| panic!("{} synthetic calculation: {error}", item.id));
            calculated.push(payroll.clone());
            request.dataset.payrolls.push(payroll);
        }

        assert_eq!(calculated.len(), 12);
        assert!(calculated
            .iter()
            .all(|payroll| payroll.donemId.as_str() >= "2026-09"));
        assert_eq!(
            calculated[0]
                .calculationMetadata
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .usedInputs
                .taxHistory
                .previousCumulativeGv,
            dec!(1234.57)
        );
        assert_eq!(
            calculated[0]
                .calculationMetadata
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .usedInputs
                .taxHistory
                .normalGvOpening
                .amount,
            dec!(1234.567)
        );
        assert_eq!(
            calculated[4]
                .calculationMetadata
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .usedInputs
                .annualParameters
                .sigortaGvYillikBrutAsgariUcretTavani,
            Some(dec!(180000))
        );

        for (period_id, boundary) in [("2027-02", "2027-03-01"), ("2027-08", "2027-09-01")] {
            let payroll = calculated
                .iter()
                .find(|item| item.donemId == period_id)
                .unwrap();
            let segments = &payroll
                .calculationMetadata
                .as_ref()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .compensation
                .segments;
            assert!(
                segments
                    .iter()
                    .any(|segment| segment.endDate.as_str() < boundary)
                    && segments
                        .iter()
                        .any(|segment| segment.startDate.as_str() == boundary),
                "{period_id} 15–14 service period should split at {boundary}: {segments:?}"
            );
        }

        for payroll in &calculated {
            let parent = periods
                .iter()
                .find(|item| item.id == payroll.donemId)
                .unwrap();
            crate::validate_payroll_calculation_metadata_with_context(
                payroll,
                parent,
                &calculated,
                &periods,
            )
            .unwrap_or_else(|error| panic!("{} persisted replay: {error}", payroll.donemId));
        }
    }

    #[test]
    fn current_only_premium_group_is_materialized_for_the_pre_raise_legacy_slice() {
        let mut request = legacy_raise_request();
        let previous_id = request.dataset.periods[0].id.clone();
        request
            .dataset
            .institutionSettings
            .get_mut(&previous_id)
            .unwrap()
            .isPrimiGruplari = None;
        materialize_legacy_raise_policy(&mut request);

        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_full_record_equal(&legacy, &shadow);
        assert!(legacy.gelirler.isPrimi.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn one_field_social_policy_keeps_unchanged_meal_raise_rounding_boundary() {
        let mut request = legacy_raise_request();
        let current_id = request.periodId.clone();
        let previous_id = request.dataset.periods[0].id.clone();
        for period_id in [&current_id, &previous_id] {
            let settings = request
                .dataset
                .institutionSettings
                .get_mut(period_id)
                .unwrap();
            settings.gunlukTabanUcret = dec!(100);
            settings.gunlukYemek = dec!(10.005);
            settings.gunlukVasitaYol = Decimal::ZERO;
            settings.giyimYardimi = Decimal::ZERO;
            settings.hizmetZammiBirimi = Decimal::ZERO;
            settings.isPrimiGruplari.as_mut().unwrap()[0].oran = Decimal::ZERO;
            settings.geceCalismaPrimiYuzde = Some(Decimal::ZERO);
            settings.geceCalismaTatiliPrimiYuzde = Some(Decimal::ZERO);
            settings.ekOdeme = None;
            settings.digerGelirVarsayilan = None;
        }
        request
            .dataset
            .institutionSettings
            .get_mut(&current_id)
            .unwrap()
            .birlestirilmisSosyalYardim = dec!(110);
        request
            .dataset
            .institutionSettings
            .get_mut(&previous_id)
            .unwrap()
            .birlestirilmisSosyalYardim = dec!(100);
        materialize_legacy_raise_policy(&mut request);

        let legacy = calculate_legacy_payroll(&request).unwrap();
        let shadow = calculate_payroll_with_policy(&request).unwrap();
        assert_eq!(legacy.gelirler.yemek, Some(dec!(310.16)));
        assert_eq!(shadow.gelirler.yemek, Some(dec!(310.16)));
        assert_eq!(legacy.gelirToplam, dec!(3514.68));
        assert_eq!(shadow.gelirToplam, dec!(3514.68));
        assert_eq!(legacy.netOdeme, dec!(2987.47));
        assert_eq!(shadow.netOdeme, dec!(2987.47));
        assert_full_record_equal(&legacy, &shadow);
    }
}
