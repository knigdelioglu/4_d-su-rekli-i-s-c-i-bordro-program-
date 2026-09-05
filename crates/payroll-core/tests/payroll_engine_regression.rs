use chrono::{Duration, NaiveDate};
use payroll_core::{
    calculate_incremental_prime_esas_kazanc, calculate_payroll, calculate_prime_esas_kazanc,
    finalize_payroll, validate_payroll_request, AccrualType, AnnualPayrollParameters, BordroDonemi,
    BordroStatus, DevredenPekKaydi, DonemselKurumDegerleri, GelirKalemleri,
    ManualPayrollIncomeInput, PayrollAccrualInput, PayrollCalculationRequest,
    PayrollDatasetSnapshot, Personel, PersonelPuantaj, PuantajOzeti,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

fn fixture_request() -> PayrollCalculationRequest {
    let period = BordroDonemi {
        id: "2026-01".into(),
        yil: 2026,
        ay: 1,
        baslangicTarihi: "2026-01-15".into(),
        bitisTarihi: "2026-02-14".into(),
        donemAdi: "Ocak 2026".into(),
        taxYear: 2026,
        taxMonth: 2,
    };

    let mut gunler = HashMap::new();
    let mut date = NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid fixture date");
    let end = NaiveDate::from_ymd_opt(2026, 2, 14).expect("valid fixture date");
    while date <= end {
        gunler.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }

    let settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        ..Default::default()
    };

    PayrollCalculationRequest {
        personnelId: "person-1".into(),
        periodId: period.id.clone(),
        calculatedAt: "2026-09-03T00:00:00.000Z".into(),
        manualIncome: Some(ManualPayrollIncomeInput {
            tediye: Some(dec!(75000.10)),
            tisIkramiyesi: Some(dec!(0.15)),
        }),
        accrual: None,
        dataset: PayrollDatasetSnapshot {
            personnel: vec![Personel {
                id: "person-1".into(),
                tcNo: "10000000000".into(),
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
            }],
            periods: vec![period],
            institutionSettings: HashMap::from([("2026-01".into(), settings)]),
            attendances: vec![PersonelPuantaj {
                id: "person-1_2026-01".into(),
                personelId: "person-1".into(),
                donemId: "2026-01".into(),
                gunler,
            }],
            payrolls: Vec::new(),
            taxOpenings: Vec::new(),
            sickLeaveRecords: Vec::new(),
            annualPayrollParameters: vec![AnnualPayrollParameters::default_for_2026()],
            zamAylari: Vec::new(),
        },
    }
}

fn explicit_normal_request(payment_date: &str) -> PayrollCalculationRequest {
    let mut request = fixture_request();
    request.manualIncome = None;
    request.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-01".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: payment_date.into(),
        sequence: 0,
        grossAmount: None,
        description: Some("Normal maaş".into()),
    });
    request
}

fn supplementary_request(payment_date: &str) -> PayrollCalculationRequest {
    let mut request = fixture_request();
    request.manualIncome = None;
    request.accrual = Some(PayrollAccrualInput {
        accrualId: format!("person-1_2026-01_tediye_{payment_date}"),
        accrualType: AccrualType::TEDIYE,
        paymentDate: payment_date.into(),
        sequence: 1,
        grossAmount: Some(dec!(2000)),
        description: Some("Tediye".into()),
    });
    request
}

#[test]
fn calculation_uses_shared_engine_and_preserves_manual_decimal_values() {
    let request = fixture_request();
    let result = calculate_payroll(&request).expect("fixture should calculate");

    assert_eq!(result.status, BordroStatus::CALCULATED);
    assert_eq!(result.gelirler.tediye, Some(dec!(75000.10)));
    assert_eq!(result.gelirler.tisIkramiyesi, Some(dec!(0.15)));
    assert_eq!(
        result.netOdeme,
        (result.gelirToplam - result.kesintiToplam).round_dp(2)
    );
}

#[test]
fn supplementary_is_independent_and_only_prior_events_must_be_authoritative() {
    calculate_payroll(&supplementary_request("2026-02-10")).expect("no NORMAL required");
    for status in [BordroStatus::DRAFT, BordroStatus::STALE] {
        let mut request = explicit_normal_request("2026-02-14");
        let mut prior = calculate_payroll(&supplementary_request("2026-02-10")).unwrap();
        prior.status = status;
        request.dataset.payrolls.push(prior);
        let error = calculate_payroll(&request).unwrap_err().to_string();
        assert!(error.contains("authoritative"));
        assert!(!error.contains("normal maaş"));
    }
    let mut earlier = supplementary_request("2026-02-10");
    earlier.dataset.payrolls.push(calculate_payroll(&explicit_normal_request("2026-02-14")).unwrap());
    calculate_payroll(&earlier).expect("later NORMAL is not a prerequisite");
}

#[test]
fn supplementary_events_calculate_without_attendance_but_normal_still_fails_closed() {
    for (sequence, accrual_type) in [
        (0, AccrualType::TEDIYE),
        (1, AccrualType::TIS_IKRAMIYE),
        (2, AccrualType::SUPPLEMENTAL),
    ] {
        let mut request = fixture_request();
        request.manualIncome = None;
        request.dataset.attendances.clear();
        request.accrual = Some(PayrollAccrualInput {
            accrualId: format!("person-1_supplementary_{sequence}"),
            accrualType: accrual_type,
            paymentDate: "2026-02-10".into(),
            sequence,
            grossAmount: Some(dec!(2000)),
            description: None,
        });

        let payroll = calculate_payroll(&request).expect("supplementary event is attendance-independent");
        assert_eq!(payroll.gelirToplam, dec!(2000));
        assert_eq!(payroll.puantajOzeti.c, 0);
        assert_eq!(payroll.puantajOzeti.t, 0);
        assert_eq!(payroll.puantajOzeti.r, 0);
        assert_eq!(payroll.odenenRaporluGun, Some(0));
        assert_eq!(payroll.raporluGun, Some(0));
        assert_eq!(payroll.statutorySnapshot.as_ref().unwrap().sgkPrimGunSayisi, 30);
        assert!(payroll.pekDetay.as_ref().unwrap().primMatrahi > Decimal::ZERO);
    }

    let mut normal_request = explicit_normal_request("2026-02-14");
    normal_request.dataset.attendances.clear();
    let error = calculate_payroll(&normal_request).expect_err("NORMAL still requires attendance");
    assert!(error.to_string().contains("puantaj"));
}

#[test]
fn normal_after_attendance_free_tediye_reuses_payment_event_state() {
    let mut tediye_request = supplementary_request("2026-02-10");
    tediye_request.dataset.attendances.clear();
    tediye_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(1000000));
    let tediye = calculate_payroll(&tediye_request).expect("attendance-free tediye should calculate");

    let mut normal_request = explicit_normal_request("2026-02-14");
    normal_request.dataset.payrolls.push(tediye.clone());
    let normal = calculate_payroll(&normal_request).expect("NORMAL should calculate after tediye");

    assert!(normal.gelirler.tabanBrutAylik.is_some());
    assert_eq!(
        normal.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
        tediye.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
    );
    assert_eq!(
        normal.gvDetay.as_ref().unwrap().ayniAyOncekiKullanilanGvIstisnasi,
        tediye.gvDetay.as_ref().unwrap().uygulananGvIstisnasi
    );
    assert_eq!(
        normal.damgaDetay.as_ref().unwrap().ayniAyOncekiKullanilanDamgaIstisnasi,
        tediye.damgaDetay.as_ref().unwrap().uygulananDamgaIstisnasi
    );
    assert_eq!(
        serde_json::to_value(&normal.devredenPekGelen).unwrap(),
        serde_json::to_value(&tediye.sonrakiDevredenPek).unwrap()
    );
}

#[test]
fn normal_explicit_payment_date_is_persisted_and_immutable() {
    let request = explicit_normal_request("2026-02-13");
    let normal = calculate_payroll(&request).expect("explicit normal should calculate");
    assert_eq!(normal.paymentDate, "2026-02-13");
    assert_eq!(normal.sequence, 0);

    let mut changed = request;
    changed.dataset.payrolls.push(normal.clone());
    changed.accrual = Some(PayrollAccrualInput {
        accrualId: normal.accrualId,
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-02-14".into(),
        sequence: 0,
        grossAmount: None,
        description: None,
    });
    let error = calculate_payroll(&changed).expect_err("normal date is immutable");
    assert!(error.to_string().contains("değiştirilemez"));
}

#[test]
fn devreden_pek_ages_once_per_tax_month_not_once_per_accrual() {
    let settings = DonemselKurumDegerleri {
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(1)),
        gunlukYemekIstisnasiSGK: Some(dec!(0)),
        ..DonemselKurumDegerleri::default()
    };
    let puantaj = PuantajOzeti {
        c: 30,
        ..PuantajOzeti::default()
    };
    let incoming = vec![DevredenPekKaydi {
        tutar: dec!(30000),
        kalanAySayisi: 3,
        kaynakDonemId: Some("previous-tax-month".into()),
    }];
    let normal_income = GelirKalemleri {
        tabanBrutAylik: Some(dec!(10000)),
        ..GelirKalemleri::default()
    };

    let (normal_pek, mut carried) = calculate_incremental_prime_esas_kazanc(
        &normal_income,
        Some(&puantaj),
        Some(&settings),
        &incoming,
        None,
        Decimal::ZERO,
    );
    assert_eq!(normal_pek.devredenPekKullanilan, dec!(20000));
    assert_eq!(carried[0].tutar, dec!(10000));
    assert_eq!(carried[0].kalanAySayisi, 3);

    let mut month_to_date = normal_pek.primMatrahi;
    for amount in [dec!(1000), dec!(1200), dec!(1500), dec!(1700), dec!(1900)] {
        let income = GelirKalemleri {
            tediye: Some(amount),
            ..GelirKalemleri::default()
        };
        let (pek, next) = calculate_incremental_prime_esas_kazanc(
            &income,
            Some(&puantaj),
            Some(&settings),
            &carried,
            None,
            month_to_date,
        );
        assert_eq!(next[0].kalanAySayisi, 3);
        month_to_date += pek.primMatrahi;
        carried = next;
    }

    let next_tax_month_income = GelirKalemleri {
        tabanBrutAylik: Some(dec!(25000)),
        ..GelirKalemleri::default()
    };
    let (_, next_tax_month_carried) = calculate_prime_esas_kazanc(
        &next_tax_month_income,
        Some(&puantaj),
        Some(&settings),
        &carried,
    );
    assert_eq!(next_tax_month_carried[0].tutar, dec!(5000));
    assert_eq!(next_tax_month_carried[0].kalanAySayisi, 2);
}

#[test]
fn payroll_chain_keeps_devreden_lifetime_across_normal_tediye_tis_and_ages_on_next_tax_month() {
    fn low_pek_settings(period_id: &str) -> DonemselKurumDegerleri {
        let mut settings = DonemselKurumDegerleri {
            donemId: period_id.into(),
            gunlukTabanUcret: dec!(200),
            gunlukYemek: dec!(0),
            birlestirilmisSosyalYardim: dec!(0),
            gunlukVasitaYol: dec!(0),
            giyimYardimi: dec!(0),
            hizmetZammiBirimi: dec!(0),
            ekOdeme: Some(dec!(0)),
            gunlukYemekIstisnasiSGK: Some(dec!(0)),
            gunlukYemekIstisnasiGV: Some(dec!(0)),
            gunlukAsgariUcret: Some(dec!(1000)),
            pekTavanKatsayisi: Some(dec!(1)),
            ..DonemselKurumDegerleri::default()
        };
        if let Some(groups) = settings.isPrimiGruplari.as_mut() {
            for group in groups {
                group.oran = Decimal::ZERO;
            }
        }
        settings
    }

    fn period(id: &str, start: &str, end: &str, work_month: i32, tax_month: i32) -> BordroDonemi {
        BordroDonemi {
            id: id.into(),
            yil: start[0..4].parse().unwrap(),
            ay: work_month,
            baslangicTarihi: start.into(),
            bitisTarihi: end.into(),
            donemAdi: id.into(),
            taxYear: 2026,
            taxMonth: tax_month,
        }
    }

    let mut seed = explicit_normal_request("2026-02-13");
    seed.dataset
        .institutionSettings
        .insert(seed.periodId.clone(), low_pek_settings(&seed.periodId));
    let mut previous = calculate_payroll(&seed).expect("seed payroll should calculate");
    previous.id = "person-1_2025-12".into();
    previous.donemId = "2025-12".into();
    previous.accrualId = "person-1_2025-12".into();
    previous.paymentDate = "2026-01-13".into();
    previous.status = BordroStatus::FINALIZED;
    previous.sonrakiDevredenPek = Some(vec![DevredenPekKaydi {
        tutar: dec!(30000),
        kalanAySayisi: 3,
        kaynakDonemId: Some("previous-tax-month".into()),
    }]);

    let previous_period = period("2025-12", "2025-12-15", "2026-01-14", 12, 1);
    seed.dataset.periods.push(previous_period);
    seed.dataset
        .institutionSettings
        .insert("2025-12".into(), low_pek_settings("2025-12"));
    seed.dataset.payrolls.push(previous);

    let mut normal = calculate_payroll(&seed).expect("normal payroll should calculate");
    assert_eq!(
        normal.devredenPekGelen.as_ref().unwrap()[0].kalanAySayisi,
        3
    );
    assert_eq!(
        normal.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi,
        2
    );
    normal.status = BordroStatus::FINALIZED;
    seed.dataset.payrolls.push(normal.clone());

    let mut tediye_request = seed.clone();
    tediye_request.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-01_tediye_1".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-02-13".into(),
        sequence: 1,
        grossAmount: Some(dec!(1000)),
        description: None,
    });
    let tediye = calculate_payroll(&tediye_request).expect("tediye should calculate");
    assert_eq!(
        tediye.devredenPekGelen.as_ref().unwrap()[0].kalanAySayisi,
        2
    );
    assert_eq!(
        tediye.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi,
        2
    );
    seed.dataset.payrolls.push(tediye.clone());

    let mut tis_request = seed.clone();
    tis_request.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-01_tis_2".into(),
        accrualType: AccrualType::TIS_IKRAMIYE,
        paymentDate: "2026-02-13".into(),
        sequence: 2,
        grossAmount: Some(dec!(1200)),
        description: None,
    });
    let tis = calculate_payroll(&tis_request).expect("TİS should calculate");
    assert_eq!(tis.devredenPekGelen.as_ref().unwrap()[0].kalanAySayisi, 2);
    assert_eq!(tis.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 2);
    seed.dataset.payrolls.push(tis);

    let next_period = period("2026-02", "2026-02-15", "2026-03-14", 2, 3);
    seed.dataset.periods.push(next_period.clone());
    let mut next_settings = low_pek_settings(&next_period.id);
    next_settings.gunlukTabanUcret = dec!(900);
    seed.dataset
        .institutionSettings
        .insert(next_period.id.clone(), next_settings);
    let mut next_attendance = seed.dataset.attendances[0].clone();
    next_attendance.id = "person-1_2026-02".into();
    next_attendance.donemId = next_period.id.clone();
    let mut next_date =
        NaiveDate::parse_from_str(&next_period.baslangicTarihi, "%Y-%m-%d").unwrap();
    let next_end = NaiveDate::parse_from_str(&next_period.bitisTarihi, "%Y-%m-%d").unwrap();
    next_attendance.gunler.clear();
    while next_date <= next_end {
        next_attendance
            .gunler
            .insert(next_date.format("%Y-%m-%d").to_string(), "Ç".into());
        next_date += Duration::days(1);
    }
    seed.dataset.attendances.push(next_attendance);
    seed.periodId = next_period.id.clone();
    seed.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-02".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-03-13".into(),
        sequence: 0,
        grossAmount: None,
        description: None,
    });
    let next = calculate_payroll(&seed).expect("next tax month should calculate");
    assert_eq!(next.devredenPekGelen.as_ref().unwrap()[0].kalanAySayisi, 2);
    assert_eq!(
        next.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi,
        1
    );
}

#[test]
fn same_tax_month_supplementary_accrual_continues_the_normal_state_without_duplication() {
    let mut normal_request = fixture_request();
    normal_request.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(dec!(5500)),
        tisIkramiyesi: Some(dec!(0)),
    });
    let normal = calculate_payroll(&normal_request).expect("normal payroll should calculate");
    let normal_snapshot = normal.clone();
    let mut finalized_normal = normal;
    finalized_normal.status = BordroStatus::FINALIZED;

    let mut tediye_request = fixture_request();
    tediye_request.manualIncome = None;
    tediye_request.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-01_tediye_1".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-02-14".into(),
        sequence: 1,
        grossAmount: Some(dec!(2000)),
        description: Some("Şubat tediye".into()),
    });
    tediye_request
        .dataset
        .payrolls
        .push(finalized_normal.clone());
    let tediye = calculate_payroll(&tediye_request).expect("same-month tediye should calculate");

    assert_eq!(tediye.accrualType, AccrualType::TEDIYE);
    assert_eq!(tediye.gelirler.tediye, Some(dec!(2000)));
    assert_eq!(tediye.gelirler.tabanBrutAylik, None);
    assert_eq!(tediye.gelirler.yemek, None);
    assert_eq!(tediye.gelirler.tisIkramiyesi, None);
    assert_eq!(
        tediye.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
        normal_snapshot
            .gvDetay
            .as_ref()
            .unwrap()
            .yeniKumulatifGvMatrahi
    );
    assert_eq!(
        tediye
            .gvDetay
            .as_ref()
            .unwrap()
            .ayniAyOncekiKullanilanGvIstisnasi,
        normal_snapshot
            .gvDetay
            .as_ref()
            .unwrap()
            .uygulananGvIstisnasi
    );
    assert!(
        tediye.gvDetay.as_ref().unwrap().uygulananGvIstisnasi
            + normal_snapshot
                .gvDetay
                .as_ref()
                .unwrap()
                .uygulananGvIstisnasi
            <= normal_snapshot
                .gvDetay
                .as_ref()
                .unwrap()
                .asgariUcretGvIstisnasi
    );
    assert!(
        tediye.damgaDetay.as_ref().unwrap().uygulananDamgaIstisnasi
            + normal_snapshot
                .damgaDetay
                .as_ref()
                .unwrap()
                .uygulananDamgaIstisnasi
            <= normal_snapshot
                .damgaDetay
                .as_ref()
                .unwrap()
                .aylikDamgaIstisnaHakki
    );
    assert!(
        tediye.pekDetay.as_ref().unwrap().primMatrahi
            + normal_snapshot.pekDetay.as_ref().unwrap().primMatrahi
            <= normal_snapshot.pekDetay.as_ref().unwrap().pekUstSinir
    );
    // The first immutable node is only copied into the input snapshot; the
    // supplementary calculation cannot mutate its result or status.
    assert_eq!(finalized_normal.id, normal_snapshot.id);
    assert_eq!(finalized_normal.status, BordroStatus::FINALIZED);
    assert_eq!(finalized_normal.netOdeme, normal_snapshot.netOdeme);
}

#[test]
fn two_same_month_supplementary_accruals_form_a_monotonic_gv_chain() {
    let mut normal_request = fixture_request();
    normal_request.manualIncome = None;
    let mut normal = calculate_payroll(&normal_request).expect("normal payroll should calculate");
    normal.status = BordroStatus::FINALIZED;

    let tediye_input = PayrollAccrualInput {
        accrualId: "person-1_2026-01_tediye_1".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-02-14".into(),
        sequence: 1,
        grossAmount: Some(dec!(1000)),
        description: None,
    };
    let mut tediye_request = fixture_request();
    tediye_request.manualIncome = None;
    tediye_request.accrual = Some(tediye_input);
    tediye_request.dataset.payrolls.push(normal.clone());
    let mut tediye = calculate_payroll(&tediye_request).expect("tediye should calculate");
    tediye.status = BordroStatus::FINALIZED;

    let mut tis_request = fixture_request();
    tis_request.manualIncome = None;
    tis_request.accrual = Some(PayrollAccrualInput {
        accrualId: "person-1_2026-01_tis_2".into(),
        accrualType: AccrualType::TIS_IKRAMIYE,
        paymentDate: "2026-02-14".into(),
        sequence: 2,
        grossAmount: Some(dec!(1200)),
        description: None,
    });
    tis_request
        .dataset
        .payrolls
        .extend([normal.clone(), tediye.clone()]);
    let tis = calculate_payroll(&tis_request).expect("TİS accrual should calculate");

    assert_eq!(tis.gelirler.tisIkramiyesi, Some(dec!(1200)));
    assert_eq!(tis.gelirler.tediye, None);
    assert_eq!(
        tis.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
        tediye.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
    );
    assert!(
        normal.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
            <= tediye.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
            && tediye.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
                <= tis.gvDetay.as_ref().unwrap().yeniKumulatifGvMatrahi
    );
    assert_eq!(tis.sequence, 2);
}

#[test]
fn decimal_json_boundary_accepts_exact_strings() {
    let input: ManualPayrollIncomeInput =
        serde_json::from_str(r#"{"tediye":"75000.10","tisIkramiyesi":"0.15"}"#)
            .expect("Decimal strings should deserialize at the shared boundary");

    assert_eq!(input.tediye, Some(dec!(75000.10)));
    assert_eq!(input.tisIkramiyesi, Some(dec!(0.15)));
}

#[test]
fn finalized_existing_payroll_is_immutable() {
    let mut request = fixture_request();
    let mut finalized = calculate_payroll(&request).expect("fixture should calculate");
    finalized.status = BordroStatus::FINALIZED;
    request.dataset.payrolls.push(finalized);

    let error = calculate_payroll(&request).expect_err("FINALIZED payroll must be immutable");
    assert!(error.to_string().contains("FINALIZED"));
}

#[test]
fn existing_accrual_ordering_metadata_is_immutable() {
    let mut request = fixture_request();
    let existing = calculate_payroll(&request).expect("fixture should calculate");
    request.dataset.payrolls.push(existing.clone());
    request.accrual = Some(PayrollAccrualInput {
        accrualId: existing.accrualId.clone(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-02-13".into(),
        sequence: 0,
        grossAmount: None,
        description: existing.accrualDescription.clone(),
    });

    let error = calculate_payroll(&request)
        .expect_err("an existing accrual must not be moved in the dependency order");
    assert!(error.to_string().contains("değiştirilemez"));
}

fn finalization_request(status: BordroStatus) -> PayrollCalculationRequest {
    let mut request = fixture_request();
    request.dataset.periods[0].taxMonth = 1;
    let mut existing = calculate_payroll(&request).expect("fixture should calculate");
    existing.status = status;
    request.dataset.payrolls.push(existing);
    request
}

#[test]
fn finalize_recalculates_and_returns_finalized_result() {
    let request = finalization_request(BordroStatus::CALCULATED);
    let result = finalize_payroll(&request).expect("calculated payroll should finalize");

    assert_eq!(result.status, BordroStatus::FINALIZED);
    assert_eq!(result.gelirler.tediye, Some(dec!(75000.10)));
    assert_eq!(result.olusturulmaTarihi, "2026-09-03T00:00:00.000Z");
}

#[test]
fn finalize_rejects_stale_and_draft_but_recalculates_calculated_source() {
    let mut calculated = finalization_request(BordroStatus::CALCULATED);
    calculated.manualIncome = Some(ManualPayrollIncomeInput {
        tediye: Some(dec!(0.1)),
        tisIkramiyesi: Some(dec!(0.15)),
    });
    let result = finalize_payroll(&calculated).expect("calculated source should be recalculated");
    assert_eq!(result.status, BordroStatus::FINALIZED);
    assert_eq!(result.gelirler.tediye, Some(dec!(0.1)));

    let stale_error = finalize_payroll(&finalization_request(BordroStatus::STALE))
        .expect_err("stale records must be recalculated before finalization");
    assert!(stale_error.to_string().contains("STALE"));

    let draft_error = finalize_payroll(&finalization_request(BordroStatus::DRAFT))
        .expect_err("draft has no authoritative result to finalize");
    assert!(draft_error.to_string().contains("DRAFT"));

    let finalized_error = finalize_payroll(&finalization_request(BordroStatus::FINALIZED))
        .expect_err("finalized records are fail-closed");
    assert!(finalized_error.to_string().contains("FINALIZED"));
}

#[test]
fn finalize_rejects_a_finalized_downstream_dependency() {
    let mut request = finalization_request(BordroStatus::CALCULATED);
    let mut later_period = request.dataset.periods[0].clone();
    later_period.id = "2026-02".into();
    later_period.ay = 2;
    later_period.baslangicTarihi = "2026-02-15".into();
    later_period.bitisTarihi = "2026-03-14".into();
    later_period.taxMonth = 2;
    request.dataset.periods.push(later_period);

    let mut later_payroll = request.dataset.payrolls[0].clone();
    later_payroll.id = "person-1_2026-02".into();
    later_payroll.donemId = "2026-02".into();
    later_payroll.accrualId = "person-1_2026-02".into();
    later_payroll.paymentDate = "2026-02-28".into();
    later_payroll.status = BordroStatus::FINALIZED;
    request.dataset.payrolls.push(later_payroll);

    let error = finalize_payroll(&request)
        .expect_err("finalization must not invalidate a downstream FINALIZED payroll");
    assert!(error.to_string().contains("downstream"));
    assert!(error.to_string().contains("FINALIZED"));
}

#[test]
fn calculation_timestamp_is_input_driven_and_existing_creation_is_preserved() {
    let mut first = fixture_request();
    first.calculatedAt = "2026-01-01T00:00:00Z".into();
    let first_result = calculate_payroll(&first).expect("first calculation should pass");

    let mut second = first.clone();
    second.calculatedAt = "2026-02-01T00:00:00Z".into();
    let second_result = calculate_payroll(&second).expect("second calculation should pass");
    assert_eq!(first_result.netOdeme, second_result.netOdeme);
    assert_eq!(first_result.gelirToplam, second_result.gelirToplam);
    assert_ne!(
        first_result.sonGuncellemeTarihi,
        second_result.sonGuncellemeTarihi
    );

    second.dataset.payrolls.push(first_result.clone());
    let updated = calculate_payroll(&second).expect("existing calculation should pass");
    assert_eq!(updated.olusturulmaTarihi, first_result.olusturulmaTarihi);
    assert_eq!(updated.sonGuncellemeTarihi, "2026-02-01T00:00:00Z");
}

#[test]
fn decimal_results_serialize_as_strings() {
    let result = calculate_payroll(&fixture_request()).expect("fixture should calculate");
    let value = serde_json::to_value(result).expect("result should serialize");

    assert!(value["gelirToplam"].is_string());
    assert!(value["netOdeme"].is_string());
    assert_eq!(value["gelirler"]["tediye"], serde_json::json!("75000.10"));
}

#[test]
fn strict_preflight_fails_closed_when_tax_chain_is_incomplete() {
    let request = fixture_request();
    let error =
        validate_payroll_request(&request).expect_err("tax month 1 is intentionally absent");
    assert!(error.to_string().contains("vergi") || error.to_string().contains("zinciri"));
}

// A/C/D/E/K/L/M/N: types never determine chronology or income ownership.
#[test]
fn payment_event_chain_shares_snapshots_and_preserves_full_normal_income() {
    for same_day in [false, true] {
        let mut first_request = supplementary_request("2026-02-10");
        first_request.accrual.as_mut().unwrap().sequence = 0;
        first_request.accrual.as_mut().unwrap().grossAmount = Some(dec!(1000000));
        let first = calculate_payroll(&first_request).unwrap();
        let mut normal_request = explicit_normal_request(if same_day { "2026-02-10" } else { "2026-02-14" });
        normal_request.accrual.as_mut().unwrap().sequence = if same_day { 1 } else { 0 };
        let standalone = calculate_payroll(&normal_request).unwrap();
        normal_request.dataset.payrolls.push(first.clone());
        let normal = calculate_payroll(&normal_request).unwrap();
        assert_eq!(normal.gelirler.tabanBrutAylik, standalone.gelirler.tabanBrutAylik);
        assert_eq!(normal.gelirler.yemek, standalone.gelirler.yemek);
        assert_eq!(normal.gelirler.vasitaYol, standalone.gelirler.vasitaYol);
        assert_eq!(normal.gelirler.isPrimi, standalone.gelirler.isPrimi);
        let gv = normal.gvDetay.as_ref().unwrap();
        let first_gv = first.gvDetay.as_ref().unwrap();
        assert_eq!(gv.oncekiKumulatifGvMatrahi, first_gv.yeniKumulatifGvMatrahi);
        assert_eq!(gv.ayniAyOncekiKullanilanGvIstisnasi, first_gv.uygulananGvIstisnasi);
        assert_eq!(normal.damgaDetay.as_ref().unwrap().ayniAyOncekiKullanilanDamgaIstisnasi,
            first.damgaDetay.as_ref().unwrap().uygulananDamgaIstisnasi);
        assert!(first.pekDetay.as_ref().unwrap().primMatrahi + normal.pekDetay.as_ref().unwrap().primMatrahi
            <= normal.statutorySnapshot.as_ref().unwrap().pekUstSinir);
        assert_eq!(serde_json::to_value(&normal.devredenPekGelen).unwrap(), serde_json::to_value(&first.sonrakiDevredenPek).unwrap());
        let mut third_request = normal_request.clone();
        third_request.dataset.payrolls.push(normal.clone());
        third_request.dataset.payrolls.reverse(); // input storage order is irrelevant
        third_request.accrual = Some(PayrollAccrualInput {
            accrualId: "third-tis".into(), accrualType: AccrualType::TIS_IKRAMIYE,
            paymentDate: if same_day { "2026-02-10" } else { "2026-02-25" }.into(),
            sequence: if same_day { 2 } else { 0 }, grossAmount: Some(dec!(2000)), description: None,
        });
        let third = calculate_payroll(&third_request).unwrap();
        assert_eq!(third.gvDetay.as_ref().unwrap().oncekiKumulatifGvMatrahi,
            first_gv.cariGvMatrahi + gv.cariGvMatrahi);
        assert_eq!(third.gvDetay.as_ref().unwrap().uygulananGvIstisnasi, dec!(0));
        assert_eq!(serde_json::to_value(&third.devredenPekGelen).unwrap(), serde_json::to_value(&normal.sonrakiDevredenPek).unwrap());
        // Exactly one normal per personnel + work period, regardless of date/sequence.
        third_request.accrual.as_mut().unwrap().accrualType = AccrualType::NORMAL;
        assert!(calculate_payroll(&third_request).unwrap_err().to_string().contains("yalnız bir NORMAL"));
    }
}

#[test]
fn legacy_normal_then_tediye_and_default_normal_sequence_remain_supported() {
    let normal = calculate_payroll(&explicit_normal_request("2026-02-14")).unwrap();
    let mut later = supplementary_request("2026-02-20");
    later.dataset.payrolls.push(normal);
    calculate_payroll(&later).unwrap();

    let mut first_request = supplementary_request("2026-02-14");
    first_request.accrual.as_mut().unwrap().sequence = 0;
    let first = calculate_payroll(&first_request).unwrap();
    let mut request = fixture_request();
    request.manualIncome = None;
    request.dataset.payrolls.push(first);
    let result = calculate_payroll(&request).unwrap();
    assert_eq!(result.accrualType, AccrualType::NORMAL);
    assert_eq!(result.sequence, 1);
}

#[test]
fn first_supplementary_event_advances_carry_month_once() {
    let mut request = supplementary_request("2026-02-10");
    request.accrual.as_mut().unwrap().grossAmount = Some(dec!(1000000));
    let previous = calculate_payroll(&request).unwrap();
    assert_eq!(previous.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 2);
    request.dataset.payrolls.push(previous);
    let next = BordroDonemi {
        id: "2026-02".into(), yil: 2026, ay: 2,
        baslangicTarihi: "2026-02-15".into(), bitisTarihi: "2026-03-14".into(),
        donemAdi: "Next month".into(), taxYear: 2026, taxMonth: 3,
    };
    let mut settings = request.dataset.institutionSettings["2026-01"].clone();
    settings.donemId = next.id.clone();
    request.dataset.institutionSettings.insert(next.id.clone(), settings);
    let mut days = HashMap::new();
    let mut date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 3, 14).unwrap();
    while date <= end {
        days.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    request.dataset.attendances.push(PersonelPuantaj {
        id: "next-attendance".into(), personelId: request.personnelId.clone(), donemId: next.id.clone(), gunler: days,
    });
    request.periodId = next.id.clone();
    request.dataset.periods.push(next);
    request.accrual = Some(PayrollAccrualInput {
        accrualId: "next-first".into(), accrualType: AccrualType::TEDIYE, paymentDate: "2026-03-10".into(),
        sequence: 0, grossAmount: Some(dec!(1000000)), description: None,
    });
    let first = calculate_payroll(&request).unwrap();
    assert_eq!(first.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 1);
    request.dataset.payrolls.push(first);
    request.accrual.as_mut().unwrap().accrualId = "next-second".into();
    request.accrual.as_mut().unwrap().sequence = 1;
    let second = calculate_payroll(&request).unwrap();
    assert_eq!(second.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 1);
}
