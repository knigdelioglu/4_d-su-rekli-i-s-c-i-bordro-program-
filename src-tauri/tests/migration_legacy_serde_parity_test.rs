use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::services::migration_service::{LegacyPayload, MigrationService};
use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
    repositories::attendance_repo::AttendanceRepository,
    repositories::payroll_repo::PayrollRepository,
    repositories::period_repo::PeriodRepository,
    repositories::personnel_repo::PersonnelRepository, repositories::retro_repo::get_allocations,
    repositories::retro_repo::get_batches, repositories::retro_repo::get_revisions,
    repositories::settings_repo::SettingsRepository,
    services::payroll_service::PayrollService,
};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::collections::HashMap;

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
fn native_v4_restore_rejects_retro_batch_payment_lifecycle_mismatch() {
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
    let error = MigrationService::replace_backup_data(&mut conn, &payload)
        .expect_err("CALCULATED batch FINALIZED payment event ile restore edilmemeli");
    assert!(error.to_string().contains("lifecycle"));
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
        kesintiler: Some(PersonelKesintileri::default()),
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
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;
    AttendanceRepository::save(&conn, &attendance)?;
    PayrollService::calculate_payroll_for_personnel(&conn, &personnel.id, &period.id)?;

    let dataset = PayrollService::build_dataset_snapshot(&conn)?;
    let payload = json!({
        "backupVersion": 5,
        "exportedAt": "2026-09-09T00:00:00.000Z",
        "donemler": dataset.periods,
        "aktifDonemId": period.id,
        "personeller": dataset.personnel,
        "kurumDegerleriMap": dataset.institutionSettings,
        "puantajlar": dataset.attendances,
        "bordrolar": dataset.payrolls,
        "taxOpenings": dataset.taxOpenings,
        "sickLeaveRecords": dataset.sickLeaveRecords,
        "annualPayrollParameters": dataset.annualPayrollParameters,
        "zamAylari": dataset.zamAylari,
        "compensationRevisions": dataset.compensationRevisions,
        "compensationRevisionOverrides": dataset.compensationRevisionOverrides,
        "retroBatches": dataset.retroBatches,
        "retroAllocations": dataset.retroAllocations
    })
    .to_string();
    Ok((conn, payload))
}

#[test]
fn native_current_v5_backup_roundtrip_replays_authoritative_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let (mut conn, payload) = current_v5_fixture()?;
    let before = PayrollRepository::get_all(&conn)?;

    MigrationService::replace_backup_data(&mut conn, &payload)?;

    let after = PayrollRepository::get_all(&conn)?;
    fn financial_snapshot_without_timestamps(payrolls: Vec<BordroKaydi>) -> Value {
        let mut value = serde_json::to_value(payrolls).expect("payroll snapshot serialize");
        for item in value.as_array_mut().expect("payroll array") {
            let object = item.as_object_mut().expect("payroll object");
            object.remove("olusturulmaTarihi");
            object.remove("sonGuncellemeTarihi");
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
fn native_legacy_sparse_backup_preserves_persisted_gv_base_without_rich_detail(
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut conn, payload) = current_v5_fixture()?;
    let mut legacy: Value = serde_json::from_str(&payload)?;
    legacy["backupVersion"] = json!(4);
    let payroll = legacy["bordrolar"]
        .as_array_mut()
        .and_then(|items| items.first_mut())
        .expect("legacy payroll");
    let persisted_base = payroll["persistedGvBase"].clone();
    payroll["gvDetay"] = Value::Null;

    MigrationService::replace_backup_data(&mut conn, &legacy.to_string())?;

    let restored = PayrollRepository::get_all(&conn)?;
    assert_eq!(restored.len(), 1);
    assert!(restored[0].gvDetay.is_none());
    assert_eq!(
        restored[0].persistedGvBase,
        Some(persisted_base.as_str().expect("persisted GV base").parse()?)
    );
    assert_eq!(restored[0].status, BordroStatus::CALCULATED);
    assert_eq!(
        PayrollRepository::sum_gv_base_for_tax_month_range(&conn, "v5-person", 2026, 1, 3)?,
        restored[0].persistedGvBase.expect("persisted GV base")
    );
    Ok(())
}

#[test]
fn native_current_v5_backup_rejects_semantically_forged_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let (mut conn, payload) = current_v5_fixture()?;
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
    payroll["gvDetay"]["yeniKumulatifGvMatrahi"] =
        json!((previous + forged_base).to_string());

    let error = MigrationService::replace_backup_data(&mut conn, &forged.to_string())
        .expect_err("semantically forged V5 snapshot must be rejected");
    assert!(error.to_string().contains("V5 backup replay"));
    assert_eq!(PayrollRepository::get_all(&conn)?.len(), 1);
    Ok(())
}
