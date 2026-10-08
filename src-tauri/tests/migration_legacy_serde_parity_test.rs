use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::services::migration_service::{LegacyPayload, MigrationService};
use bordro_programi_lib::{
    db::{
        create_in_memory_connection,
        migrations::{
            get_migrations, CALCULATION_METADATA_MIGRATION_VERSION,
            COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
        },
    },
    repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
    repositories::attendance_repo::AttendanceRepository,
    repositories::compensation_policy_repo::CompensationPolicyRepository,
    repositories::payroll_repo::PayrollRepository,
    repositories::period_repo::PeriodRepository,
    repositories::personnel_repo::PersonnelRepository,
    repositories::retro_repo::get_allocations,
    repositories::retro_repo::get_batches,
    repositories::retro_repo::get_revisions,
    repositories::settings_repo::SettingsRepository,
    services::payroll_service::PayrollService,
};
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::collections::HashMap;
#[path = "common/compensation_policy.rs"]
mod compensation_policy_test_support;

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

fn legacy_raise_months(conn: &Connection) -> Result<Option<Vec<i32>>, Box<dyn std::error::Error>> {
    SettingsRepository::get_app_setting(conn, "zam_aylari")?
        .map(|value| serde_json::from_str(&value).map_err(Into::into))
        .transpose()
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
fn native_v3_restore_imports_retro_graph_inside_outer_transaction() {
    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let payload = json!({
        "backupVersion": 3,
        "donemler": [{
            "id": "2026-03",
            "yil": 2026,
            "ay": 3,
            "baslangicTarihi": "2026-03-15",
            "bitisTarihi": "2026-04-14",
            "donemAdi": "Mart 2026",
            "taxYear": 2026,
            "taxMonth": 4
        }],
        "personeller": [{
            "id": "retro-person",
            "tcNo": "10000000001",
            "ad": "Retro",
            "soyad": "Test",
            "grup": "1. Grup"
        }],
        "taxOpenings": [],
        "sickLeaveRecords": [],
        "annualPayrollParameters": [],
        "compensationRevisions": [{
            "id": "revision-1",
            "reason": "COLLECTIVE_AGREEMENT",
            "title": "2026 TİS",
            "effectiveFrom": "2026-03-15",
            "status": "FINALIZED",
            "scope": "SELECTED_PERSONNEL",
            "personnelIds": ["retro-person"]
        }],
        "compensationRevisionOverrides": [],
        "retroBatches": [{
            "id": "batch-1",
            "revisionId": "revision-1",
            "personnelId": "retro-person",
            "paymentDate": "2026-06-20",
            "status": "FINALIZED",
            "totalGrossDelta": 10
        }],
        "retroAllocations": [{
            "id": "allocation-1",
            "batchId": "batch-1",
            "personnelId": "retro-person",
            "sourcePeriodId": "2026-03",
            "earningCode": "BASE_WAGE",
            "originalRecognizedAmount": 100,
            "targetAmount": 110,
            "deltaAmount": 10,
            "sgkTreatment": "WAGE_SOURCE_MONTH",
            "incomeTaxTreatment": "TAXABLE",
            "stampTaxTreatment": "TAXABLE"
        }]
    })
    .to_string();

    MigrationService::replace_backup_data(&mut conn, &payload)
        .expect("retro graph içeren V3 yedek tek transaction ile içe aktarılmalı");

    assert_eq!(get_revisions(&conn).expect("revision okunmalı").len(), 1);
    assert_eq!(get_batches(&conn).expect("batch okunmalı").len(), 1);
    let restored_batch = &get_batches(&conn).expect("batch okunmalı")[0];
    assert_eq!(
        restored_batch.status,
        bordro_programi_lib::domain::models::CompensationRevisionStatus::STALE
    );
    assert_eq!(
        restored_batch.settlementStatus,
        bordro_programi_lib::domain::models::RetroSettlementStatus::UNSETTLED
    );
    let allocations = get_allocations(&conn).expect("allocation okunmalı");
    assert_eq!(allocations.len(), 1);
    assert_eq!(allocations[0].deltaAmount, Decimal::from(10));
    assert_eq!(restored_batch.payableSettlementAmount, Decimal::from(10));
    assert_eq!(restored_batch.offsetSettlementAmount, Decimal::ZERO);
    assert_eq!(restored_batch.recoverableAmount, Decimal::ZERO);
    assert_eq!(restored_batch.outstandingReceivable, Decimal::ZERO);
    assert_eq!(allocations[0].payableSettlementAmount, Decimal::from(10));
    assert_eq!(allocations[0].offsetSettlementAmount, Decimal::ZERO);
    assert_eq!(allocations[0].recoverableAmount, Decimal::ZERO);
}

#[test]
fn native_v3_restore_downgrades_linked_nonfinal_retro_event_with_batch() {
    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let mut retro_payroll = legacy_payroll_value();
    retro_payroll["id"] = json!("retro-payment-v3");
    retro_payroll["accrualId"] = json!("batch-v3-linked");
    retro_payroll["accrualType"] = json!("RETRO_ADJUSTMENT");
    retro_payroll["donemId"] = json!("2026-05");
    retro_payroll["paymentDate"] = json!("2026-06-20");
    retro_payroll["status"] = json!("CALCULATED");
    let payload = json!({
        "backupVersion": 3,
        "donemler": [
            {
                "id": "2026-03",
                "yil": 2026,
                "ay": 3,
                "baslangicTarihi": "2026-03-15",
                "bitisTarihi": "2026-04-14",
                "donemAdi": "Mart 2026",
                "taxYear": 2026,
                "taxMonth": 4
            },
            {
                "id": "2026-05",
                "yil": 2026,
                "ay": 5,
                "baslangicTarihi": "2026-05-15",
                "bitisTarihi": "2026-06-14",
                "donemAdi": "Mayıs 2026",
                "taxYear": 2026,
                "taxMonth": 6
            }
        ],
        "personeller": [{
            "id": "person-1",
            "tcNo": "10000000004",
            "ad": "V3",
            "soyad": "Linked",
            "grup": "1. Grup"
        }],
        "bordrolar": [retro_payroll],
        "taxOpenings": [],
        "sickLeaveRecords": [],
        "annualPayrollParameters": [],
        "compensationRevisions": [{
            "id": "revision-v3-linked",
            "reason": "COLLECTIVE_AGREEMENT",
            "title": "2026 V3 linked",
            "effectiveFrom": "2026-03-15",
            "status": "FINALIZED",
            "scope": "SELECTED_PERSONNEL",
            "personnelIds": ["person-1"]
        }],
        "compensationRevisionOverrides": [],
        "retroBatches": [{
            "id": "batch-v3-linked",
            "revisionId": "revision-v3-linked",
            "personnelId": "person-1",
            "paymentDate": "2026-06-20",
            "status": "FINALIZED",
            "totalGrossDelta": 0
        }],
        "retroAllocations": []
    })
    .to_string();

    MigrationService::replace_backup_data(&mut conn, &payload)
        .expect("tutarsız V3 graph güvenli biçimde stale'e indirilmeli");

    let batch = get_batches(&conn).expect("batch okunmalı").remove(0);
    assert_eq!(
        batch.status,
        bordro_programi_lib::domain::models::CompensationRevisionStatus::STALE
    );
    assert_eq!(
        batch.settlementStatus,
        bordro_programi_lib::domain::models::RetroSettlementStatus::UNSETTLED
    );
    let payroll = PayrollRepository::get_all(&conn)
        .expect("payment event okunmalı")
        .into_iter()
        .next()
        .expect("payment event bulunmalı");
    assert_eq!(payroll.status, BordroStatus::STALE);
}

#[test]
fn native_v3_restore_preserves_multi_accrual_payment_event_identity() {
    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    let mut retro_payroll = legacy_payroll_value();
    retro_payroll["id"] = json!("payroll-v3-retro");
    retro_payroll["accrualId"] = json!("payroll-v3-retro");
    retro_payroll["accrualType"] = json!("RETRO_ADJUSTMENT");
    retro_payroll["paymentDate"] = json!("2026-02-14");
    retro_payroll["sequence"] = json!(1);
    let payload = json!({
        "backupVersion": 3,
        "donemler": [{
            "id": "2026-01",
            "yil": 2026,
            "ay": 1,
            "baslangicTarihi": "2026-01-15",
            "bitisTarihi": "2026-02-14",
            "donemAdi": "Ocak 2026",
            "taxYear": 2026,
            "taxMonth": 2
        }],
        "personeller": [{
            "id": "person-1",
            "tcNo": "10000000002",
            "ad": "V3",
            "soyad": "Retro",
            "grup": "1. Grup"
        }],
        "bordrolar": [legacy_payroll_value(), retro_payroll],
        "taxOpenings": [],
        "sickLeaveRecords": [],
        "annualPayrollParameters": []
    })
    .to_string();

    MigrationService::replace_backup_data(&mut conn, &payload)
        .expect("V3 çoklu tahakkuk kimliği korunarak içe aktarılmalı");

    let payrolls = PayrollRepository::get_all(&conn).expect("tahakkuklar okunmalı");
    let restored = payrolls
        .iter()
        .find(|payroll| payroll.id == "payroll-v3-retro")
        .expect("V3 retro payment event bulunmalı");
    assert_eq!(
        restored.accrualType,
        bordro_programi_lib::domain::models::AccrualType::RETRO_ADJUSTMENT
    );
    assert_eq!(restored.accrualId, "payroll-v3-retro");
    assert_eq!(restored.sequence, 1);
}

#[test]
fn native_v4_restore_preserves_calculated_retro_with_stale_linked_payment() {
    let mut retro_payroll = legacy_payroll_value();
    retro_payroll["id"] = json!("retro-payment-v4");
    retro_payroll["accrualId"] = json!("batch-v4");
    retro_payroll["accrualType"] = json!("RETRO_ADJUSTMENT");
    retro_payroll["donemId"] = json!("2026-03");
    retro_payroll["paymentDate"] = json!("2026-04-20");
    retro_payroll["sequence"] = json!(0);
    retro_payroll["gelirler"] = json!({"ekOdeme": 10});
    retro_payroll["gelirToplam"] = json!(10);
    retro_payroll["kesintiToplam"] = json!(0);
    retro_payroll["netOdeme"] = json!(10);
    retro_payroll["status"] = json!("FINALIZED");

    let payload = json!({
        "backupVersion": 4,
        "donemler": [{
            "id": "2026-03",
            "yil": 2026,
            "ay": 3,
            "baslangicTarihi": "2026-03-15",
            "bitisTarihi": "2026-04-14",
            "donemAdi": "Mart 2026",
            "taxYear": 2026,
            "taxMonth": 4
        }],
        "personeller": [{
            "id": "person-1",
            "tcNo": "10000000003",
            "ad": "V4",
            "soyad": "Retro",
            "grup": "1. Grup"
        }],
        "bordrolar": [retro_payroll],
        "taxOpenings": [],
        "sickLeaveRecords": [],
        "annualPayrollParameters": [],
        "compensationRevisions": [{
            "id": "revision-v4",
            "reason": "COLLECTIVE_AGREEMENT",
            "title": "2026 V4",
            "effectiveFrom": "2026-03-15",
            "status": "CALCULATED",
            "scope": "SELECTED_PERSONNEL",
            "personnelIds": ["person-1"]
        }],
        "compensationRevisionOverrides": [],
        "retroBatches": [{
            "id": "batch-v4",
            "revisionId": "revision-v4",
            "personnelId": "person-1",
            "paymentDate": "2026-04-20",
            "status": "CALCULATED",
            "settlementStatus": "UNSETTLED",
            "totalGrossDelta": 10
        }],
        "retroAllocations": [{
            "id": "allocation-v4",
            "batchId": "batch-v4",
            "personnelId": "person-1",
            "sourcePeriodId": "2026-03",
            "earningCode": "BASE_WAGE",
            "originalRecognizedAmount": 0,
            "targetAmount": 10,
            "deltaAmount": 10,
            "sgkTreatment": "WAGE_SOURCE_MONTH",
            "incomeTaxTreatment": "TAXABLE",
            "stampTaxTreatment": "TAXABLE"
        }]
    })
    .to_string();

    let mut conn = create_in_memory_connection().expect("in-memory SQLite kurulmalı");
    MigrationService::replace_backup_data(&mut conn, &payload)
        .expect("V4 legacy normalization sonrası CALCULATED + STALE recovery state korunmalı");

    let batch = get_batches(&conn)
        .expect("retro batch okunmalı")
        .into_iter()
        .next()
        .expect("batch restore edilmeli");
    let payment = PayrollRepository::get_all(&conn)
        .expect("payment event okunmalı")
        .into_iter()
        .next()
        .expect("payment event restore edilmeli");
    assert_eq!(batch.status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(payment.status, BordroStatus::STALE);
    assert_eq!(payment.accrualType, AccrualType::RETRO_ADJUSTMENT);
    assert_eq!(payment.accrualId, batch.id);
    assert_eq!(payment.personelId, batch.personnelId);
    assert_eq!(payment.paymentDate, batch.paymentDate);
    assert_eq!(payment.gelirToplam, batch.payableSettlementAmount);
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

fn current_v5_fixture() -> Result<(rusqlite::Connection, String), Box<dyn std::error::Error>> {
    current_v5_fixture_with_nafaka(None)
}

fn current_v5_fixture_with_nafaka(
    nafaka_tutar: Option<Decimal>,
) -> Result<(rusqlite::Connection, String), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;
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
    let prior_period = BordroDonemi {
        id: "2025-12".into(),
        yil: 2025,
        ay: 12,
        baslangicTarihi: "2025-12-15".into(),
        bitisTarihi: "2026-01-14".into(),
        donemAdi: "Aralık 2025".into(),
        taxYear: 2026,
        taxMonth: 1,
    };
    let personnel = Personel {
        id: "v5-person".into(),
        tcNo: "10000000005".into(),
        ad: "Current".into(),
        soyad: "Backup".into(),
        grup: "1. Grup".into(),
        unvan: None,
        sgkSicilNo: "V5".into(),
        iban: "TR00".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri {
            nafakaTutar: nafaka_tutar,
            ..PersonelKesintileri::default()
        }),
    };
    let settings = DonemselKurumDegerleri {
        donemId: period.id.clone(),
        gunlukTabanUcret: dec!(1000),
        gunlukYemek: dec!(0),
        birlestirilmisSosyalYardim: dec!(0),
        gunlukVasitaYol: dec!(0),
        giyimYardimi: dec!(0),
        hizmetZammiBirimi: dec!(0),
        isPrimiGruplari: Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(0),
            aktif: true,
        }]),
        ekOdeme: Some(dec!(0)),
        digerGelirVarsayilan: Some(dec!(0)),
        sgkIsciOraniYuzde: Some(dec!(14)),
        issizlikIsciOraniYuzde: Some(dec!(1)),
        damgaVergisiOraniBinde: Some(dec!(7.59)),
        sendikaAidatiYuzde: Some(dec!(0)),
        besOraniYuzde: Some(dec!(0)),
        gunlukYemekIstisnasiSGK: Some(dec!(0)),
        gunlukYemekIstisnasiGV: Some(dec!(0)),
        gunlukAsgariUcret: Some(dec!(1101)),
        pekTavanKatsayisi: Some(dec!(3)),
        sgkIsverenOraniYuzde: Some(dec!(21.75)),
        issizlikIsverenOraniYuzde: Some(dec!(2)),
        ..DonemselKurumDegerleri::default()
    };
    let mut prior_settings = settings.clone();
    prior_settings.donemId = prior_period.id.clone();
    let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")?;
    let mut date = start;
    let mut days = HashMap::new();
    while date <= end {
        days.insert(date.format("%Y-%m-%d").to_string(), "Ç".into());
        date += Duration::days(1);
    }
    let attendance = PersonelPuantaj {
        id: format!("{}_{}", personnel.id, period.id),
        personelId: personnel.id.clone(),
        donemId: period.id.clone(),
        gunler: days,
    };

    PersonnelRepository::save(&conn, &personnel)?;
    PeriodRepository::save(&conn, &prior_period)?;
    PeriodRepository::save(&conn, &period)?;
    SettingsRepository::save_institution_settings(&conn, &prior_settings)?;
    SettingsRepository::save_institution_settings(&conn, &settings)?;
    compensation_policy_test_support::seed_period_baseline(&conn, &prior_period, &prior_settings)?;
    compensation_policy_test_support::seed_period_baseline(&conn, &period, &settings)?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;
    AttendanceRepository::save(&conn, &attendance)?;
    PayrollService::calculate_payroll_for_personnel(&conn, &personnel.id, &period.id)?;

    let dataset = PayrollService::build_dataset_snapshot(&conn)?;
    let institution_settings = SettingsRepository::get_all_institution_settings(&conn)?;
    let mut v5_payrolls = dataset.payrolls.clone();
    for payroll in &mut v5_payrolls {
        preserve_v1_insurance_cap(payroll);
        payroll.calculationMetadata = None;
    }
    let payload = json!({
        "backupVersion": 5,
        "exportedAt": "2026-09-09T00:00:00.000Z",
        "donemler": dataset.periods,
        "aktifDonemId": period.id,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": institution_settings,
        "puantajlar": dataset.attendances,
        "bordrolar": v5_payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "zamAylari": legacy_raise_months(&conn)?,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations
    })
    .to_string();
    Ok((conn, payload))
}

fn current_v5_payload_from_database(
    conn: &rusqlite::Connection,
) -> Result<String, Box<dyn std::error::Error>> {
    let dataset = PayrollService::build_dataset_snapshot(conn)?;
    let institution_settings = SettingsRepository::get_all_institution_settings(conn)?;
    let mut v5_payrolls = dataset.payrolls.clone();
    for payroll in &mut v5_payrolls {
        preserve_v1_insurance_cap(payroll);
        payroll.calculationMetadata = None;
    }
    let active_period_id =
        SettingsRepository::get_app_setting(conn, "active_period_id")?.unwrap_or_default();
    Ok(json!({
        "backupVersion": 5,
        "exportedAt": "2026-10-03T00:00:00.000Z",
        "donemler": dataset.periods,
        "aktifDonemId": active_period_id,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": institution_settings,
        "puantajlar": dataset.attendances,
        "bordrolar": v5_payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "zamAylari": legacy_raise_months(conn)?,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations
    })
    .to_string())
}

/// Version-5 fixtures model the historical v1 calculation contract. That
/// version based its 15% insurance limit on gross income less all meal and
/// road amounts, so do not export a newly calculated v2 result as though it
/// were a v1 backup.
fn preserve_v1_insurance_cap(payroll: &mut BordroKaydi) {
    let Some(gv) = payroll.gvDetay.as_mut() else {
        return;
    };
    let v1_wage_base = (payroll.gelirToplam
        - payroll.gelirler.yemek.unwrap_or_default()
        - payroll.gelirler.vasitaYol.unwrap_or_default())
    .max(Decimal::ZERO);
    gv.sigortaGvAylikLimiti = (v1_wage_base * dec!(0.15))
        .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
}

fn v5_dated_road_fixture(
    finalize: bool,
) -> Result<(rusqlite::Connection, String), Box<dyn std::error::Error>> {
    let (conn, _) = current_v5_fixture()?;
    let mut prior_settings =
        SettingsRepository::get_institution_settings(&conn, "2025-12")?.expect("prior settings");
    prior_settings.gunlukVasitaYol = dec!(10);
    SettingsRepository::save_institution_settings(&conn, &prior_settings)?;

    let mut current_settings =
        SettingsRepository::get_institution_settings(&conn, "2026-01")?.expect("current settings");
    current_settings.gunlukVasitaYol = dec!(10);
    current_settings.transportRateChanges = Some(vec![TransportRateChange {
        effectiveFrom: "2026-02-01".into(),
        gunlukVasitaYol: dec!(20),
    }]);
    SettingsRepository::save_institution_settings(&conn, &current_settings)?;

    // Recreate the test policy inputs to match an exact archived legacy road
    // schedule. The target payroll below is recalculated by the native service.
    PayrollRepository::delete_accrual(&conn, "v5-person", "2026-01", "v5-person_2026-01")?;
    conn.execute(
        "DELETE FROM compensation_policy_versions WHERE id = ?1",
        ["test-policy-2026-01"],
    )?;
    let mut baseline_state =
        compensation_policy_test_support::state_from_settings(&current_settings);
    baseline_state.gunlukVasitaYol = dec!(10);
    CompensationPolicyRepository::insert_version(
        &conn,
        &CompensationPolicyVersion {
            id: "v5-road-baseline".into(),
            effectiveFrom: "2026-01-15".into(),
            state: baseline_state.clone(),
            boundaryScope: CompensationBoundaryScope::FullEarning,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "V5 dated-road replay fixture".into(),
            supersedesVersionId: None,
        },
    )?;
    let mut dated_road_state = baseline_state;
    dated_road_state.gunlukVasitaYol = dec!(20);
    CompensationPolicyRepository::insert_version(
        &conn,
        &CompensationPolicyVersion {
            id: "v5-road-change".into(),
            effectiveFrom: "2026-02-01".into(),
            state: dated_road_state,
            boundaryScope: CompensationBoundaryScope::RoadOnly,
            roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
            createdAt: "2026-01-01T00:00:00Z".into(),
            source: "TEST_FIXTURE".into(),
            reason: "V5 dated-road replay fixture".into(),
            supersedesVersionId: None,
        },
    )?;
    SettingsRepository::set_app_setting(&conn, "zam_aylari", "[3,9]")?;
    let calculated =
        PayrollService::calculate_payroll_for_personnel(&conn, "v5-person", "2026-01")?;
    assert_eq!(calculated.gelirler.vasitaYol, Some(dec!(450)));
    if finalize {
        let finalized =
            PayrollService::finalize_payroll_for_personnel(&conn, "v5-person", "2026-01")?;
        assert_eq!(finalized.status, BordroStatus::FINALIZED);
    } else {
        assert_eq!(calculated.status, BordroStatus::CALCULATED);
    }
    let payload = current_v5_payload_from_database(&conn)?;
    Ok((conn, payload))
}

fn current_v6_payload_from_database(
    conn: &rusqlite::Connection,
) -> Result<String, Box<dyn std::error::Error>> {
    let dataset = PayrollService::build_dataset_snapshot(conn)?;
    let active_period_id =
        SettingsRepository::get_app_setting(conn, "active_period_id")?.unwrap_or_default();
    Ok(json!({
        "backupVersion": 6,
        "exportedAt": "2026-10-03T00:00:00.000Z",
        "donemler": dataset.periods,
        "aktifDonemId": active_period_id,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": SettingsRepository::get_all_institution_settings(conn)?,
        "puantajlar": dataset.attendances,
        "bordrolar": dataset.payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "zamAylari": legacy_raise_months(conn)?,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations,
        "compensationPolicyVersions": dataset.compensationPolicyVersions
    })
    .to_string())
}

fn v5_backup_fingerprint(conn: &rusqlite::Connection) -> Result<Value, Box<dyn std::error::Error>> {
    let dataset = PayrollService::build_dataset_snapshot(conn)?;
    let mut batches = dataset
        .retroBatches
        .iter()
        .map(|batch| {
            json!({
                "id": batch.id,
                "status": batch.status,
                "settlementStatus": batch.settlementStatus,
                "totalGrossDelta": batch.totalGrossDelta,
                "payableSettlementAmount": batch.payableSettlementAmount,
                "recoverableAmount": batch.recoverableAmount,
                "outstandingReceivable": batch.outstandingReceivable
            })
        })
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let mut payrolls = dataset
        .payrolls
        .iter()
        .map(|payroll| {
            json!({
                "id": payroll.id,
                "status": payroll.status,
                "gelirToplam": payroll.gelirToplam,
                "kesintiToplam": payroll.kesintiToplam,
                "netOdeme": payroll.netOdeme
            })
        })
        .collect::<Vec<_>>();
    payrolls.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    Ok(json!({
        "personnelCount": dataset.personnel.len(),
        "periodCount": dataset.periods.len(),
        "timesheetCount": dataset.attendances.len(),
        "payrollCount": dataset.payrolls.len(),
        "retroBatchCount": dataset.retroBatches.len(),
        "allocationCount": dataset.retroAllocations.len(),
        "paymentEventCount": dataset.payrolls.iter().filter(|item| item.accrualType == AccrualType::RETRO_ADJUSTMENT).count(),
        "batches": batches,
        "payrolls": payrolls
    }))
}

#[test]
fn native_current_v5_backup_roundtrip_replays_authoritative_snapshot(
) -> Result<(), Box<dyn std::error::Error>> {
    let (source, payload) = current_v5_fixture()?;
    let before = PayrollRepository::get_all(&source)?;
    let mut target = create_in_memory_connection()?;

    MigrationService::replace_backup_data(&mut target, &payload)?;

    let after = PayrollRepository::get_all(&target)?;
    fn financial_snapshot_without_timestamps(payrolls: Vec<BordroKaydi>) -> Value {
        let mut value = serde_json::to_value(payrolls).expect("payroll snapshot serialize");
        for item in value.as_array_mut().expect("payroll array") {
            let object = item.as_object_mut().expect("payroll object");
            object.remove("olusturulmaTarihi");
            object.remove("sonGuncellemeTarihi");
            object.remove("calculationMetadata");
        }
        value
    }
    assert_eq!(
        financial_snapshot_without_timestamps(before),
        financial_snapshot_without_timestamps(after)
    );
    Ok(())
}

#[test]
fn native_v5_legacy_replay_preserves_real_calculated_and_finalized_dated_road_rows(
) -> Result<(), Box<dyn std::error::Error>> {
    for (finalize, expected_status) in [
        (false, BordroStatus::CALCULATED),
        (true, BordroStatus::FINALIZED),
    ] {
        let (_source, payload) = v5_dated_road_fixture(finalize)?;
        let payload_value: Value = serde_json::from_str(&payload)?;
        assert_eq!(payload_value["zamAylari"], json!([3, 9]));
        assert_eq!(
            payload_value["kurumDegerleriMap"]["2026-01"]["transportRateChanges"][0]
                ["effectiveFrom"],
            json!("2026-02-01")
        );

        let mut restored = create_in_memory_connection()?;
        MigrationService::replace_backup_data(&mut restored, &payload)?;
        let payroll = PayrollRepository::get_all(&restored)?
            .into_iter()
            .find(|payroll| payroll.donemId == "2026-01")
            .expect("restored V5 payroll");
        assert_eq!(payroll.status, expected_status);
        assert_eq!(payroll.gelirler.vasitaYol, Some(dec!(450)));
        assert!(payroll.calculationMetadata.is_none());
        assert!(CompensationPolicyRepository::get_all_versions(&restored)?.is_empty());
        let archived = SettingsRepository::get_raw_institution_settings(&restored, "2026-01")?
            .expect("archived source settings");
        assert_eq!(
            archived["transportRateChanges"][0]["gunlukVasitaYol"],
            json!("20")
        );
    }
    Ok(())
}

#[test]
fn native_v5_restore_preserves_absent_and_empty_archived_road_schedules(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, base_payload) = current_v5_fixture()?;
    for empty_schedule in [false, true] {
        let mut payload: Value = serde_json::from_str(&base_payload)?;
        payload["zamAylari"] = json!([3, 9]);
        let current_settings = payload["kurumDegerleriMap"]["2026-01"]
            .as_object_mut()
            .expect("current archived settings");
        if empty_schedule {
            current_settings.insert("transportRateChanges".into(), json!([]));
        } else {
            current_settings.remove("transportRateChanges");
        }

        let mut restored = create_in_memory_connection()?;
        MigrationService::replace_backup_data(&mut restored, &payload.to_string())?;
        let archived = SettingsRepository::get_raw_institution_settings(&restored, "2026-01")?
            .expect("restored archived settings");
        if empty_schedule {
            assert_eq!(archived["transportRateChanges"], json!([]));
        } else {
            assert!(archived.get("transportRateChanges").is_none());
        }
        assert!(CompensationPolicyRepository::get_all_versions(&restored)?.is_empty());
    }
    Ok(())
}

#[test]
fn missing_legacy_gv_chain_skips_only_replay_and_keeps_v5_authority_checks(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, base_payload) = current_v5_fixture()?;
    let without_chain = |payload: &str| -> Result<Value, Box<dyn std::error::Error>> {
        let mut value: Value = serde_json::from_str(payload)?;
        value["zamAylari"] = json!([3, 9]);
        value["taxOpenings"] = json!([]);
        value["donemler"] = json!([value["donemler"][1].clone()]);
        value["kurumDegerleriMap"]
            .as_object_mut()
            .expect("settings map")
            .remove("2025-12");
        Ok(value)
    };

    let valid = without_chain(&base_payload)?;
    let mut restored = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut restored, &valid.to_string())?;
    let retained = PayrollRepository::get_all(&restored)?;
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].status, BordroStatus::CALCULATED);

    for tamper in ["persisted-gv", "negative-pek"] {
        let mut forged = without_chain(&base_payload)?;
        let payroll = &mut forged["bordrolar"][0];
        if tamper == "persisted-gv" {
            let current = payroll["persistedGvBase"]
                .as_str()
                .unwrap()
                .parse::<Decimal>()?;
            payroll["persistedGvBase"] = json!((current + Decimal::ONE).to_string());
        } else {
            payroll["pekDetay"]["primMatrahi"] = json!("-1");
        }
        let mut target = create_in_memory_connection()?;
        MigrationService::replace_backup_data(&mut target, &forged.to_string())
            .expect_err("missing legacy GV history must not bypass persisted V5 authority checks");
        assert!(PayrollRepository::get_all(&target)?.is_empty());
    }
    Ok(())
}

#[test]
fn native_v5_zero_recovery_and_overpayment_backup_roundtrips_and_rejects_tampering(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, base_payload) = current_v5_fixture()?;
    let mut source = create_in_memory_connection()?;
    let mut runtime_state: Value = serde_json::from_str(&base_payload)?;
    runtime_state["compensationRevisions"] = json!([{
        "id": "revision-receivable-replay",
        "reason": "COLLECTIVE_AGREEMENT",
        "title": "Receivable replay",
        "effectiveFrom": "2026-01-15",
        "status": "CALCULATED",
        "scope": "SELECTED_PERSONNEL",
        "personnelIds": ["v5-person"]
    }]);
    runtime_state["retroBatches"] = json!([
        {
            "id": "retro-0cbb12f3-4999-43d6-8215-9b710093c5d4",
            "revisionId": "revision-receivable-replay",
            "personnelId": "v5-person",
            "paymentDate": "2027-03-14",
            "status": "CALCULATED",
            "settlementStatus": "UNSETTLED",
            "totalGrossDelta": "0.00",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoveredAmount": "0.00",
            "recoverableAmount": "0.00",
            "outstandingReceivable": "160872.98",
            "createdAt": "2026-09-30T11:43:10.586Z",
            "calculatedAt": "2026-09-30T11:43:10.586Z"
        },
        {
            "id": "retro-43d8c24e-6c31-416b-b203-c334568697c1",
            "revisionId": "revision-receivable-replay",
            "personnelId": "v5-person",
            "paymentDate": "2027-03-14",
            "status": "CALCULATED",
            "settlementStatus": "OVERPAYMENT",
            "totalGrossDelta": "-160872.98",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoveredAmount": "0.00",
            "recoverableAmount": "160872.98",
            "outstandingReceivable": "160872.98"
        },
        {
            "id": "retro-f0000000-0000-4000-8000-000000000001",
            "revisionId": "revision-receivable-replay",
            "personnelId": "v5-person",
            "paymentDate": "2027-03-14",
            "status": "CALCULATED",
            "settlementStatus": "OVERPAYMENT",
            "totalGrossDelta": "-10.00",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoveredAmount": "0.00",
            "recoverableAmount": "10.00",
            "outstandingReceivable": "160882.98",
            "createdAt": "2026-10-01T11:43:10.586Z",
            "calculatedAt": "2026-10-01T11:43:10.586Z"
        }
    ]);
    runtime_state["retroAllocations"] = json!([
        {
            "id": "allocation-prior-overpayment",
            "batchId": "retro-43d8c24e-6c31-416b-b203-c334568697c1",
            "personnelId": "v5-person",
            "sourcePeriodId": "2025-12",
            "earningCode": "BASE_WAGE",
            "originalRecognizedAmount": "160872.98",
            "targetAmount": "0.00",
            "deltaAmount": "-160872.98",
            "sgkTreatment": "WAGE_SOURCE_MONTH",
            "incomeTaxTreatment": "TAXABLE",
            "stampTaxTreatment": "TAXABLE",
            "originalEmployerLowerBound": "0.00",
            "targetEmployerLowerBound": "0.00",
            "employerLowerBoundDelta": "0.00",
            "employerLowerBoundPremiumDelta": "0.00",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoverableAmount": "160872.98"
        },
        {
            "id": "allocation-zero-recovery",
            "batchId": "retro-0cbb12f3-4999-43d6-8215-9b710093c5d4",
            "personnelId": "v5-person",
            "sourcePeriodId": "2025-12",
            "earningCode": "BASE_WAGE",
            "originalRecognizedAmount": "0.00",
            "targetAmount": "0.00",
            "deltaAmount": "0.00",
            "sgkTreatment": "WAGE_SOURCE_MONTH",
            "incomeTaxTreatment": "TAXABLE",
            "stampTaxTreatment": "TAXABLE",
            "originalEmployerLowerBound": "0.00",
            "targetEmployerLowerBound": "0.00",
            "employerLowerBoundDelta": "0.00",
            "employerLowerBoundPremiumDelta": "0.00",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoverableAmount": "0.00"
        },
        {
            "id": "allocation-later-overpayment",
            "batchId": "retro-f0000000-0000-4000-8000-000000000001",
            "personnelId": "v5-person",
            "sourcePeriodId": "2025-12",
            "earningCode": "BASE_WAGE",
            "originalRecognizedAmount": "10.00",
            "targetAmount": "0.00",
            "deltaAmount": "-10.00",
            "sgkTreatment": "WAGE_SOURCE_MONTH",
            "incomeTaxTreatment": "TAXABLE",
            "stampTaxTreatment": "TAXABLE",
            "originalEmployerLowerBound": "0.00",
            "targetEmployerLowerBound": "0.00",
            "employerLowerBoundDelta": "0.00",
            "employerLowerBoundPremiumDelta": "0.00",
            "payableSettlementAmount": "0.00",
            "offsetSettlementAmount": "0.00",
            "recoverableAmount": "10.00"
        }
    ]);
    MigrationService::replace_backup_data(&mut source, &runtime_state.to_string())?;
    let before_fingerprint = v5_backup_fingerprint(&source)?;
    let backup_payload = current_v5_payload_from_database(&source)?;
    let mut restored = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut restored, &backup_payload)?;
    assert_eq!(v5_backup_fingerprint(&restored)?, before_fingerprint);

    MigrationService::replace_backup_data(&mut restored, &base_payload)?;
    assert_ne!(
        v5_backup_fingerprint(&restored)?,
        before_fingerprint,
        "controlled database change must differ from the saved backup"
    );
    MigrationService::replace_backup_data(&mut restored, &backup_payload)?;
    assert_eq!(
        v5_backup_fingerprint(&restored)?,
        before_fingerprint,
        "restoring after a controlled change must recover the saved fingerprint"
    );

    let restored_dataset = PayrollService::build_dataset_snapshot(&restored)?;
    let zero = restored_dataset
        .retroBatches
        .iter()
        .find(|batch| batch.id.starts_with("retro-0cbb"))
        .expect("zero-difference recovery batch must restore");
    let prior_overpayment = restored_dataset
        .retroBatches
        .iter()
        .find(|batch| batch.id.starts_with("retro-43d8"))
        .expect("eventless overpayment batch must restore");
    assert_eq!(zero.totalGrossDelta, Decimal::ZERO);
    assert_eq!(zero.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(zero.outstandingReceivable, dec!(160872.98));
    assert_eq!(
        prior_overpayment.settlementStatus,
        RetroSettlementStatus::OVERPAYMENT
    );
    assert_eq!(prior_overpayment.outstandingReceivable, dec!(160872.98));
    let later_overpayment = restored_dataset
        .retroBatches
        .iter()
        .find(|batch| batch.id.starts_with("retro-f000"))
        .expect("later eventless overpayment batch must restore");
    assert_eq!(
        later_overpayment.settlementStatus,
        RetroSettlementStatus::OVERPAYMENT
    );
    assert_eq!(later_overpayment.recoverableAmount, dec!(10.00));
    assert_eq!(later_overpayment.outstandingReceivable, dec!(160882.98));
    let zero_allocation = restored_dataset
        .retroAllocations
        .iter()
        .find(|allocation| allocation.batchId == zero.id)
        .expect("zero-recovery allocation must restore");
    assert_eq!(zero_allocation.payableSettlementAmount, Decimal::ZERO);
    assert_eq!(
        restored_dataset
            .payrolls
            .iter()
            .filter(|item| item.accrualType == AccrualType::RETRO_ADJUSTMENT)
            .count(),
        0
    );

    let mut payroll_chain_check = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut payroll_chain_check, &backup_payload)?;
    let before_legacy_recalculation =
        serde_json::to_value(PayrollRepository::get_all(&payroll_chain_check)?)?;
    let legacy_recalculation_error = PayrollService::calculate_payroll_for_personnel(
        &payroll_chain_check,
        "v5-person",
        "2026-01",
    )
    .expect_err("V5 restore must not invent a persistent compensation baseline");
    assert!(legacy_recalculation_error
        .to_string()
        .contains("Bu tarih için ücret politikası tanımlı değil"));
    assert_eq!(
        serde_json::to_value(PayrollRepository::get_all(&payroll_chain_check)?)?,
        before_legacy_recalculation,
        "missing policy baseline must leave restored payroll unchanged"
    );
    assert!(CompensationPolicyRepository::get_all_versions(&payroll_chain_check)?.is_empty());

    let before_failure = v5_backup_fingerprint(&restored)?;
    let mut corrupted: Value = serde_json::from_str(&backup_payload)?;
    let zero_index = corrupted["retroBatches"]
        .as_array()
        .unwrap()
        .iter()
        .position(|batch| batch["id"] == "retro-0cbb12f3-4999-43d6-8215-9b710093c5d4")
        .expect("zero batch in exported backup");
    corrupted["retroBatches"][zero_index]["outstandingReceivable"] = json!("0.00");
    let error = MigrationService::replace_backup_data(&mut restored, &corrupted.to_string())
        .expect_err("forged receivable snapshot must be rejected");
    assert!(error
        .to_string()
        .contains("outstanding receivable snapshot"));
    assert_eq!(
        v5_backup_fingerprint(&restored)?,
        before_failure,
        "failed restore rolls back all data"
    );
    let integrity: String = restored.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    assert_eq!(integrity, "ok");
    Ok(())
}

#[test]
fn native_previous_v5_backup_without_nafaka_restores_and_stays_canonical(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, payload) = current_v5_fixture()?;
    let backup: Value = serde_json::from_str(&payload)?;
    assert!(backup["personeller"][0]["kesintiler"]
        .get("nafakaTutar")
        .is_none());
    assert!(backup["bordrolar"][0]["kesintiler"].get("nafaka").is_none());

    let mut clean_db = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut clean_db, &payload)?;

    let personnel = PersonnelRepository::get_by_id(&clean_db, "v5-person")?
        .expect("legacy backup personeli restore edilmeli");
    let payroll = PayrollRepository::get_all(&clean_db)?
        .into_iter()
        .next()
        .expect("legacy backup bordrosu restore edilmeli");
    assert_eq!(
        personnel
            .kesintiler
            .as_ref()
            .and_then(|items| items.nafakaTutar),
        None
    );
    assert_eq!(payroll.kesintiler.nafaka, None);
    let canonical_person = serde_json::to_value(&personnel)?;
    let canonical_payroll = serde_json::to_value(&payroll)?;
    assert!(canonical_person["kesintiler"].get("nafakaTutar").is_none());
    assert!(canonical_payroll["kesintiler"].get("nafaka").is_none());
    Ok(())
}

#[test]
fn native_v5_backup_restore_preserves_nafaka_to_the_kurus() -> Result<(), Box<dyn std::error::Error>>
{
    let (source, payload) = current_v5_fixture_with_nafaka(Some(dec!(125.50)))?;
    let source_payroll = PayrollRepository::get_all(&source)?
        .into_iter()
        .next()
        .expect("source nafaka bordrosu hesaplanmış olmalı");
    let mut clean_db = create_in_memory_connection()?;

    MigrationService::replace_backup_data(&mut clean_db, &payload)?;

    let personnel = PersonnelRepository::get_by_id(&clean_db, "v5-person")?
        .expect("nafaka tanımlı personel restore edilmeli");
    let payroll = PayrollRepository::get_all(&clean_db)?
        .into_iter()
        .next()
        .expect("nafaka kesintili bordro restore edilmeli");
    assert_eq!(
        personnel
            .kesintiler
            .as_ref()
            .and_then(|items| items.nafakaTutar),
        Some(dec!(125.50))
    );
    assert_eq!(payroll.kesintiler.nafaka, Some(dec!(125.50)));
    assert_eq!(payroll.kesintiToplam, source_payroll.kesintiToplam);
    assert_eq!(payroll.netOdeme, source_payroll.netOdeme);
    Ok(())
}

#[test]
fn normal_payroll_save_replaces_last_modified_timestamp() -> Result<(), Box<dyn std::error::Error>>
{
    let (conn, _) = current_v5_fixture()?;
    let mut payroll = PayrollRepository::get_all(&conn)?
        .into_iter()
        .next()
        .expect("calculated payroll exists");
    let created_at = payroll.olusturulmaTarihi.clone();
    payroll.sonGuncellemeTarihi = "2099-01-01T00:00:00.000Z".into();

    PayrollRepository::save_in_transaction(&conn, &payroll)?;

    let saved = PayrollRepository::get_all(&conn)?
        .into_iter()
        .next()
        .expect("payroll remains present");
    assert_ne!(saved.sonGuncellemeTarihi, "2099-01-01T00:00:00.000Z");
    assert_eq!(saved.olusturulmaTarihi, created_at);
    Ok(())
}

#[test]
fn native_reference_backup_restores_calculated_entitlement_with_stale_payment_exactly(
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = include_str!("../../tests/fixtures/session3-reference.json");
    let mut expected: Value = serde_json::from_str(payload)?;
    assert_eq!(expected["zamAylari"], json!([]));
    expected.as_object_mut().unwrap().remove("zamAylari");
    // Pre-policy backups retain their recorded daily-limited method explicitly.
    let historical_periods = expected["bordrolar"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|payroll| payroll["donemId"].as_str().map(str::to_owned))
        .chain(
            expected["retroAllocations"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|allocation| allocation["sourcePeriodId"].as_str().map(str::to_owned)),
        )
        .collect::<std::collections::BTreeSet<_>>();
    for (id, settings) in expected["kurumDegerleriMap"].as_object_mut().unwrap() {
        if historical_periods.contains(id) {
            settings["yemekTamGvIstisnaDamgaTabi"] = false.into();
        } else {
            settings["yemekTamGvIstisnaDamgaTabi"] = true.into();
        }
    }
    let archived_settings = expected["kurumDegerleriMap"].clone();
    for settings in expected["kurumDegerleriMap"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        let legacy: DonemselKurumDegerleri = serde_json::from_value(settings.clone())?;
        *settings = serde_json::to_value(payroll_core::PayrollPeriodInstitutionSettings::from(
            &legacy,
        ))?;
    }

    let mut conn = create_in_memory_connection()?;

    MigrationService::replace_backup_data(&mut conn, payload)?;
    assert!(SettingsRepository::get_app_setting(&conn, "zam_aylari")?.is_none());

    const LEGACY_COMPENSATION_FIELDS: &[&str] = &[
        "gunlukTabanUcret",
        "gunlukYemek",
        "birlestirilmisSosyalYardim",
        "gunlukVasitaYol",
        "transportRateChanges",
        "giyimYardimi",
        "hizmetZammiBirimi",
        "isPrimiYuzde",
        "isPrimiGruplari",
        "geceCalismaPrimiYuzde",
        "geceCalismaTatiliPrimiYuzde",
        "ekOdeme",
        "digerGelirVarsayilan",
    ];
    for (period_id, archived) in archived_settings.as_object().unwrap() {
        let raw = SettingsRepository::get_raw_institution_settings(&conn, period_id)?
            .expect("restored raw settings should exist");
        for field in LEGACY_COMPENSATION_FIELDS {
            if let Some(value) = archived.get(*field) {
                assert_eq!(raw.get(*field), Some(value), "archived {period_id}.{field}");
            }
        }
    }

    let dataset = PayrollService::build_dataset_snapshot(&conn)?;
    let mut restored = json!({
        "donemler": dataset.periods,
        "aktifDonemId": SettingsRepository::get_app_setting(&conn, "active_period_id")?,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": dataset.institutionSettings,
        "puantajlar": dataset.attendances,
        "bordrolar": dataset.payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations,
    });
    // SQLite's Decimal persistence emits a canonical string scale; compare
    // decimal value, not its original lexical scale. Audit timestamps remain
    // part of the exact authoritative restore snapshot.
    fn normalize_restore_comparison(value: &mut Value, field_name: Option<&str>) {
        fn is_decimal_field(field: &str) -> bool {
            matches!(
                field,
                "devirKumulatifGvMatrahi"
                    | "devirKumulatifAsgariGvMatrahi"
                    | "gvCumulativeOpening"
                    | "asgariGvCumulativeOpening"
                    | "limit"
                    | "oran"
                    | "sigortaGvYillikBrutAsgariUcretTavani"
                    | "sabitSendikaAidati"
                    | "oksOraniYuzde"
                    | "sabitBesTutar"
                    | "icraTutar"
                    | "kisiBorcuTutar"
                    | "dogumAskerlikBorclanmasiTutar"
                    | "hayatSaglikSigortasiTutar"
                    | "digerKesintiTutar"
                    | "dogumAskerlikGvIndirimTutar"
                    | "hayatSigortasiPrimiTutar"
                    | "saglikSigortasiPrimiTutar"
                    | "tabanBrutAylik"
                    | "tediye"
                    | "tisIkramiyesi"
                    | "ekOdeme"
                    | "yemek"
                    | "birlestirilmisSosyalYardim"
                    | "vasitaYol"
                    | "giyimYardimi"
                    | "isPrimi"
                    | "geceCalismasiUcreti"
                    | "geceCalismasiTatiliUcreti"
                    | "hizmetZammi"
                    | "digerGelir"
                    | "sabitTutar"
                    | "value"
                    | "totalGrossDelta"
                    | "payableSettlementAmount"
                    | "offsetSettlementAmount"
                    | "recoveredAmount"
                    | "recoverableAmount"
                    | "outstandingReceivable"
                    | "originalRecognizedAmount"
                    | "previousAuthoritativeRetroAmount"
                    | "targetAmount"
                    | "deltaAmount"
                    | "originalPek"
                    | "retroPekDelta"
                    | "adjustedPek"
                    | "workerSgkDelta"
                    | "workerUnemploymentDelta"
                    | "employerSgkDelta"
                    | "employerUnemploymentDelta"
                    | "originalEmployerLowerBound"
                    | "targetEmployerLowerBound"
                    | "employerLowerBoundDelta"
                    | "employerLowerBoundPremiumDelta"
                    | "amount"
                    | "tutar"
                    | "hesaplananPek"
                    | "hamPek"
                    | "devredenPekKullanilan"
                    | "primMatrahi"
                    | "aylikOncekiPekTuketimi"
                    | "aylikSonrasiPekTuketimi"
                    | "finalPek"
                    | "devredenPekAşanTutar"
                    | "pekAltSinir"
                    | "pekUstSinir"
                    | "altSinirTamamlamaFarki"
                    | "yemekIstisnasiTutar"
                    | "isverenSgkPrimi"
                    | "isverenIssizlikPrimi"
                    | "pekAltSinirTamamlamaIsverenPrimi"
                    | "isverenPrimToplami"
                    | "sgkIsverenOraniYuzde"
                    | "isverenIssizlikOraniYuzde"
                    | "gelirToplam"
                    | "kesintiToplam"
                    | "netOdeme"
                    | "oncekiKumulatifGvMatrahi"
                    | "oncekiKumulatifAsgariGvMatrahi"
                    | "manuelKumulatifGvMatrahi"
                    | "persistedGvBase"
                    | "grossAmount"
                    | "isciSgkPrimi"
                    | "isciIssizlikPrimi"
                    | "gelirVergisi"
                    | "damgaVergisi"
                    | "sendikaAidati"
                    | "bes"
                    | "kisiBorcu"
                    | "dogumAskerlikBorclanmasi"
                    | "hayatSaglikSigortasi"
                    | "digerKesinti"
                    | "cariGvMatrahi"
                    | "yeniKumulatifGvMatrahi"
                    | "brutGelirVergisi"
                    | "asgariUcretGvMatrahi"
                    | "asgariUcretReferansKumulatifMatrahi"
                    | "asgariUcretGvIstisnasi"
                    | "ayniAyOncekiKullanilanGvIstisnasi"
                    | "tahakkukOncesiKalanGvIstisnasi"
                    | "uygulananGvIstisnasi"
                    | "tahakkukSonrasiKalanGvIstisnasi"
                    | "kesilenGelirVergisi"
                    | "dogumAskerlikGvIndirimi"
                    | "sigortaGvIndirimAdayi"
                    | "sigortaGvAylikLimiti"
                    | "sigortaGvYillikKalanLimiti"
                    | "uygulanabilirSigortaGvIndirimi"
                    | "brutDamgaVergisi"
                    | "aylikDamgaIstisnaHakki"
                    | "ayniAyOncekiKullanilanDamgaIstisnasi"
                    | "uygulananDamgaIstisnasi"
                    | "kalanDamgaIstisnasi"
                    | "gunlukAsgariUcret"
                    | "pekTavanKatsayisi"
                    | "gunlukYemekIstisnasiSGK"
                    | "gunlukYemekIstisnasiGV"
                    | "sgkYemekIstisnasiToplam"
                    | "gvYemekIstisnasiToplam"
                    | "gunlukIsPrimi"
            )
        }

        match value {
            Value::String(text) if field_name.is_some_and(is_decimal_field) => {
                if let Ok(decimal) = text.parse::<Decimal>() {
                    *text = decimal.normalize().to_string();
                }
            }
            Value::Array(items) => {
                for item in items {
                    normalize_restore_comparison(item, field_name);
                }
            }
            Value::Object(fields) => {
                for (name, field) in fields {
                    normalize_restore_comparison(field, Some(name));
                }
            }
            _ => {}
        }
    }
    normalize_restore_comparison(&mut expected, None);
    normalize_restore_comparison(&mut restored, None);
    let mut numeric_identifier = json!({"id": "00123", "amount": "1.00", "tcNo": "000123"});
    normalize_restore_comparison(&mut numeric_identifier, None);
    assert_eq!(numeric_identifier["amount"], "1");
    assert_eq!(numeric_identifier["id"], "00123");
    assert_eq!(numeric_identifier["tcNo"], "000123");
    let mut changed_numeric_identifier = json!({"id": "123", "amount": "1.0", "tcNo": "000123"});
    normalize_restore_comparison(&mut changed_numeric_identifier, None);
    assert_ne!(numeric_identifier["id"], changed_numeric_identifier["id"]);
    assert_eq!(
        numeric_identifier["amount"],
        changed_numeric_identifier["amount"]
    );
    for key in [
        "donemler",
        "aktifDonemId",
        "personeller",
        "kurumDegerleriMap",
        "puantajlar",
        "bordrolar",
        "taxOpenings",
        "sickLeaveRecords",
        "annualPayrollParameters",
        "compensationRevisions",
        "compensationRevisionOverrides",
        "retroBatches",
        "retroAllocations",
    ] {
        assert_eq!(restored[key], expected[key], "restored field {key}");
    }

    let entitlement = dataset
        .payrolls
        .iter()
        .find(|payroll| payroll.id == "p-1_2026-06")
        .expect("reference entitlement payroll must remain present");
    assert!(entitlement.isPrimiDetay.is_some());
    let batch = dataset
        .retroBatches
        .iter()
        .find(|batch| batch.id == "retro-d8374d9a-fa2d-4c24-a9c5-edc0a1800c93")
        .expect("reference recovery batch must remain present");
    let stale_payment = dataset
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch.id)
        .expect("reference stale payment event must remain present");
    assert_eq!(batch.status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(stale_payment.status, BordroStatus::STALE);
    assert_eq!(stale_payment.gelirToplam, batch.payableSettlementAmount);

    let mut forged: Value = serde_json::from_str(payload)?;
    let forged_entitlement = forged["bordrolar"]
        .as_array_mut()
        .and_then(|payrolls| {
            payrolls
                .iter_mut()
                .find(|payroll| payroll["id"] == "p-1_2026-06")
        })
        .expect("reference entitlement payroll must exist");
    let daily_premium = forged_entitlement["isPrimiDetay"]["gunlukIsPrimi"]
        .as_str()
        .expect("daily premium decimal string")
        .parse::<Decimal>()?;
    forged_entitlement["isPrimiDetay"]["gunlukIsPrimi"] =
        json!((daily_premium + dec!(1)).to_string());
    let mut fresh_conn = create_in_memory_connection()?;
    let error = MigrationService::replace_backup_data(&mut fresh_conn, &forged.to_string())
        .expect_err("a forged work-premium amount must not be accepted");
    assert!(error.to_string().contains("iş primi snapshot"));
    assert!(PayrollRepository::get_all(&fresh_conn)?.is_empty());
    Ok(())
}

#[test]
fn native_calculated_retro_with_stale_link_restores_exactly_to_empty_database(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_, baseline) = current_v5_fixture()?;
    let mut expected: Value = serde_json::from_str(&baseline)?;
    let normal = expected["bordrolar"][0].clone();
    let gross = normal["gelirToplam"].clone();
    let payment_date = normal["paymentDate"].clone();
    expected["bordrolar"][0]["status"] = json!("STALE");

    let revision_id = "recovery-revision";
    let batch_id = "recovery-batch";
    expected["compensationRevisions"] = json!([{
        "id": revision_id,
        "reason": "COLLECTIVE_AGREEMENT",
        "title": "Recovery fixture",
        "effectiveFrom": "2026-01-15",
        "status": "CALCULATED",
        "scope": "SELECTED_PERSONNEL",
        "personnelIds": ["v5-person"],
        "effectiveTo": null,
        "decisionDate": null,
        "signedAt": null,
        "description": null,
        "personnelGroup": null,
        "createdAt": null,
        "updatedAt": null
    }]);
    expected["retroBatches"] = json!([{
        "id": batch_id,
        "revisionId": revision_id,
        "personnelId": "v5-person",
        "paymentDate": payment_date,
        "status": "CALCULATED",
        "settlementStatus": "UNSETTLED",
        "totalGrossDelta": gross,
        "payableSettlementAmount": gross,
        "offsetSettlementAmount": "0",
        "recoveredAmount": "0",
        "recoverableAmount": "0",
        "outstandingReceivable": "0",
        "description": null,
        "createdAt": null,
        "calculatedAt": null,
        "finalizedAt": null
    }]);
    expected["retroAllocations"] = json!([{
        "id": "recovery-allocation",
        "batchId": batch_id,
        "personnelId": "v5-person",
        "sourcePeriodId": "2026-01",
        "earningCode": "BASE_WAGE",
        "originalRecognizedAmount": "0",
        "previousAuthoritativeRetroAmount": "0",
        "targetAmount": gross,
        "deltaAmount": gross,
        "originalPek": "0",
        "retroPekDelta": "0",
        "adjustedPek": "0",
        "workerSgkDelta": "0",
        "workerUnemploymentDelta": "0",
        "employerSgkDelta": "0",
        "employerUnemploymentDelta": "0",
        "originalEmployerLowerBound": "0",
        "targetEmployerLowerBound": "0",
        "employerLowerBoundDelta": "0",
        "employerLowerBoundPremiumDelta": "0",
        "payableSettlementAmount": gross,
        "offsetSettlementAmount": "0",
        "recoverableAmount": "0",
        "originalSourceCarry": null,
        "targetSourceCarry": null,
        "metadata": null,
        "sgkTreatment": "WAGE_SOURCE_MONTH",
        "incomeTaxTreatment": "TAXABLE",
        "stampTaxTreatment": "TAXABLE"
    }]);

    let mut stale_payment = normal;
    stale_payment["id"] = json!(batch_id);
    stale_payment["accrualId"] = json!(batch_id);
    stale_payment["accrualType"] = json!("RETRO_ADJUSTMENT");
    stale_payment["sequence"] = json!(1);
    stale_payment["status"] = json!("STALE");
    expected["bordrolar"]
        .as_array_mut()
        .unwrap()
        .push(stale_payment);

    let mut conn = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut conn, &expected.to_string())?;

    let dataset = PayrollService::build_dataset_snapshot(&conn)?;
    let restored = json!({
        "donemler": dataset.periods,
        "aktifDonemId": SettingsRepository::get_app_setting(&conn, "active_period_id")?,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": SettingsRepository::get_all_institution_settings(&conn)?,
        "puantajlar": dataset.attendances,
        "bordrolar": dataset.payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "zamAylari": legacy_raise_months(&conn)?,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations,
    });
    for key in [
        "donemler",
        "aktifDonemId",
        "personeller",
        "kurumDegerleriMap",
        "puantajlar",
        "bordrolar",
        "taxOpenings",
        "sickLeaveRecords",
        "annualPayrollParameters",
        "zamAylari",
        "compensationRevisions",
        "compensationRevisionOverrides",
        "retroBatches",
        "retroAllocations",
    ] {
        assert_eq!(restored[key], expected[key], "restored field {key}");
    }
    let batch = dataset
        .retroBatches
        .first()
        .expect("recovery batch persists");
    let payment = dataset
        .payrolls
        .iter()
        .find(|payroll| payroll.accrualId == batch_id)
        .expect("stale payment event persists");
    assert_eq!(batch.status, CompensationRevisionStatus::CALCULATED);
    assert_eq!(payment.status, BordroStatus::STALE);
    assert_eq!(payment.gelirToplam, batch.payableSettlementAmount);

    let good_payrolls = serde_json::to_value(PayrollRepository::get_all(&conn)?)?;
    let good_batches = get_batches(&conn)?;
    let good_allocations = get_allocations(&conn)?;
    for (field, value) in [
        ("personelId", json!("forged-person")),
        ("paymentDate", json!("2026-02-13")),
        ("status", json!("FINALIZED")),
    ] {
        let mut forged = expected.clone();
        let event = forged["bordrolar"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|event| event["accrualId"] == batch_id)
            .unwrap();
        event[field] = value;
        assert!(MigrationService::replace_backup_data(&mut conn, &forged.to_string()).is_err());
        assert_eq!(
            serde_json::to_value(PayrollRepository::get_all(&conn)?)?,
            good_payrolls
        );
        assert_eq!(get_batches(&conn)?, good_batches);
        assert_eq!(get_allocations(&conn)?, good_allocations);
    }

    let mut forged_gross = expected;
    let event = forged_gross["bordrolar"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|event| event["accrualId"] == batch_id)
        .unwrap();
    event["gelirler"]["digerGelir"] = json!("1");
    event["gelirToplam"] = json!("31001");
    event["netOdeme"] = json!("26351");
    assert!(MigrationService::replace_backup_data(&mut conn, &forged_gross.to_string()).is_err());
    assert_eq!(
        serde_json::to_value(PayrollRepository::get_all(&conn)?)?,
        good_payrolls
    );
    assert_eq!(get_batches(&conn)?, good_batches);
    assert_eq!(get_allocations(&conn)?, good_allocations);
    Ok(())
}

#[test]
fn native_legacy_sparse_backup_preserves_persisted_gv_base_without_rich_detail(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, payload) = current_v5_fixture()?;
    let mut legacy: Value = serde_json::from_str(&payload)?;
    legacy["backupVersion"] = json!(4);
    let payroll = legacy["bordrolar"]
        .as_array_mut()
        .and_then(|items| items.first_mut())
        .expect("legacy payroll");
    let persisted_base = payroll["persistedGvBase"].clone();
    payroll["gvDetay"] = Value::Null;

    let mut target = create_in_memory_connection()?;
    MigrationService::replace_backup_data(&mut target, &legacy.to_string())?;

    let restored = PayrollRepository::get_all(&target)?;
    assert_eq!(restored.len(), 1);
    assert!(restored[0].gvDetay.is_none());
    assert_eq!(
        restored[0].persistedGvBase,
        Some(
            persisted_base
                .as_str()
                .expect("persisted GV base")
                .parse()?
        )
    );
    assert_eq!(restored[0].status, BordroStatus::CALCULATED);
    assert_eq!(
        PayrollRepository::sum_gv_base_for_tax_month_range(&target, "v5-person", 2026, 1, 3)?,
        restored[0].persistedGvBase.expect("persisted GV base")
    );
    Ok(())
}

#[test]
fn native_current_v5_backup_rejects_semantically_forged_snapshot(
) -> Result<(), Box<dyn std::error::Error>> {
    let (_source, payload) = current_v5_fixture()?;
    let mut conn = create_in_memory_connection()?;
    let prior_dataset = PayrollService::build_dataset_snapshot(&conn)?;
    let mut forged: Value = serde_json::from_str(&payload)?;
    let payroll = forged["bordrolar"]
        .as_array_mut()
        .and_then(|items| items.first_mut())
        .expect("current payroll");
    let current_base = payroll["gvDetay"]["cariGvMatrahi"]
        .as_str()
        .expect("GV base decimal string")
        .parse::<Decimal>()?;
    let forged_base = current_base + dec!(1);
    payroll["persistedGvBase"] = json!(forged_base.to_string());
    payroll["gvDetay"]["cariGvMatrahi"] = json!(forged_base.to_string());
    let previous = payroll["gvDetay"]["oncekiKumulatifGvMatrahi"]
        .as_str()
        .expect("previous GV decimal string")
        .parse::<Decimal>()?;
    payroll["gvDetay"]["yeniKumulatifGvMatrahi"] = json!((previous + forged_base).to_string());

    let error = MigrationService::replace_backup_data(&mut conn, &forged.to_string())
        .expect_err("semantically forged V5 snapshot must be rejected");
    assert!(error.to_string().contains("V5 backup replay"));
    let after_failed_restore = PayrollService::build_dataset_snapshot(&conn)?;
    assert_eq!(
        serde_json::to_value(after_failed_restore)?,
        serde_json::to_value(prior_dataset)?,
        "a failed restore must preserve the complete previous dataset"
    );
    Ok(())
}

#[test]
fn native_backup_rejects_duplicate_annual_payroll_parameter_years(
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut conn, payload) = current_v5_fixture()?;
    let mut modified: Value = serde_json::from_str(&payload)?;
    let duplicate_param = modified["annualPayrollParameters"][0].clone();
    modified["annualPayrollParameters"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_param);

    let error = MigrationService::replace_backup_data(&mut conn, &modified.to_string())
        .expect_err("duplicate annual parameter years must be rejected");
    assert!(
        error.to_string().contains("mükerrer") || error.to_string().contains("yinelenen"),
        "error must mention duplicate year: {error}"
    );
    Ok(())
}

#[test]
fn native_kurus_boundary_rejects_more_than_two_decimals() -> Result<(), Box<dyn std::error::Error>>
{
    let conn = create_in_memory_connection()?;
    let mut personnel = Personel {
        id: "person-precision".into(),
        tcNo: "11111111110".into(),
        ad: "Hassas".into(),
        soyad: "Test".into(),
        grup: "1. Grup".into(),
        unvan: Some("İşçi".into()),
        sgkSicilNo: "".into(),
        iban: "".into(),
        hizmetYili: 1,
        aciklama: None,
        devirKumulatifGvMatrahi: None,
        devirKumulatifGvMatrahiYili: None,
        devirKumulatifGvMatrahiBaslangicAyi: None,
        devirKumulatifAsgariGvMatrahi: None,
        devirKumulatifAsgariGvMatrahiYili: None,
        kesintiler: Some(PersonelKesintileri {
            sabitBesTutar: Some(dec!(10.005)), // >2 decimal places
            ..PersonelKesintileri::default()
        }),
    };

    let error = PersonnelRepository::save(&conn, &personnel)
        .expect_err("values with >2 decimal digits must be rejected at kuruş persistence boundary");
    assert!(
        error.to_string().contains("kuruş hassasiyeti aşıldı")
            || error.to_string().contains("2'den fazla ondalık"),
        "error must mention kuruş precision: {error}"
    );

    // scale <= 2 must succeed
    personnel.kesintiler.as_mut().unwrap().sabitBesTutar = Some(dec!(10.50));
    PersonnelRepository::save(&conn, &personnel).expect("values with scale <= 2 must succeed");
    Ok(())
}

#[test]
fn native_retro_ordinary_line_item_rejects_negative_income_and_deductions() {
    let mut payroll = BordroKaydi {
        id: "p-retro".into(),
        personelId: "p1".into(),
        donemId: "2026-06".into(),
        accrualId: "b1".into(),
        accrualType: AccrualType::RETRO_ADJUSTMENT,
        paymentDate: "2026-06-15".into(),
        sequence: 1,
        accrualDescription: None,
        puantajOzeti: PuantajOzeti::default(),
        gelirler: GelirKalemleri::default(),
        gelirToplam: Decimal::ZERO,
        kesintiler: KesintiKalemleri::default(),
        kesintiToplam: Decimal::ZERO,
        netOdeme: Decimal::ZERO,
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: "2026-06-15T00:00:00Z".into(),
        sonGuncellemeTarihi: "2026-06-15T00:00:00Z".into(),
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
        calculationMetadata: None,
    };

    payroll.gelirler.digerGelir = Some(dec!(-50));
    let err = payroll_core::validate_ordinary_payroll_snapshot(&payroll)
        .expect_err("negative digerGelir in retro must be rejected");
    assert!(err
        .to_string()
        .contains("Ordinary gelir kalemi negatif olamaz"));

    payroll.gelirler.digerGelir = None;
    payroll.gelirler.tabanBrutAylik = Some(dec!(100));
    payroll.gelirToplam = dec!(100);
    payroll.kesintiler.isciSgkPrimi = Some(dec!(-14)); // Allowed signed delta in retro
    payroll.kesintiToplam = dec!(-14);
    payroll.netOdeme = dec!(114);
    payroll_core::validate_ordinary_payroll_snapshot(&payroll)
        .expect("signed worker SGK in retro must be allowed");

    payroll.kesintiler.bes = Some(dec!(-10)); // bes is ordinary, must NOT be negative
    let err = payroll_core::validate_ordinary_payroll_snapshot(&payroll)
        .expect_err("negative bes in retro must be rejected");
    assert!(err
        .to_string()
        .contains("Ordinary kesinti kalemi negatif olamaz"));
}

#[test]
fn calculation_metadata_migration_preserves_legacy_payroll_rows() {
    let mut conn = Connection::open_in_memory().expect("SQLite açılmalı");
    let previous_version = CALCULATION_METADATA_MIGRATION_VERSION - 1;
    get_migrations()
        .to_version(&mut conn, previous_version as usize)
        .expect("önceki şema kurulmalı");
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         INSERT INTO personnel (id, tc_no, ad, soyad, grup, created_at, updated_at)
         VALUES ('migration-person', '10000000098', 'Migration', 'Person', '1. Grup', 't', 't');
         INSERT INTO payroll_periods
             (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, tax_year, tax_month, created_at)
         VALUES ('2026-01', 2026, 1, '2026-01-15', '2026-02-14', 'Ocak', 2026, 2, 't');
         INSERT INTO payroll_records
             (id, personnel_id, period_id, accrual_id, payment_date, gross_total, sgk_base,
              gv_base, previous_cumulative_gv, new_cumulative_gv, income_tax, stamp_tax,
              total_deductions, net_payment, puantaj_summary_json, calculated_at, updated_at)
         VALUES ('migration-payroll', 'migration-person', '2026-01', 'migration-accrual',
                 '2026-02-14', 12345, 10000, 8500, 0, 8500, 1000, 75, 1500, 10845,
                 '{}', 't', 't');",
    )
    .expect("önceki şemaya temsilî bordro yazılmalı");

    get_migrations()
        .to_latest(&mut conn)
        .expect("calculation metadata migration uygulanmalı");

    let latest_version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version okunmalı");
    let (gross_total, metadata): (i64, Option<String>) = conn
        .query_row(
            "SELECT gross_total, calculation_metadata_json
             FROM payroll_records WHERE id = 'migration-payroll'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("eski payroll satırı korunmalı");

    assert!(
        latest_version >= COMPENSATION_POLICY_FLAGS_MIGRATION_VERSION,
        "all migrations through the compensation-policy flags must apply"
    );
    assert_eq!(gross_total, 12345);
    assert_eq!(
        metadata, None,
        "legacy rows must remain explicitly metadata-free"
    );
}

#[test]
fn v6_restore_keeps_v5_legacy_replay_and_metadata_validation_atomic() {
    let (_, v5_json) = current_v5_fixture().expect("V5 replay fixture kurulmalı");
    let mut legacy_v6: Value = serde_json::from_str(&v5_json).expect("V5 payload parse edilmeli");
    legacy_v6["backupVersion"] = json!(6);
    legacy_v6["compensationPolicyVersions"] = json!([]);

    let mut replay_conn = create_in_memory_connection().expect("hedef DB kurulmalı");
    MigrationService::replace_backup_data(&mut replay_conn, &legacy_v6.to_string())
        .expect("V6 içindeki legacy NORMAL bordro V5 replay ile doğrulanmalı");
    let replayed = PayrollRepository::get_all(&replay_conn)
        .expect("legacy payroll okunmalı")
        .remove(0);
    assert_eq!(replayed.status, BordroStatus::CALCULATED);
    assert!(replayed.calculationMetadata.is_none());
    assert!(CompensationPolicyRepository::get_all_versions(&replay_conn)
        .expect("policy timeline okunmalı")
        .is_empty());

    let mut draft_v6 = legacy_v6;
    draft_v6["bordrolar"][0]["status"] = json!("DRAFT");
    draft_v6["bordrolar"][0]["calculationMetadata"] = json!({
        "provenance": "COMPENSATION_POLICY_ENGINE"
    });
    let mut metadata_conn = create_in_memory_connection().expect("hedef DB kurulmalı");
    MigrationService::replace_backup_data(&mut metadata_conn, &draft_v6.to_string())
        .expect("DRAFT metadata snapshot olmadan saklanabilir");
    let restored = PayrollRepository::get_all(&metadata_conn)
        .expect("metadata payroll okunmalı")
        .remove(0);
    let metadata = restored
        .calculationMetadata
        .expect("provenance envelope SQLite'dan geri okunmalı");
    assert_eq!(
        metadata.provenance,
        PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE
    );
    assert!(metadata.snapshot.is_none());

    let mut invalid = draft_v6;
    invalid["bordrolar"][0]["status"] = json!("CALCULATED");
    let error = MigrationService::replace_backup_data(&mut metadata_conn, &invalid.to_string())
        .expect_err("current CALCULATED NORMAL snapshot metadata olmadan reddedilmeli");
    assert!(error.to_string().contains("snapshot"));
    let preserved = PayrollRepository::get_all(&metadata_conn)
        .expect("önceki metadata payroll korunmalı")
        .remove(0);
    assert_eq!(preserved.status, BordroStatus::DRAFT);
    assert!(preserved.calculationMetadata.is_some());
}

fn precise_compensation_policy_version() -> CompensationPolicyVersion {
    CompensationPolicyVersion {
        id: "precise-policy-v1".into(),
        effectiveFrom: "2020-01-01".into(),
        state: CompensationPolicyState {
            gunlukTabanUcret: dec!(1000.123456789),
            gunlukYemek: dec!(0.123456789),
            birlestirilmisSosyalYardim: dec!(0),
            gunlukVasitaYol: dec!(0),
            giyimYardimi: dec!(0),
            hizmetZammiBirimi: dec!(0),
            isPrimiYuzde: Some(dec!(0)),
            isPrimiGruplari: vec![CompensationPolicyGroup {
                id: "1. Grup".into(),
                ad: "1. Grup".into(),
                oran: dec!(0),
                aktif: true,
            }],
            geceCalismaPrimiYuzde: dec!(0),
            geceCalismaTatiliPrimiYuzde: dec!(0),
            ekOdeme: Some(dec!(0)),
            digerGelirVarsayilan: Some(dec!(0)),
        },
        boundaryScope: CompensationBoundaryScope::FullEarning,
        roadRoundingMode: RoadRoundingMode::PerCompensationSlice,
        createdAt: "2026-10-06T00:00:00Z".into(),
        source: "ADMINISTRATIVE_DECISION".into(),
        reason: "V6 persistence exact decimal fixture".into(),
        supersedesVersionId: None,
    }
}

#[test]
fn v6_policy_timeline_and_draft_metadata_round_trip_without_precision_loss() {
    let (_, v5_json) = current_v5_fixture().expect("payroll fixture kurulmalı");
    let policy_version = precise_compensation_policy_version();
    let mut payload: Value = serde_json::from_str(&v5_json).expect("base payload parse edilmeli");
    payload["backupVersion"] = json!(6);
    payload["compensationPolicyVersions"] =
        serde_json::to_value([policy_version]).expect("policy timeline encode edilmeli");
    payload["bordrolar"][0]["status"] = json!("DRAFT");
    payload["bordrolar"][0]["calculationMetadata"] = json!({
        "provenance": "COMPENSATION_POLICY_ENGINE"
    });

    let mut target = create_in_memory_connection().expect("hedef DB kurulmalı");
    MigrationService::replace_backup_data(&mut target, &payload.to_string())
        .expect("V6 policy ve DRAFT metadata restore edilmeli");
    let restored = PayrollRepository::get_all(&target)
        .expect("persisted payroll okunmalı")
        .remove(0);
    let metadata = restored
        .calculationMetadata
        .expect("calculation metadata restore edilmeli");
    assert_eq!(
        metadata.provenance,
        PayrollCalculationProvenance::COMPENSATION_POLICY_ENGINE
    );
    assert!(metadata.snapshot.is_none());
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&target)
            .expect("V6 policy history okunmalı")[0]
            .state
            .gunlukTabanUcret
            .to_string(),
        "1000.123456789"
    );
    let stored_json: String = target
        .query_row(
            "SELECT calculation_metadata_json FROM payroll_records WHERE id = ?1",
            [&restored.id],
            |row| row.get(0),
        )
        .expect("metadata JSON envelope SQL'den okunmalı");
    assert!(stored_json.contains("\"provenance\":\"COMPENSATION_POLICY_ENGINE\""));

    let mut missing_snapshot = payload.clone();
    missing_snapshot["bordrolar"][0]["status"] = json!("CALCULATED");
    let error = MigrationService::replace_backup_data(&mut target, &missing_snapshot.to_string())
        .expect_err("current CALCULATED NORMAL snapshot olmadan V6 reddedilmeli");
    assert!(error.to_string().contains("snapshot"));
    assert_eq!(
        PayrollRepository::get_all(&target)
            .expect("payroll failure sonrasında korunmalı")
            .len(),
        1
    );

    let mut conflicting_policy = payload.clone();
    conflicting_policy["compensationPolicyVersions"][0]["state"]["gunlukTabanUcret"] =
        json!("1000.923456789");
    let error = MigrationService::replace_backup_data(&mut target, &conflicting_policy.to_string())
        .expect_err("aynı immutable policy ID farklı full-state ile değiştirilemez");
    assert!(error.to_string().contains("Immutable policy restore"));
    assert_eq!(
        CompensationPolicyRepository::get_all_versions(&target).expect("existing policy korunmalı")
            [0]
        .state
        .gunlukTabanUcret
        .to_string(),
        "1000.123456789"
    );
    assert_eq!(
        PayrollRepository::get_all(&target)
            .expect("payroll conflict sonrasında korunmalı")
            .len(),
        1
    );

    let mut conflicting_boundary = payload.clone();
    conflicting_boundary["compensationPolicyVersions"][0]["boundaryScope"] = json!("ROAD_ONLY");
    let error =
        MigrationService::replace_backup_data(&mut target, &conflicting_boundary.to_string())
            .expect_err("aynı immutable policy ID farklı boundary flag ile değiştirilemez");
    assert!(error.to_string().contains("Immutable policy restore"));
}

#[test]
fn v5_restore_keeps_head_compatibility_when_export_timestamp_and_active_period_are_absent() {
    let (_, v5_json) = current_v5_fixture().expect("V5 replay fixture kurulmalı");
    let mut payload: Value = serde_json::from_str(&v5_json).expect("V5 payload parse edilmeli");
    payload.as_object_mut().unwrap().remove("exportedAt");
    payload.as_object_mut().unwrap().remove("aktifDonemId");

    let mut target = create_in_memory_connection().expect("izole hedef SQLite kurulmalı");
    MigrationService::replace_backup_data(&mut target, &payload.to_string())
        .expect("HEAD V5 semantiği bu iki ortak alanı zorunlu tutmuyordu");
    assert_eq!(
        PayrollRepository::get_all(&target)
            .expect("V5 bordrosu okunmalı")
            .len(),
        1
    );
    assert!(get_revisions(&target)
        .expect("V5 revision grafiği okunmalı")
        .is_empty());
}

#[test]
fn pre_v6_and_unversioned_native_backups_reject_compensation_fields_without_silent_drop() {
    let (_, v5_json) = current_v5_fixture().expect("tam V5 payload kurulmalı");
    let versions = [
        None,
        Some(json!(null)),
        Some(json!(1)),
        Some(json!(2)),
        Some(json!(3)),
        Some(json!(4)),
        Some(json!(5)),
    ];

    for version in versions {
        for (field, value) in [
            (
                "compensationPolicyVersions",
                serde_json::to_value([precise_compensation_policy_version()])
                    .expect("policy timeline serialize edilmeli"),
            ),
            (
                "calculationMetadata",
                json!({ "provenance": "COMPENSATION_POLICY_ENGINE" }),
            ),
        ] {
            let mut payload: Value =
                serde_json::from_str(&v5_json).expect("V5 payload parse edilmeli");
            if let Some(version) = &version {
                payload["backupVersion"] = version.clone();
            } else {
                payload.as_object_mut().unwrap().remove("backupVersion");
            }
            match field {
                "compensationPolicyVersions" => payload[field] = value,
                "calculationMetadata" => payload["bordrolar"][0][field] = value,
                _ => unreachable!("fixture yalnız test alanlarını kullanır"),
            }

            let mut target = create_in_memory_connection().expect("izole hedef SQLite kurulmalı");
            let error = MigrationService::replace_backup_data(&mut target, &payload.to_string())
                .expect_err("V6 alanları eski veya sürümsüz backup içinde sessizce atılamaz");
            assert!(
                error.to_string().contains("V1-V5") || error.to_string().contains("sürümsüz"),
                "unexpected {field} rejection: {error}"
            );
            assert!(PayrollRepository::get_all(&target)
                .expect("rejected restore DB'yi değiştirmemeli")
                .is_empty());
        }
    }
}

#[test]
fn v5_empty_or_null_compensation_timeline_has_no_financial_history_to_import() {
    let (_, v5_json) = current_v5_fixture().expect("tam V5 payload kurulmalı");
    for value in [json!([]), json!(null)] {
        let mut payload: Value = serde_json::from_str(&v5_json).expect("V5 payload parse edilmeli");
        payload["compensationPolicyVersions"] = value;
        let mut target = create_in_memory_connection().expect("izole hedef SQLite kurulmalı");
        MigrationService::replace_backup_data(&mut target, &payload.to_string())
            .expect("boş/null timeline veri taşımadığı için V5 uyumludur");
        assert!(CompensationPolicyRepository::get_all_versions(&target)
            .expect("timeline okunmalı")
            .is_empty());
    }
}

#[test]
fn native_v6_current_snapshot_round_trips_calculated_and_finalized_records_exactly() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../crates/payroll-core/tests/fixtures/v6-current-engine-snapshot.json"
    ))
    .expect("shared core/WASM/browser snapshot fixture parse edilmeli");
    let fixture_metadata = fixture["bordrolar"][0]["calculationMetadata"].clone();
    let expected_used_inputs = fixture_metadata["snapshot"]["usedInputs"].clone();
    let policy_version: CompensationPolicyVersion =
        serde_json::from_value(fixture["compensationPolicyVersions"][0].clone())
            .expect("shared policy version typed olmalı");

    for status in [BordroStatus::CALCULATED, BordroStatus::FINALIZED] {
        let (source, _) = current_v5_fixture().expect("native kaynak veritabanı kurulmalı");
        SettingsRepository::set_app_setting(&source, "active_period_id", "2026-01")
            .expect("aktif dönem ayarı kurulmalı");
        PayrollRepository::delete_accrual(&source, "v5-person", "2026-01", "v5-person_2026-01")
            .expect("V5 başlangıç bordrosu current snapshot fixture'ına yer açmak için silinmeli");
        CompensationPolicyRepository::insert_version(&source, &policy_version)
            .expect("immutable policy source SQLite'ya yazılmalı");
        let source_policy_versions = CompensationPolicyRepository::get_all_versions(&source)
            .expect("V6 kaynak policy timeline okunmalı");
        let mut payroll: BordroKaydi = serde_json::from_value(fixture["bordrolar"][0].clone())
            .expect("shared full current-engine payroll typed olmalı");
        payroll.personelId = "v5-person".into();
        payroll.id = "v5-person_2026-01".into();
        payroll.accrualId = payroll.id.clone();
        payroll.status = status;
        payroll.calculationMetadata = Some(
            serde_json::from_value(fixture_metadata.clone())
                .expect("canonical full snapshot metadata deserialize edilmeli"),
        );
        PayrollRepository::save_for_restore_in_transaction(&source, &payroll)
            .expect("tam current snapshot SQLite repository'ye yazılmalı");

        let saved = PayrollRepository::get_all(&source)
            .expect("native repository metadata'yı geri okumalı")
            .remove(0);
        if status == BordroStatus::CALCULATED {
            let mut mismatched_boundary = saved.clone();
            mismatched_boundary
                .calculationMetadata
                .as_mut()
                .unwrap()
                .snapshot
                .as_mut()
                .unwrap()
                .compensation
                .segments[0]
                .boundaryScope = CompensationBoundaryScope::RoadOnly;
            assert!(PayrollRepository::save_for_restore_in_transaction(
                &source,
                &mismatched_boundary
            )
            .expect_err("native payroll storage link must include immutable boundary flags")
            .to_string()
            .contains("boundary"));
        }
        let saved_value = serde_json::to_value(&saved).expect("native payroll serialize edilmeli");
        assert_eq!(
            saved_value["calculationMetadata"]["snapshot"]["usedInputs"],
            expected_used_inputs
        );
        assert_eq!(
            saved_value["calculationMetadata"]["provenance"],
            "COMPENSATION_POLICY_ENGINE"
        );
        assert_eq!(
            saved_value["calculationMetadata"]["snapshot"]["usedInputs"]["manualIncome"]["tediye"],
            "0"
        );
        assert!(
            saved_value["calculationMetadata"]["snapshot"]["usedInputs"]["manualIncome"]
                ["tisIkramiyesi"]
                .is_null()
        );
        assert_eq!(
            saved_value["calculationMetadata"]["snapshot"]["usedInputs"]["taxHistory"]
                ["previousPaymentEvents"][0]["gvTaxableBase"],
            "0.004"
        );

        let mut exported: Value = serde_json::from_str(
            &current_v6_payload_from_database(&source).expect("native dataset export üretilmeli"),
        )
        .expect("native export JSON parse edilmeli");
        exported["backupVersion"] = json!(6);
        exported["compensationPolicyVersions"] = serde_json::to_value(
            CompensationPolicyRepository::get_all_versions(&source)
                .expect("immutable policy timeline export edilmeli"),
        )
        .expect("policy timeline JSON'a çevrilmeli");
        assert_eq!(
            exported["bordrolar"][0]["calculationMetadata"]["snapshot"]["usedInputs"],
            expected_used_inputs
        );

        let mut target = create_in_memory_connection().expect("boş izole restore DB kurulmalı");
        MigrationService::replace_backup_data(&mut target, &exported.to_string())
            .expect("V6 full snapshot boş SQLite'ya restore edilmeli");
        let restored = PayrollRepository::get_all(&target)
            .expect("restore edilen bordro okunmalı")
            .remove(0);
        let restored_value =
            serde_json::to_value(&restored).expect("restore edilen bordro serialize edilmeli");
        assert_eq!(restored.status, status);
        assert_eq!(
            restored_value["calculationMetadata"]["snapshot"]["usedInputs"],
            expected_used_inputs
        );
        assert_eq!(
            CompensationPolicyRepository::get_all_versions(&target)
                .expect("policy timeline restore edilmeli"),
            source_policy_versions
        );

        let baseline_payroll = restored_value.clone();
        let baseline_policy =
            serde_json::to_value(CompensationPolicyRepository::get_all_versions(&target).unwrap())
                .unwrap();
        for mutation in [
            "missing-input",
            "period-date",
            "snapshot-version",
            "policy-link",
            "policy-boundary-flag",
            "financial-net",
        ] {
            let mut corrupted = exported.clone();
            let snapshot = &mut corrupted["bordrolar"][0]["calculationMetadata"]["snapshot"];
            match mutation {
                "missing-input" => snapshot["usedInputs"]["attendance"] = Value::Null,
                "period-date" => {
                    snapshot["usedInputs"]["attendance"]["days"][0]["date"] = json!("2026-01-14")
                }
                "snapshot-version" => snapshot["snapshotSchemaVersion"] = json!(99),
                "policy-link" => {
                    snapshot["compensation"]["segments"][0]["versionId"] = json!("missing-policy")
                }
                "policy-boundary-flag" => {
                    snapshot["compensation"]["segments"][0]["boundaryScope"] = json!("ROAD_ONLY")
                }
                "financial-net" => {}
                _ => unreachable!("test mutation sabit listeden gelir"),
            }
            if mutation == "financial-net" {
                let net: Decimal = corrupted["bordrolar"][0]["netOdeme"]
                    .as_str()
                    .expect("canonical decimal net string")
                    .parse()
                    .expect("canonical net decimal parses");
                corrupted["bordrolar"][0]["netOdeme"] = json!((net + Decimal::ONE).to_string());
            }
            let error = MigrationService::replace_backup_data(&mut target, &corrupted.to_string())
                .expect_err("tam snapshot mutation restore'u atomik reddetmeli");
            assert!(!error.to_string().is_empty());
            assert_eq!(
                serde_json::to_value(PayrollRepository::get_all(&target).unwrap().remove(0))
                    .unwrap(),
                baseline_payroll,
                "{mutation} failure payroll row'unu değiştirmemeli"
            );
            assert_eq!(
                serde_json::to_value(
                    CompensationPolicyRepository::get_all_versions(&target).unwrap()
                )
                .unwrap(),
                baseline_policy,
                "{mutation} failure policy history'yi değiştirmemeli"
            );
        }

        if status == BordroStatus::FINALIZED {
            let mut attempted_mutation = restored;
            attempted_mutation
                .calculationMetadata
                .as_mut()
                .unwrap()
                .snapshot
                .as_mut()
                .unwrap()
                .usedInputs
                .manualIncome
                .tediye = Some(dec!(0.01));
            assert!(PayrollRepository::save_for_restore_in_transaction(
                &target,
                &attempted_mutation
            )
            .is_err());
            assert_eq!(
                serde_json::to_value(PayrollRepository::get_all(&target).unwrap().remove(0))
                    .unwrap(),
                baseline_payroll,
                "FINALIZED record metadata mutation must remain immutable"
            );
        }

        if status == BordroStatus::CALCULATED {
            let mut conflicting_policy = exported.clone();
            let policy_index = conflicting_policy["compensationPolicyVersions"]
                .as_array()
                .unwrap()
                .iter()
                .position(|version| version["id"] == "policy-v1")
                .expect("fixture policy in timeline");
            conflicting_policy["compensationPolicyVersions"][policy_index]["state"]
                ["gunlukTabanUcret"] = json!("2443.281234568");
            conflicting_policy["bordrolar"][0]["calculationMetadata"]["snapshot"]["compensation"]
                ["segments"][0]["state"]["gunlukTabanUcret"] = json!("2443.281234568");
            let error =
                MigrationService::replace_backup_data(&mut target, &conflicting_policy.to_string())
                    .expect_err(
                        "aynı immutable policy ID farklı snapshot state ile restore edilemez",
                    );
            assert!(
                error.to_string().contains("Immutable policy restore"),
                "unexpected policy conflict error: {error}"
            );
            assert_eq!(
                serde_json::to_value(PayrollRepository::get_all(&target).unwrap().remove(0))
                    .unwrap(),
                baseline_payroll,
                "policy conflict sonrası payroll korunmalı"
            );
            assert_eq!(
                serde_json::to_value(
                    CompensationPolicyRepository::get_all_versions(&target).unwrap()
                )
                .unwrap(),
                baseline_policy,
                "policy conflict sonrası immutable history korunmalı"
            );
        }
    }
}
