//! Canonical financial projection of a persisted NORMAL snapshot.
//!
//! This adapter turns the frozen schema-2 inputs back into the same request
//! consumed by the regular compensation engine. It deliberately does not read
//! mutable current settings or synthesize a policy-version row.

use super::compensation;
use super::*;

pub(crate) fn replay_saved_normal_financial_projection(
    payroll: &BordroKaydi,
    period: &BordroDonemi,
    source_payrolls: &[BordroKaydi],
    source_periods: &[BordroDonemi],
) -> Result<BordroKaydi> {
    let snapshot = payroll
        .calculationMetadata
        .as_ref()
        .and_then(|metadata| metadata.snapshot.as_ref())
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} canonical financial replay cannot run without schema-2 inputs.",
                effective_accrual_id(payroll)
            ))
        })?;
    let inputs = &snapshot.usedInputs;
    let attendance_input = inputs.attendance.as_ref().ok_or_else(|| {
        DomainError::InvalidData("NORMAL replay has no saved daily attendance.".into())
    })?;
    let paid_sick_input = inputs.paidSickLeave.as_ref().ok_or_else(|| {
        DomainError::InvalidData("NORMAL replay has no saved paid sick-leave inputs.".into())
    })?;
    let settings = settings_from_saved_inputs(
        &inputs.institutionDeductionSettings,
        &inputs.statutoryParameters,
        &period.id,
    );
    let personnel = personnel_from_saved_inputs(payroll, inputs);
    let attendance = PersonelPuantaj {
        id: format!("{}_{}", payroll.personelId, period.id),
        personelId: payroll.personelId.clone(),
        donemId: period.id.clone(),
        gunler: attendance_input
            .days
            .iter()
            .map(|day| (day.date.clone(), day.code.clone()))
            .collect(),
    };
    let sick_leave_records = paid_sick_input
        .sourceEpisodes
        .iter()
        .map(|episode| SickLeaveRecord {
            id: episode.id.clone(),
            personnelId: payroll.personelId.clone(),
            startDate: episode.startDate.clone(),
            endDate: episode.endDate.clone(),
            createdAt: None,
            updatedAt: None,
        })
        .collect();
    let annual_parameters = AnnualPayrollParameters {
        year: inputs.annualParameters.year,
        gelirVergisiDilimleri: inputs
            .annualParameters
            .gelirVergisiDilimleri
            .iter()
            .map(|bracket| TaxBracket {
                limit: bracket.limit,
                oran: bracket.oran,
            })
            .collect(),
        sigortaGvYillikBrutAsgariUcretTavani: inputs
            .annualParameters
            .sigortaGvYillikBrutAsgariUcretTavani,
        updatedAt: None,
    };

    // Same-payment-month event details affect the existing PEK reconciliation
    // branch. Use the exact persisted event records and verify that each row
    // still matches the values frozen into this NORMAL snapshot.
    let mut prior_payrolls = Vec::new();
    let mut periods = vec![period.clone()];
    for event in &inputs.taxHistory.previousPaymentEvents {
        if event.taxYear != period.taxYear || event.taxMonth != period.taxMonth {
            continue;
        }
        let source = source_payrolls
            .iter()
            .filter(|row| {
                row.personelId == payroll.personelId
                    && row.donemId == event.periodId
                    && effective_accrual_id(row) == event.accrualId
            })
            .collect::<Vec<_>>();
        let source = match source.as_slice() {
            [source] => *source,
            [] => {
                return Err(DomainError::InvalidData(format!(
                    "{} canonical financial replay is missing same-payment-month event {} ({}); preserve the event row or provide its saved source context.",
                    effective_accrual_id(payroll), event.accrualId, event.periodId
                )))
            }
            _ => {
                return Err(DomainError::InvalidData(format!(
                    "{} canonical financial replay found duplicate same-payment-month event {}.",
                    effective_accrual_id(payroll), event.accrualId
                )))
            }
        };
        validate_saved_prior_event(source, event)?;

        if event.periodId != period.id && !periods.iter().any(|item| item.id == event.periodId) {
            let source_period = source_periods
                .iter()
                .find(|item| item.id == event.periodId)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} canonical replay is missing parent period {} for prior event {}.",
                        effective_accrual_id(payroll),
                        event.periodId,
                        event.accrualId
                    ))
                })?;
            if source_period.taxYear != event.taxYear || source_period.taxMonth != event.taxMonth {
                return Err(DomainError::InvalidData(format!(
                    "{} prior payment event {} tax period differs from its saved parent period.",
                    effective_accrual_id(payroll),
                    event.accrualId
                )));
            }
            periods.push(source_period.clone());
        }
        prior_payrolls.push(source.clone());
    }

    let mut institution_settings = std::collections::HashMap::new();
    institution_settings.insert(period.id.clone(), settings);
    let mut dataset = PayrollDatasetSnapshot {
        personnel: vec![personnel],
        periods,
        institutionSettings: institution_settings,
        attendances: vec![attendance],
        payrolls: prior_payrolls,
        annualPayrollParameters: vec![annual_parameters],
        sickLeaveRecords: sick_leave_records,
        ..PayrollDatasetSnapshot::default()
    };
    dataset.payrolls.sort_by(|left, right| {
        let left_order = (
            left.paymentDate.as_str(),
            left.sequence,
            effective_accrual_id(left),
        );
        let right_order = (
            right.paymentDate.as_str(),
            right.sequence,
            effective_accrual_id(right),
        );
        left_order.cmp(&right_order)
    });

    let manual_income = inputs
        .manualIncome
        .supplied
        .then(|| ManualPayrollIncomeInput {
            tediye: inputs.manualIncome.tediye,
            tisIkramiyesi: inputs.manualIncome.tisIkramiyesi,
        });
    let request = PayrollCalculationRequest {
        personnelId: payroll.personelId.clone(),
        periodId: period.id.clone(),
        calculatedAt: payroll.sonGuncellemeTarihi.clone(),
        manualIncome: manual_income,
        accrual: Some(PayrollAccrualInput {
            accrualId: effective_accrual_id(payroll),
            accrualType: payroll.accrualType,
            paymentDate: payroll.paymentDate.clone(),
            sequence: payroll.sequence,
            grossAmount: inputs.accrualGrossAmount,
            description: payroll.accrualDescription.clone(),
        }),
        dataset,
    };
    let index = PayrollDatasetIndex::build(&request.dataset);
    let timeline = compensation::resolve_compensation_timeline(
        period,
        snapshot.compensation.segments.clone(),
    )?;
    calculate_payroll_with_index_policy_override(&request, &index, Some(&timeline), Some(snapshot))
}

fn settings_from_saved_inputs(
    deductions: &PayrollInstitutionDeductionInputsSnapshot,
    statutory: &PayrollStatutoryInputsSnapshot,
    period_id: &str,
) -> PayrollPeriodInstitutionSettings {
    PayrollPeriodInstitutionSettings {
        donemId: period_id.to_string(),
        yemekTamGvIstisnaDamgaTabi: deductions.yemekTamGvIstisnaDamgaTabi,
        tediyeListesi: None,
        tisIkramiyeListesi: None,
        tediyeTisNotu: None,
        sgkIsciOraniYuzde: Some(statutory.sgkIsciOraniYuzde),
        issizlikIsciOraniYuzde: Some(statutory.issizlikIsciOraniYuzde),
        gelirVergisiOraniYuzde: None,
        damgaVergisiOraniBinde: deductions.damgaVergisiOraniBinde,
        sendikaAidatiYuzde: deductions.sendikaAidatiYuzde,
        sabitSendikaAidati: deductions.sabitSendikaAidati,
        besOraniYuzde: deductions.besOraniYuzde,
        sabitBesTutar: deductions.sabitBesTutar,
        gunlukYemekIstisnasiSGK: Some(statutory.gunlukYemekIstisnasiSGK),
        gunlukYemekIstisnasiGV: Some(statutory.gunlukYemekIstisnasiGV),
        statutoryParameterSegments: Some(
            statutory
                .statutoryParameterSegments
                .iter()
                .map(|segment| StatutoryParameterSegment {
                    effectiveFrom: segment.effectiveFrom.clone(),
                    gunlukAsgariUcret: segment.gunlukAsgariUcret,
                    pekTavanKatsayisi: segment.pekTavanKatsayisi,
                    gunlukYemekIstisnasiSGK: segment.gunlukYemekIstisnasiSGK,
                    gunlukYemekIstisnasiGV: segment.gunlukYemekIstisnasiGV,
                })
                .collect(),
        ),
        statutoryParameterSnapshot: Some(StatutoryParameterSnapshot {
            gunlukAsgariUcret: statutory.gunlukAsgariUcret,
            sgkIsciOraniYuzde: statutory.sgkIsciOraniYuzde,
            issizlikIsciOraniYuzde: statutory.issizlikIsciOraniYuzde,
            pekTavanKatsayisi: statutory.pekTavanKatsayisi,
            gunlukYemekIstisnasiSGK: statutory.gunlukYemekIstisnasiSGK,
            gunlukYemekIstisnasiGV: statutory.gunlukYemekIstisnasiGV,
            statutoryParameterSegments: statutory
                .statutoryParameterSegments
                .iter()
                .map(|segment| StatutoryParameterSegment {
                    effectiveFrom: segment.effectiveFrom.clone(),
                    gunlukAsgariUcret: segment.gunlukAsgariUcret,
                    pekTavanKatsayisi: segment.pekTavanKatsayisi,
                    gunlukYemekIstisnasiSGK: segment.gunlukYemekIstisnasiSGK,
                    gunlukYemekIstisnasiGV: segment.gunlukYemekIstisnasiGV,
                })
                .collect(),
        }),
        pekTavanKatsayisi: Some(statutory.pekTavanKatsayisi),
        gunlukAsgariUcret: Some(statutory.gunlukAsgariUcret),
        sgkIsverenOraniYuzde: statutory.sgkIsverenOraniYuzde,
        issizlikIsverenOraniYuzde: statutory.issizlikIsverenOraniYuzde,
    }
}

fn personnel_from_saved_inputs(
    payroll: &BordroKaydi,
    inputs: &PayrollUsedInputSnapshot,
) -> Personel {
    let kesintiler = inputs
        .personalDeductions
        .as_ref()
        .map(|deductions| PersonelKesintileri {
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
            gvIndirimleri: deductions
                .gvIndirimleri
                .as_ref()
                .map(|gv| GvIndirimGirdileri {
                    dogumAskerlikGvIndirimTutar: gv.dogumAskerlikGvIndirimTutar,
                    hayatSigortasiPrimiTutar: gv.hayatSigortasiPrimiTutar,
                    saglikSigortasiPrimiTutar: gv.saglikSigortasiPrimiTutar,
                }),
        });
    Personel {
        id: payroll.personelId.clone(),
        tcNo: String::new(),
        ad: String::new(),
        soyad: String::new(),
        grup: inputs.personnelGroup.clone(),
        unvan: None,
        sgkSicilNo: String::new(),
        iban: String::new(),
        hizmetYili: inputs.serviceYear,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler,
    }
}

fn validate_saved_prior_event(
    source: &BordroKaydi,
    event: &PayrollPriorPaymentEventSnapshot,
) -> Result<()> {
    let id = effective_accrual_id(source);
    if !matches!(
        source.status,
        BordroStatus::CALCULATED | BordroStatus::FINALIZED
    ) || source.accrualType != event.accrualType
        || source.paymentDate != event.paymentDate
        || source.sequence != event.sequence
    {
        return Err(DomainError::InvalidData(format!(
            "{} saved prior payment event {} identity/provenance differs from its authoritative row.",
            id, event.accrualId
        )));
    }
    for (field, saved, row) in income_fields(&event.income, &source.gelirler) {
        if saved != row {
            return Err(DomainError::InvalidData(format!(
                "{} saved prior event financial input mismatch: income.{field}.",
                id
            )));
        }
    }
    let pek = source.pekDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior event PEK detail is missing.",
            event.accrualId
        ))
    })?;
    for (field, saved, row) in [
        (
            "pekPrimeBase",
            Some(event.pekPrimeBase),
            Some(pek.primMatrahi),
        ),
        (
            "monthPekConsumptionAfter",
            event.monthPekConsumptionAfter,
            pek.aylikSonrasiPekTuketimi,
        ),
        ("pekCeiling", Some(event.pekCeiling), Some(pek.pekUstSinir)),
    ] {
        if saved != row {
            return Err(DomainError::InvalidData(format!(
                "{} saved prior event financial input mismatch: {field}.",
                id
            )));
        }
    }
    let statutory = source.statutorySnapshot.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior event statutory detail is missing.",
            event.accrualId
        ))
    })?;
    if event.mealSgkExemption != statutory.sgkYemekIstisnasiToplam {
        return Err(DomainError::InvalidData(format!(
            "{} saved prior event financial input mismatch: mealSgkExemption.",
            id
        )));
    }
    let gv = source.gvDetay.as_ref().ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} prior event GV detail is missing.",
            event.accrualId
        ))
    })?;
    for (field, saved, row) in [
        ("gvTaxableBase", event.gvTaxableBase, gv.cariGvMatrahi),
        (
            "insuranceGvDeductionUsed",
            event.insuranceGvDeductionUsed,
            gv.uygulanabilirSigortaGvIndirimi,
        ),
        (
            "gvExemptionUsed",
            event.gvExemptionUsed,
            gv.uygulananGvIstisnasi,
        ),
    ] {
        if saved != row {
            return Err(DomainError::InvalidData(format!(
                "{} saved prior event financial input mismatch: {field}.",
                id
            )));
        }
    }
    if let Some(detail) = source.damgaDetay.as_ref() {
        if event.stampExemptionUsed != detail.uygulananDamgaIstisnasi {
            return Err(DomainError::InvalidData(format!(
                "{} saved prior event financial input mismatch: stampExemptionUsed.",
                id
            )));
        }
    }
    let outgoing = source.sonrakiDevredenPek.clone().unwrap_or_default();
    let saved_outgoing = event
        .outgoingPekCarry
        .iter()
        .map(|item| DevredenPekKaydi {
            tutar: item.tutar,
            kalanAySayisi: item.kalanAySayisi,
            kaynakDonemId: item.kaynakDonemId.clone(),
        })
        .collect::<Vec<_>>();
    if outgoing != saved_outgoing {
        return Err(DomainError::InvalidData(format!(
            "{} saved prior event financial input mismatch: outgoingPekCarry.",
            id
        )));
    }
    Ok(())
}

fn income_fields<'a>(
    saved: &'a PayrollIncomeInputsSnapshot,
    row: &'a GelirKalemleri,
) -> [(&'static str, Option<Decimal>, Option<Decimal>); 13] {
    [
        ("tabanBrutAylik", saved.tabanBrutAylik, row.tabanBrutAylik),
        ("tediye", saved.tediye, row.tediye),
        ("tisIkramiyesi", saved.tisIkramiyesi, row.tisIkramiyesi),
        ("ekOdeme", saved.ekOdeme, row.ekOdeme),
        ("yemek", saved.yemek, row.yemek),
        (
            "birlestirilmisSosyalYardim",
            saved.birlestirilmisSosyalYardim,
            row.birlestirilmisSosyalYardim,
        ),
        ("vasitaYol", saved.vasitaYol, row.vasitaYol),
        ("giyimYardimi", saved.giyimYardimi, row.giyimYardimi),
        ("isPrimi", saved.isPrimi, row.isPrimi),
        (
            "geceCalismasiUcreti",
            saved.geceCalismasiUcreti,
            row.geceCalismasiUcreti,
        ),
        (
            "geceCalismasiTatiliUcreti",
            saved.geceCalismasiTatiliUcreti,
            row.geceCalismasiTatiliUcreti,
        ),
        ("hizmetZammi", saved.hizmetZammi, row.hizmetZammi),
        ("digerGelir", saved.digerGelir, row.digerGelir),
    ]
}
