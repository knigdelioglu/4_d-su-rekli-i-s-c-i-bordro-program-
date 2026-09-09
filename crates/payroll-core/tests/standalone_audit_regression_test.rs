#[path = "support/independent_audit.rs"]
mod support;
use payroll_core::*;
use rust_decimal_macros::dec;

fn with_full_year_periods(mut req: PayrollCalculationRequest, year: i32) -> PayrollCalculationRequest {
    for month in 1..=12 {
        let p_req = support::dated_request(year, month);
        let period = p_req.dataset.periods[0].clone();
        if !req.dataset.periods.iter().any(|p| p.id == period.id) {
            req.dataset.periods.push(period.clone());
        }
        if let Some(settings) = p_req.dataset.institutionSettings.get(&period.id) {
            req.dataset.institutionSettings.entry(period.id).or_insert_with(|| settings.clone());
        }
    }
    req
}

#[test]
fn test_audit_sendika_aidati_deducted_from_gv_matrah() {
    let mut req = support::dated_request(2026, 7);
    req = with_full_year_periods(req, 2026);
    req.dataset.annualPayrollParameters = vec![AnnualPayrollParameters::default_for_2026()];

    // Set union membership on personnel
    let mut person = req.dataset.personnel[0].clone();
    let mut deductions = person.kesintiler.clone().unwrap_or_default();
    deductions.sendikaUyesi = Some(true);
    deductions.sabitSendikaAidati = Some(dec!(1000)); // 1,000 TL fixed union fee
    person.kesintiler = Some(deductions);
    req.dataset.personnel[0] = person.clone();

    let payroll = calculate_payroll_checked(&req).expect("Payroll with union fee should calculate");
    let gv_detay = payroll.gvDetay.as_ref().expect("GV detay must exist");

    // Total gross income
    let gross = payroll.gelirToplam;
    let sgk_worker = payroll.kesintiler.isciSgkPrimi.unwrap_or_default();
    let unemployment_worker = payroll.kesintiler.isciIssizlikPrimi.unwrap_or_default();
    let meal_gv_exemption = payroll.statutorySnapshot.as_ref().unwrap().gvYemekIstisnasiToplam;
    let actual_meal_exemption = payroll.gelirler.yemek.unwrap_or_default().min(meal_gv_exemption);

    // GVK 63/4: gv_matrah = gross - sgk - unemployment - meal_exemption - union_fee
    let expected_gv_base = (gross - sgk_worker - unemployment_worker - actual_meal_exemption - dec!(1000)).max(dec!(0));
    assert_eq!(
        gv_detay.cariGvMatrahi,
        expected_gv_base,
        "GV matrahı sendika aidatı (1,000 TL) düşülerek hesaplanmalıdır"
    );
    assert_eq!(payroll.kesintiler.sendikaAidati, Some(dec!(1000)));
}

#[test]
fn test_audit_february_28_days_with_1_unpaid_day_results_in_27_prim_days() {
    let mut req = support::dated_request(2026, 2);
    req = with_full_year_periods(req, 2026);
    req.dataset.annualPayrollParameters = vec![AnnualPayrollParameters::default_for_2026()];

    let person_id = req.dataset.personnel[0].id.clone();
    let mut settings = req.dataset.institutionSettings.get(&req.periodId).unwrap().clone();

    // In 4/D bordro geometry: yil/ay is the start date's year/month.
    // 15 Feb - 14 Mar period starts on 2026-02-15 -> yil: 2026, ay: 2, taxYear: 2026, taxMonth: 3.
    let period_feb_mar = BordroDonemi {
        id: "donem-2026-02".into(),
        yil: 2026,
        ay: 2,
        baslangicTarihi: "2026-02-15".into(),
        bitisTarihi: "2026-03-14".into(),
        donemAdi: "Şubat-Mart 2026".into(),
        taxYear: 2026,
        taxMonth: 3,
    };
    settings.donemId = period_feb_mar.id.clone();

    let mut att_days = std::collections::HashMap::new();
    // 15 Feb to 28 Feb (14 days): 1 day is unpaid sick leave 'R'
    for day in 15..=28 {
        let code = if day == 20 { "R" } else { "Ç" };
        att_days.insert(format!("2026-02-{day:02}"), code.to_string());
    }
    // 1 Mar to 14 Mar (14 days): all worked 'Ç'
    for day in 1..=14 {
        att_days.insert(format!("2026-03-{day:02}"), "Ç".to_string());
    }
    let attendance_feb_mar = PersonelPuantaj {
        id: format!("{person_id}_donem-2026-02"),
        personelId: person_id.clone(),
        donemId: "donem-2026-02".into(),
        gunler: att_days,
    };

    let resolved_feb_mar = payroll_engine::resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        &attendance_feb_mar,
        &period_feb_mar,
        &settings,
        &[], // No employer-paid sick dates
    ).expect("Period starting 2026-02-15 should resolve");

    // Total calendar days = 28. 1 unpaid day = 27 prim days!
    assert_eq!(
        resolved_feb_mar.sgkPrimGunSayisi,
        27,
        "28 çeken takvim periyodunda 1 eksik gün olduğunda SGK prim günü tam 27 olmalıdır (SGK 2020/20 Genelgesi)"
    );
    // PEK alt sınır = 27 x 1,101.00 TL = 29,727.00 TL
    assert_eq!(resolved_feb_mar.pekAltSinir, dec!(29727));
}

#[test]
fn test_audit_multi_accrual_same_month_stamp_exemption_exhaustion() {
    let mut req_base = support::dated_request(2026, 8);
    req_base = with_full_year_periods(req_base, 2026);
    req_base.dataset.annualPayrollParameters = vec![AnnualPayrollParameters::default_for_2026()];

    // Monthly minimum wage gross = 33,030 TL. Stamp rate = 0.00759. Monthly entitlement = 250.70 TL.
    // 1. First event: TEDIYE 20,000 TL gross on 2026-08-05
    let mut req_tediye = req_base.clone();
    req_tediye.accrual = Some(PayrollAccrualInput {
        accrualId: "ted-aug".into(),
        accrualType: AccrualType::TEDIYE,
        paymentDate: "2026-08-05".into(),
        sequence: 1,
        grossAmount: Some(dec!(20000)),
        description: Some("Tediye".into()),
    });
    let p_tediye = calculate_payroll_checked(&req_tediye).unwrap();
    let stamp_tediye = p_tediye.damgaDetay.as_ref().unwrap();
    assert_eq!(stamp_tediye.brutDamgaVergisi, dec!(151.80));
    assert_eq!(stamp_tediye.aylikDamgaIstisnaHakki, dec!(250.70));
    assert_eq!(stamp_tediye.uygulananDamgaIstisnasi, dec!(151.80));
    assert_eq!(stamp_tediye.kalanDamgaIstisnasi, dec!(98.90));
    assert_eq!(stamp_tediye.kesilenDamgaVergisi, dec!(0));

    // 2. Second event: NORMAL salary on 2026-08-14
    let mut req_normal = req_base.clone();
    req_normal.accrual = Some(PayrollAccrualInput {
        accrualId: "norm-aug".into(),
        accrualType: AccrualType::NORMAL,
        paymentDate: "2026-08-14".into(),
        sequence: 1,
        grossAmount: None,
        description: Some("Maaş".into()),
    });
    req_normal.dataset.payrolls.push(p_tediye.clone());
    let p_normal = calculate_payroll_checked(&req_normal).unwrap();
    let stamp_normal = p_normal.damgaDetay.as_ref().unwrap();

    assert_eq!(stamp_normal.ayniAyOncekiKullanilanDamgaIstisnasi, dec!(151.80));
    assert_eq!(stamp_normal.uygulananDamgaIstisnasi, dec!(98.90));
    assert_eq!(stamp_normal.kalanDamgaIstisnasi, dec!(0));
    assert_eq!(
        stamp_tediye.uygulananDamgaIstisnasi + stamp_normal.uygulananDamgaIstisnasi,
        dec!(250.70),
        "Aynı ay toplam uygulanan damga vergisi istisnası tam 250.70 TL olmalıdır"
    );

    // 3. Third event: TIS_IKRAMIYE on 2026-08-20
    let mut req_tis = req_base.clone();
    req_tis.accrual = Some(PayrollAccrualInput {
        accrualId: "tis-aug".into(),
        accrualType: AccrualType::TIS_IKRAMIYE,
        paymentDate: "2026-08-20".into(),
        sequence: 1,
        grossAmount: Some(dec!(10000)),
        description: Some("İkramiye".into()),
    });
    req_tis.dataset.payrolls.push(p_tediye);
    req_tis.dataset.payrolls.push(p_normal);
    let p_tis = calculate_payroll_checked(&req_tis).unwrap();
    let stamp_tis = p_tis.damgaDetay.as_ref().unwrap();

    assert_eq!(stamp_tis.ayniAyOncekiKullanilanDamgaIstisnasi, dec!(250.70));
    assert_eq!(stamp_tis.uygulananDamgaIstisnasi, dec!(0));
    assert_eq!(stamp_tis.kalanDamgaIstisnasi, dec!(0));
    assert_eq!(stamp_tis.kesilenDamgaVergisi, dec!(75.90));
}

#[test]
fn test_audit_pek_lower_bound_zero_deduction_for_worker() {
    let mut req = support::dated_request(2026, 5);
    req = with_full_year_periods(req, 2026);
    let period_id = req.periodId.clone();

    let mut settings = req.dataset.institutionSettings.get(&period_id).unwrap().clone();
    settings.gunlukTabanUcret = dec!(400);
    req.dataset.institutionSettings.insert(period_id.clone(), settings);

    let payroll = calculate_payroll_checked(&req).unwrap();
    let pek = payroll.pekDetay.as_ref().unwrap();

    // Actual worker PEK includes base wage + meal subject to SGK
    let worker_pek = pek.primMatrahi;
    let worker_sgk = payroll.kesintiler.isciSgkPrimi.unwrap();
    let worker_unemployment = payroll.kesintiler.isciIssizlikPrimi.unwrap();

    // Worker pays ONLY on actual worker_pek
    assert_eq!(worker_sgk, (worker_pek * dec!(0.14)).round_dp(2));
    assert_eq!(worker_unemployment, (worker_pek * dec!(0.01)).round_dp(2));

    // Employer completes the floor difference up to alt sınır (33,030 TL)
    let alt_sinir = pek.pekAltSinir;
    let expected_fark = alt_sinir - worker_pek;
    assert_eq!(pek.altSinirTamamlamaFarki, expected_fark);

    // Employer pays both worker SGK (14%) and unemployment (1%) difference:
    let expected_tamamlama_primi = (expected_fark * dec!(0.14)).round_dp(2) + (expected_fark * dec!(0.01)).round_dp(2);
    assert_eq!(pek.pekAltSinirTamamlamaIsverenPrimi, Some(expected_tamamlama_primi));
}

#[test]
fn test_audit_direct_statutory_deductions_union_fee_deduction() {
    let person = Personel {
        id: "direct-union".into(),
        tcNo: "11111111111".into(),
        ad: "Union".into(),
        soyad: "Member".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "1".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri {
            sendikaUyesi: Some(true),
            sabitSendikaAidati: Some(dec!(800)),
            besUyesi: Some(false),
            oksOraniYuzde: None,
            sabitBesTutar: None,
            icraTutar: None,
            kisiBorcuTutar: None,
            dogumAskerlikBorclanmasiTutar: None,
            hayatSaglikSigortasiTutar: None,
            digerKesintiTutar: None,
            gvIndirimleri: None,
        }),
    };
    let mut gelirler = GelirKalemleri::default();
    gelirler.tabanBrutAylik = Some(dec!(35000));
    let puantaj = PuantajOzeti { c: 30, ..PuantajOzeti::default() };
    let kurum = DonemselKurumDegerleri {
        gunlukTabanUcret: dec!(1166.67),
        gunlukAsgariUcret: Some(dec!(1101)),
        pekTavanKatsayisi: Some(dec!(9)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        ..DonemselKurumDegerleri::default()
    };
    let snapshot = ResolvedStatutorySnapshot {
        source: StatutorySnapshotSource::AttendanceBacked,
        segments: vec![],
        sgkPrimGunSayisi: 30,
        pekAltSinir: dec!(33030),
        pekUstSinir: dec!(297270),
        sgkYemekIstisnasiToplam: dec!(0),
        gvYemekIstisnasiToplam: dec!(0),
        gvReferansGunlukAsgariUcret: dec!(1101),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
    };
    let brackets = default_gelir_vergisi_dilimleri_2026();
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv: dec!(0),
        incoming_devreden_pek: &[],
        previous_cumulative_asgari_gv: dec!(0),
        tax_brackets: &brackets,
    };

    let (kesintiler, _pek, _) = calculate_statutory_deductions_with_tax_brackets(
        &gelirler,
        Some(&kurum),
        Some(&person),
        Some(&puantaj),
        &tax_inputs,
        Some(&snapshot),
    );

    assert_eq!(kesintiler.sendikaAidati, Some(dec!(800)));
    // Worker SGK = 35,000 * 14% = 4,900 TL
    // Worker Unemployment = 35,000 * 1% = 350 TL
    // Taxable base without union fee would be 35,000 - 4,900 - 350 = 29,750 TL
    // With 800 TL union fee, taxable base MUST BE 29,750 - 800 = 28,950 TL!
    // Minimum wage exempt tax: 28,075.50 * 15% = 4,211.33 TL
    // Gross tax on 28,950 TL: 28,950 * 15% = 4,342.50 TL
    // Expected withheld income tax: 4,342.50 - 4,211.33 = 131.17 TL!
    assert_eq!(
        kesintiler.gelirVergisi,
        Some(dec!(131.17)),
        "Sendika aidatı GV matrahından düşüldüğünde kesilen vergi 131.17 TL olmalıdır"
    );
}
