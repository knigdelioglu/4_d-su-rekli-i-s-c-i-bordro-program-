//! Provenance captured by the compensation-policy calculation while it still
//! owns the exact immutable request and resolved inputs used for the result.

use super::compensation::CompensationTimeline;
use super::*;
use crate::payroll_snapshot::{
    PAID_SICK_LEAVE_RULE_VERSION, PAYROLL_CALCULATION_VERSION, PAYROLL_SNAPSHOT_SCHEMA_VERSION,
};
use rust_decimal_macros::dec;

fn personal_deductions_snapshot(
    person: &Personel,
) -> Option<PayrollPersonalDeductionInputsSnapshot> {
    person
        .kesintiler
        .as_ref()
        .map(|deductions| PayrollPersonalDeductionInputsSnapshot {
            sendikaUyesi: deductions.sendikaUyesi,
            sabitSendikaAidati: deductions.sabitSendikaAidati,
            besUyesi: deductions.besUyesi,
            oksOraniYuzde: deductions.oksOraniYuzde,
            sabitBesTutar: deductions.sabitBesTutar,
            icraTutar: deductions.icraTutar,
            nafakaTutar: deductions.nafakaTutar,
            kisiBorcuTutar: deductions.kisiBorcuTutar,
            dogumAskerlikBorclanmasiTutar: deductions.dogumAskerlikBorclanmasiTutar,
            hayatSaglikSigortasiTutar: deductions.hayatSaglikSigortasiTutar,
            digerKesintiTutar: deductions.digerKesintiTutar,
            gvIndirimleri: deductions.gvIndirimleri.as_ref().map(|gv| {
                PayrollGvDeductionInputsSnapshot {
                    dogumAskerlikGvIndirimTutar: gv.dogumAskerlikGvIndirimTutar,
                    hayatSigortasiPrimiTutar: gv.hayatSigortasiPrimiTutar,
                    saglikSigortasiPrimiTutar: gv.saglikSigortasiPrimiTutar,
                }
            }),
        })
}

fn income_snapshot(income: &GelirKalemleri) -> PayrollIncomeInputsSnapshot {
    PayrollIncomeInputsSnapshot {
        tabanBrutAylik: income.tabanBrutAylik,
        tediye: income.tediye,
        tisIkramiyesi: income.tisIkramiyesi,
        ekOdeme: income.ekOdeme,
        yemek: income.yemek,
        birlestirilmisSosyalYardim: income.birlestirilmisSosyalYardim,
        vasitaYol: income.vasitaYol,
        giyimYardimi: income.giyimYardimi,
        isPrimi: income.isPrimi,
        geceCalismasiUcreti: income.geceCalismasiUcreti,
        geceCalismasiTatiliUcreti: income.geceCalismasiTatiliUcreti,
        hizmetZammi: income.hizmetZammi,
        digerGelir: income.digerGelir,
    }
}

fn tax_opening_snapshot(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    period: &BordroDonemi,
    normal_gv: bool,
    resolved: Option<ResolvedTaxOpening>,
) -> PayrollTaxOpeningSnapshot {
    let explicit = index.tax_opening(dataset, &person.id, period.taxYear);
    let component = explicit.and_then(|opening| {
        if normal_gv {
            opening
                .gvCumulativeOpening
                .zip(opening.effectiveFromPeriodId.as_deref())
                .map(|(amount, period_id)| (amount, period_id, opening.id.as_str()))
        } else {
            opening
                .asgariGvCumulativeOpening
                .zip(opening.asgariGvEffectiveFromPeriodId.as_deref())
                .map(|(amount, period_id)| (amount, period_id, opening.id.as_str()))
        }
    });

    let (source, opening_id, amount, start_month) = if let Some((amount, period_id, id)) = component
    {
        let start_month = index
            .period(dataset, period_id)
            .map(|source_period| source_period.taxMonth)
            .unwrap_or_else(|| resolved.map(|opening| opening.start_tax_month).unwrap_or(1));
        (
            PayrollTaxOpeningSource::EXPLICIT_OPENING,
            Some(id.to_string()),
            amount,
            start_month,
        )
    } else {
        let legacy = if normal_gv {
            let value = person.devirKumulatifGvMatrahi.unwrap_or_default();
            let year = person.devirKumulatifGvMatrahiYili.unwrap_or(period.taxYear);
            (value > Decimal::ZERO && year == period.taxYear).then_some(value)
        } else {
            let value = person.devirKumulatifAsgariGvMatrahi.unwrap_or_default();
            let year = person
                .devirKumulatifAsgariGvMatrahiYili
                .unwrap_or(period.taxYear);
            (value > Decimal::ZERO && year == period.taxYear).then_some(value)
        };
        if let (Some(amount), Some(resolved)) = (legacy, resolved) {
            (
                PayrollTaxOpeningSource::LEGACY_PERSONNEL_VALUE,
                None,
                amount,
                resolved.start_tax_month,
            )
        } else {
            (
                PayrollTaxOpeningSource::IMPLICIT_ZERO,
                None,
                Decimal::ZERO,
                1,
            )
        }
    };

    PayrollTaxOpeningSnapshot {
        source,
        openingId: opening_id,
        effectiveFromTaxYear: period.taxYear,
        effectiveFromTaxMonth: start_month,
        amount,
    }
}

fn prior_event_snapshot(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    payroll: &BordroKaydi,
    current_settings: &PayrollPeriodInstitutionSettings,
) -> Result<PayrollPriorPaymentEventSnapshot> {
    let period = period_by_id(dataset, index, &payroll.donemId)?;
    let gv = payroll.gvDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior payment event GV provenance eksik; compensation snapshot üretilemez.",
            effective_accrual_id(payroll)
        ))
    })?;
    let pek = payroll.pekDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior payment event PEK provenance eksik; compensation snapshot üretilemez.",
            effective_accrual_id(payroll)
        ))
    })?;
    let statutory = payroll.statutorySnapshot.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior payment event statutory provenance eksik; compensation snapshot üretilemez.",
            effective_accrual_id(payroll)
        ))
    })?;
    let stamp_exemption = payroll
        .damgaDetay
        .as_ref()
        .map(|detail| detail.uygulananDamgaIstisnasi)
        .unwrap_or_else(|| {
            let rate = current_settings.damgaVergisiOraniBinde.unwrap_or_default() / dec!(1000);
            (round2(payroll.gelirToplam * rate)
                - payroll.kesintiler.damgaVergisi.unwrap_or_default())
            .max(Decimal::ZERO)
        });

    Ok(PayrollPriorPaymentEventSnapshot {
        periodId: payroll.donemId.clone(),
        accrualId: effective_accrual_id(payroll),
        accrualType: payroll.accrualType,
        taxYear: period.taxYear,
        taxMonth: period.taxMonth,
        paymentDate: effective_payment_date(payroll, period),
        sequence: payroll.sequence,
        gvTaxableBase: payroll_gv_base(payroll)?,
        insuranceGvDeductionUsed: gv.uygulanabilirSigortaGvIndirimi,
        gvExemptionUsed: gv.uygulananGvIstisnasi,
        stampExemptionUsed: stamp_exemption,
        pekPrimeBase: pek.primMatrahi,
        monthPekConsumptionAfter: pek.aylikSonrasiPekTuketimi,
        pekCeiling: pek.pekUstSinir,
        mealSgkExemption: statutory.sgkYemekIstisnasiToplam,
        income: income_snapshot(&payroll.gelirler),
        outgoingPekCarry: payroll
            .sonrakiDevredenPek
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|carry| PayrollPekCarrySnapshot {
                tutar: carry.tutar,
                kalanAySayisi: carry.kalanAySayisi,
                kaynakDonemId: carry.kaynakDonemId.clone(),
            })
            .collect(),
    })
}

fn minimum_wage_references(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    period: &BordroDonemi,
    opening: &PayrollTaxOpeningSnapshot,
) -> Result<Vec<PayrollMinimumWageMonthInputSnapshot>> {
    let mut references = Vec::new();
    for tax_month in opening.effectiveFromTaxMonth..period.taxMonth {
        let source_period = index
            .periods_for_tax_month(dataset, period.taxYear, tax_month)
            .next()
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} vergi yılı {}. ayı için asgari GV yasal dönem girdisi eksik.",
                    period.taxYear, tax_month
                ))
            })?;
        let settings = historical_statutory_settings(dataset, source_period)?;
        validate_kurum_degerleri_for_asgari_gv_reference(&settings)?;
        let statutory = resolve_statutory_snapshot_for_payment_month(source_period, &settings)?;
        let sgk_rate = settings.sgkIsciOraniYuzde.ok_or_else(|| {
            DomainError::ValidationError(format!(
                "{} dönemi için SGK işçi oranı eksik.",
                source_period.id
            ))
        })?;
        let unemployment_rate = settings.issizlikIsciOraniYuzde.ok_or_else(|| {
            DomainError::ValidationError(format!(
                "{} dönemi için işsizlik işçi oranı eksik.",
                source_period.id
            ))
        })?;
        let monthly_gv_base = calculate_aylik_asgari_ucret_gv_matrahi(
            statutory.gvReferansGunlukAsgariUcret,
            sgk_rate / dec!(100),
            unemployment_rate / dec!(100),
        );
        references.push(PayrollMinimumWageMonthInputSnapshot {
            periodId: source_period.id.clone(),
            taxYear: source_period.taxYear,
            taxMonth: source_period.taxMonth,
            dailyMinimumWage: statutory.gvReferansGunlukAsgariUcret,
            sgkWorkerRatePercent: sgk_rate,
            unemploymentWorkerRatePercent: unemployment_rate,
            monthlyGvBase: monthly_gv_base,
        });
    }
    Ok(references)
}

// Keep the inputs explicit: each value is resolved by a distinct payroll-state stage.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_compensation_policy_snapshot(
    request: &PayrollCalculationRequest,
    index: &PayrollDatasetIndex,
    person: &Personel,
    period: &BordroDonemi,
    accrual: &PayrollAccrualInput,
    settings: &PayrollPeriodInstitutionSettings,
    attendance: &PersonelPuantaj,
    sick_records: &[SickLeaveRecord],
    paid_sick_dates: &[NaiveDate],
    timeline: &CompensationTimeline,
    incoming_pek: &IncomingDevredenPekState,
    same_payment_month_pek_used: Decimal,
    payroll: &BordroKaydi,
) -> Result<PayrollCalculationSnapshot> {
    let dataset = &request.dataset;
    let openings = resolve_tax_openings(dataset, index, person, period)?;
    let normal_gv_opening =
        tax_opening_snapshot(dataset, index, person, period, true, openings.normal);
    let minimum_wage_gv_opening =
        tax_opening_snapshot(dataset, index, person, period, false, openings.asgari);

    let current_order = accrual_order_for_input(period, accrual)?;
    let mut prior_events =
        ordered_prior_payment_events(dataset, index, &person.id, &current_order)?
            .into_iter()
            .filter(|(_, prior)| {
                matches!(
                    prior.status,
                    BordroStatus::CALCULATED | BordroStatus::FINALIZED
                ) && index
                    .period(dataset, &prior.donemId)
                    .is_some_and(|prior_period| {
                        prior_period.taxYear == period.taxYear
                            && prior_period.taxMonth <= period.taxMonth
                    })
            })
            .map(|(_, prior)| prior_event_snapshot(dataset, index, prior, settings))
            .collect::<Result<Vec<_>>>()?;
    // The order from the payment-event index is canonical and intentionally
    // retained; do not sort by work-period ID or accrual type here.
    prior_events.shrink_to_fit();

    let previous_cumulative_gv = payroll.oncekiKumulatifGvMatrahi.ok_or_else(|| {
        DomainError::InvalidData("NORMAL calculation GV opening provenance eksik.".into())
    })?;
    let previous_cumulative_minimum_gv =
        payroll.oncekiKumulatifAsgariGvMatrahi.ok_or_else(|| {
            DomainError::InvalidData(
                "NORMAL calculation asgari GV opening provenance eksik.".into(),
            )
        })?;
    let previous_annual_insurance_gv =
        previous_insurance_gv(dataset, index, &person.id, period, accrual)?;
    let gv_detail = payroll
        .gvDetay
        .as_ref()
        .ok_or_else(|| DomainError::InvalidData("NORMAL calculation GV snapshot eksik.".into()))?;
    let stamp_detail = payroll.damgaDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData("NORMAL calculation damga snapshot eksik.".into())
    })?;
    let tax_history = PayrollTaxHistoryInputsSnapshot {
        normalGvOpening: normal_gv_opening,
        minimumWageGvOpening: minimum_wage_gv_opening.clone(),
        previousCumulativeGv: previous_cumulative_gv,
        previousCumulativeMinimumWageGv: previous_cumulative_minimum_gv,
        previousAnnualInsuranceGvDeduction: previous_annual_insurance_gv,
        samePaymentMonthPekUsed: same_payment_month_pek_used,
        samePaymentMonthGvExemptionUsed: gv_detail.ayniAyOncekiKullanilanGvIstisnasi,
        samePaymentMonthStampExemptionUsed: stamp_detail.ayniAyOncekiKullanilanDamgaIstisnasi,
        previousPaymentEvents: std::mem::take(&mut prior_events),
        minimumWageMonthlyReferences: minimum_wage_references(
            dataset,
            index,
            period,
            &minimum_wage_gv_opening,
        )?,
    };

    let statutory = payroll.statutorySnapshot.as_ref().ok_or_else(|| {
        DomainError::InvalidData("NORMAL calculation statutory provenance eksik.".into())
    })?;
    let sgk_worker_rate = statutory
        .sgkIsciOraniYuzde
        .ok_or_else(|| DomainError::ValidationError("SGK işçi oranı snapshot'ı eksik.".into()))?;
    let unemployment_worker_rate = statutory.issizlikIsciOraniYuzde.ok_or_else(|| {
        DomainError::ValidationError("İşsizlik işçi oranı snapshot'ı eksik.".into())
    })?;
    if statutory.segments.is_empty() {
        return Err(DomainError::InvalidData(
            "NORMAL statutory parameter segment'i boş.".into(),
        ));
    }
    let statutory_parameters = PayrollStatutoryInputsSnapshot {
        // Replay needs the period's statutory baseline plus the saved dated
        // segments. The resolved period reference can be a later segment's
        // value, which would incorrectly apply that value before its
        // effective date when reconstructing this snapshot.
        gunlukAsgariUcret: settings.gunlukAsgariUcret.ok_or_else(|| {
            DomainError::ValidationError("Günlük asgari ücret snapshot'ı eksik.".into())
        })?,
        sgkIsciOraniYuzde: sgk_worker_rate,
        issizlikIsciOraniYuzde: unemployment_worker_rate,
        pekTavanKatsayisi: settings.pekTavanKatsayisi.ok_or_else(|| {
            DomainError::ValidationError("PEK tavan katsayısı snapshot'ı eksik.".into())
        })?,
        gunlukYemekIstisnasiSGK: settings.gunlukYemekIstisnasiSGK.ok_or_else(|| {
            DomainError::ValidationError("Günlük SGK yemek istisnası eksik.".into())
        })?,
        gunlukYemekIstisnasiGV: settings
            .gunlukYemekIstisnasiGV
            .or(settings.gunlukYemekIstisnasiSGK)
            .ok_or_else(|| {
                DomainError::ValidationError("Günlük GV yemek istisnası eksik.".into())
            })?,
        sgkIsverenOraniYuzde: settings.sgkIsverenOraniYuzde,
        issizlikIsverenOraniYuzde: settings.issizlikIsverenOraniYuzde,
        statutoryParameterSegments: settings
            .statutoryParameterSegments
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|segment| PayrollStatutoryInputSegmentSnapshot {
                effectiveFrom: segment.effectiveFrom,
                gunlukAsgariUcret: segment.gunlukAsgariUcret,
                pekTavanKatsayisi: segment.pekTavanKatsayisi,
                gunlukYemekIstisnasiSGK: segment.gunlukYemekIstisnasiSGK,
                gunlukYemekIstisnasiGV: segment.gunlukYemekIstisnasiGV,
            })
            .collect(),
    };

    let annual = index
        .annual_parameters(dataset, period.taxYear)
        .ok_or_else(|| {
            DomainError::NotFound(format!("{} yıllık parametreleri eksik.", period.taxYear))
        })?;
    let attendance_is_complete = attendance_has_full_calendar_coverage(attendance, period)?;
    let attendance = PayrollAttendanceInputSnapshot {
        coverage: if attendance_is_complete {
            PayrollAttendanceCoverage::COMPLETE
        } else {
            PayrollAttendanceCoverage::PARTIAL
        },
        days: {
            let mut days = attendance
                .gunler
                .iter()
                .map(|(date, code)| PayrollAttendanceDaySnapshot {
                    date: date.clone(),
                    code: code.clone(),
                })
                .collect::<Vec<_>>();
            days.sort_by(|left, right| left.date.cmp(&right.date));
            days
        },
    };
    let mut source_episodes = sick_records
        .iter()
        .map(|record| PayrollSickLeaveEpisodeSnapshot {
            id: record.id.clone(),
            startDate: record.startDate.clone(),
            endDate: record.endDate.clone(),
        })
        .collect::<Vec<_>>();
    source_episodes.sort_by(|left, right| {
        left.startDate
            .cmp(&right.startDate)
            .then_with(|| left.endDate.cmp(&right.endDate))
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(PayrollCalculationSnapshot {
        snapshotSchemaVersion: PAYROLL_SNAPSHOT_SCHEMA_VERSION,
        calculationVersion: PAYROLL_CALCULATION_VERSION.to_string(),
        compensation: ResolvedCompensationSnapshot {
            segments: timeline.resolved_segments().to_vec(),
        },
        usedInputs: PayrollUsedInputSnapshot {
            personnelGroup: person.grup.clone(),
            serviceYear: person.hizmetYili,
            personalDeductions: personal_deductions_snapshot(person),
            attendance: Some(attendance),
            paidSickLeave: Some(PayrollPaidSickLeaveInputSnapshot {
                ruleVersion: PAID_SICK_LEAVE_RULE_VERSION.to_string(),
                sourceEpisodes: source_episodes,
                paidDates: paid_sick_dates
                    .iter()
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .collect(),
            }),
            manualIncome: PayrollManualIncomeInputSnapshot {
                supplied: request.manualIncome.is_some(),
                tediye: payroll.gelirler.tediye,
                tisIkramiyesi: payroll.gelirler.tisIkramiyesi,
            },
            accrualGrossAmount: accrual.grossAmount,
            institutionDeductionSettings: PayrollInstitutionDeductionInputsSnapshot {
                yemekTamGvIstisnaDamgaTabi: settings.yemekTamGvIstisnaDamgaTabi,
                damgaVergisiOraniBinde: settings.damgaVergisiOraniBinde,
                sendikaAidatiYuzde: settings.sendikaAidatiYuzde,
                sabitSendikaAidati: settings.sabitSendikaAidati,
                besOraniYuzde: settings.besOraniYuzde,
                sabitBesTutar: settings.sabitBesTutar,
            },
            statutoryParameters: statutory_parameters,
            annualParameters: PayrollAnnualParametersSnapshot {
                year: annual.year,
                gelirVergisiDilimleri: annual
                    .gelirVergisiDilimleri
                    .iter()
                    .map(|bracket| PayrollTaxBracketSnapshot {
                        limit: bracket.limit,
                        oran: bracket.oran,
                    })
                    .collect(),
                sigortaGvYillikBrutAsgariUcretTavani: annual.sigortaGvYillikBrutAsgariUcretTavani,
            },
            taxHistory: tax_history,
            incomingPekCarry: PayrollIncomingPekCarrySnapshot {
                source: incoming_pek.source,
                sourceId: incoming_pek.source_id.clone(),
                sourcePeriodId: incoming_pek.source_period_id.clone(),
                sourceTaxYear: incoming_pek.source_tax_year,
                sourceTaxMonth: incoming_pek.source_tax_month,
                taxMonthsElapsed: incoming_pek.tax_months_elapsed,
                records: incoming_pek
                    .records
                    .iter()
                    .map(|carry| PayrollPekCarrySnapshot {
                        tutar: carry.tutar,
                        kalanAySayisi: carry.kalanAySayisi,
                        kaynakDonemId: carry.kaynakDonemId.clone(),
                    })
                    .collect(),
            },
        },
    })
}
