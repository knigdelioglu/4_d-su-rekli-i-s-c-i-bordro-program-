use bordro_programi_lib::domain::calculations::{
    calculate_prime_esas_kazanc, calculate_statutory_deductions,
};
use bordro_programi_lib::domain::models::{
    DevredenPekKaydi, DonemselKurumDegerleri, GelirKalemleri, PuantajOzeti,
};
use rust_decimal_macros::dec;

fn kurum() -> DonemselKurumDegerleri {
    let mut k = DonemselKurumDegerleri::default();
    // 30 prim günü => alt sınır 30.000, üst sınır 90.000.
    // Test değerleri devreden PEK sınır davranışını okunabilir kılar.
    k.gunlukAsgariUcret = Some(dec!(1000));
    k.pekTavanKatsayisi = Some(dec!(3));
    k.sgkIsciOraniYuzde = Some(dec!(14));
    k.issizlikIsciOraniYuzde = Some(dec!(1));
    k
}

fn puantaj_30() -> PuantajOzeti {
    PuantajOzeti {
        c: 30,
        ..Default::default()
    }
}

fn gelir(tutar: rust_decimal::Decimal) -> GelirKalemleri {
    GelirKalemleri {
        tabanBrutAylik: Some(tutar),
        ..Default::default()
    }
}

fn devreden(tutar: rust_decimal::Decimal, kalan_ay: i32, kaynak: &str) -> DevredenPekKaydi {
    DevredenPekKaydi {
        tutar,
        kalanAySayisi: kalan_ay,
        kaynakDonemId: Some(kaynak.to_string()),
    }
}

#[test]
fn devreden_yokken_mevcut_isci_prim_sonucu_degismez() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(50000));

    let (kesintiler, pek, sonraki) =
        calculate_statutory_deductions(&g, Some(&k), None, Some(&p), dec!(0), &[], dec!(0));

    assert_eq!(pek.hamPek, dec!(50000));
    assert_eq!(pek.hesaplananPek, dec!(50000));
    assert_eq!(pek.devredenPekKullanilan, dec!(0));
    assert_eq!(pek.primMatrahi, dec!(50000));
    assert_eq!(pek.finalPek, dec!(50000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(7000)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(500)));
    assert!(sonraki.is_empty());
}

#[test]
fn devreden_tamamen_tavana_sigarsa_isci_prim_matrahina_girer() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(50000));
    let gelen = vec![devreden(dec!(20000), 2, "onceki")];

    let (kesintiler, pek, sonraki) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.hamPek, dec!(50000));
    assert_eq!(pek.devredenPekKullanilan, dec!(20000));
    assert_eq!(pek.primMatrahi, dec!(70000));
    assert_eq!(pek.finalPek, dec!(70000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(9800)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(700)));
    assert!(sonraki.is_empty());
}

#[test]
fn devreden_kismen_sigarsa_yalniz_kullanilan_kisim_primlenir() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(80000));
    let gelen = vec![devreden(dec!(20000), 2, "onceki")];

    let (kesintiler, pek, sonraki) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.devredenPekKullanilan, dec!(10000));
    assert_eq!(pek.primMatrahi, dec!(90000));
    assert_eq!(pek.finalPek, dec!(90000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(12600)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(900)));
    assert_eq!(sonraki.len(), 1);
    assert_eq!(sonraki[0].tutar, dec!(10000));
    assert_eq!(sonraki[0].kalanAySayisi, 1);
}

#[test]
fn tavan_boslugu_yoksa_devreden_isci_primine_girmez() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(100000));
    let gelen = vec![devreden(dec!(20000), 2, "onceki")];

    let (kesintiler, pek, sonraki) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.devredenPekKullanilan, dec!(0));
    assert_eq!(pek.primMatrahi, dec!(90000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(12600)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(900)));
    assert_eq!(sonraki.len(), 1);
    assert_eq!(sonraki[0].tutar, dec!(20000));
    assert_eq!(sonraki[0].kalanAySayisi, 1);
}

#[test]
fn birden_fazla_devreden_kaydi_tavan_boslugunu_sirayla_doldurur() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(60000));
    let gelen = vec![
        devreden(dec!(20000), 2, "a"),
        devreden(dec!(30000), 2, "b"),
    ];

    let (kesintiler, pek, sonraki) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.devredenPekKullanilan, dec!(30000));
    assert_eq!(pek.primMatrahi, dec!(90000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(12600)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(900)));
    assert_eq!(sonraki.len(), 1);
    assert_eq!(sonraki[0].kaynakDonemId.as_deref(), Some("b"));
    assert_eq!(sonraki[0].tutar, dec!(20000));
    assert_eq!(sonraki[0].kalanAySayisi, 1);
}

#[test]
fn devreden_iki_aylik_omru_korunur() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(80000));
    let ilk_gelen = vec![devreden(dec!(20000), 2, "kaynak")];

    let (ilk_pek, ilk_sonraki) =
        calculate_prime_esas_kazanc(&g, Some(&p), Some(&k), &ilk_gelen);
    assert_eq!(ilk_pek.devredenPekKullanilan, dec!(10000));
    assert_eq!(ilk_sonraki.len(), 1);
    assert_eq!(ilk_sonraki[0].tutar, dec!(10000));
    assert_eq!(ilk_sonraki[0].kalanAySayisi, 1);

    let (ikinci_pek, ikinci_sonraki) =
        calculate_prime_esas_kazanc(&g, Some(&p), Some(&k), &ilk_sonraki);
    assert_eq!(ikinci_pek.devredenPekKullanilan, dec!(10000));
    assert_eq!(ikinci_pek.primMatrahi, dec!(90000));
    assert!(ikinci_sonraki.is_empty());
}

#[test]
fn devreden_alt_sinir_farkini_azaltir_ama_isciye_yapay_fark_yansitilmaz() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(10000));
    let gelen = vec![devreden(dec!(5000), 2, "onceki")];

    let (kesintiler, pek, _) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.primMatrahi, dec!(15000));
    assert_eq!(pek.pekAltSinir, dec!(30000));
    assert_eq!(pek.altSinirTamamlamaFarki, dec!(15000));
    assert_eq!(pek.finalPek, dec!(30000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(2100)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(150)));
    assert_eq!(pek.finalPek - pek.primMatrahi, pek.altSinirTamamlamaFarki);
}

#[test]
fn devreden_alt_siniri_asarsa_yapay_tamamlama_farki_kalmaz() {
    let k = kurum();
    let p = puantaj_30();
    let g = gelir(dec!(10000));
    let gelen = vec![devreden(dec!(25000), 2, "onceki")];

    let (kesintiler, pek, _) = calculate_statutory_deductions(
        &g,
        Some(&k),
        None,
        Some(&p),
        dec!(0),
        &gelen,
        dec!(0),
    );

    assert_eq!(pek.devredenPekKullanilan, dec!(25000));
    assert_eq!(pek.primMatrahi, dec!(35000));
    assert_eq!(pek.altSinirTamamlamaFarki, dec!(0));
    assert_eq!(pek.finalPek, dec!(35000));
    assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(4900)));
    assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(350)));
}

#[test]
fn prim_matrahi_daima_ham_artı_kullanilan_devreden_tavanla_sinirli_olur() {
    let k = kurum();
    let p = puantaj_30();

    let senaryolar = [
        (dec!(50000), dec!(0), dec!(50000)),
        (dec!(50000), dec!(20000), dec!(70000)),
        (dec!(80000), dec!(20000), dec!(90000)),
        (dec!(100000), dec!(20000), dec!(90000)),
    ];

    for (ham, gelen_tutar, beklenen_prim_matrahi) in senaryolar {
        let g = gelir(ham);
        let gelen = if gelen_tutar > dec!(0) {
            vec![devreden(gelen_tutar, 2, "onceki")]
        } else {
            vec![]
        };
        let (pek, _) = calculate_prime_esas_kazanc(&g, Some(&p), Some(&k), &gelen);
        assert_eq!(pek.primMatrahi, beklenen_prim_matrahi);
        assert!(pek.primMatrahi <= pek.pekUstSinir);
        assert_eq!(
            pek.primMatrahi,
            (pek.hamPek + pek.devredenPekKullanilan).min(pek.pekUstSinir)
        );
    }
}
