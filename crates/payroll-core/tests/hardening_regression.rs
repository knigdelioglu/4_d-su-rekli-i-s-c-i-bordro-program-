use payroll_core::{
    validate_gv_base_reconciliation, validate_ordinary_payroll_line_items, BordroKaydi,
    BordroStatus, GelirKalemleri, KesintiKalemleri, RetroAllocation, RetroEarningCode,
    RetroSgkTreatment, RetroTaxTreatment,
};
use rust_decimal_macros::dec;

fn sparse_payroll() -> BordroKaydi {
    BordroKaydi {
        id: "hardening-payroll".into(),
        personelId: "person-1".into(),
        donemId: "2026-01".into(),
        accrualId: "hardening-payroll".into(),
        accrualType: payroll_core::AccrualType::NORMAL,
        paymentDate: "2026-02-14".into(),
        sequence: 0,
        accrualDescription: None,
        puantajOzeti: Default::default(),
        gelirler: GelirKalemleri::default(),
        gelirToplam: dec!(0),
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: dec!(0),
        netOdeme: dec!(0),
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: String::new(),
        sonGuncellemeTarihi: String::new(),
        notlar: None,
        oncekiKumulatifGvMatrahi: None,
        oncekiKumulatifAsgariGvMatrahi: None,
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: None,
        sonrakiDevredenPek: None,
        pekDetay: None,
        isPrimiDetay: None,
        gvDetay: None,
        persistedGvBase: None,
        damgaDetay: None,
        statutorySnapshot: None,
        odenenRaporluGun: None,
        raporluGun: None,
    }
}

#[test]
fn ordinary_negative_income_line_item_is_rejected_even_when_total_is_positive() {
    let income = GelirKalemleri {
        tabanBrutAylik: Some(dec!(100)),
        digerGelir: Some(dec!(-40)),
        ..Default::default()
    };
    let error = validate_ordinary_payroll_line_items(&income, &KesintiKalemleri::default())
        .expect_err("negative ordinary income must fail closed");
    assert!(error.to_string().contains("digerGelir"));
}

#[test]
fn ordinary_negative_deduction_line_item_is_rejected() {
    let deductions = KesintiKalemleri {
        icra: Some(dec!(-10)),
        ..Default::default()
    };
    let error = validate_ordinary_payroll_line_items(&GelirKalemleri::default(), &deductions)
        .expect_err("negative ordinary deduction must fail closed");
    assert!(error.to_string().contains("icra"));
}

#[test]
fn persisted_gv_base_must_match_current_snapshot_detail() {
    let mut payroll = sparse_payroll();
    payroll.gvDetay = Some(payroll_core::GvHesapDetayi {
        oncekiKumulatifGvMatrahi: dec!(0),
        cariGvMatrahi: dec!(27500),
        yeniKumulatifGvMatrahi: dec!(27500),
        brutGelirVergisi: dec!(0),
        asgariUcretGvMatrahi: dec!(0),
        asgariUcretReferansKumulatifMatrahi: dec!(0),
        asgariUcretGvIstisnasi: dec!(0),
        ayniAyOncekiKullanilanGvIstisnasi: dec!(0),
        tahakkukOncesiKalanGvIstisnasi: dec!(0),
        uygulananGvIstisnasi: dec!(0),
        tahakkukSonrasiKalanGvIstisnasi: dec!(0),
        kesilenGelirVergisi: dec!(0),
        dogumAskerlikGvIndirimi: dec!(0),
        sigortaGvIndirimAdayi: dec!(0),
        sigortaGvAylikLimiti: dec!(0),
        sigortaGvYillikKalanLimiti: dec!(0),
        uygulanabilirSigortaGvIndirimi: dec!(0),
    });
    payroll.persistedGvBase = Some(dec!(34000));
    let error = validate_gv_base_reconciliation(&payroll)
        .expect_err("current GV scalar/detail mismatch must be rejected");
    assert!(error.to_string().contains("eşleşmiyor"));
}

#[test]
fn signed_retro_allocation_delta_remains_valid_domain_data() {
    let allocation = RetroAllocation {
        id: "retro-negative".into(),
        batchId: "batch-negative".into(),
        personnelId: "person-1".into(),
        sourcePeriodId: "2026-01".into(),
        earningCode: RetroEarningCode::BASE_WAGE,
        originalRecognizedAmount: dec!(100),
        previousAuthoritativeRetroAmount: dec!(0),
        targetAmount: dec!(0),
        deltaAmount: dec!(-100),
        sgkTreatment: RetroSgkTreatment::WAGE_SOURCE_MONTH,
        incomeTaxTreatment: RetroTaxTreatment::TAXABLE,
        stampTaxTreatment: RetroTaxTreatment::TAXABLE,
        originalPek: dec!(100),
        retroPekDelta: dec!(-100),
        adjustedPek: dec!(0),
        workerSgkDelta: dec!(-14),
        workerUnemploymentDelta: dec!(-1),
        employerSgkDelta: dec!(-21.75),
        employerUnemploymentDelta: dec!(-2),
        originalEmployerLowerBound: dec!(0),
        targetEmployerLowerBound: dec!(0),
        employerLowerBoundDelta: dec!(0),
        employerLowerBoundPremiumDelta: dec!(0),
        originalSourceCarry: None,
        targetSourceCarry: None,
        payableSettlementAmount: dec!(0),
        offsetSettlementAmount: dec!(0),
        recoverableAmount: dec!(100),
        metadata: None,
    };

    assert_eq!(allocation.deltaAmount, dec!(-100));
    assert!(allocation.deltaAmount < dec!(0));
}
