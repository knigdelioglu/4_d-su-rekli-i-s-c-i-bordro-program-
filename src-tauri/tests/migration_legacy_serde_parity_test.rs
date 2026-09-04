use bordro_programi_lib::domain::models::{
    BordroKaydi, BordroStatus, GvHesapDetayi, IsPrimiGrupItem, PekDetayi,
};
use bordro_programi_lib::services::migration_service::{LegacyPayload, MigrationService};
use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
    repositories::personnel_repo::PersonnelRepository,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

fn legacy_payroll_value() -> Value {
    json!({
        "id": "payroll-1",
        "personelId": "person-1",
        "donemId": "2026-01",
        "puantajOzeti": { "Ç": 20 },
        "gelirler": {},
        "gelirToplam": 0,
        "kesintiler": {},
        "kesintiToplam": 0,
        "netOdeme": 0,
        "olusturulmaTarihi": "2026-02-14T10:00:00.000Z",
        "sonGuncellemeTarihi": "2026-02-14T10:00:00.000Z"
    })
}

#[test]
fn native_legacy_personel_import_defaults_match_browser_canonicalization() {
    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let payload = json!({
        "backupVersion": 1,
        "personeller": [{
            "id": "person-1",
            "tcNo": "10000000000",
            "ad": "Ada",
            "soyad": "Yılmaz",
            "grup": "1. Grup"
        }]
    })
    .to_string();

    MigrationService::migrate_legacy_data(&mut conn, &payload)
        .expect("native legacy personel migration başarılı olmalı");

    let person = PersonnelRepository::get_all(&conn)
        .expect("personel okunmalı")
        .into_iter()
        .next()
        .expect("personel migration sonrası mevcut olmalı");
    assert_eq!(person.sgkSicilNo, "");
    assert_eq!(person.iban, "");
    assert_eq!(person.hizmetYili, 1);
}

#[test]
fn native_legacy_migration_adds_missing_annual_parameters_for_imported_tax_years() {
    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let payload = json!({
        "backupVersion": 1,
        "donemler": [{
            "id": "2026-01",
            "yil": 2026,
            "ay": 1,
            "baslangicTarihi": "2026-01-15",
            "bitisTarihi": "2026-02-14",
            "donemAdi": "Ocak 2026",
            "taxYear": 2026,
            "taxMonth": 2
        }]
    })
    .to_string();

    MigrationService::migrate_legacy_data(&mut conn, &payload)
        .expect("native legacy dönem migration başarılı olmalı");

    let parameters =
        AnnualPayrollParametersRepository::get_all(&conn).expect("yıllık parametreler okunmalı");
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].year, 2026);
    assert_eq!(parameters[0].gelirVergisiDilimleri.len(), 5);
    assert_eq!(
        parameters[0].sigortaGvYillikBrutAsgariUcretTavani,
        Some(Decimal::from(396360))
    );
}

#[test]
fn native_legacy_serde_defaults_match_browser_legacy_fixture_contract() {
    let parsed: LegacyPayload = serde_json::from_value(json!({
        "backupVersion": 1,
        "personeller": [{
            "id": "person-1",
            "tcNo": "10000000000",
            "ad": "Ada",
            "soyad": "Yılmaz",
            "grup": "1. Grup"
        }],
        "bordrolar": [legacy_payroll_value()]
    }))
    .expect("legacy payload native Serde ile parse edilebilmeli");

    let personeller = parsed.personeller.expect("personeller bulunmalı");
    let person = &personeller[0];
    assert_eq!(person.sgkSicilNo, None);
    assert_eq!(person.iban, None);
    assert_eq!(person.hizmetYili, None);

    let payroll: BordroKaydi = serde_json::from_value(legacy_payroll_value())
        .expect("legacy bordro native Serde ile parse edilebilmeli");
    assert_eq!(payroll.status, BordroStatus::CALCULATED);
    assert_eq!(payroll.puantajOzeti.c, 20);
    assert_eq!(payroll.puantajOzeti.t, 0);
    assert_eq!(payroll.puantajOzeti.g, 0);
    assert_eq!(payroll.puantajOzeti.i, 0);
    assert_eq!(payroll.puantajOzeti.gc, 0);
    assert_eq!(payroll.puantajOzeti.gct, 0);
    assert_eq!(payroll.puantajOzeti.r, 0);
    assert_eq!(payroll.gelirler.tabanBrutAylik, None);
    assert_eq!(payroll.kesintiler.gelirVergisi, None);
}

#[test]
fn native_legacy_serde_default_decimal_and_boolean_fields_match_browser_defaults() {
    let pek: PekDetayi = serde_json::from_value(json!({
        "hesaplananPek": 0,
        "finalPek": 0,
        "devredenPekAşanTutar": 0,
        "pekAltSinir": 0,
        "pekUstSinir": 0,
        "fiiliYemekGunu": 0,
        "yemekIstisnasiTutar": 0
    }))
    .expect("PekDetayi serde(default) ile parse edilebilmeli");
    assert_eq!(pek.hamPek, Decimal::ZERO);
    assert_eq!(pek.devredenPekKullanilan, Decimal::ZERO);
    assert_eq!(pek.primMatrahi, Decimal::ZERO);
    assert_eq!(pek.altSinirTamamlamaFarki, Decimal::ZERO);

    let gv: GvHesapDetayi = serde_json::from_value(json!({
        "cariGvMatrahi": 0,
        "yeniKumulatifGvMatrahi": 0,
        "brutGelirVergisi": 0,
        "asgariUcretGvMatrahi": 0,
        "asgariUcretReferansKumulatifMatrahi": 0,
        "asgariUcretGvIstisnasi": 0,
        "uygulananGvIstisnasi": 0,
        "kesilenGelirVergisi": 0
    }))
    .expect("GvHesapDetayi serde(default) ile parse edilebilmeli");
    assert_eq!(gv.dogumAskerlikGvIndirimi, Decimal::ZERO);
    assert_eq!(gv.sigortaGvIndirimAdayi, Decimal::ZERO);
    assert_eq!(gv.sigortaGvAylikLimiti, Decimal::ZERO);
    assert_eq!(gv.sigortaGvYillikKalanLimiti, Decimal::ZERO);
    assert_eq!(gv.uygulanabilirSigortaGvIndirimi, Decimal::ZERO);

    let group: IsPrimiGrupItem = serde_json::from_value(json!({
        "id": "group-1",
        "ad": "1. Grup",
        "oran": 9
    }))
    .expect("IsPrimiGrupItem aktif varsayılanı parse edilebilmeli");
    assert!(group.aktif);
}

#[test]
fn native_legacy_serde_rejects_explicit_invalid_types() {
    let mut invalid_summary = legacy_payroll_value();
    invalid_summary["puantajOzeti"]["T"] = json!("20");
    assert!(serde_json::from_value::<BordroKaydi>(invalid_summary).is_err());

    let invalid_pek = json!({
        "hesaplananPek": 0,
        "finalPek": 0,
        "devredenPekAşanTutar": 0,
        "pekAltSinir": 0,
        "pekUstSinir": 0,
        "fiiliYemekGunu": 0,
        "yemekIstisnasiTutar": 0,
        "hamPek": null
    });
    assert!(serde_json::from_value::<PekDetayi>(invalid_pek).is_err());

    let invalid_group = json!({
        "id": "group-1",
        "ad": "1. Grup",
        "oran": 9,
        "aktif": null
    });
    assert!(serde_json::from_value::<IsPrimiGrupItem>(invalid_group).is_err());
}
