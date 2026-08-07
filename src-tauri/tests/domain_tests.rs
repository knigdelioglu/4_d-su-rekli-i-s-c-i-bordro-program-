#[cfg(test)]
mod tests {
    use bordro_programi_lib::db::create_in_memory_connection;
    use bordro_programi_lib::domain::models::*;
    use bordro_programi_lib::domain::DomainError;
    use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
    use bordro_programi_lib::repositories::period_repo::PeriodRepository;
    use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
    use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
    use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
    use bordro_programi_lib::services::migration_service::MigrationService;
    use rust_decimal_macros::dec;

    fn setup_test_person(id: &str) -> Personel {
        Personel {
            id: id.to_string(),
            tcNo: "11111111111".to_string(),
            ad: "Ahmet".to_string(),
            soyad: "Test".to_string(),
            grup: "1. Grup".to_string(),
            unvan: None,
            sgkSicilNo: "12345".to_string(),
            iban: "TR00".to_string(),
            hizmetYili: 5,
            aciklama: None,
            kesintiler: None,
        }
    }

    #[test]
    fn test_cumulative_gv_regression_and_collision() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-p1");
        PersonnelRepository::save(&conn, &person)?;

        let mayis2026 = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
        };

        let haziran2026 = BordroDonemi {
            id: "2026-06".into(),
            yil: 2026,
            ay: 6,
            baslangicTarihi: "2026-06-15".into(),
            bitisTarihi: "2026-07-14".into(),
            donemAdi: "Haziran 2026".into(),
        };

        let temmuz2026 = BordroDonemi {
            id: "2026-07".into(),
            yil: 2026,
            ay: 7,
            baslangicTarihi: "2026-07-15".into(),
            bitisTarihi: "2026-08-14".into(),
            donemAdi: "Temmuz 2026".into(),
        };

        let ocak2027 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
        };

        PeriodRepository::save(&conn, &mayis2026)?;
        PeriodRepository::save(&conn, &haziran2026)?;
        PeriodRepository::save(&conn, &temmuz2026)?;
        PeriodRepository::save(&conn, &ocak2027)?;

        // Set Tax Opening for 2026 starting May (120,000 TL)
        let tax_opening = PersonelTaxOpening {
            id: "opt_1".into(),
            personnelId: "test-p1".into(),
            year: 2026,
            gvCumulativeOpening: dec!(120000),
            effectiveFromPeriodId: "2026-05".into(),
            createdAt: None,
            updatedAt: None,
        };
        TaxOpeningRepository::save(&conn, &tax_opening)?;

        // 1. May 2026 previous cumulative should be 120,000 TL
        let prev_mayis = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &mayis2026)?;
        assert_eq!(prev_mayis, dec!(120000));

        // Save May payroll (net GV matrah = 65,000 TL)
        let mayis_bordro = BordroKaydi {
            id: "test-p1_2026-05".into(),
            personelId: "test-p1".into(),
            donemId: "2026-05".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri { tabanBrutAylik: Some(dec!(70000)), ..Default::default() },
            gelirToplam: dec!(70000),
            kesintiler: KesintiKalemleri { isciSgkPrimi: Some(dec!(4500)), isciIssizlikPrimi: Some(dec!(500)), ..Default::default() },
            kesintiToplam: dec!(5000),
            netOdeme: dec!(65000),
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: "".into(),
            sonGuncellemeTarihi: "".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: Some(dec!(120000)),
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
        };
        PayrollRepository::save(&conn, &mayis_bordro)?;

        // 2. June 2026 previous cumulative should be 120,000 + 65,000 = 185,000 TL
        let prev_haziran = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &haziran2026)?;
        assert_eq!(prev_haziran, dec!(185000));

        // Save June payroll (net GV matrah = 70,000 TL)
        let haziran_bordro = BordroKaydi {
            id: "test-p1_2026-06".into(),
            personelId: "test-p1".into(),
            donemId: "2026-06".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri { tabanBrutAylik: Some(dec!(75000)), ..Default::default() },
            gelirToplam: dec!(75000),
            kesintiler: KesintiKalemleri { isciSgkPrimi: Some(dec!(4500)), isciIssizlikPrimi: Some(dec!(500)), ..Default::default() },
            kesintiToplam: dec!(5000),
            netOdeme: dec!(70000),
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: "".into(),
            sonGuncellemeTarihi: "".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: Some(dec!(185000)),
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
        };
        PayrollRepository::save(&conn, &haziran_bordro)?;

        // 3. July 2026 previous cumulative should be 120,000 + 65,000 + 70,000 = 255,000 TL
        let prev_temmuz = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &temmuz2026)?;
        assert_eq!(prev_temmuz, dec!(255000));

        // 4. January 2027 previous cumulative should be 0 TL (2026 devri 2027'ye taşınmaz)
        let prev_ocak2027 = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &ocak2027)?;
        assert_eq!(prev_ocak2027, dec!(0));

        // 5. Collision scenario test: Add Jan 2026 period & payroll before May 2026 start month
        let ocak2026 = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
        };
        PeriodRepository::save(&conn, &ocak2026)?;

        let ocak_bordro = BordroKaydi {
            id: "test-p1_2026-01".into(),
            personelId: "test-p1".into(),
            donemId: "2026-01".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri { tabanBrutAylik: Some(dec!(40000)), ..Default::default() },
            gelirToplam: dec!(40000),
            kesintiler: KesintiKalemleri { isciSgkPrimi: Some(dec!(3000)), isciIssizlikPrimi: Some(dec!(300)), ..Default::default() },
            kesintiToplam: dec!(3300),
            netOdeme: dec!(36700),
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: "".into(),
            sonGuncellemeTarihi: "".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: None,
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
        };
        PayrollRepository::save(&conn, &ocak_bordro)?;

        // Calculating for May 2026 now MUST trigger TaxOpeningConflict error!
        let collision_result = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &mayis2026);
        assert!(matches!(collision_result, Err(DomainError::TaxOpeningConflict(_))));

        Ok(())
    }

    #[test]
    fn test_migration_idempotent_and_rollback() -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = create_in_memory_connection()?;

        let invalid_json = "{ invalid_json }";
        let res = MigrationService::migrate_legacy_data(&mut conn, invalid_json);
        assert!(res.is_err());
        assert!(!MigrationService::is_migrated(&conn)?);

        let valid_payload = r#"{
            "donemler": [{ "id": "2026-05", "yil": 2026, "ay": 5, "baslangicTarihi": "2026-05-15", "bitisTarihi": "2026-06-14", "donemAdi": "Mayıs 2026" }],
            "personeller": [{ "id": "p-10", "tcNo": "10000000000", "ad": "Veli", "soyad": "Test", "grup": "1. Grup", "sgkSicilNo": "", "iban": "" }]
        }"#;

        MigrationService::migrate_legacy_data(&mut conn, valid_payload)?;
        assert!(MigrationService::is_migrated(&conn)?);

        let count = PersonnelRepository::get_all(&conn)?.len();
        assert_eq!(count, 1);

        // Second run must be idempotent and not duplicate data
        MigrationService::migrate_legacy_data(&mut conn, valid_payload)?;
        let count_after = PersonnelRepository::get_all(&conn)?.len();
        assert_eq!(count_after, 1);

        Ok(())
    }
}
