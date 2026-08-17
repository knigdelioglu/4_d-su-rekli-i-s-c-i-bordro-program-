use bordro_programi_lib::domain::calculations::{
    calculate_statutory_deductions_with_tax_brackets, default_gelir_vergisi_dilimleri_2026,
    StatutoryDeductionTaxInputs,
};
use bordro_programi_lib::domain::models::*;
use rust_decimal_macros::dec;

fn oks_person() -> Personel {
    Personel {
        id: "helper-authority".into(),
        tcNo: "11111111111".into(),
        ad: "Helper".into(),
        soyad: "Authority".into(),
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
            sendikaUyesi: Some(false),
            besUyesi: Some(true),
            sabitSendikaAidati: None,
            oksOraniYuzde: None,
            sabitBesTutar: None,
            icraTutar: None,
            kisiBorcuTutar: None,
            dogumAskerlikBorclanmasiTutar: None,
            hayatSaglikSigortasiTutar: None,
            digerKesintiTutar: None,
            gvIndirimleri: None,
        }),
    }
}

#[test]
fn zero_gross_with_used_deferred_pek_still_accrues_worker_premiums_and_oks() {
    let gelirler = GelirKalemleri::default();
    let puantaj = PuantajOzeti::default();
    let personel = oks_person();
    let kurum = DonemselKurumDegerleri {
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        besOraniYuzde: Some(dec!(3)),
        gunlukAsgariUcret: Some(dec!(1000)),
        pekTavanKatsayisi: Some(dec!(3)),
        ..DonemselKurumDegerleri::default()
    };
    let snapshot = ResolvedStatutorySnapshot {
        segments: vec![],
        sgkPrimGunSayisi: 1,
        pekAltSinir: dec!(1000),
        pekUstSinir: dec!(30000),
        sgkYemekIstisnasiToplam: dec!(0),
        gvYemekIstisnasiToplam: dec!(0),
        gvReferansGunlukAsgariUcret: dec!(1000),
    };
    let incoming = vec![DevredenPekKaydi {
        tutar: dec!(20000),
        kalanAySayisi: 2,
        kaynakDonemId: Some("prior".into()),
    }];
    let brackets = default_gelir_vergisi_dilimleri_2026();
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv: dec!(0),
        incoming_devreden_pek: &incoming,
        previous_cumulative_asgari_gv: dec!(0),
        tax_brackets: &brackets,
    };

    let (kesintiler, pek, sonraki) = calculate_statutory_deductions_with_tax_brackets(
        &gelirler,
        Some(&kurum),
        Some(&personel),
        Some(&puantaj),
        &tax_inputs,
        Some(&snapshot),
    );

    assert_eq!(pek.hesaplananPek, dec!(0));
    assert_eq!(pek.devredenPekKullanilan, dec!(20000));
    assert_eq!(pek.primMatrahi, dec!(20000));
    assert!(sonraki.is_empty());

    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(2800)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(200)));
    assert_eq!(kesintiler.bes, Some(dec!(600)));
    assert_eq!(kesintiler.gelirVergisi, Some(dec!(0)));
    assert_eq!(kesintiler.damgaVergisi, Some(dec!(0)));
}
