use chrono::{Duration, NaiveDate};
use payroll_core::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::{json, Value};

fn period() -> BordroDonemi {
    BordroDonemi {
        id: "2026-05".into(),
        yil: 2026,
        ay: 5,
        baslangicTarihi: "2026-05-15".into(),
        bitisTarihi: "2026-06-14".into(),
        donemAdi: "Mayıs 2026".into(),
        taxYear: 2026,
        taxMonth: 6,
    }
}

fn state() -> CompensationPolicyState {
    CompensationPolicyState {
        gunlukTabanUcret: dec!(2443.281234567),
        gunlukYemek: dec!(0),
        birlestirilmisSosyalYardim: dec!(0),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiYuzde: Some(dec!(0)),
        isPrimiGruplari: vec![CompensationPolicyGroup {
            id: "g1".into(),
            ad: "Grup 1".into(),
            oran: dec!(10),
            aktif: true,
        }],
        geceCalismaPrimiYuzde: dec!(0),
        geceCalismaTatiliPrimiYuzde: dec!(0),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: None,

        socialBenefitAccrualPolicies: None,
    }
}

fn full_attendance() -> PayrollAttendanceInputSnapshot {
    let start = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 6, 14).unwrap();
    let mut date = start;
    let mut days = Vec::new();
    while date <= end {
        days.push(PayrollAttendanceDaySnapshot {
            date: date.format("%Y-%m-%d").to_string(),
            code: "Ç".into(),
        });
        date += Duration::days(1);
    }
    PayrollAttendanceInputSnapshot {
        coverage: PayrollAttendanceCoverage::COMPLETE,
        days,
    }
}

fn minimum_wage_month_reference(tax_month: i32) -> PayrollMinimumWageMonthInputSnapshot {
    let daily_minimum_wage = dec!(1000);
    let sgk_rate = dec!(14);
    let unemployment_rate = dec!(1);
    PayrollMinimumWageMonthInputSnapshot {
        periodId: format!("2026-{tax_month:02}"),
        taxYear: 2026,
        taxMonth: tax_month,
        dailyMinimumWage: daily_minimum_wage,
        sgkWorkerRatePercent: sgk_rate,
        unemploymentWorkerRatePercent: unemployment_rate,
        monthlyGvBase: calculate_aylik_asgari_ucret_gv_matrahi(
            daily_minimum_wage,
            sgk_rate / dec!(100),
            unemployment_rate / dec!(100),
        ),
    }
}

fn minimum_wage_month_chain(
    start_month: i32,
    end_month_exclusive: i32,
) -> Vec<PayrollMinimumWageMonthInputSnapshot> {
    (start_month..end_month_exclusive)
        .map(minimum_wage_month_reference)
        .collect()
}

fn tax_history_mut(payroll: &mut BordroKaydi) -> &mut PayrollTaxHistoryInputsSnapshot {
    &mut payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .usedInputs
        .taxHistory
}

fn incoming_carry_mut(payroll: &mut BordroKaydi) -> &mut PayrollIncomingPekCarrySnapshot {
    &mut payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .usedInputs
        .incomingPekCarry
}

fn valid_minimum_wage_reference_context() -> (BordroKaydi, BordroDonemi) {
    let (mut payroll, period) = full_context();
    let history = tax_history_mut(&mut payroll);
    history.minimumWageGvOpening = PayrollTaxOpeningSnapshot {
        source: PayrollTaxOpeningSource::EXPLICIT_OPENING,
        openingId: Some("minimum-wage-opening-january".into()),
        effectiveFromTaxYear: 2026,
        effectiveFromTaxMonth: 1,
        amount: Decimal::ZERO,
    };
    history.minimumWageMonthlyReferences = minimum_wage_month_chain(1, 6);
    history.previousCumulativeMinimumWageGv = dec!(127500);
    (payroll, period)
}

fn prior_payment_event() -> PayrollPriorPaymentEventSnapshot {
    PayrollPriorPaymentEventSnapshot {
        periodId: "2026-05".into(),
        accrualId: "prior-payment-event".into(),
        accrualType: AccrualType::NORMAL,
        taxYear: 2026,
        taxMonth: 6,
        paymentDate: "2026-06-05".into(),
        sequence: 0,
        gvTaxableBase: dec!(0.004),
        insuranceGvDeductionUsed: dec!(0.006),
        gvExemptionUsed: dec!(0.006),
        stampExemptionUsed: dec!(0.006),
        pekPrimeBase: dec!(80),
        monthPekConsumptionAfter: Some(dec!(100.006)),
        pekCeiling: dec!(100000),
        mealSgkExemption: Decimal::ZERO,
        income: PayrollIncomeInputsSnapshot {
            tabanBrutAylik: None,
            tediye: None,
            tisIkramiyesi: None,
            ekOdeme: None,
            yemek: None,
            birlestirilmisSosyalYardim: None,
            vasitaYol: None,
            giyimYardimi: None,
            isPrimi: None,
            geceCalismasiUcreti: None,
            geceCalismasiTatiliUcreti: None,
            hizmetZammi: None,
            digerGelir: None,
        },
        outgoingPekCarry: Vec::new(),
    }
}

fn snapshot() -> PayrollCalculationSnapshot {
    PayrollCalculationSnapshot {
        snapshotSchemaVersion: PAYROLL_SNAPSHOT_SCHEMA_VERSION,
        calculationVersion: PAYROLL_CALCULATION_VERSION.into(),
        compensation: ResolvedCompensationSnapshot {
            segments: vec![ResolvedCompensationSegment {
                versionId: "policy-v1".into(),
                startDate: "2026-05-15".into(),
                endDate: "2026-06-14".into(),
                state: state(),
                boundaryScope: CompensationBoundaryScope::FullEarning,
                roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            }],
        },
        usedInputs: PayrollUsedInputSnapshot {
            personnelGroup: "g1".into(),
            serviceYear: 4,
            personalDeductions: None,
            attendance: Some(full_attendance()),
            paidSickLeave: Some(PayrollPaidSickLeaveInputSnapshot {
                ruleVersion: PAID_SICK_LEAVE_RULE_VERSION.into(),
                sourceEpisodes: Vec::new(),
                paidDates: Vec::new(),
            }),
            manualIncome: PayrollManualIncomeInputSnapshot {
                supplied: true,
                tediye: Some(Decimal::ZERO),
                tisIkramiyesi: None,
            },
            accrualGrossAmount: None,
            institutionDeductionSettings: PayrollInstitutionDeductionInputsSnapshot {
                yemekTamGvIstisnaDamgaTabi: true,
                damgaVergisiOraniBinde: Some(dec!(7.59)),
                sendikaAidatiYuzde: Some(dec!(65)),
                sabitSendikaAidati: None,
                besOraniYuzde: Some(dec!(3)),
                sabitBesTutar: None,
            },
            statutoryParameters: PayrollStatutoryInputsSnapshot {
                gunlukAsgariUcret: dec!(1000),
                sgkIsciOraniYuzde: dec!(14),
                issizlikIsciOraniYuzde: dec!(1),
                pekTavanKatsayisi: dec!(7.5),
                gunlukYemekIstisnasiSGK: dec!(0),
                gunlukYemekIstisnasiGV: dec!(0),
                sgkIsverenOraniYuzde: Some(dec!(21.75)),
                issizlikIsverenOraniYuzde: Some(dec!(2)),
                statutoryParameterSegments: Vec::new(),
            },
            annualParameters: PayrollAnnualParametersSnapshot {
                year: 2026,
                gelirVergisiDilimleri: vec![PayrollTaxBracketSnapshot {
                    limit: dec!(1000000),
                    oran: dec!(0.15),
                }],
                sigortaGvYillikBrutAsgariUcretTavani: Some(dec!(100000)),
            },
            taxHistory: PayrollTaxHistoryInputsSnapshot {
                normalGvOpening: PayrollTaxOpeningSnapshot {
                    source: PayrollTaxOpeningSource::IMPLICIT_ZERO,
                    openingId: None,
                    effectiveFromTaxYear: 2026,
                    effectiveFromTaxMonth: 1,
                    amount: Decimal::ZERO,
                },
                minimumWageGvOpening: PayrollTaxOpeningSnapshot {
                    source: PayrollTaxOpeningSource::EXPLICIT_OPENING,
                    openingId: Some("minimum-wage-current-month-opening".into()),
                    effectiveFromTaxYear: 2026,
                    effectiveFromTaxMonth: 6,
                    amount: Decimal::ZERO,
                },
                previousCumulativeGv: Decimal::ZERO,
                previousCumulativeMinimumWageGv: Decimal::ZERO,
                previousAnnualInsuranceGvDeduction: Decimal::ZERO,
                samePaymentMonthPekUsed: Decimal::ZERO,
                samePaymentMonthGvExemptionUsed: Decimal::ZERO,
                samePaymentMonthStampExemptionUsed: Decimal::ZERO,
                previousPaymentEvents: Vec::new(),
                minimumWageMonthlyReferences: Vec::new(),
            },
            incomingPekCarry: PayrollIncomingPekCarrySnapshot {
                source: PayrollPekCarrySource::NONE,
                sourceId: None,
                sourcePeriodId: None,
                sourceTaxYear: None,
                sourceTaxMonth: None,
                taxMonthsElapsed: 0,
                records: Vec::new(),
            },
        },
    }
}

fn current_payroll(status: BordroStatus) -> BordroKaydi {
    BordroKaydi {
        id: "person-1_2026-05".into(),
        personelId: "person-1".into(),
        donemId: "2026-05".into(),
        accrualId: "person-1_2026-05".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-06-10".into(),
        sequence: 0,
        accrualDescription: None,
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri::default(),
        gelirToplam: Decimal::ZERO,
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: Decimal::ZERO,
        netOdeme: Decimal::ZERO,
        status,
        olusturulmaTarihi: "2026-06-10T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-06-10T00:00:00Z".into(),
        notlar: None,
        oncekiKumulatifGvMatrahi: Some(Decimal::ZERO),
        oncekiKumulatifAsgariGvMatrahi: Some(Decimal::ZERO),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: Some(Vec::new()),
        sonrakiDevredenPek: Some(Vec::new()),
        pekDetay: Some(PekDetayi {
            hesaplananPek: Decimal::ZERO,
            hamPek: Decimal::ZERO,
            devredenPekKullanilan: Decimal::ZERO,
            primMatrahi: Decimal::ZERO,
            aylikOncekiPekTuketimi: Some(Decimal::ZERO),
            aylikSonrasiPekTuketimi: Some(Decimal::ZERO),
            finalPek: Decimal::ZERO,
            devredenPekAşanTutar: Decimal::ZERO,
            pekAltSinir: Decimal::ZERO,
            pekUstSinir: Decimal::ZERO,
            altSinirTamamlamaFarki: Decimal::ZERO,
            fiiliYemekGunu: 0,
            yemekIstisnasiTutar: Decimal::ZERO,
            isverenSgkPrimi: Some(Decimal::ZERO),
            isverenIssizlikPrimi: Some(Decimal::ZERO),
            pekAltSinirTamamlamaIsverenPrimi: Some(Decimal::ZERO),
            isverenPrimToplami: Some(Decimal::ZERO),
            sgkIsverenOraniYuzde: Some(dec!(21.75)),
            isverenIssizlikOraniYuzde: Some(dec!(2)),
        }),
        isPrimiDetay: Some(IsPrimiHesapDetayi {
            grupId: "g1".into(),
            grupAd: "Grup 1".into(),
            oran: dec!(10),
            hakGunu: 0,
            gunlukIsPrimi: Decimal::ZERO,
            tutar: Decimal::ZERO,
        }),
        gvDetay: Some(GvHesapDetayi {
            oncekiKumulatifGvMatrahi: Decimal::ZERO,
            cariGvMatrahi: Decimal::ZERO,
            yeniKumulatifGvMatrahi: Decimal::ZERO,
            brutGelirVergisi: Decimal::ZERO,
            asgariUcretGvMatrahi: Decimal::ZERO,
            asgariUcretReferansKumulatifMatrahi: Decimal::ZERO,
            asgariUcretGvIstisnasi: Decimal::ZERO,
            ayniAyOncekiKullanilanGvIstisnasi: Decimal::ZERO,
            tahakkukOncesiKalanGvIstisnasi: Decimal::ZERO,
            uygulananGvIstisnasi: Decimal::ZERO,
            tahakkukSonrasiKalanGvIstisnasi: Decimal::ZERO,
            kesilenGelirVergisi: Decimal::ZERO,
            dogumAskerlikGvIndirimi: Decimal::ZERO,
            sigortaGvIndirimAdayi: Decimal::ZERO,
            sigortaGvAylikLimiti: Decimal::ZERO,
            sigortaGvYillikKalanLimiti: Decimal::ZERO,
            uygulanabilirSigortaGvIndirimi: Decimal::ZERO,
        }),
        persistedGvBase: Some(Decimal::ZERO),
        damgaDetay: Some(DamgaVergisiHesapDetayi::default()),
        statutorySnapshot: Some(ResolvedStatutorySnapshot {
            source: StatutorySnapshotSource::AttendanceBacked,
            segments: vec![ResolvedStatutorySegmentSnapshot {
                effectiveFrom: "2026-05-15".into(),
                effectiveTo: "2026-06-14".into(),
                sgkPrimGunSayisi: 30,
                fiiliYemekGunu: 0,
                gunlukAsgariUcret: dec!(1000),
                pekTavanKatsayisi: dec!(7.5),
                gunlukYemekIstisnasiSGK: Decimal::ZERO,
                gunlukYemekIstisnasiGV: Decimal::ZERO,
            }],
            sgkPrimGunSayisi: 30,
            pekAltSinir: Decimal::ZERO,
            pekUstSinir: Decimal::ZERO,
            sgkYemekIstisnasiToplam: Decimal::ZERO,
            gvYemekIstisnasiToplam: Decimal::ZERO,
            gvReferansGunlukAsgariUcret: dec!(1000),
            sgkIsciOraniYuzde: Some(dec!(14)),
            issizlikIsciOraniYuzde: Some(dec!(1)),
        }),
        odenenRaporluGun: Some(0),
        raporluGun: Some(0),
        calculationMetadata: Some(PayrollCalculationMetadata {
            provenance: PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE,
            snapshot: Some(snapshot()),
        }),
    }
}

fn full_context() -> (BordroKaydi, BordroDonemi) {
    (current_payroll(BordroStatus::CALCULATED), period())
}

#[test]
fn validates_a_current_normal_snapshot_with_scoped_inputs() {
    let (payroll, period) = full_context();
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn current_month_minimum_wage_opening_allows_an_empty_prior_reference_range() {
    let (payroll, period) = full_context();
    let history = &payroll
        .calculationMetadata
        .as_ref()
        .unwrap()
        .snapshot
        .as_ref()
        .unwrap()
        .usedInputs
        .taxHistory;
    assert_eq!(
        history.minimumWageGvOpening.effectiveFromTaxMonth,
        period.taxMonth
    );
    assert!(history.minimumWageMonthlyReferences.is_empty());
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn requires_every_minimum_wage_reference_month_and_matches_its_statutory_formula() {
    let (payroll, period) = valid_minimum_wage_reference_context();
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    for missing_index in [0, 2, 4] {
        let (mut missing, period) = valid_minimum_wage_reference_context();
        tax_history_mut(&mut missing)
            .minimumWageMonthlyReferences
            .remove(missing_index);
        assert!(matches!(
            validate_payroll_calculation_metadata_structure(&missing, &period),
            Err(PayrollCalculationSnapshotError::InvalidTaxHistory(_))
        ));
    }

    let (mut duplicate, period) = valid_minimum_wage_reference_context();
    let history = tax_history_mut(&mut duplicate);
    history.minimumWageMonthlyReferences[2] = history.minimumWageMonthlyReferences[1].clone();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&duplicate, &period),
        Err(PayrollCalculationSnapshotError::InvalidTaxHistory(_))
    ));

    let (mut current_month, period) = valid_minimum_wage_reference_context();
    tax_history_mut(&mut current_month).minimumWageMonthlyReferences[4] =
        minimum_wage_month_reference(period.taxMonth);
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&current_month, &period),
        Err(PayrollCalculationSnapshotError::InvalidTaxHistory(_))
    ));

    let (mut wrong_formula, period) = valid_minimum_wage_reference_context();
    let history = tax_history_mut(&mut wrong_formula);
    history.minimumWageMonthlyReferences[2].monthlyGvBase += dec!(0.01);
    history.previousCumulativeMinimumWageGv += dec!(0.01);
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&wrong_formula, &period),
        Err(PayrollCalculationSnapshotError::InvalidTaxHistory(message))
            if message.contains("monthly GV reference")
    ));
}

#[test]
fn explicit_minimum_wage_opening_validates_only_months_from_its_start_anchor() {
    let (mut payroll, period) = full_context();
    let history = tax_history_mut(&mut payroll);
    history.minimumWageGvOpening = PayrollTaxOpeningSnapshot {
        source: PayrollTaxOpeningSource::EXPLICIT_OPENING,
        openingId: Some("minimum-wage-opening-march".into()),
        effectiveFromTaxYear: 2026,
        effectiveFromTaxMonth: 3,
        amount: dec!(10.126),
    };
    history.minimumWageMonthlyReferences = minimum_wage_month_chain(3, 6);
    history.previousCumulativeMinimumWageGv = dec!(76510.13);

    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn accepts_none_and_sourced_zero_pek_carry_states() {
    let (none, period) = full_context();
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&none, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    for source in [
        PayrollPekCarrySource::PAYMENT_EVENT,
        PayrollPekCarrySource::RETRO_OVERRIDE,
    ] {
        for records in [
            Vec::new(),
            vec![PayrollPekCarrySnapshot {
                tutar: Decimal::ZERO,
                kalanAySayisi: 0,
                kaynakDonemId: Some("2026-06".into()),
            }],
        ] {
            let (mut payroll, period) = full_context();
            *incoming_carry_mut(&mut payroll) = PayrollIncomingPekCarrySnapshot {
                source,
                sourceId: Some("source-event-1".into()),
                sourcePeriodId: Some("2026-06".into()),
                sourceTaxYear: Some(2026),
                sourceTaxMonth: Some(6),
                taxMonthsElapsed: 0,
                records,
            };
            assert_eq!(
                validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
                PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
            );
        }
    }
}

#[test]
fn rejects_missing_or_orphaned_pek_carry_source_components() {
    for missing_component in 0..4 {
        let (mut payroll, period) = full_context();
        let carry = incoming_carry_mut(&mut payroll);
        carry.source = PayrollPekCarrySource::PAYMENT_EVENT;
        carry.sourceId = Some("source-event-1".into());
        carry.sourcePeriodId = Some("2026-06".into());
        carry.sourceTaxYear = Some(2026);
        carry.sourceTaxMonth = Some(6);
        match missing_component {
            0 => carry.sourceId = None,
            1 => carry.sourcePeriodId = None,
            2 => carry.sourceTaxYear = None,
            _ => carry.sourceTaxMonth = None,
        }
        assert!(matches!(
            validate_payroll_calculation_metadata_structure(&payroll, &period),
            Err(PayrollCalculationSnapshotError::InvalidPekCarry(_))
        ));
    }

    for orphaned_component in 0..4 {
        let (mut payroll, period) = full_context();
        let carry = incoming_carry_mut(&mut payroll);
        match orphaned_component {
            0 => carry.sourceId = Some("orphan-source".into()),
            1 => carry.sourcePeriodId = Some("2026-06".into()),
            2 => carry.sourceTaxYear = Some(2026),
            _ => carry.sourceTaxMonth = Some(6),
        }
        assert!(matches!(
            validate_payroll_calculation_metadata_structure(&payroll, &period),
            Err(PayrollCalculationSnapshotError::InvalidPekCarry(_))
        ));
    }

    let (mut nonempty_none, period) = full_context();
    incoming_carry_mut(&mut nonempty_none)
        .records
        .push(PayrollPekCarrySnapshot {
            tutar: dec!(100),
            kalanAySayisi: 2,
            kaynakDonemId: Some("2026-05".into()),
        });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&nonempty_none, &period),
        Err(PayrollCalculationSnapshotError::InvalidPekCarry(_))
    ));

    let (mut elapsed_none, period) = full_context();
    incoming_carry_mut(&mut elapsed_none).taxMonthsElapsed = 1;
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&elapsed_none, &period),
        Err(PayrollCalculationSnapshotError::InvalidPekCarry(_))
    ));
}

#[test]
fn sourced_pek_carry_requires_canonical_ids_and_a_valid_elapsed_month_anchor() {
    for invalidity in 0..6 {
        let (mut payroll, period) = full_context();
        let carry = incoming_carry_mut(&mut payroll);
        carry.source = PayrollPekCarrySource::RETRO_OVERRIDE;
        carry.sourceId = Some("source-event-1".into());
        carry.sourcePeriodId = Some("2026-06".into());
        carry.sourceTaxYear = Some(2026);
        carry.sourceTaxMonth = Some(6);
        match invalidity {
            0 => carry.sourceId = Some("  ".into()),
            1 => carry.sourcePeriodId = Some("2026-06 ".into()),
            2 => carry.sourceTaxYear = Some(2027),
            3 => carry.sourceTaxMonth = Some(13),
            4 => carry.taxMonthsElapsed = 1,
            _ => carry.sourceTaxYear = Some(0),
        }
        assert!(matches!(
            validate_payroll_calculation_metadata_structure(&payroll, &period),
            Err(PayrollCalculationSnapshotError::InvalidPekCarry(_))
        ));
    }

    let (mut prior_year_source, period) = full_context();
    *incoming_carry_mut(&mut prior_year_source) = PayrollIncomingPekCarrySnapshot {
        source: PayrollPekCarrySource::PAYMENT_EVENT,
        sourceId: Some("prior-year-event".into()),
        sourcePeriodId: Some("2025-12".into()),
        sourceTaxYear: Some(2025),
        sourceTaxMonth: Some(12),
        taxMonthsElapsed: 6,
        records: Vec::new(),
    };
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&prior_year_source, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );
}

#[test]
fn cumulative_tax_reconciliation_matches_engine_rounding_without_rounding_sources() {
    let (mut payroll, period) = valid_minimum_wage_reference_context();
    let history = tax_history_mut(&mut payroll);
    history.normalGvOpening = PayrollTaxOpeningSnapshot {
        source: PayrollTaxOpeningSource::EXPLICIT_OPENING,
        openingId: Some("normal-gv-opening".into()),
        effectiveFromTaxYear: 2026,
        effectiveFromTaxMonth: 1,
        amount: dec!(1.234),
    };
    history.minimumWageGvOpening.amount = dec!(1.236);
    history.previousCumulativeGv = dec!(1.24);
    history.previousCumulativeMinimumWageGv = dec!(127501.24);
    history.previousAnnualInsuranceGvDeduction = dec!(0.01);
    history.samePaymentMonthPekUsed = dec!(100.01);
    history.samePaymentMonthGvExemptionUsed = dec!(0.01);
    history.samePaymentMonthStampExemptionUsed = dec!(0.01);
    history.previousPaymentEvents = vec![prior_payment_event()];

    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    let (mut exact_opening, period) = full_context();
    let history = tax_history_mut(&mut exact_opening);
    history.normalGvOpening = PayrollTaxOpeningSnapshot {
        source: PayrollTaxOpeningSource::EXPLICIT_OPENING,
        openingId: Some("normal-gv-high-precision-opening".into()),
        effectiveFromTaxYear: 2026,
        effectiveFromTaxMonth: 1,
        amount: dec!(1.234),
    };
    history.previousCumulativeGv = dec!(1.23);
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&exact_opening, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    let (mut wrong_normal, period) = (payroll.clone(), period.clone());
    tax_history_mut(&mut wrong_normal).previousCumulativeGv = dec!(1.23);
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&wrong_normal, &period),
        Err(PayrollCalculationSnapshotError::InvalidTaxHistory(_))
    ));

    let (mut wrong_minimum, period) = (payroll.clone(), period.clone());
    tax_history_mut(&mut wrong_minimum).previousCumulativeMinimumWageGv = dec!(127501.23);
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&wrong_minimum, &period),
        Err(PayrollCalculationSnapshotError::InvalidTaxHistory(_))
    ));
}

#[test]
fn snapshot_financial_values_are_decimal_strings_with_explicit_null_zero_and_exact_scale() {
    let snapshot = snapshot();
    let serialized = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        serialized["compensation"]["segments"][0]["state"]["gunlukTabanUcret"],
        "2443.281234567"
    );
    assert_eq!(serialized["usedInputs"]["manualIncome"]["tediye"], "0");
    assert!(serialized["usedInputs"]["manualIncome"]["tisIkramiyesi"].is_null());

    let restored: PayrollCalculationSnapshot = serde_json::from_value(serialized).unwrap();
    assert_eq!(
        restored.compensation.segments[0].state.gunlukTabanUcret,
        dec!(2443.281234567)
    );
    assert_eq!(restored.usedInputs.manualIncome.tediye, Some(Decimal::ZERO));
    assert_eq!(restored.usedInputs.manualIncome.tisIkramiyesi, None);
}

#[test]
fn snapshot_financial_boundary_rejects_numeric_and_missing_required_nullable_values() {
    let numeric = json!({ "supplied": true, "tediye": 0, "tisIkramiyesi": null });
    assert!(serde_json::from_value::<PayrollManualIncomeInputSnapshot>(numeric).is_err());

    let missing = json!({ "supplied": true, "tisIkramiyesi": null });
    assert!(serde_json::from_value::<PayrollManualIncomeInputSnapshot>(missing).is_err());

    let explicit_null = json!({ "supplied": true, "tediye": null, "tisIkramiyesi": null });
    let parsed: PayrollManualIncomeInputSnapshot = serde_json::from_value(explicit_null).unwrap();
    assert_eq!(parsed.tediye, None);
}

#[test]
fn rejects_unsupported_schema_and_calculation_versions_explicitly() {
    let (mut payroll, period) = full_context();
    payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .snapshotSchemaVersion = 1;
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&payroll, &period),
        Err(PayrollCalculationSnapshotError::UnsupportedSchemaVersion(1))
    ));

    let (mut payroll, period) = full_context();
    payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .calculationVersion = "future-math".into();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&payroll, &period),
        Err(PayrollCalculationSnapshotError::UnsupportedCalculationVersion(version)) if version == "future-math"
    ));
}

#[test]
fn rejects_segment_gaps_overlaps_out_of_order_and_out_of_period_ranges() {
    let (mut gap, period) = full_context();
    let segments = &mut gap
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].endDate = "2026-05-31".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v2".into(),
        startDate: "2026-06-02".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&gap, &period),
        Err(PayrollCalculationSnapshotError::SegmentGap(_))
    ));

    let (mut overlap, period) = full_context();
    let segments = &mut overlap
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].endDate = "2026-05-31".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v2".into(),
        startDate: "2026-05-31".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&overlap, &period),
        Err(PayrollCalculationSnapshotError::SegmentOverlap(_))
    ));

    let (mut out_of_order, period) = full_context();
    let segments = &mut out_of_order
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].startDate = "2026-05-15".into();
    segments[0].endDate = "2026-05-19".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v2".into(),
        startDate: "2026-05-20".into(),
        endDate: "2026-06-01".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v3".into(),
        startDate: "2026-05-18".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&out_of_order, &period),
        Err(PayrollCalculationSnapshotError::SegmentOutOfOrder(_))
    ));

    let (mut out_of_period, period) = full_context();
    out_of_period
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments[0]
        .endDate = "2026-06-15".into();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&out_of_period, &period),
        Err(PayrollCalculationSnapshotError::SegmentOutOfPeriod(_))
    ));

    let (mut reversed, period) = full_context();
    let segment = &mut reversed
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments[0];
    segment.startDate = "2026-06-01".into();
    segment.endDate = "2026-05-31".into();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&reversed, &period),
        Err(PayrollCalculationSnapshotError::InvalidSegment(_, _))
    ));
}

#[test]
fn rejects_repeated_compensation_version_ids_in_resolved_segments() {
    let (mut payroll, period) = full_context();
    let segments = &mut payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].endDate = "2026-05-31".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v1".into(),
        startDate: "2026-06-01".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });

    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&payroll, &period),
        Err(PayrollCalculationSnapshotError::IdentityMismatch(_))
    ));
}

#[test]
fn snapshot_boundary_flags_validate_clipped_baseline_scope_state_and_mode() {
    let (mut baseline, period) = full_context();
    baseline
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments[0]
        .boundaryScope = CompensationBoundaryScope::RoadOnly;
    validate_payroll_calculation_metadata_structure(&baseline, &period)
        .expect("period-start clipped ROAD_ONLY is a natural baseline");

    let (mut contradictory, period) = full_context();
    let segments = &mut contradictory
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].endDate = "2026-05-31".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v2".into(),
        startDate: "2026-06-01".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::RoadOnly,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
    });
    segments[1].state.gunlukYemek += Decimal::ONE;
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&contradictory, &period),
        Err(PayrollCalculationSnapshotError::InvalidSegment(1, _))
    ));

    let (mut mixed_mode, period) = full_context();
    let segments = &mut mixed_mode
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .compensation
        .segments;
    segments[0].endDate = "2026-05-31".into();
    segments.push(ResolvedCompensationSegment {
        versionId: "policy-v2".into(),
        startDate: "2026-06-01".into(),
        endDate: "2026-06-14".into(),
        state: state(),
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PeriodTotal,
    });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&mixed_mode, &period),
        Err(PayrollCalculationSnapshotError::InvalidSegment(1, _))
    ));
}

#[test]
fn rejects_daily_attendance_and_paid_sick_date_mismatches() {
    let (mut payroll, period) = full_context();
    let inputs = &mut payroll
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .usedInputs;
    let attendance = inputs.attendance.as_mut().unwrap();
    let sick_date = "2026-05-20";
    attendance
        .days
        .iter_mut()
        .find(|day| day.date == sick_date)
        .unwrap()
        .code = "R".into();
    let next_paid_date = "2026-05-21";
    attendance
        .days
        .iter_mut()
        .find(|day| day.date == next_paid_date)
        .unwrap()
        .code = "R".into();
    inputs.paidSickLeave = Some(PayrollPaidSickLeaveInputSnapshot {
        ruleVersion: PAID_SICK_LEAVE_RULE_VERSION.into(),
        sourceEpisodes: vec![PayrollSickLeaveEpisodeSnapshot {
            id: "episode-1".into(),
            startDate: sick_date.into(),
            endDate: "2026-05-22".into(),
        }],
        paidDates: vec![sick_date.into(), next_paid_date.into()],
    });
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
    );

    let (mut mismatch, period) = full_context();
    let inputs = &mut mismatch
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .usedInputs;
    let attendance = inputs.attendance.as_mut().unwrap();
    attendance
        .days
        .iter_mut()
        .find(|day| day.date == sick_date)
        .unwrap()
        .code = "G".into();
    attendance
        .days
        .iter_mut()
        .find(|day| day.date == next_paid_date)
        .unwrap()
        .code = "R".into();
    inputs.paidSickLeave = Some(PayrollPaidSickLeaveInputSnapshot {
        ruleVersion: PAID_SICK_LEAVE_RULE_VERSION.into(),
        sourceEpisodes: vec![PayrollSickLeaveEpisodeSnapshot {
            id: "episode-1".into(),
            startDate: sick_date.into(),
            endDate: "2026-05-22".into(),
        }],
        paidDates: vec![sick_date.into(), next_paid_date.into()],
    });
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&mismatch, &period),
        Err(PayrollCalculationSnapshotError::PaidSickLeaveMismatch(_))
    ));

    let (mut bad_coverage, period) = full_context();
    let attendance = bad_coverage
        .calculationMetadata
        .as_mut()
        .unwrap()
        .snapshot
        .as_mut()
        .unwrap()
        .usedInputs
        .attendance
        .as_mut()
        .unwrap();
    attendance.days.pop();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&bad_coverage, &period),
        Err(PayrollCalculationSnapshotError::InvalidAttendance(_))
    ));
}

#[test]
fn validates_parent_period_tax_and_payment_identity() {
    let (mut payroll, period) = full_context();
    payroll.donemId = "2026-04".into();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&payroll, &period),
        Err(PayrollCalculationSnapshotError::IdentityMismatch(_))
    ));

    let (mut payroll, period) = full_context();
    payroll.paymentDate = "2026-05-10".into();
    assert!(matches!(
        validate_payroll_calculation_metadata_structure(&payroll, &period),
        Err(PayrollCalculationSnapshotError::InvalidContext(_))
    ));
}

#[test]
fn preserves_legacy_and_draft_while_gating_explicit_current_normal_records() {
    let (mut payroll, period) = full_context();
    payroll.calculationMetadata = None;
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&payroll, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::Legacy
    );
    let serialized = serde_json::to_value(&payroll).unwrap();
    assert!(serialized.get("calculationMetadata").is_none());

    let (mut draft, period) = full_context();
    draft.status = BordroStatus::DRAFT;
    draft.calculationMetadata.as_mut().unwrap().snapshot = None;
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&draft, &period).unwrap(),
        PayrollCalculationSnapshotDisposition::SnapshotNotRequiredYet
    );

    let (mut calculated, period) = full_context();
    calculated.calculationMetadata.as_mut().unwrap().snapshot = None;
    assert_eq!(
        validate_payroll_calculation_metadata_structure(&calculated, &period),
        Err(PayrollCalculationSnapshotError::MissingRequiredSnapshot)
    );
}

#[test]
fn generated_typescript_decimal_contract_includes_snapshot_fields() {
    let snapshot = snapshot();
    let value: Value = serde_json::to_value(snapshot).unwrap();
    assert_eq!(
        value["usedInputs"]["statutoryParameters"]["gunlukAsgariUcret"],
        "1000"
    );
    assert_eq!(
        value["usedInputs"]["annualParameters"]["gelirVergisiDilimleri"][0]["oran"],
        "0.15"
    );
}

#[test]
fn calculation_snapshot_enum_json_tokens_round_trip_as_canonical_uppercase_literals() {
    fn assert_token<T>(token: &str, expected: T)
    where
        T: serde::de::DeserializeOwned + serde::Serialize + PartialEq + std::fmt::Debug,
    {
        let decoded: T =
            serde_json::from_value(json!(token)).expect("canonical enum token deserializes");
        assert_eq!(decoded, expected);
        assert_eq!(
            serde_json::to_string(&decoded).expect("canonical enum token serializes"),
            format!("\"{token}\"")
        );
    }

    assert_token(
        "COMPENSATION_POLICY_ENGINE",
        PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE,
    );
    assert_token("COMPLETE", PayrollAttendanceCoverage::COMPLETE);
    assert_token("PARTIAL", PayrollAttendanceCoverage::PARTIAL);
    assert_token(
        "EXPLICIT_OPENING",
        PayrollTaxOpeningSource::EXPLICIT_OPENING,
    );
    assert_token(
        "LEGACY_PERSONNEL_VALUE",
        PayrollTaxOpeningSource::LEGACY_PERSONNEL_VALUE,
    );
    assert_token("IMPLICIT_ZERO", PayrollTaxOpeningSource::IMPLICIT_ZERO);
    assert_token("NONE", PayrollPekCarrySource::NONE);
    assert_token("PAYMENT_EVENT", PayrollPekCarrySource::PAYMENT_EVENT);
    assert_token("RETRO_OVERRIDE", PayrollPekCarrySource::RETRO_OVERRIDE);
}

#[test]
fn shared_v6_current_engine_fixture_replays_its_full_financial_projection() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/v6-current-engine-snapshot.json"))
            .expect("shared V6 fixture should be valid JSON");
    let periods: Vec<BordroDonemi> = serde_json::from_value(fixture["donemler"].clone())
        .expect("shared periods should deserialize");
    let payrolls: Vec<BordroKaydi> = serde_json::from_value(fixture["bordrolar"].clone())
        .expect("shared payrolls should deserialize");
    let policies: Vec<CompensationPolicyVersion> =
        serde_json::from_value(fixture["compensationPolicyVersions"].clone())
            .expect("shared policy timeline should deserialize");
    let ordered_policies = validate_compensation_policy_timeline(&policies).unwrap();

    for payroll in &payrolls {
        let period = periods
            .iter()
            .find(|period| period.id == payroll.donemId)
            .expect("shared payroll should have a parent period");
        assert_eq!(
            validate_payroll_calculation_metadata_with_context(
                payroll, period, &payrolls, &periods,
            )
            .expect("shared V6 payroll should pass full financial replay"),
            PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
        );
        validate_compensation_snapshot_policy_links(payroll, &ordered_policies)
            .expect("shared snapshot should link to its immutable policy timeline");
    }

    assert_eq!(
        fixture["bordrolar"][0]["calculationMetadata"]["snapshot"]["usedInputs"]["taxHistory"]
            ["previousPaymentEvents"][0]["gvTaxableBase"],
        "0.004"
    );
}
