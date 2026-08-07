use super::models::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::cmp::min;

fn round2(val: Decimal) -> Decimal {
    val.round_dp(2)
}

fn floor_dec(val: Decimal) -> Decimal {
    val.floor()
}

pub struct TaxBracket {
    pub limit: Decimal,
    pub oran: Decimal,
}

pub fn get_gelir_vergisi_dilimleri_2026() -> Vec<TaxBracket> {
    vec![
        TaxBracket { limit: dec!(190000), oran: dec!(0.15) },
        TaxBracket { limit: dec!(400000), oran: dec!(0.20) },
        TaxBracket { limit: dec!(1500000), oran: dec!(0.27) },
        TaxBracket { limit: dec!(5300000), oran: dec!(0.35) },
        TaxBracket { limit: dec!(79228162514264337593543950335), oran: dec!(0.40) }, // Decimal::MAX
    ]
}

pub fn calculate_total_tax_for_cumulative_matrah(kumulatif: Decimal) -> Decimal {
    if kumulatif <= dec!(0) {
        return dec!(0);
    }
    let mut remaining = kumulatif;
    let mut total_tax = dec!(0);
    let mut previous_limit = dec!(0);

    for bracket in get_gelir_vergisi_dilimleri_2026() {
        let bracket_size = bracket.limit - previous_limit;
        let taxable_in_bracket = remaining.min(bracket_size);
        if taxable_in_bracket > dec!(0) {
            total_tax += taxable_in_bracket * bracket.oran;
            remaining -= taxable_in_bracket;
        }
        previous_limit = bracket.limit;
        if remaining <= dec!(0) {
            break;
        }
    }
    total_tax
}

pub fn calculate_gelir_vergisi_2026(matrah: Decimal, kumulatif_onceki: Decimal) -> Decimal {
    if matrah <= dec!(0) {
        return dec!(0);
    }
    let total_tax_current = calculate_total_tax_for_cumulative_matrah(kumulatif_onceki + matrah);
    let total_tax_previous = calculate_total_tax_for_cumulative_matrah(kumulatif_onceki);
    round2(total_tax_current - total_tax_previous)
}

pub struct NightWorkPolicy;

impl NightWorkPolicy {
    pub fn calculate_gece_calismasi_primi(
        gunluk_taban_ucret: Decimal,
        gece_calisma_orani_yuzde: Decimal,
        gc_gun_sayisi: i32,
    ) -> Decimal {
        if gc_gun_sayisi <= 0 || gece_calisma_orani_yuzde <= dec!(0) {
            return dec!(0);
        }
        round2(gunluk_taban_ucret * (gece_calisma_orani_yuzde / dec!(100)) * Decimal::from(gc_gun_sayisi))
    }

    pub fn calculate_gece_calismasi_tatili_primi(
        gunluk_taban_ucret: Decimal,
        gece_calisma_tatili_orani_yuzde: Decimal,
        gct_gun_sayisi: i32,
    ) -> Decimal {
        if gct_gun_sayisi <= 0 || gece_calisma_tatili_orani_yuzde <= dec!(0) {
            return dec!(0);
        }
        round2(gunluk_taban_ucret * (gece_calisma_tatili_orani_yuzde / dec!(100)) * Decimal::from(gct_gun_sayisi))
    }
}

pub fn calculate_gelir_toplam(gelirler: &GelirKalemleri) -> Decimal {
    let mut sum = dec!(0);
    if let Some(v) = gelirler.tabanBrutAylik { sum += v; }
    if let Some(v) = gelirler.tediye { sum += v; }
    if let Some(v) = gelirler.tisIkramiyesi { sum += v; }
    if let Some(v) = gelirler.ekOdeme { sum += v; }
    if let Some(v) = gelirler.yemek { sum += v; }
    if let Some(v) = gelirler.birlestirilmisSosyalYardim { sum += v; }
    if let Some(v) = gelirler.vasitaYol { sum += v; }
    if let Some(v) = gelirler.giyimYardimi { sum += v; }
    if let Some(v) = gelirler.isPrimi { sum += v; }
    if let Some(v) = gelirler.geceCalismasiUcreti { sum += v; }
    if let Some(v) = gelirler.geceCalismasiTatiliUcreti { sum += v; }
    if let Some(v) = gelirler.hizmetZammi { sum += v; }
    if let Some(v) = gelirler.digerGelir { sum += v; }
    round2(sum)
}

pub fn calculate_kesinti_toplam(kesintiler: &KesintiKalemleri) -> Decimal {
    let mut sum = dec!(0);
    if let Some(v) = kesintiler.isciSgkPrimi { sum += v; }
    if let Some(v) = kesintiler.isciIssizlikPrimi { sum += v; }
    if let Some(v) = kesintiler.gelirVergisi { sum += v; }
    if let Some(v) = kesintiler.damgaVergisi { sum += v; }
    if let Some(v) = kesintiler.sendikaAidati { sum += v; }
    if let Some(v) = kesintiler.bes { sum += v; }
    if let Some(v) = kesintiler.icra { sum += v; }
    if let Some(v) = kesintiler.kisiBorcu { sum += v; }
    if let Some(v) = kesintiler.dogumAskerlikBorclanmasi { sum += v; }
    if let Some(v) = kesintiler.hayatSaglikSigortasi { sum += v; }
    if let Some(v) = kesintiler.digerKesinti { sum += v; }
    round2(sum)
}

pub fn get_grup_is_primi_orani(
    grup: Option<&str>,
    is_primi_gruplari: Option<&[IsPrimiGrupItem]>,
) -> Decimal {
    let default_list = vec![
        IsPrimiGrupItem { id: "1. Grup".into(), ad: "1. Grup".into(), oran: dec!(9) },
        IsPrimiGrupItem { id: "2. Grup".into(), ad: "2. Grup".into(), oran: dec!(8) },
        IsPrimiGrupItem { id: "3. Grup".into(), ad: "3. Grup".into(), oran: dec!(7) },
    ];
    let list = is_primi_gruplari.unwrap_or(&default_list);

    let grp_str = match grup {
        Some(g) if !g.is_empty() => g,
        _ => return list.first().map(|g| g.oran).unwrap_or(dec!(9)),
    };

    if let Some(found) = list.iter().find(|g| g.ad == grp_str || g.id == grp_str) {
        return found.oran;
    }

    if grp_str.contains("1. Grup") { return dec!(9); }
    if grp_str.contains("2. Grup") { return dec!(8); }
    if grp_str.contains("3. Grup") { return dec!(7); }

    list.first().map(|g| g.oran).unwrap_or(dec!(9))
}

pub fn auto_fill_gelirler_from_puantaj(
    puantaj_ozeti: &PuantajOzeti,
    kurum_degerleri: &DonemselKurumDegerleri,
    hizmet_yili: i32,
    grup: Option<&str>,
) -> GelirKalemleri {
    let hakedis_gun = puantaj_ozeti.c + puantaj_ozeti.t + puantaj_ozeti.g + puantaj_ozeti.i + puantaj_ozeti.gc + puantaj_ozeti.gct;
    let hakedis_dec = Decimal::from(hakedis_gun);
    let fiili_calisma_gun = Decimal::from(puantaj_ozeti.c + puantaj_ozeti.gc);

    let taban_brut_aylik = round2(hakedis_dec * kurum_degerleri.gunlukTabanUcret);
    let yemek = round2(fiili_calisma_gun * kurum_degerleri.gunlukYemek);
    let vasita_yol = round2(fiili_calisma_gun * kurum_degerleri.gunlukVasitaYol);
    let birlestirilmis_sosyal_yardim = kurum_degerleri.birlestirilmisSosyalYardim;
    let giyim_yardimi = kurum_degerleri.giyimYardimi;
    let hizmet_zammi = round2(Decimal::from(hizmet_yili) * kurum_degerleri.hizmetZammiBirimi);

    let is_primi_orani = if let Some(custom) = kurum_degerleri.isPrimiYuzde {
        if custom > dec!(0) { custom } else { get_grup_is_primi_orani(grup, kurum_degerleri.isPrimiGruplari.as_deref()) }
    } else {
        get_grup_is_primi_orani(grup, kurum_degerleri.isPrimiGruplari.as_deref())
    };

    let gunluk_is_primi = round2(kurum_degerleri.gunlukTabanUcret * (is_primi_orani / dec!(100)));
    let is_primi = round2(gunluk_is_primi * fiili_calisma_gun);

    let gc_orani = kurum_degerleri.geceCalismaPrimiYuzde.unwrap_or(dec!(0));
    let gct_orani = kurum_degerleri.geceCalismaTatiliPrimiYuzde.unwrap_or(dec!(0));

    let gece_calismasi_ucreti = NightWorkPolicy::calculate_gece_calismasi_primi(
        kurum_degerleri.gunlukTabanUcret,
        gc_orani,
        puantaj_ozeti.gc,
    );

    let gece_calismasi_tatili_ucreti = NightWorkPolicy::calculate_gece_calismasi_tatili_primi(
        kurum_degerleri.gunlukTabanUcret,
        gct_orani,
        puantaj_ozeti.gct,
    );

    let mut tediye: Option<Decimal> = None;
    if let Some(ref t_list) = kurum_degerleri.tediyeListesi {
        if let Some(active) = t_list.iter().find(|t| t.aktifDonemdeOdensin) {
            tediye = Some(match active.sabitTutar {
                Some(sabit) if sabit > dec!(0) => sabit,
                _ => round2(Decimal::from(active.gunSayisi) * kurum_degerleri.gunlukTabanUcret),
            });
        }
    }

    let mut tis_ikramiyesi: Option<Decimal> = None;
    if let Some(ref tis_list) = kurum_degerleri.tisIkramiyeListesi {
        if let Some(active) = tis_list.iter().find(|t| t.aktifDonemdeOdensin) {
            tis_ikramiyesi = Some(match active.sabitTutar {
                Some(sabit) if sabit > dec!(0) => sabit,
                _ => round2(Decimal::from(active.gunSayisi) * kurum_degerleri.gunlukTabanUcret),
            });
        }
    }

    GelirKalemleri {
        tabanBrutAylik: Some(taban_brut_aylik),
        tediye,
        tisIkramiyesi: tis_ikramiyesi,
        ekOdeme: kurum_degerleri.ekOdeme,
        yemek: Some(yemek),
        birlestirilmisSosyalYardim: Some(birlestirilmis_sosyal_yardim),
        vasitaYol: Some(vasita_yol),
        giyimYardimi: Some(giyim_yardimi),
        isPrimi: Some(is_primi),
        geceCalismasiUcreti: Some(gece_calismasi_ucreti),
        geceCalismasiTatiliUcreti: Some(gece_calismasi_tatili_ucreti),
        hizmetZammi: Some(hizmet_zammi),
        digerGelir: kurum_degerleri.digerGelirVarsayilan,
    }
}

pub fn calculate_prime_esas_kazanc(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = min(30, raw_prim_gun.max(0));
    let fiili_yemek_gunu = puantaj_ozeti.map_or(0, |p| p.c + p.gc);

    let default_k = DonemselKurumDegerleri::default();
    let k = kurum_degerleri.unwrap_or(&default_k);

    let gunluk_yemek_istisnasi = k.gunlukYemekIstisnasiSGK.unwrap_or(dec!(300.00));
    let brut_yemek = gelirler.yemek.unwrap_or(dec!(0));
    let yemek_istisnasi_tutar = brut_yemek.min(gunluk_yemek_istisnasi * Decimal::from(fiili_yemek_gunu));
    let sgk_tabi_yemek = (brut_yemek - yemek_istisnasi_tutar).max(dec!(0));

    let vasita_yol = gelirler.vasitaYol.unwrap_or(dec!(0));
    let taban_brut = gelirler.tabanBrutAylik.unwrap_or(dec!(0));
    let tediye = gelirler.tediye.unwrap_or(dec!(0));
    let tis_ikramiyesi = gelirler.tisIkramiyesi.unwrap_or(dec!(0));
    let ek_odeme = gelirler.ekOdeme.unwrap_or(dec!(0));
    let birlestirilmis_sosyal_yardim = gelirler.birlestirilmisSosyalYardim.unwrap_or(dec!(0));
    let giyim_yardimi = gelirler.giyimYardimi.unwrap_or(dec!(0));
    let is_primi = gelirler.isPrimi.unwrap_or(dec!(0));
    let gece_calismasi = gelirler.geceCalismasiUcreti.unwrap_or(dec!(0));
    let gece_calismasi_tatili = gelirler.geceCalismasiTatiliUcreti.unwrap_or(dec!(0));
    let hizmet_zammi = gelirler.hizmetZammi.unwrap_or(dec!(0));
    let diger_gelir = gelirler.digerGelir.unwrap_or(dec!(0));

    let ucretler = taban_brut
        + sgk_tabi_yemek
        + vasita_yol
        + birlestirilmis_sosyal_yardim
        + giyim_yardimi
        + hizmet_zammi
        + diger_gelir
        + gece_calismasi
        + gece_calismasi_tatili;

    let ucret_disi_odemeler = tediye + tis_ikramiyesi + ek_odeme + is_primi;
    let ham_pek = ucretler + ucret_disi_odemeler;

    let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
    let pek_alt_sinir = round2(gunluk_asgari * Decimal::from(prim_gun_sayisi));
    let tavan_katsayi = k.pekTavanKatsayisi.unwrap_or(dec!(9));
    let pek_ust_sinir = round2(gunluk_asgari * tavan_katsayi * Decimal::from(prim_gun_sayisi));

    let mut pek_matrah_adayi = ham_pek;
    let mut eklenecek_devreden_toplam = dec!(0);
    let mut sonraki_devreden_list: Vec<DevredenPekKaydi> = Vec::new();

    for item in devreden_pek_gelen {
        if item.tutar <= dec!(0) || item.kalanAySayisi <= 0 {
            continue;
        }
        let tavan_boslugu = (pek_ust_sinir - pek_matrah_adayi).max(dec!(0));
        let eklenecek = item.tutar.min(tavan_boslugu);

        if eklenecek > dec!(0) {
            pek_matrah_adayi += eklenecek;
            eklenecek_devreden_toplam += eklenecek;
        }

        let kalan_tutar = round2(item.tutar - eklenecek);
        if kalan_tutar > dec!(0) && item.kalanAySayisi > 1 {
            sonraki_devreden_list.push(DevredenPekKaydi {
                tutar: kalan_tutar,
                kalanAySayisi: item.kalanAySayisi - 1,
                kaynakDonemId: item.kaynakDonemId.clone(),
            });
        }
    }

    let mut devreden_pek_asan_tutar = dec!(0);
    if pek_matrah_adayi > pek_ust_sinir {
        let ucret_tavan_kapasitesi = (pek_ust_sinir - ucretler - eklenecek_devreden_toplam).max(dec!(0));
        let ucret_disi_kullanilan = ucret_disi_odemeler.min(ucret_tavan_kapasitesi);
        devreden_pek_asan_tutar = round2((ucret_disi_odemeler - ucret_disi_kullanilan).max(dec!(0)));
        pek_matrah_adayi = pek_ust_sinir;
    }

    if devreden_pek_asan_tutar > dec!(0) {
        sonraki_devreden_list.push(DevredenPekKaydi {
            tutar: devreden_pek_asan_tutar,
            kalanAySayisi: 2,
            kaynakDonemId: None,
        });
    }

    let mut final_pek = pek_matrah_adayi;
    if final_pek < pek_alt_sinir && ham_pek > dec!(0) {
        final_pek = pek_alt_sinir;
    }

    let isveren_sgk_rate = k.sgkIsverenOraniYuzde.unwrap_or(dec!(21.75)) / dec!(100);
    let isveren_issizlik_rate = k.issizlikIsverenOraniYuzde.unwrap_or(dec!(2.00)) / dec!(100);

    let isveren_sgk_primi = round2(final_pek * isveren_sgk_rate);
    let isveren_issizlik_primi = round2(final_pek * isveren_issizlik_rate);
    let isveren_prim_toplami = isveren_sgk_primi + isveren_issizlik_primi;

    let det = PekDetayi {
        hesaplananPek: round2(ham_pek),
        finalPek: round2(final_pek),
        devredenPekAşanTutar: devreden_pek_asan_tutar,
        pekAltSinir: pek_alt_sinir,
        pekUstSinir: pek_ust_sinir,
        fiiliYemekGunu: fiili_yemek_gunu,
        yemekIstisnasiTutar: round2(yemek_istisnasi_tutar),
        isverenSgkPrimi: Some(isveren_sgk_primi),
        isverenIssizlikPrimi: Some(isveren_issizlik_primi),
        isverenPrimToplami: Some(isveren_prim_toplami),
        sgkIsverenOraniYuzde: Some(k.sgkIsverenOraniYuzde.unwrap_or(dec!(21.75))),
        isverenIssizlikOraniYuzde: Some(k.issizlikIsverenOraniYuzde.unwrap_or(dec!(2.00))),
    };

    (det, sonraki_devreden_list)
}

pub fn calculate_statutory_deductions(
    gelirler: &GelirKalemleri,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    personel: Option<&Personel>,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kumulatif_gv_matrahi_onceki: Decimal,
    devreden_pek_gelen: &[DevredenPekKaydi],
    kumulatif_asgari_gv_onceki: Decimal,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {
    let brut_gelir = calculate_gelir_toplam(gelirler);
    let default_k = DonemselKurumDegerleri::default();
    let k = kurum_degerleri.unwrap_or(&default_k);

    if brut_gelir <= dec!(0) {
        let (pek_detay, sonraki) = calculate_prime_esas_kazanc(gelirler, puantaj_ozeti, kurum_degerleri, devreden_pek_gelen);
        return (KesintiKalemleri::default(), pek_detay, sonraki);
    }

    let (pek_detay, sonraki_devreden) = calculate_prime_esas_kazanc(gelirler, puantaj_ozeti, kurum_degerleri, devreden_pek_gelen);
    let pek_matrah = pek_detay.finalPek;

    let sgk_rate = k.sgkIsciOraniYuzde.unwrap_or(dec!(14)) / dec!(100);
    let issizlik_rate = k.issizlikIsciOraniYuzde.unwrap_or(dec!(1)) / dec!(100);
    let dv_rate = k.damgaVergisiOraniBinde.unwrap_or(dec!(7.59)) / dec!(1000);

    let isci_sgk_primi = round2(pek_matrah * sgk_rate);
    let isci_issizlik_primi = round2(pek_matrah * issizlik_rate);

    let gelir_vergisi_matrah = (brut_gelir - isci_sgk_primi - isci_issizlik_primi).max(dec!(0));

    let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
    let aylik_brut_asgari = round2(gunluk_asgari * dec!(30));
    let aylik_asgari_sgk = round2(aylik_brut_asgari * (sgk_rate + issizlik_rate));
    let asgari_ucret_gv_matrah = (aylik_brut_asgari - aylik_asgari_sgk).max(dec!(0));

    let asgari_ucret_gv_istisnasi = calculate_gelir_vergisi_2026(asgari_ucret_gv_matrah, kumulatif_asgari_gv_onceki);
    let ham_gelir_vergisi = calculate_gelir_vergisi_2026(gelir_vergisi_matrah, kumulatif_gv_matrahi_onceki);
    let gelir_vergisi = (round2(ham_gelir_vergisi - asgari_ucret_gv_istisnasi)).max(dec!(0));

    let asgari_ucret_dv_istisnasi = round2(aylik_brut_asgari * dv_rate);
    let ham_damga_vergisi = round2(brut_gelir * dv_rate);
    let damga_vergisi = (round2(ham_damga_vergisi - asgari_ucret_dv_istisnasi)).max(dec!(0));

    let p_kesintiler = personel.and_then(|p| p.kesintiler.as_ref());

    let is_sendika = p_kesintiler.and_then(|pk| pk.sendikaUyesi).unwrap_or(false);
    let sendika_aidati = if is_sendika {
        if let Some(sabit) = p_kesintiler.and_then(|pk| pk.sabitSendikaAidati) {
            if sabit > dec!(0) { Some(sabit) } else { None }
        } else if let Some(sabit) = k.sabitSendikaAidati {
            if sabit > dec!(0) { Some(sabit) } else { None }
        } else {
            None
        }.unwrap_or_else(|| {
            let sendika_orani = k.sendikaAidatiYuzde.unwrap_or(dec!(65)) / dec!(100);
            round2(k.gunlukTabanUcret * sendika_orani)
        })
    } else {
        dec!(0)
    };

    let is_oks = p_kesintiler.and_then(|pk| pk.besUyesi).unwrap_or(false);
    let bes = if is_oks {
        if let Some(sabit) = p_kesintiler.and_then(|pk| pk.sabitBesTutar) {
            if sabit > dec!(0) { Some(sabit) } else { None }
        } else if let Some(sabit) = k.sabitBesTutar {
            if sabit > dec!(0) { Some(sabit) } else { None }
        } else {
            None
        }.unwrap_or_else(|| {
            let custom_oran = p_kesintiler.and_then(|pk| pk.oksOraniYuzde);
            let oks_orani = custom_oran.unwrap_or_else(|| k.besOraniYuzde.unwrap_or(dec!(3))) / dec!(100);
            floor_dec(pek_matrah * oks_orani)
        })
    } else {
        dec!(0)
    };

    let icra = p_kesintiler.and_then(|pk| pk.icraTutar).filter(|v| *v > dec!(0));
    let kisi_borcu = p_kesintiler.and_then(|pk| pk.kisiBorcuTutar).filter(|v| *v > dec!(0));
    let dogum_askerlik = p_kesintiler.and_then(|pk| pk.dogumAskerlikBorclanmasiTutar).filter(|v| *v > dec!(0));
    let hayat_saglik = p_kesintiler.and_then(|pk| pk.hayatSaglikSigortasiTutar).filter(|v| *v > dec!(0));
    let diger_kesinti = p_kesintiler.and_then(|pk| pk.digerKesintiTutar).filter(|v| *v > dec!(0));

    let kesintiler = KesintiKalemleri {
        isciSgkPrimi: Some(isci_sgk_primi),
        isciIssizlikPrimi: Some(isci_issizlik_primi),
        gelirVergisi: Some(gelir_vergisi),
        damgaVergisi: Some(damga_vergisi),
        sendikaAidati: Some(sendika_aidati),
        bes: Some(bes),
        icra,
        kisiBorcu: kisi_borcu,
        dogumAskerlikBorclanmasi: dogum_askerlik,
        hayatSaglikSigortasi: hayat_saglik,
        digerKesinti: diger_kesinti,
    };

    (kesintiler, pek_detay, sonraki_devreden)
}
