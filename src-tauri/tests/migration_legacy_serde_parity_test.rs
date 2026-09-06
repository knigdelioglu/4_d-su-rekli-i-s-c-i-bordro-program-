use bordro_programi_lib::domain::models::{
    BordroKaydi, BordroStatus, GvHesapDetayi, IsPrimiGrupItem, PekDetayi,
};
use bordro_programi_lib::services::migration_service::{LegacyPayload, MigrationService};
use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
    repositories::personnel_repo::PersonnelRepository,
    repositories::payroll_repo::PayrollRepository,
    repositories::retro_repo::get_allocations,
    repositories::retro_repo::get_batches,
    repositories::retro_repo::get_revisions,
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
    assert_eq!(restored.accrualType, bordro_programi_lib::domain::models::AccrualType::RETRO_ADJUSTMENT);
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
