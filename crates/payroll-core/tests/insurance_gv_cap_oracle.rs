use payroll_core::{
    calculate_payroll, AnnualPayrollParameters, DonemselKurumDegerleri, GvIndirimGirdileri,
    IsPrimiGrupItem, PayrollPeriodInstitutionSettings, PersonelKesintileri,
    PAYROLL_CALCULATION_VERSION,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[path = "support/audit_fixture.rs"]
mod audit_fixture;

/// Independent GVK 63/3 oracle: monthly insurance premiums are limited to
/// 15% of taxable gross wage remuneration. This fixture gives road allowance
/// the same taxable wage treatment as salary and uses a binding premium.
#[test]
fn taxable_road_allowance_is_included_in_monthly_insurance_limit_base() {
    let mut request = audit_fixture::request();
    let source_period = request
        .dataset
        .periods
        .iter()
        .find(|period| period.id == "source")
        .expect("source period")
        .clone();
    let settings = DonemselKurumDegerleri {
        donemId: "source".into(),
        gunlukTabanUcret: dec!(2000),
        gunlukYemek: Decimal::ZERO,
        birlestirilmisSosyalYardim: Decimal::ZERO,
        gunlukVasitaYol: dec!(100),
        giyimYardimi: Decimal::ZERO,
        hizmetZammiBirimi: Decimal::ZERO,
        isPrimiYuzde: Some(Decimal::ZERO),
        isPrimiGruplari: Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: Decimal::ZERO,
            aktif: true,
        }]),
        ..Default::default()
    };
    audit_fixture::compensation_policy::replace_period_baseline(
        &mut request.dataset,
        &source_period,
        &settings,
    );
    request.dataset.institutionSettings.insert(
        "source".into(),
        PayrollPeriodInstitutionSettings::from(&settings),
    );
    request.dataset.personnel[0].kesintiler = Some(PersonelKesintileri {
        gvIndirimleri: Some(GvIndirimGirdileri {
            dogumAskerlikGvIndirimTutar: None,
            hayatSigortasiPrimiTutar: None,
            saglikSigortasiPrimiTutar: Some(dec!(20000)),
        }),
        ..Default::default()
    });
    assert_eq!(
        request.dataset.annualPayrollParameters[0].sigortaGvYillikBrutAsgariUcretTavani,
        AnnualPayrollParameters::default_for_2026().sigortaGvYillikBrutAsgariUcretTavani
    );

    let result = calculate_payroll(&request).expect("road payroll should calculate");
    // The source service period is 15 Dec–14 Jan (31 paid days):
    // 31×2,000 salary + 31×100 taxable road allowance = 65,100 gross.
    // 15% × 65,100 = 9,765; the 20,000 health premium binds this cap.
    assert_eq!(result.gelirToplam, dec!(65100));
    assert_eq!(
        result
            .calculationMetadata
            .as_ref()
            .and_then(|metadata| metadata.snapshot.as_ref())
            .map(|snapshot| snapshot.calculationVersion.as_str()),
        Some(PAYROLL_CALCULATION_VERSION)
    );
    let gv = result.gvDetay.expect("GV detail");
    assert_eq!(gv.sigortaGvAylikLimiti, dec!(9765));
    assert_eq!(gv.uygulanabilirSigortaGvIndirimi, dec!(9765));
}
