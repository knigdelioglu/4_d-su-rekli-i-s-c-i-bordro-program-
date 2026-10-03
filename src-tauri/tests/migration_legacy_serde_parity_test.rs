use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::services::migration_service::{LegacyPayload, MigrationService};
use bordro_programi_lib::{
    db::create_in_memory_connection,
    repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository,
    repositories::attendance_repo::AttendanceRepository,
    repositories::payroll_repo::PayrollRepository, repositories::period_repo::PeriodRepository,
    repositories::personnel_repo::PersonnelRepository, repositories::retro_repo::get_allocations,
    repositories::retro_repo::get_batches, repositories::retro_repo::get_revisions,
    repositories::settings_repo::SettingsRepository, services::payroll_service::PayrollService,
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

fn current_v5_payload_from_database(
    conn: &rusqlite::Connection,
) -> Result<String, Box<dyn std::error::Error>> {
    let dataset = PayrollService::build_dataset_snapshot(conn)?;
    let active_period_id =
        SettingsRepository::get_app_setting(conn, "active_period_id")?.unwrap_or_default();
    Ok(json!({
        "backupVersion": 5,
        "exportedAt": "2026-10-03T00:00:00.000Z",
        "donemler": dataset.periods,
        "aktifDonemId": active_period_id,
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
fn native_v5_zero_recovery_and_overpayment_backup_roundtrips_and_rejects_tampering(
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut source, base_payload) = current_v5_fixture()?;
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
    let recalculated = PayrollService::calculate_payroll_for_personnel(
        &payroll_chain_check,
        "v5-person",
        "2026-01",
    )?;
    assert_eq!(recalculated.status, BordroStatus::CALCULATED);

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
    let mut conn = create_in_memory_connection()?;

    MigrationService::replace_backup_data(&mut conn, payload)?;

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
        "zamAylari": dataset.zamAylari,
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
        "zamAylari",
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
        Some(
            persisted_base
                .as_str()
                .expect("persisted GV base")
                .parse()?
        )
    );
    assert_eq!(restored[0].status, BordroStatus::CALCULATED);
    assert_eq!(
        PayrollRepository::sum_gv_base_for_tax_month_range(&conn, "v5-person", 2026, 1, 3)?,
        restored[0].persistedGvBase.expect("persisted GV base")
    );
    Ok(())
}

#[test]
fn native_current_v5_backup_rejects_semantically_forged_snapshot(
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut conn, payload) = current_v5_fixture()?;
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
