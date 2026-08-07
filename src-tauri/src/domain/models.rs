use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonelKesintileri {
    pub sendikaUyesi: Option<bool>,
    pub sabitSendikaAidati: Option<Decimal>,
    pub besUyesi: Option<bool>,
    pub oksOraniYuzde: Option<Decimal>,
    pub sabitBesTutar: Option<Decimal>,
    pub icraTutar: Option<Decimal>,
    pub kisiBorcuTutar: Option<Decimal>,
    pub dogumAskerlikBorclanmasiTutar: Option<Decimal>,
    pub hayatSaglikSigortasiTutar: Option<Decimal>,
    pub digerKesintiTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Personel {
    pub id: String,
    pub tcNo: String,
    pub ad: String,
    pub soyad: String,
    pub grup: String,
    pub unvan: Option<String>,
    pub sgkSicilNo: String,
    pub iban: String,
    pub hizmetYili: i32,
    pub aciklama: Option<String>,
    pub kesintiler: Option<PersonelKesintileri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BordroDonemi {
    pub id: String, // e.g. "2026-05"
    pub yil: i32,   // e.g. 2026
    pub ay: i32,    // 1-12
    pub baslangicTarihi: String,
    pub bitisTarihi: String,
    pub donemAdi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonelTaxOpening {
    pub id: String,
    pub personnelId: String,
    pub year: i32,
    pub gvCumulativeOpening: Decimal,
    pub effectiveFromPeriodId: String, // e.g. "2026-05"
    pub createdAt: Option<String>,
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonelPuantaj {
    pub id: String, // `${personelId}_${donemId}`
    pub personelId: String,
    pub donemId: String,
    pub gunler: HashMap<String, String>, // key: "YYYY-MM-DD", val: "Ç", "T", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PuantajOzeti {
    #[serde(rename = "Ç")]
    pub c: i32,
    #[serde(rename = "T")]
    pub t: i32,
    #[serde(rename = "G")]
    pub g: i32,
    #[serde(rename = "İ")]
    pub i: i32,
    #[serde(rename = "GÇ")]
    pub gc: i32,
    #[serde(rename = "GÇT")]
    pub gct: i32,
    #[serde(rename = "R")]
    pub r: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GelirKalemleri {
    pub tabanBrutAylik: Option<Decimal>,
    pub tediye: Option<Decimal>,
    pub tisIkramiyesi: Option<Decimal>,
    pub ekOdeme: Option<Decimal>,
    pub yemek: Option<Decimal>,
    pub birlestirilmisSosyalYardim: Option<Decimal>,
    pub vasitaYol: Option<Decimal>,
    pub giyimYardimi: Option<Decimal>,
    pub isPrimi: Option<Decimal>,
    pub hizmetZammi: Option<Decimal>,
    pub digerGelir: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KesintiKalemleri {
    pub isciSgkPrimi: Option<Decimal>,
    pub isciIssizlikPrimi: Option<Decimal>,
    pub gelirVergisi: Option<Decimal>,
    pub damgaVergisi: Option<Decimal>,
    pub sendikaAidati: Option<Decimal>,
    pub bes: Option<Decimal>,
    pub icra: Option<Decimal>,
    pub kisiBorcu: Option<Decimal>,
    pub dogumAskerlikBorclanmasi: Option<Decimal>,
    pub hayatSaglikSigortasi: Option<Decimal>,
    pub digerKesinti: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevredenPekKaydi {
    pub tutar: Decimal,
    pub kalanAySayisi: i32,
    pub kaynakDonemId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PekDetayi {
    pub hesaplananPek: Decimal,
    pub finalPek: Decimal,
    pub devredenPekAşanTutar: Decimal,
    pub pekAltSinir: Decimal,
    pub pekUstSinir: Decimal,
    pub fiiliYemekGunu: i32,
    pub yemekIstisnasiTutar: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BordroStatus {
    DRAFT,
    CALCULATED,
    FINALIZED,
}

impl Default for BordroStatus {
    fn default() -> Self {
        BordroStatus::CALCULATED
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollIncomeItem {
    pub id: String,
    pub payrollId: String,
    pub itemType: String,
    pub description: String,
    pub amount: Decimal,
    pub source: String, // "MANUAL" or "CALCULATED"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollDeductionItem {
    pub id: String,
    pub payrollId: String,
    pub itemType: String,
    pub description: String,
    pub amount: Decimal,
    pub source: String, // "MANUAL" or "CALCULATED"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BordroKaydi {
    pub id: String,
    pub personelId: String,
    pub donemId: String,
    pub puantajOzeti: PuantajOzeti,
    pub gelirler: GelirKalemleri,
    pub gelirToplam: Decimal,
    pub kesintiler: KesintiKalemleri,
    pub kesintiToplam: Decimal,
    pub netOdeme: Decimal,
    pub status: BordroStatus,
    pub olusturulmaTarihi: String,
    pub sonGuncellemeTarihi: String,
    pub notlar: Option<String>,
    pub oncekiKumulatifGvMatrahi: Option<Decimal>,
    pub oncekiKumulatifAsgariGvMatrahi: Option<Decimal>,
    pub manuelKumulatifGvMatrahi: Option<Decimal>,
    pub devredenPekGelen: Option<Vec<DevredenPekKaydi>>,
    pub sonrakiDevredenPek: Option<Vec<DevredenPekKaydi>>,
    pub pekDetay: Option<PekDetayi>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TediyeKalemi {
    pub id: i32,
    pub ad: String,
    pub odemeAyi: String,
    pub gunSayisi: i32,
    pub aktifDonemdeOdensin: bool,
    pub sabitTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TisIkramiyeKalemi {
    pub id: i32,
    pub ad: String,
    pub odemeAyi: String,
    pub gunSayisi: i32,
    pub aktifDonemdeOdensin: bool,
    pub sabitTutar: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsPrimiGrupItem {
    pub id: String,
    pub ad: String,
    pub oran: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonemselKurumDegerleri {
    pub donemId: String,
    pub gunlukTabanUcret: Decimal,
    pub gunlukYemek: Decimal,
    pub birlestirilmisSosyalYardim: Decimal,
    pub gunlukVasitaYol: Decimal,
    pub giyimYardimi: Decimal,
    pub hizmetZammiBirimi: Decimal,
    pub isPrimiYuzde: Option<Decimal>,
    pub isPrimiGruplari: Option<Vec<IsPrimiGrupItem>>,
    pub ekOdeme: Option<Decimal>,
    pub digerGelirVarsayilan: Option<Decimal>,
    pub tediyeListesi: Option<Vec<TediyeKalemi>>,
    pub tisIkramiyeListesi: Option<Vec<TisIkramiyeKalemi>>,
    pub tediyeTisNotu: Option<String>,

    pub sgkIsciOraniYuzde: Option<Decimal>,
    pub issizlikIsciOraniYuzde: Option<Decimal>,
    pub gelirVergisiOraniYuzde: Option<Decimal>,
    pub damgaVergisiOraniBinde: Option<Decimal>,
    pub sendikaAidatiYuzde: Option<Decimal>,
    pub sabitSendikaAidati: Option<Decimal>,
    pub besOraniYuzde: Option<Decimal>,
    pub sabitBesTutar: Option<Decimal>,

    pub gunlukYemekIstisnasiSGK: Option<Decimal>,
    pub pekTavanKatsayisi: Option<Decimal>,
    pub gunlukAsgariUcret: Option<Decimal>,
}

impl Default for DonemselKurumDegerleri {
    fn default() -> Self {
        use rust_decimal_macros::dec;
        Self {
            donemId: "".to_string(),
            gunlukTabanUcret: dec!(2443.28),
            gunlukYemek: dec!(300.75),
            birlestirilmisSosyalYardim: dec!(5089.70),
            gunlukVasitaYol: dec!(128.93),
            giyimYardimi: dec!(269.70),
            hizmetZammiBirimi: dec!(24.67),
            isPrimiYuzde: Some(dec!(0)),
            isPrimiGruplari: Some(vec![
                IsPrimiGrupItem { id: "1. Grup".into(), ad: "1. Grup".into(), oran: dec!(9) },
                IsPrimiGrupItem { id: "2. Grup".into(), ad: "2. Grup".into(), oran: dec!(8) },
                IsPrimiGrupItem { id: "3. Grup".into(), ad: "3. Grup".into(), oran: dec!(7) },
            ]),
            ekOdeme: Some(dec!(0)),
            digerGelirVarsayilan: Some(dec!(0)),
            tediyeListesi: None,
            tisIkramiyeListesi: None,
            tediyeTisNotu: None,
            sgkIsciOraniYuzde: Some(dec!(14)),
            issizlikIsciOraniYuzde: Some(dec!(1)),
            gelirVergisiOraniYuzde: Some(dec!(15)),
            damgaVergisiOraniBinde: Some(dec!(7.59)),
            sendikaAidatiYuzde: Some(dec!(65)),
            sabitSendikaAidati: Some(dec!(0)),
            besOraniYuzde: Some(dec!(3)),
            sabitBesTutar: Some(dec!(0)),
            gunlukYemekIstisnasiSGK: Some(dec!(300.00)),
            pekTavanKatsayisi: Some(dec!(9)),
            gunlukAsgariUcret: Some(dec!(1101.00)),
        }
    }
}
