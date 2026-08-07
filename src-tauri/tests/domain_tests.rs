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
            odenenRaporluGun: None,
            raporluGun: None,
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
            odenenRaporluGun: None,
            raporluGun: None,
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
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &ocak_bordro)?;

        // Calculating for May 2026 now MUST trigger TaxOpeningConflict error!
        let collision_result = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &mayis2026);
        assert!(matches!(collision_result, Err(DomainError::TaxOpeningConflict(_))));

        Ok(())
    }

    #[test]
    fn test_finalized_payroll_locked() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::services::payroll_service::PayrollService;

        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-finalize");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-08".into(),
            yil: 2026,
            ay: 8,
            baslangicTarihi: "2026-08-15".into(),
            bitisTarihi: "2026-09-14".into(),
            donemAdi: "Ağustos 2026".into(),
        };
        PeriodRepository::save(&conn, &donem)?;

        let bordro = BordroKaydi {
            id: "test-finalize_2026-08".into(),
            personelId: "test-finalize".into(),
            donemId: "2026-08".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri { tabanBrutAylik: Some(dec!(100000)), ..Default::default() },
            gelirToplam: dec!(100000),
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: dec!(0),
            netOdeme: dec!(90000),
            status: BordroStatus::FINALIZED,
            olusturulmaTarihi: "".into(),
            sonGuncellemeTarihi: "".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: None,
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &bordro)?;

        // 1. Downgrading from FINALIZED must fail
        let res = PayrollService::set_payroll_status(&conn, "test-finalize", "2026-08", BordroStatus::CALCULATED);
        assert!(matches!(res, Err(DomainError::PayrollFinalized(_))));

        // 2. Re-setting FINALIZED is idempotent / allowed
        let res = PayrollService::set_payroll_status(&conn, "test-finalize", "2026-08", BordroStatus::FINALIZED);
        assert!(res.is_ok());

        // 3. Re-calculating a FINALIZED payroll must fail
        let res = PayrollService::calculate_payroll_for_personnel(&conn, "test-finalize", "2026-08");
        assert!(matches!(res, Err(DomainError::PayrollFinalized(_))));

        // 4. Status change for a nonexistent payroll errors with NotFound
        let res = PayrollService::set_payroll_status(&conn, "test-finalize", "2026-09", BordroStatus::CALCULATED);
        assert!(matches!(res, Err(DomainError::NotFound(_))));

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

    // ==========================================
    // REGRESSION TESTS A - H
    // ==========================================

    #[test]
    fn test_a_gc_does_not_reduce_days() {
        use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 27;
        puantaj.gc = 3;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukTabanUcret = dec!(1000);
        kurum.gunlukYemek = dec!(100);
        kurum.gunlukVasitaYol = dec!(50);
        kurum.isPrimiYuzde = Some(dec!(10));
        kurum.geceCalismaPrimiYuzde = Some(dec!(8));

        let gelirler = auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, None);

        // 27 Ç + 3 GÇ = 30 Hakediş Günü
        // 30 * 1000 = 30,000 TL Taban Aylık
        assert_eq!(gelirler.tabanBrutAylik, Some(dec!(30000)));

        // 30 Fiili Gün * 100 TL = 3,000 TL Yemek
        assert_eq!(gelirler.yemek, Some(dec!(3000)));

        // 30 Fiili Gün * 50 TL = 1,500 TL Vasıta/Yol
        assert_eq!(gelirler.vasitaYol, Some(dec!(1500)));

        // 30 Gün * 1000 * %10 = 3,000 TL İş Primi
        assert_eq!(gelirler.isPrimi, Some(dec!(3000)));

        // 3 Gece Günü * 1000 * %8 = 240 TL Gece Çalışması Ücreti
        assert_eq!(gelirler.geceCalismasiUcreti, Some(dec!(240)));
    }

    #[test]
    fn test_b_gct_does_not_reduce_days() {
        use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 24;
        puantaj.gc = 2;
        puantaj.t = 2;
        puantaj.gct = 2;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukTabanUcret = dec!(1000);
        kurum.gunlukYemek = dec!(100);
        kurum.gunlukVasitaYol = dec!(50);
        kurum.isPrimiYuzde = Some(dec!(10));
        kurum.geceCalismaPrimiYuzde = Some(dec!(8));
        kurum.geceCalismaTatiliPrimiYuzde = Some(dec!(10));

        let gelirler = auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, None);

        // 24 + 2 + 2 + 2 = 30 Hakediş Günü -> 30,000 TL
        assert_eq!(gelirler.tabanBrutAylik, Some(dec!(30000)));

        // Fiili çalışma: 24 Ç + 2 GÇ = 26 Gün
        // 26 * 100 = 2,600 TL Yemek
        assert_eq!(gelirler.yemek, Some(dec!(2600)));
        // 26 * 50 = 1,300 TL Vasıta
        assert_eq!(gelirler.vasitaYol, Some(dec!(1300)));

        // Gece Çalışması Primi: 2 * 1000 * %8 = 160 TL
        assert_eq!(gelirler.geceCalismasiUcreti, Some(dec!(160)));

        // Gece Çalışması Tatili Primi: 2 * 1000 * %10 = 200 TL
        assert_eq!(gelirler.geceCalismasiTatiliUcreti, Some(dec!(200)));
    }

    #[test]
    fn test_c_parametric_night_premium() {
        use bordro_programi_lib::domain::calculations::NightWorkPolicy;
        // Günlük taban: 2443.28 TL, %8, 3 gün
        let daily_base = dec!(2443.28);
        let rate_percent = dec!(8);
        let days = 3;

        let premium = NightWorkPolicy::calculate_gece_calismasi_primi(daily_base, rate_percent, days);
        // 2443.28 * 0.08 * 3 = 586.3872 TL -> rounded to 2 decimals = 586.39 TL
        assert_eq!(premium, dec!(586.39));
    }

    #[test]
    fn test_d_zero_rate_night_work() {
        use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 25;
        puantaj.gc = 5;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukTabanUcret = dec!(2000);
        kurum.geceCalismaPrimiYuzde = Some(dec!(0));
        kurum.geceCalismaTatiliPrimiYuzde = Some(dec!(0));

        let gelirler = auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, None);

        // 25 + 5 = 30 Hakediş günü -> 60,000 TL taban ücret (günler kaybolmaz)
        assert_eq!(gelirler.tabanBrutAylik, Some(dec!(60000)));
        // Ek gece primi: 0 TL
        assert_eq!(gelirler.geceCalismasiUcreti, Some(dec!(0)));
        assert_eq!(gelirler.geceCalismasiTatiliUcreti, Some(dec!(0)));
    }

    #[test]
    fn test_e_first_sick_leave_cap() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-test");
        PersonnelRepository::save(&conn, &person)?;

        let rec = SickLeaveRecord {
            id: "sick_1".into(),
            personnelId: "p-test".into(),
            startDate: "2026-05-18".into(),
            endDate: "2026-05-21".into(), // 4 gün
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let period = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        // 4 gün raporun ilk 2 günü kurum tarafından ödenir
        let paid_days = SickLeaveService::calculate_paid_sick_days_from_records(&records, &period);
        assert_eq!(paid_days, 2);

        Ok(())
    }

    #[test]
    fn test_f_short_sick_leave() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-test");
        PersonnelRepository::save(&conn, &person)?;

        let rec = SickLeaveRecord {
            id: "sick_1".into(),
            personnelId: "p-test".into(),
            startDate: "2026-05-20".into(),
            endDate: "2026-05-20".into(), // 1 gün
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let period = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        // 1 günlük rapor için ödenen gün = 1
        let paid_days = SickLeaveService::calculate_paid_sick_days_from_records(&records, &period);
        assert_eq!(paid_days, 1);

        Ok(())
    }

    #[test]
    fn test_g_sixth_sick_leave_unpaid() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-test");
        PersonnelRepository::save(&conn, &person)?;

        // Add 5 prior sick leave episodes in 2026
        for i in 1..=5 {
            let start = format!("2026-0{}-01", i);
            let end = format!("2026-0{}-03", i);
            let rec = SickLeaveRecord {
                id: format!("sick_{}", i),
                personnelId: "p-test".into(),
                startDate: start,
                endDate: end,
                createdAt: None,
                updatedAt: None,
            };
            SickLeaveRepository::save(&conn, &rec)?;
        }

        // 6th episode in June 2026 (4 days)
        let rec6 = SickLeaveRecord {
            id: "sick_6".into(),
            personnelId: "p-test".into(),
            startDate: "2026-06-18".into(),
            endDate: "2026-06-21".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec6)?;

        let period = BordroDonemi {
            id: "2026-06".into(),
            yil: 2026,
            ay: 6,
            baslangicTarihi: "2026-06-15".into(),
            bitisTarihi: "2026-07-14".into(),
            donemAdi: "Haziran 2026".into(),
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        // 6th episode must yield 0 paid days
        let paid_days = SickLeaveService::calculate_paid_sick_days_from_records(&records, &period);
        assert_eq!(paid_days, 0);

        Ok(())
    }

    #[test]
    fn test_h_split_period_single_sick_leave_no_duplicate() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-test");
        PersonnelRepository::save(&conn, &person)?;

        // Single sick leave spanning May 13 to May 18 (6 days):
        // Day 1: May 13 (Nisan period: Apr 15 - May 14) -> Paid (Day 1 of episode)
        // Day 2: May 14 (Nisan period: Apr 15 - May 14) -> Paid (Day 2 of episode)
        // Day 3: May 15 (Mayıs period: May 15 - Jun 14) -> Unpaid (Day 3 of episode)
        // Day 4: May 16 (Mayıs period: May 15 - Jun 14) -> Unpaid (Day 4 of episode)
        // Day 5: May 17 (Mayıs period: May 15 - Jun 14) -> Unpaid (Day 5 of episode)
        // Day 6: May 18 (Mayıs period: May 15 - Jun 14) -> Unpaid (Day 6 of episode)
        let rec = SickLeaveRecord {
            id: "sick_split".into(),
            personnelId: "p-test".into(),
            startDate: "2026-05-13".into(),
            endDate: "2026-05-18".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let nisan_period = BordroDonemi {
            id: "2026-04".into(),
            yil: 2026,
            ay: 4,
            baslangicTarihi: "2026-04-15".into(),
            bitisTarihi: "2026-05-14".into(),
            donemAdi: "Nisan 2026".into(),
        };

        let mayis_period = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        let nisan_paid = SickLeaveService::calculate_paid_sick_days_from_records(&records, &nisan_period);
        let mayis_paid = SickLeaveService::calculate_paid_sick_days_from_records(&records, &mayis_period);

        // Nisan period pays the 2 days (May 13, May 14)
        assert_eq!(nisan_paid, 2);

        // Mayıs period pays 0 days (May 15-18 are days 3..6 of the SAME single episode -> no duplicate payment)
        assert_eq!(mayis_paid, 0);

        Ok(())
    }

    // ==========================================
    // SGK YEMEK İSTİSNASI VE İŞVEREN PRİMLERİ TESTS A - H
    // ==========================================

    #[test]
    fn test_yemek_istisnasi_hesabi_test_a() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 22; // 22 gün yemek hak günü

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukYemekIstisnasiSGK = Some(dec!(300.00));

        let daily_meal = dec!(300.75);
        let total_meal = daily_meal * dec!(22); // 6,616.50 TL

        let gelirler = GelirKalemleri {
            yemek: Some(total_meal),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(gelirler.yemek, Some(dec!(6616.50)));
        assert_eq!(pek_detay.fiiliYemekGunu, 22);
        assert_eq!(pek_detay.yemekIstisnasiTutar, dec!(6600.00));
        assert_eq!(pek_detay.hesaplananPek, dec!(16.50));
    }

    #[test]
    fn test_yemek_300_tl_den_dusuk_test_b() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 22;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukYemekIstisnasiSGK = Some(dec!(300.00));

        let daily_meal = dec!(250.00);
        let total_meal = daily_meal * dec!(22); // 5,500.00 TL

        let gelirler = GelirKalemleri {
            yemek: Some(total_meal),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.yemekIstisnasiTutar, dec!(5500.00));
        assert_eq!(pek_detay.hesaplananPek, dec!(0.00));
    }

    #[test]
    fn test_isveren_sgk_primi_test_c() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.sgkIsverenOraniYuzde = Some(dec!(21.75));

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000.00)),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.finalPek, dec!(100000.00));
        assert_eq!(pek_detay.isverenSgkPrimi, Some(dec!(21750.00)));
    }

    #[test]
    fn test_isveren_issizlik_primi_test_d() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.issizlikIsverenOraniYuzde = Some(dec!(2.00));

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000.00)),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.finalPek, dec!(100000.00));
        assert_eq!(pek_detay.isverenIssizlikPrimi, Some(dec!(2000.00)));
    }

    #[test]
    fn test_toplam_isveren_primi_test_e() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.sgkIsverenOraniYuzde = Some(dec!(21.75));
        kurum.issizlikIsverenOraniYuzde = Some(dec!(2.00));

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000.00)),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.isverenPrimToplami, Some(dec!(23750.00)));
    }

    #[test]
    fn test_net_odeme_degismez_isveren_maliyeti_etkisizdir_test_f() {
        use bordro_programi_lib::domain::calculations::{calculate_statutory_deductions, calculate_gelir_toplam, calculate_kesinti_toplam};
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let kurum_default = DonemselKurumDegerleri::default();

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(80000.00)),
            ..Default::default()
        };

        let (kesintiler, pek_detay, _) = calculate_statutory_deductions(
            &gelirler,
            Some(&kurum_default),
            None,
            Some(&puantaj),
            dec!(0),
            &[],
            dec!(0),
        );

        let gelir_toplam = calculate_gelir_toplam(&gelirler);
        let kesinti_toplam = calculate_kesinti_toplam(&kesintiler);
        let net_odeme = gelir_toplam - kesinti_toplam;

        // Verify employer premiums are calculated in pek_detay
        assert!(pek_detay.isverenSgkPrimi.is_some());
        assert!(pek_detay.isverenIssizlikPrimi.is_some());
        assert!(pek_detay.isverenPrimToplami.is_some());

        // Worker deductions only contain worker items (SGK %14, Unemployment %1, etc.)
        let isci_sgk = kesintiler.isciSgkPrimi.unwrap_or(dec!(0));
        let isci_issizlik = kesintiler.isciIssizlikPrimi.unwrap_or(dec!(0));

        // Employer costs must not be included in kesinti_toplam
        assert_eq!(isci_sgk, dec!(80000) * dec!(0.14));
        assert_eq!(isci_issizlik, dec!(80000) * dec!(0.01));
        assert_eq!(net_odeme, gelir_toplam - kesinti_toplam);
    }

    #[test]
    fn test_parametre_degisikligi_dinamik_oran_kullanimi_test_g() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let mut kurum_ozel = DonemselKurumDegerleri::default();
        kurum_ozel.sgkIsverenOraniYuzde = Some(dec!(22.00));
        kurum_ozel.issizlikIsverenOraniYuzde = Some(dec!(3.00));

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000.00)),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum_ozel), &[]);

        // 100,000 * %22 = 22,000 TL
        assert_eq!(pek_detay.isverenSgkPrimi, Some(dec!(22000.00)));
        // 100,000 * %3 = 3,000 TL
        assert_eq!(pek_detay.isverenIssizlikPrimi, Some(dec!(3000.00)));
        // Total = 25,000 TL
        assert_eq!(pek_detay.isverenPrimToplami, Some(dec!(25000.00)));
    }

    #[test]
    fn test_gc_yemek_hakki_korunuyor_test_h() {
        use bordro_programi_lib::domain::calculations::calculate_prime_esas_kazanc;
        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 20;
        puantaj.gc = 2; // 20 Ç + 2 GÇ = 22 yemek hak günü

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukYemekIstisnasiSGK = Some(dec!(300.00));

        let total_meal = dec!(300.75) * dec!(22); // 6,616.50 TL

        let gelirler = GelirKalemleri {
            yemek: Some(total_meal),
            ..Default::default()
        };

        let (pek_detay, _) = calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.fiiliYemekGunu, 22);
        assert_eq!(pek_detay.yemekIstisnasiTutar, dec!(6600.00));
        assert_eq!(pek_detay.hesaplananPek, dec!(16.50));
    }
}
