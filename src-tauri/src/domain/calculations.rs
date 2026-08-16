use super::errors::DomainError;
use super::models::*;
use super::Result;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal_macros::dec;
use std::cmp::min;
use std::collections::HashSet;

fn round2(val: Decimal) -> Decimal {
    val.round_dp(2)
}

/// Gelir vergisi kalemlerinin parasal yuvarlama politikası (GİB uygulaması).
/// Yarım kuruşluk değerler sıfırdan uzağa yuvarlanır (MidpointAwayFromZero / banker's
/// rounding değil): Ocak asgari istisnası tax(28.075,50) = 4.211,325 → 4.211,33.
/// Yalnız GV kalemlerinde (brüt GV, asgari istisna, kesilen GV ve asgari ara değerler)
/// kullanılır. `round2` (MidpointNearestEven) ve `round_sgk_amount` dokunulmaz.
fn round_gv_amount(val: Decimal) -> Decimal {
    val.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// SGK/işsizlik prim tahakkuklarının parasal yuvarlama politikası.
/// SGK'nın resmî örneklerine uygun olarak midpoint değerler sıfırdan uzağa yuvarlanır
/// (banker's rounding değil): 33.030 × %21,75 = 7.184,025 → 7.184,03.
/// Genel round2()'dan (MidpointNearestEven) farklıdır ve yalnız SGK prim kalemlerinde kullanılır.
fn round_sgk_amount(val: Decimal) -> Decimal {
    val.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn floor_dec(val: Decimal) -> Decimal {
    val.floor()
}

/// Test/geriye dönük fixture için 2026 tarifesi. Üretim hesaplaması bu değeri
/// doğrudan kullanmaz; `annual_payroll_parameters` tablosundaki tarife kullanılır.
pub fn default_gelir_vergisi_dilimleri_2026() -> Vec<TaxBracket> {
    vec![
        TaxBracket {
            limit: dec!(190000),
            oran: dec!(0.15),
        },
        TaxBracket {
            limit: dec!(400000),
            oran: dec!(0.20),
        },
        TaxBracket {
            limit: dec!(1500000),
            oran: dec!(0.27),
        },
        TaxBracket {
            limit: dec!(5300000),
            oran: dec!(0.35),
        },
        TaxBracket {
            limit: Decimal::from(OPEN_ENDED_TAX_BRACKET_LIMIT),
            oran: dec!(0.40),
        },
    ]
}

pub fn calculate_total_tax_for_cumulative_matrah_with_brackets(
    kumulatif: Decimal,
    brackets: &[TaxBracket],
) -> Decimal {
    if kumulatif <= dec!(0) {
        return dec!(0);
    }
    let mut remaining = kumulatif;
    let mut total_tax = dec!(0);
    let mut previous_limit = dec!(0);

    for bracket in brackets {
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

    // Son dilim, kullanıcı arayüzünde pratik bir üst limit ile girilmiş olsa
    // bile, bu limitin üzerindeki matrahı da kendi oranıyla kapsar. Böylece
    // eksik/sonu sonsuz olmayan bir tarife yanlışlıkla matrahın kalanını
    // vergisiz bırakmaz.
    match brackets.last() {
        Some(last) if remaining > dec!(0) => total_tax += remaining * last.oran,
        _ => {}
    }
    total_tax
}

/// Test fixture/geriye dönük API. Üretim yolu yıllık parametre tablosunu kullanır.
pub fn calculate_total_tax_for_cumulative_matrah(kumulatif: Decimal) -> Decimal {
    calculate_total_tax_for_cumulative_matrah_with_brackets(
        kumulatif,
        &default_gelir_vergisi_dilimleri_2026(),
    )
}

pub fn calculate_gelir_vergisi_with_brackets(
    matrah: Decimal,
    kumulatif_onceki: Decimal,
    brackets: &[TaxBracket],
) -> Decimal {
    if matrah <= dec!(0) {
        return dec!(0);
    }
    let total_tax_current = calculate_total_tax_for_cumulative_matrah_with_brackets(
        kumulatif_onceki + matrah,
        brackets,
    );
    let total_tax_previous =
        calculate_total_tax_for_cumulative_matrah_with_brackets(kumulatif_onceki, brackets);
    round_gv_amount(total_tax_current - total_tax_previous)
}

/// Test fixture/geriye dönük API. Üretim yolu yıllık parametre tablosunu kullanır.
pub fn calculate_gelir_vergisi_2026(matrah: Decimal, kumulatif_onceki: Decimal) -> Decimal {
    calculate_gelir_vergisi_with_brackets(
        matrah,
        kumulatif_onceki,
        &default_gelir_vergisi_dilimleri_2026(),
    )
}

/// GV matrahı = brüt gelir - işçi SGK - işçi işsizlik (negatif olamaz).
pub fn calculate_gv_matrah(
    brut_gelir: Decimal,
    isci_sgk: Decimal,
    isci_issizlik: Decimal,
) -> Decimal {
    (brut_gelir - isci_sgk - isci_issizlik).max(dec!(0))
}

/// Asgari ücretin takvim referans aylık GV matrahı:
/// aylık brüt asgari - (işçi SGK + işsizlik). Oranlar 0-1 aralığındadır (ör. 0.14).
pub fn calculate_aylik_asgari_ucret_gv_matrahi(
    gunluk_asgari: Decimal,
    sgk_isci_orani: Decimal,
    issizlik_isci_orani: Decimal,
) -> Decimal {
    let aylik_brut_asgari = round2(gunluk_asgari * dec!(30));
    let aylik_asgari_sgk = round2(aylik_brut_asgari * (sgk_isci_orani + issizlik_isci_orani));
    (aylik_brut_asgari - aylik_asgari_sgk).max(dec!(0))
}

/// Gelir vergisi bloğunun tam, denetlenebilir hesap detayını döndürür.
/// İstisna BİR kez toplam cari matraha uygulanır (matrah indirimi değil, vergi
/// düşümü); kesilecek GV negatif olamaz; GV kalemlerinde round_gv_amount kullanılır.
pub fn calculate_gv_hesap_detayi_with_brackets(
    cari_gv_matrahi: Decimal,
    kumulatif_gv_matrahi_onceki: Decimal,
    asgari_ucret_aylik_gv_matrahi: Decimal,
    kumulatif_asgari_gv_onceki: Decimal,
    brackets: &[TaxBracket],
) -> GvHesapDetayi {
    let brut_gelir_vergisi = calculate_gelir_vergisi_with_brackets(
        cari_gv_matrahi,
        kumulatif_gv_matrahi_onceki,
        brackets,
    );
    let asgari_ucret_gv_istisnasi = calculate_gelir_vergisi_with_brackets(
        asgari_ucret_aylik_gv_matrahi,
        kumulatif_asgari_gv_onceki,
        brackets,
    );
    let uygulanan_gv_istisnasi = min(brut_gelir_vergisi, asgari_ucret_gv_istisnasi);
    let kesilen_gelir_vergisi = (brut_gelir_vergisi - uygulanan_gv_istisnasi).max(dec!(0));

    GvHesapDetayi {
        cariGvMatrahi: cari_gv_matrahi,
        yeniKumulatifGvMatrahi: kumulatif_gv_matrahi_onceki + cari_gv_matrahi,
        brutGelirVergisi: brut_gelir_vergisi,
        asgariUcretGvMatrahi: asgari_ucret_aylik_gv_matrahi,
        asgariUcretReferansKumulatifMatrahi: kumulatif_asgari_gv_onceki
            + asgari_ucret_aylik_gv_matrahi,
        asgariUcretGvIstisnasi: asgari_ucret_gv_istisnasi,
        uygulananGvIstisnasi: uygulanan_gv_istisnasi,
        kesilenGelirVergisi: kesilen_gelir_vergisi,
        dogumAskerlikGvIndirimi: dec!(0),
        sigortaGvIndirimAdayi: dec!(0),
        sigortaGvAylikLimiti: dec!(0),
        sigortaGvYillikKalanLimiti: dec!(0),
        uygulanabilirSigortaGvIndirimi: dec!(0),
    }
}

/// Test fixture/geriye dönük API. Üretim yolu yıllık parametre tablosunu kullanır.
pub fn calculate_gv_hesap_detayi(
    cari_gv_matrahi: Decimal,
    kumulatif_gv_matrahi_onceki: Decimal,
    asgari_ucret_aylik_gv_matrahi: Decimal,
    kumulatif_asgari_gv_onceki: Decimal,
) -> GvHesapDetayi {
    calculate_gv_hesap_detayi_with_brackets(
        cari_gv_matrahi,
        kumulatif_gv_matrahi_onceki,
        asgari_ucret_aylik_gv_matrahi,
        kumulatif_asgari_gv_onceki,
        &default_gelir_vergisi_dilimleri_2026(),
    )
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
        round2(
            gunluk_taban_ucret
                * (gece_calisma_orani_yuzde / dec!(100))
                * Decimal::from(gc_gun_sayisi),
        )
    }

    pub fn calculate_gece_calismasi_tatili_primi(
        gunluk_taban_ucret: Decimal,
        gece_calisma_tatili_orani_yuzde: Decimal,
        gct_gun_sayisi: i32,
    ) -> Decimal {
        if gct_gun_sayisi <= 0 || gece_calisma_tatili_orani_yuzde <= dec!(0) {
            return dec!(0);
        }
        round2(
            gunluk_taban_ucret
                * (gece_calisma_tatili_orani_yuzde / dec!(100))
                * Decimal::from(gct_gun_sayisi),
        )
    }
}

pub fn calculate_gelir_toplam(gelirler: &GelirKalemleri) -> Decimal {
    let mut sum = dec!(0);
    if let Some(v) = gelirler.tabanBrutAylik {
        sum += v;
    }
    if let Some(v) = gelirler.tediye {
        sum += v;
    }
    if let Some(v) = gelirler.tisIkramiyesi {
        sum += v;
    }
    if let Some(v) = gelirler.ekOdeme {
        sum += v;
    }
    if let Some(v) = gelirler.yemek {
        sum += v;
    }
    if let Some(v) = gelirler.birlestirilmisSosyalYardim {
        sum += v;
    }
    if let Some(v) = gelirler.vasitaYol {
        sum += v;
    }
    if let Some(v) = gelirler.giyimYardimi {
        sum += v;
    }
    if let Some(v) = gelirler.isPrimi {
        sum += v;
    }
    if let Some(v) = gelirler.geceCalismasiUcreti {
        sum += v;
    }
    if let Some(v) = gelirler.geceCalismasiTatiliUcreti {
        sum += v;
    }
    if let Some(v) = gelirler.hizmetZammi {
        sum += v;
    }
    if let Some(v) = gelirler.digerGelir {
        sum += v;
    }
    round2(sum)
}

pub fn calculate_kesinti_toplam(kesintiler: &KesintiKalemleri) -> Decimal {
    let mut sum = dec!(0);
    if let Some(v) = kesintiler.isciSgkPrimi {
        sum += v;
    }
    if let Some(v) = kesintiler.isciIssizlikPrimi {
        sum += v;
    }
    if let Some(v) = kesintiler.gelirVergisi {
        sum += v;
    }
    if let Some(v) = kesintiler.damgaVergisi {
        sum += v;
    }
    if let Some(v) = kesintiler.sendikaAidati {
        sum += v;
    }
    if let Some(v) = kesintiler.bes {
        sum += v;
    }
    if let Some(v) = kesintiler.icra {
        sum += v;
    }
    if let Some(v) = kesintiler.kisiBorcu {
        sum += v;
    }
    if let Some(v) = kesintiler.dogumAskerlikBorclanmasi {
        sum += v;
    }
    if let Some(v) = kesintiler.hayatSaglikSigortasi {
        sum += v;
    }
    if let Some(v) = kesintiler.digerKesinti {
        sum += v;
    }
    round2(sum)
}

/// Personelin is primi grubunu tanimli gruplar arasinda cozer ve grubu dondurur.
///
/// Is primi oraninin tek authoritative kaynagi personelin grubudur:
/// kurum genelindeki tek oran (isPrimiYuzde) burada kullanilmaz. Sessiz fallback yok.
/// Grup bos/tanimsiz, hicbir aktif kayitla eslesmiyor, grup pasif veya oran gecersizse
/// [DomainError::ValidationError] doner; motor tahmini tutar uretmez.
pub fn resolve_is_primi_grubu(
    grup: Option<&str>,
    is_primi_gruplari: Option<&[IsPrimiGrupItem]>,
) -> Result<IsPrimiGrupItem> {
    let grp = match grup {
        Some(g) if !g.trim().is_empty() => g.trim(),
        _ => {
            return Err(DomainError::ValidationError(
                "Personelin is primi grubu tanimli degil.".to_string(),
            ))
        }
    };

    let list = match is_primi_gruplari {
        Some(list) if !list.is_empty() => list,
        _ => {
            return Err(DomainError::ValidationError(format!(
                "Is primi gruplari tanimli degil. Personel grubu: '{}'. Tanimli gruplardan birine atayin.",
                grp
            )))
        }
    };

    if let Some(found) = list
        .iter()
        .find(|g| (g.ad == grp || g.id == grp) && g.aktif)
    {
        if found.oran < dec!(0) {
            Err(DomainError::ValidationError(format!(
                "Is primi grubu '{}' orani gecersiz (negatif).",
                found.ad
            )))
        } else {
            Ok(found.clone())
        }
    } else if let Some(pasif) = list.iter().find(|g| g.ad == grp || g.id == grp) {
        Err(DomainError::ValidationError(format!(
            "Is primi grubu '{}' pasif durumda ve kullanilamaz.",
            pasif.ad
        )))
    } else {
        Err(DomainError::ValidationError(format!(
            "Personelin is primi grubu gecersiz: '{}'. Tanimli gruplardan birini secin.",
            grp
        )))
    }
}

/// Production bordro hesabında sessiz yasal parametre varsayılanlarına izin
/// vermez. UI/test kolaylığı için kullanılan düşük seviyeli hesap fonksiyonları
/// bu doğrulamayı kendileri çağırmaz; authoritative servis yolu çağırır.
pub fn validate_kurum_degerleri_for_payroll(
    kurum_degerleri: &DonemselKurumDegerleri,
) -> Result<()> {
    if kurum_degerleri.gunlukTabanUcret <= Decimal::ZERO {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi günlük taban ücreti sıfırdan büyük olmalıdır.",
            kurum_degerleri.donemId
        )));
    }
    for (field, value) in [
        ("gunlukYemek", kurum_degerleri.gunlukYemek),
        (
            "birlestirilmisSosyalYardim",
            kurum_degerleri.birlestirilmisSosyalYardim,
        ),
        ("gunlukVasitaYol", kurum_degerleri.gunlukVasitaYol),
        ("giyimYardimi", kurum_degerleri.giyimYardimi),
        ("hizmetZammiBirimi", kurum_degerleri.hizmetZammiBirimi),
    ] {
        if value < Decimal::ZERO {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi kurum değeri negatif olamaz: {}.",
                kurum_degerleri.donemId, field
            )));
        }
    }

    let required_non_negative = [
        ("sgkIsciOraniYuzde", kurum_degerleri.sgkIsciOraniYuzde),
        (
            "issizlikIsciOraniYuzde",
            kurum_degerleri.issizlikIsciOraniYuzde,
        ),
        ("sendikaAidatiYuzde", kurum_degerleri.sendikaAidatiYuzde),
        ("besOraniYuzde", kurum_degerleri.besOraniYuzde),
        (
            "geceCalismaPrimiYuzde",
            kurum_degerleri.geceCalismaPrimiYuzde,
        ),
        (
            "geceCalismaTatiliPrimiYuzde",
            kurum_degerleri.geceCalismaTatiliPrimiYuzde,
        ),
        (
            "gunlukYemekIstisnasiSGK",
            kurum_degerleri.gunlukYemekIstisnasiSGK,
        ),
        (
            "gunlukYemekIstisnasiGV",
            kurum_degerleri.gunlukYemekIstisnasiGV,
        ),
        ("gunlukAsgariUcret", kurum_degerleri.gunlukAsgariUcret),
        ("pekTavanKatsayisi", kurum_degerleri.pekTavanKatsayisi),
        ("sgkIsverenOraniYuzde", kurum_degerleri.sgkIsverenOraniYuzde),
        (
            "issizlikIsverenOraniYuzde",
            kurum_degerleri.issizlikIsverenOraniYuzde,
        ),
    ];
    for (field, value) in required_non_negative {
        let value = value.ok_or_else(|| {
            DomainError::ValidationError(format!(
                "{} dönemi için zorunlu yasal parametre eksik: {}.",
                kurum_degerleri.donemId, field
            ))
        })?;
        if value < Decimal::ZERO {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi yasal parametresi negatif olamaz: {}.",
                kurum_degerleri.donemId, field
            )));
        }
    }

    let percent_fields = [
        ("sgkIsciOraniYuzde", kurum_degerleri.sgkIsciOraniYuzde),
        (
            "issizlikIsciOraniYuzde",
            kurum_degerleri.issizlikIsciOraniYuzde,
        ),
        ("sendikaAidatiYuzde", kurum_degerleri.sendikaAidatiYuzde),
        ("besOraniYuzde", kurum_degerleri.besOraniYuzde),
        (
            "geceCalismaPrimiYuzde",
            kurum_degerleri.geceCalismaPrimiYuzde,
        ),
        (
            "geceCalismaTatiliPrimiYuzde",
            kurum_degerleri.geceCalismaTatiliPrimiYuzde,
        ),
        ("sgkIsverenOraniYuzde", kurum_degerleri.sgkIsverenOraniYuzde),
        (
            "issizlikIsverenOraniYuzde",
            kurum_degerleri.issizlikIsverenOraniYuzde,
        ),
    ];
    for (field, value) in percent_fields {
        if let Some(value) = value {
            if value > dec!(100) {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemi yasal parametresi %0-%100 arasında olmalıdır: {}.",
                    kurum_degerleri.donemId, field
                )));
            }
        }
    }

    let damga = kurum_degerleri.damgaVergisiOraniBinde.ok_or_else(|| {
        DomainError::ValidationError(format!(
            "{} dönemi için zorunlu yasal parametre eksik: damgaVergisiOraniBinde.",
            kurum_degerleri.donemId
        ))
    })?;
    if !(Decimal::ZERO..=dec!(1000)).contains(&damga) {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi damga vergisi oranı binde 0-1000 arasında olmalıdır.",
            kurum_degerleri.donemId
        )));
    }

    if kurum_degerleri
        .gunlukAsgariUcret
        .is_some_and(|value| value <= Decimal::ZERO)
    {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi günlük asgari ücret sıfırdan büyük olmalıdır.",
            kurum_degerleri.donemId
        )));
    }
    if kurum_degerleri
        .pekTavanKatsayisi
        .is_some_and(|value| value < Decimal::ONE)
    {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi PEK tavan katsayısı en az 1 olmalıdır.",
            kurum_degerleri.donemId
        )));
    }

    for (field, value) in [
        ("sabitSendikaAidati", kurum_degerleri.sabitSendikaAidati),
        ("sabitBesTutar", kurum_degerleri.sabitBesTutar),
        ("ekOdeme", kurum_degerleri.ekOdeme),
        ("digerGelirVarsayilan", kurum_degerleri.digerGelirVarsayilan),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi sabit tutarı negatif olamaz: {}.",
                kurum_degerleri.donemId, field
            )));
        }
    }

    let groups = match kurum_degerleri.isPrimiGruplari.as_deref() {
        Some(groups) if !groups.is_empty() => groups,
        _ => {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi iş primi grupları eksik.",
                kurum_degerleri.donemId
            )))
        }
    };
    let mut active_ids = HashSet::new();
    let mut active_names = HashSet::new();
    for group in groups {
        if group.id.trim().is_empty()
            || group.ad.trim().is_empty()
            || group.oran < Decimal::ZERO
            || group.oran > dec!(100)
        {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi iş primi gruplarında geçersiz tanım/oran var.",
                kurum_degerleri.donemId
            )));
        }
        if group.aktif
            && (!active_ids.insert(group.id.trim().to_string())
                || !active_names.insert(group.ad.trim().to_string()))
        {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi aktif iş primi gruplarında tekrar eden id/ad var.",
                kurum_degerleri.donemId
            )));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct GvIndirimHesabi {
    pub dogum_askerlik_indirimi: Decimal,
    pub sigorta_adayi: Decimal,
    pub sigorta_aylik_limiti: Decimal,
    pub sigorta_yillik_kalan_limiti: Decimal,
    pub uygulanabilir_sigorta_indirimi: Decimal,
}

pub fn calculate_gv_indirimleri(
    sigorta_limit_brut_ucret: Decimal,
    dogum_askerlik_gv_indirimi: Decimal,
    hayat_sigortasi_primi: Decimal,
    saglik_sigortasi_primi: Decimal,
    sigorta_gv_yillik_tavan: Decimal,
    sigorta_gv_yil_once_kullanilan: Decimal,
) -> GvIndirimHesabi {
    let life_candidate = (hayat_sigortasi_primi.max(Decimal::ZERO) * dec!(0.50)).round_dp(2);
    let health_candidate = saglik_sigortasi_primi.max(Decimal::ZERO).round_dp(2);
    let sigorta_adayi = (life_candidate + health_candidate).round_dp(2);
    let sigorta_aylik_limiti =
        (sigorta_limit_brut_ucret.max(Decimal::ZERO) * dec!(0.15)).round_dp(2);
    let sigorta_yillik_kalan_limiti = (sigorta_gv_yillik_tavan.max(Decimal::ZERO)
        - sigorta_gv_yil_once_kullanilan.max(Decimal::ZERO))
    .max(Decimal::ZERO)
    .round_dp(2);
    let uygulanabilir_sigorta_indirimi = sigorta_adayi
        .min(sigorta_aylik_limiti)
        .min(sigorta_yillik_kalan_limiti)
        .max(Decimal::ZERO)
        .round_dp(2);

    GvIndirimHesabi {
        dogum_askerlik_indirimi: dogum_askerlik_gv_indirimi.max(Decimal::ZERO).round_dp(2),
        sigorta_adayi,
        sigorta_aylik_limiti,
        sigorta_yillik_kalan_limiti,
        uygulanabilir_sigorta_indirimi,
    }
}

pub fn validate_pek_bounds(alt_sinir: Decimal, ust_sinir: Decimal) -> Result<()> {
    if alt_sinir < Decimal::ZERO || ust_sinir < Decimal::ZERO || alt_sinir > ust_sinir {
        return Err(DomainError::ValidationError(format!(
            "PEK sınırları geçersiz: alt={}, üst={}.",
            alt_sinir, ust_sinir
        )));
    }
    Ok(())
}

/// Is primi hesap detayi: tek-final-rounding uygular.
/// ```text
/// tutar = round2(gunluk_taban x oran / 100 x hak_gunu)
/// ```
/// Gunluk deger (gunluk x oran/100) yalniz gosterim icindir; bordro toplaminin
/// authoritative girdisi degildir.
pub fn calculate_is_primi_detayi(
    gunluk_taban_ucret: Decimal,
    is_primi_hak_gunu: i32,
    grup: Option<&str>,
    is_primi_gruplari: Option<&[IsPrimiGrupItem]>,
) -> Result<IsPrimiHesapDetayi> {
    let item = resolve_is_primi_grubu(grup, is_primi_gruplari)?;
    let oran_katsayi = item.oran / dec!(100);
    let gunluk_is_primi = round2(gunluk_taban_ucret * oran_katsayi);
    let tutar = round2(gunluk_taban_ucret * oran_katsayi * Decimal::from(is_primi_hak_gunu));

    Ok(IsPrimiHesapDetayi {
        grupId: item.id,
        grupAd: item.ad,
        oran: item.oran,
        hakGunu: is_primi_hak_gunu,
        gunlukIsPrimi: gunluk_is_primi,
        tutar,
    })
}

/// Puantajdan gelir kalemlerini otomatik doldurur.
///
/// Is primi: oran personelin grubundan (grup) gelir; hak gunu = C + GC. Sessiz fallback yok:
/// grup cozumleme hatalari [DomainError] olarak doner ve motor tahmin uretmez.
pub fn calculate_gunluk_gelirler_from_puantaj(
    puantaj_ozeti: &PuantajOzeti,
    kurum_degerleri: &DonemselKurumDegerleri,
    grup: Option<&str>,
) -> Result<(GelirKalemleri, IsPrimiHesapDetayi)> {
    let hakedis_gun = puantaj_ozeti.c
        + puantaj_ozeti.t
        + puantaj_ozeti.g
        + puantaj_ozeti.i
        + puantaj_ozeti.gc
        + puantaj_ozeti.gct;
    let hakedis_dec = Decimal::from(hakedis_gun);
    let fiili_calisma_gun = Decimal::from(puantaj_ozeti.c + puantaj_ozeti.gc);

    let taban_brut_aylik = round2(hakedis_dec * kurum_degerleri.gunlukTabanUcret);
    let yemek = round2(fiili_calisma_gun * kurum_degerleri.gunlukYemek);
    let vasita_yol = round2(fiili_calisma_gun * kurum_degerleri.gunlukVasitaYol);

    // Is primi: oran personelin grubundan gelir; hak gunu = C + GC.
    let is_primi_hak_gunu = puantaj_ozeti.c + puantaj_ozeti.gc;
    let is_primi_detay = calculate_is_primi_detayi(
        kurum_degerleri.gunlukTabanUcret,
        is_primi_hak_gunu,
        grup,
        kurum_degerleri.isPrimiGruplari.as_deref(),
    )?;

    let gc_orani = kurum_degerleri.geceCalismaPrimiYuzde.unwrap_or(dec!(0));
    let gct_orani = kurum_degerleri
        .geceCalismaTatiliPrimiYuzde
        .unwrap_or(dec!(0));

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

    let gelirler = GelirKalemleri {
        tabanBrutAylik: Some(taban_brut_aylik),
        yemek: Some(yemek),
        vasitaYol: Some(vasita_yol),
        isPrimi: Some(is_primi_detay.tutar),
        geceCalismasiUcreti: Some(gece_calismasi_ucreti),
        geceCalismasiTatiliUcreti: Some(gece_calismasi_tatili_ucreti),
        ..GelirKalemleri::default()
    };

    Ok((gelirler, is_primi_detay))
}

pub fn auto_fill_gelirler_from_puantaj(
    puantaj_ozeti: &PuantajOzeti,
    kurum_degerleri: &DonemselKurumDegerleri,
    hizmet_yili: i32,
    grup: Option<&str>,
) -> Result<(GelirKalemleri, IsPrimiHesapDetayi)> {
    let (mut gelirler, is_primi_detay) =
        calculate_gunluk_gelirler_from_puantaj(puantaj_ozeti, kurum_degerleri, grup)?;

    let birlestirilmis_sosyal_yardim = kurum_degerleri.birlestirilmisSosyalYardim;
    let giyim_yardimi = kurum_degerleri.giyimYardimi;
    let hizmet_zammi = round2(Decimal::from(hizmet_yili) * kurum_degerleri.hizmetZammiBirimi);

    // Tediye ve TİS ikramiyesi authoritative olarak otomatik üretilmez.
    // Dönem ayarlarındaki legacy takvim/listeler yalnız referans/migration verisidir;
    // production bordro yolu kişi+dönem bazındaki ManualPayrollIncomeInput'u uygular.
    gelirler.tediye = None;
    gelirler.tisIkramiyesi = None;
    gelirler.ekOdeme = kurum_degerleri.ekOdeme;
    gelirler.birlestirilmisSosyalYardim = Some(birlestirilmis_sosyal_yardim);
    gelirler.giyimYardimi = Some(giyim_yardimi);
    gelirler.hizmetZammi = Some(hizmet_zammi);
    gelirler.digerGelir = kurum_degerleri.digerGelirVarsayilan;

    Ok((gelirler, is_primi_detay))
}

pub fn calculate_prime_esas_kazanc(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    calculate_prime_esas_kazanc_with_statutory_snapshot(
        gelirler,
        puantaj_ozeti,
        kurum_degerleri,
        devreden_pek_gelen,
        None,
    )
}

fn calculate_prime_esas_kazanc_with_statutory_snapshot(
    gelirler: &GelirKalemleri,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    devreden_pek_gelen: &[DevredenPekKaydi],
    statutory_snapshot: Option<&ResolvedStatutorySnapshot>,
) -> (PekDetayi, Vec<DevredenPekKaydi>) {
    let raw_prim_gun = puantaj_ozeti.map_or(0, |p| p.c + p.t + p.g + p.i + p.gc + p.gct + p.r);
    let prim_gun_sayisi = statutory_snapshot
        .map_or(raw_prim_gun, |snapshot| snapshot.sgkPrimGunSayisi)
        .clamp(0, 30);
    let fiili_yemek_gunu = puantaj_ozeti.map_or(0, |p| p.c + p.gc);

    let default_k = DonemselKurumDegerleri::default();
    let k = kurum_degerleri.unwrap_or(&default_k);

    let brut_yemek = gelirler.yemek.unwrap_or(dec!(0));
    let yemek_istisnasi_tutar = if let Some(snapshot) = statutory_snapshot {
        brut_yemek.min(snapshot.sgkYemekIstisnasiToplam)
    } else {
        let gunluk_yemek_istisnasi = k.gunlukYemekIstisnasiSGK.unwrap_or(dec!(300.00));
        brut_yemek.min(gunluk_yemek_istisnasi * Decimal::from(fiili_yemek_gunu))
    };
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

    let (pek_alt_sinir, pek_ust_sinir) = if let Some(snapshot) = statutory_snapshot {
        (snapshot.pekAltSinir, snapshot.pekUstSinir)
    } else {
        let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
        let tavan_katsayi = k.pekTavanKatsayisi.unwrap_or(dec!(9));
        (
            round2(gunluk_asgari * Decimal::from(prim_gun_sayisi)),
            round2(gunluk_asgari * tavan_katsayi * Decimal::from(prim_gun_sayisi)),
        )
    };

    debug_assert!(
        pek_alt_sinir <= pek_ust_sinir,
        "PEK alt sınırı üst sınırı aşamaz"
    );

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
        let ucret_tavan_kapasitesi =
            (pek_ust_sinir - ucretler - eklenecek_devreden_toplam).max(dec!(0));
        let ucret_disi_kullanilan = ucret_disi_odemeler.min(ucret_tavan_kapasitesi);
        devreden_pek_asan_tutar =
            round2((ucret_disi_odemeler - ucret_disi_kullanilan).max(dec!(0)));
        pek_matrah_adayi = pek_ust_sinir;
    }

    if devreden_pek_asan_tutar > dec!(0) {
        sonraki_devreden_list.push(DevredenPekKaydi {
            tutar: devreden_pek_asan_tutar,
            kalanAySayisi: 2,
            kaynakDonemId: None,
        });
    }

    // `prim_matrahi`, cari ayda gerçekten primlendirilen kazançtır: cari ham PEK +
    // bu ay tavana sığan devreden PEK. Alt sınır tamamlama işçi matrahına dahil edilmez.
    let prim_matrahi = round2(pek_matrah_adayi.max(dec!(0)));
    let mut final_pek = prim_matrahi;
    let alt_sinir_tamamlama_farki = if prim_matrahi > dec!(0) && prim_matrahi < pek_alt_sinir {
        round2(pek_alt_sinir - prim_matrahi)
    } else {
        dec!(0)
    };

    if final_pek < pek_alt_sinir && prim_matrahi > dec!(0) {
        final_pek = pek_alt_sinir;
    }

    let isveren_sgk_rate = k.sgkIsverenOraniYuzde.unwrap_or(dec!(21.75)) / dec!(100);
    let isveren_issizlik_rate = k.issizlikIsverenOraniYuzde.unwrap_or(dec!(2.00)) / dec!(100);
    let isci_sgk_rate = k.sgkIsciOraniYuzde.unwrap_or(dec!(14)) / dec!(100);
    let isci_issizlik_rate = k.issizlikIsciOraniYuzde.unwrap_or(dec!(1)) / dec!(100);

    let isveren_sgk_primi = round_sgk_amount(final_pek * isveren_sgk_rate);
    let isveren_issizlik_primi = round_sgk_amount(final_pek * isveren_issizlik_rate);

    let isveren_alt_sinir_sgk_farki = round_sgk_amount(alt_sinir_tamamlama_farki * isci_sgk_rate);
    let isveren_alt_sinir_issizlik_farki =
        round_sgk_amount(alt_sinir_tamamlama_farki * isci_issizlik_rate);
    let pek_alt_sinir_tamamlama_isveren_primi =
        isveren_alt_sinir_sgk_farki + isveren_alt_sinir_issizlik_farki;

    let isveren_prim_toplami =
        isveren_sgk_primi + isveren_issizlik_primi + pek_alt_sinir_tamamlama_isveren_primi;

    let det = PekDetayi {
        hesaplananPek: round2(ham_pek),
        hamPek: round2(ham_pek),
        devredenPekKullanilan: round2(eklenecek_devreden_toplam),
        primMatrahi: prim_matrahi,
        finalPek: round2(final_pek),
        devredenPekAşanTutar: devreden_pek_asan_tutar,
        pekAltSinir: pek_alt_sinir,
        pekUstSinir: pek_ust_sinir,
        altSinirTamamlamaFarki: alt_sinir_tamamlama_farki,
        fiiliYemekGunu: fiili_yemek_gunu,
        yemekIstisnasiTutar: round2(yemek_istisnasi_tutar),
        isverenSgkPrimi: Some(isveren_sgk_primi),
        isverenIssizlikPrimi: Some(isveren_issizlik_primi),
        pekAltSinirTamamlamaIsverenPrimi: Some(pek_alt_sinir_tamamlama_isveren_primi),
        isverenPrimToplami: Some(isveren_prim_toplami),
        sgkIsverenOraniYuzde: Some(k.sgkIsverenOraniYuzde.unwrap_or(dec!(21.75))),
        isverenIssizlikOraniYuzde: Some(k.issizlikIsverenOraniYuzde.unwrap_or(dec!(2.00))),
    };

    (det, sonraki_devreden_list)
}

pub struct StatutoryDeductionTaxInputs<'a> {
    pub previous_cumulative_gv: Decimal,
    pub incoming_devreden_pek: &'a [DevredenPekKaydi],
    pub previous_cumulative_asgari_gv: Decimal,
    pub tax_brackets: &'a [TaxBracket],
}

pub fn calculate_statutory_deductions_with_tax_brackets(
    gelirler: &GelirKalemleri,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    personel: Option<&Personel>,
    puantaj_ozeti: Option<&PuantajOzeti>,
    tax_inputs: &StatutoryDeductionTaxInputs<'_>,
    statutory_snapshot: Option<&ResolvedStatutorySnapshot>,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {
    let brut_gelir = calculate_gelir_toplam(gelirler);
    let default_k = DonemselKurumDegerleri::default();
    let k = kurum_degerleri.unwrap_or(&default_k);

    // PEK önce çözülür. Cari brüt sıfır olsa bile tavan içine alınan devreden PEK
    // varsa işçi SGK/işsizlik ve OKS bu authoritative matrah üzerinden tahakkuk eder.
    let (pek_detay, sonraki_devreden) = calculate_prime_esas_kazanc_with_statutory_snapshot(
        gelirler,
        puantaj_ozeti,
        kurum_degerleri,
        tax_inputs.incoming_devreden_pek,
        statutory_snapshot,
    );

    // Sıfır brüt + sıfır PEK için önceki davranışı koru; ancak PEK varken kesintileri
    // gelir toplamına bakarak erken sıfırlama.
    if brut_gelir <= dec!(0) && pek_detay.primMatrahi <= dec!(0) {
        return (KesintiKalemleri::default(), pek_detay, sonraki_devreden);
    }

    // İşçi SGK, işsizlik ve OKS aynı authoritative PEK matrahını izler. Cari ayda
    // tavana sığan devreden tutar dahildir; alt sınır işveren tamamlama farkı dahil değildir.
    let worker_pek_matrah = pek_detay.primMatrahi;
    let oks_pek_matrah = pek_detay.primMatrahi;

    let sgk_rate = k.sgkIsciOraniYuzde.unwrap_or(dec!(14)) / dec!(100);
    let issizlik_rate = k.issizlikIsciOraniYuzde.unwrap_or(dec!(1)) / dec!(100);
    let dv_rate = k.damgaVergisiOraniBinde.unwrap_or(dec!(7.59)) / dec!(1000);

    let isci_sgk_primi = round_sgk_amount(worker_pek_matrah * sgk_rate);
    let isci_issizlik_primi = round_sgk_amount(worker_pek_matrah * issizlik_rate);

    let gelir_vergisi_matrah = calculate_gv_matrah(brut_gelir, isci_sgk_primi, isci_issizlik_primi);

    let gunluk_asgari = k.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
    let aylik_brut_asgari = round2(gunluk_asgari * dec!(30));
    let aylik_asgari_sgk = round2(aylik_brut_asgari * (sgk_rate + issizlik_rate));
    let asgari_ucret_gv_matrah = (aylik_brut_asgari - aylik_asgari_sgk).max(dec!(0));

    let gv_detay = calculate_gv_hesap_detayi_with_brackets(
        gelir_vergisi_matrah,
        tax_inputs.previous_cumulative_gv,
        asgari_ucret_gv_matrah,
        tax_inputs.previous_cumulative_asgari_gv,
        tax_inputs.tax_brackets,
    );
    let gelir_vergisi = gv_detay.kesilenGelirVergisi;

    let asgari_ucret_dv_istisnasi = round2(aylik_brut_asgari * dv_rate);
    let ham_damga_vergisi = round2(brut_gelir * dv_rate);
    let damga_vergisi = (round2(ham_damga_vergisi - asgari_ucret_dv_istisnasi)).max(dec!(0));

    let p_kesintiler = personel.and_then(|p| p.kesintiler.as_ref());

    let is_sendika = p_kesintiler.and_then(|pk| pk.sendikaUyesi).unwrap_or(false);
    let sendika_aidati = if is_sendika {
        p_kesintiler
            .and_then(|pk| pk.sabitSendikaAidati)
            .filter(|&sabit| sabit > dec!(0))
            .or_else(|| k.sabitSendikaAidati.filter(|&sabit| sabit > dec!(0)))
            .unwrap_or_else(|| {
                let sendika_orani = k.sendikaAidatiYuzde.unwrap_or(dec!(65)) / dec!(100);
                round2(k.gunlukTabanUcret * sendika_orani)
            })
    } else {
        dec!(0)
    };

    let is_oks = p_kesintiler.and_then(|pk| pk.besUyesi).unwrap_or(false);
    let bes = if is_oks {
        p_kesintiler
            .and_then(|pk| pk.sabitBesTutar)
            .filter(|&sabit| sabit > dec!(0))
            .or_else(|| k.sabitBesTutar.filter(|&sabit| sabit > dec!(0)))
            .unwrap_or_else(|| {
                let custom_oran = p_kesintiler.and_then(|pk| pk.oksOraniYuzde);
                let oks_orani =
                    custom_oran.unwrap_or_else(|| k.besOraniYuzde.unwrap_or(dec!(3))) / dec!(100);
                floor_dec(oks_pek_matrah * oks_orani)
            })
    } else {
        dec!(0)
    };

    let icra = p_kesintiler
        .and_then(|pk| pk.icraTutar)
        .filter(|v| *v > dec!(0));
    let kisi_borcu = p_kesintiler
        .and_then(|pk| pk.kisiBorcuTutar)
        .filter(|v| *v > dec!(0));
    let dogum_askerlik = p_kesintiler
        .and_then(|pk| pk.dogumAskerlikBorclanmasiTutar)
        .filter(|v| *v > dec!(0));
    let hayat_saglik = p_kesintiler
        .and_then(|pk| pk.hayatSaglikSigortasiTutar)
        .filter(|v| *v > dec!(0));
    let diger_kesinti = p_kesintiler
        .and_then(|pk| pk.digerKesintiTutar)
        .filter(|v| *v > dec!(0));

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

/// Test fixture/geriye dönük API. Üretim yolu yıllık parametre tablosunu kullanır.
pub fn calculate_statutory_deductions(
    gelirler: &GelirKalemleri,
    kurum_degerleri: Option<&DonemselKurumDegerleri>,
    personel: Option<&Personel>,
    puantaj_ozeti: Option<&PuantajOzeti>,
    kumulatif_gv_matrahi_onceki: Decimal,
    devreden_pek_gelen: &[DevredenPekKaydi],
    kumulatif_asgari_gv_onceki: Decimal,
) -> (KesintiKalemleri, PekDetayi, Vec<DevredenPekKaydi>) {
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv: kumulatif_gv_matrahi_onceki,
        incoming_devreden_pek: devreden_pek_gelen,
        previous_cumulative_asgari_gv: kumulatif_asgari_gv_onceki,
        tax_brackets: &default_gelir_vergisi_dilimleri_2026(),
    };

    calculate_statutory_deductions_with_tax_brackets(
        gelirler,
        kurum_degerleri,
        personel,
        puantaj_ozeti,
        &tax_inputs,
        None,
    )
}
