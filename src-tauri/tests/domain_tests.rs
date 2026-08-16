#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use bordro_programi_lib::db::create_in_memory_connection;
    use bordro_programi_lib::domain::models::*;
    use bordro_programi_lib::domain::DomainError;
    use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
    use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
    use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
    use bordro_programi_lib::repositories::period_repo::PeriodRepository;
    use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
    use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
    use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
    use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
    use bordro_programi_lib::services::migration_service::MigrationService;
    use bordro_programi_lib::services::payroll_service::PayrollService;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

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
            devirKumulatifGvMatrahi: None,
            devirKumulatifGvMatrahiYili: None,
            devirKumulatifGvMatrahiBaslangicAyi: None,
            devirKumulatifAsgariGvMatrahi: None,
            devirKumulatifAsgariGvMatrahiYili: None,
            kesintiler: None,
        }
    }

    fn ensure_test_annual_parameters(
        conn: &rusqlite::Connection,
        years: &[i32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for year in years {
            let mut parameters = AnnualPayrollParameters::default_for_2026();
            parameters.year = *year;
            AnnualPayrollParametersRepository::save(conn, &parameters)?;
        }
        Ok(())
    }

    fn ensure_test_institution_settings(
        conn: &rusqlite::Connection,
        period_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for period_id in period_ids {
            let settings = DonemselKurumDegerleri {
                donemId: (*period_id).to_string(),
                ..DonemselKurumDegerleri::default()
            };
            SettingsRepository::save_institution_settings(conn, &settings)?;
        }
        Ok(())
    }

    fn thirty_work_days(period: &BordroDonemi) -> HashMap<String, String> {
        let start = chrono::NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .expect("test period start date must be valid");
        let end = chrono::NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d")
            .expect("test period end date must be valid");

        (0..30)
            .map(|offset| {
                let date = start + chrono::Duration::days(offset);
                assert!(
                    date <= end,
                    "generated attendance date {date} must stay inside period {}",
                    period.id
                );
                (date.format("%Y-%m-%d").to_string(), "Ç".to_string())
            })
            .collect()
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
            taxYear: 2026,
            taxMonth: 6,
        };

        let haziran2026 = BordroDonemi {
            id: "2026-06".into(),
            yil: 2026,
            ay: 6,
            baslangicTarihi: "2026-06-15".into(),
            bitisTarihi: "2026-07-14".into(),
            donemAdi: "Haziran 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };

        let temmuz2026 = BordroDonemi {
            id: "2026-07".into(),
            yil: 2026,
            ay: 7,
            baslangicTarihi: "2026-07-15".into(),
            bitisTarihi: "2026-08-14".into(),
            donemAdi: "Temmuz 2026".into(),
            taxYear: 2026,
            taxMonth: 8,
        };

        let ocak2027 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
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
        let prev_mayis =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &mayis2026)?;
        assert_eq!(prev_mayis, dec!(120000));

        // Save May payroll (net GV matrah = 65,000 TL)
        let mayis_bordro = BordroKaydi {
            id: "test-p1_2026-05".into(),
            personelId: "test-p1".into(),
            donemId: "2026-05".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(dec!(70000)),
                ..Default::default()
            },
            gelirToplam: dec!(70000),
            kesintiler: KesintiKalemleri {
                isciSgkPrimi: Some(dec!(4500)),
                isciIssizlikPrimi: Some(dec!(500)),
                ..Default::default()
            },
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
            isPrimiDetay: None,
            gvDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &mayis_bordro)?;

        // 2. June 2026 previous cumulative should be 120,000 + 65,000 = 185,000 TL
        let prev_haziran =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &haziran2026)?;
        assert_eq!(prev_haziran, dec!(185000));

        // Save June payroll (net GV matrah = 70,000 TL)
        let haziran_bordro = BordroKaydi {
            id: "test-p1_2026-06".into(),
            personelId: "test-p1".into(),
            donemId: "2026-06".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(dec!(75000)),
                ..Default::default()
            },
            gelirToplam: dec!(75000),
            kesintiler: KesintiKalemleri {
                isciSgkPrimi: Some(dec!(4500)),
                isciIssizlikPrimi: Some(dec!(500)),
                ..Default::default()
            },
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
            isPrimiDetay: None,
            gvDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &haziran_bordro)?;

        // 3. July 2026 previous cumulative should be 120,000 + 65,000 + 70,000 = 255,000 TL
        let prev_temmuz =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &temmuz2026)?;
        assert_eq!(prev_temmuz, dec!(255000));

        // 4. January 2027 previous cumulative should be 0 TL (2026 devri 2027'ye taşınmaz)
        let prev_ocak2027 =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &ocak2027)?;
        assert_eq!(prev_ocak2027, dec!(0));

        // 5. Collision scenario test: Add Jan 2026 period & payroll before May 2026 start month
        let ocak2026 = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &ocak2026)?;

        let ocak_bordro = BordroKaydi {
            id: "test-p1_2026-01".into(),
            personelId: "test-p1".into(),
            donemId: "2026-01".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(dec!(40000)),
                ..Default::default()
            },
            gelirToplam: dec!(40000),
            kesintiler: KesintiKalemleri {
                isciSgkPrimi: Some(dec!(3000)),
                isciIssizlikPrimi: Some(dec!(300)),
                ..Default::default()
            },
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
            isPrimiDetay: None,
            gvDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &ocak_bordro)?;

        // Calculating for May 2026 now MUST trigger TaxOpeningConflict error!
        let collision_result =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-p1", &mayis2026);
        assert!(matches!(
            collision_result,
            Err(DomainError::TaxOpeningConflict(_))
        ));

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
            taxYear: 2026,
            taxMonth: 9,
        };
        PeriodRepository::save(&conn, &donem)?;

        let bordro = BordroKaydi {
            id: "test-finalize_2026-08".into(),
            personelId: "test-finalize".into(),
            donemId: "2026-08".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(dec!(100000)),
                ..Default::default()
            },
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
            isPrimiDetay: None,
            gvDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &bordro)?;

        // 1. Downgrading from FINALIZED must fail
        let res = PayrollService::set_payroll_status(
            &conn,
            "test-finalize",
            "2026-08",
            BordroStatus::CALCULATED,
        );
        assert!(matches!(res, Err(DomainError::PayrollFinalized(_))));

        // 2. Re-setting FINALIZED is idempotent / allowed
        let res = PayrollService::set_payroll_status(
            &conn,
            "test-finalize",
            "2026-08",
            BordroStatus::FINALIZED,
        );
        assert!(res.is_ok());

        // 3. Re-calculating a FINALIZED payroll must fail
        let res =
            PayrollService::calculate_payroll_for_personnel(&conn, "test-finalize", "2026-08");
        assert!(matches!(res, Err(DomainError::PayrollFinalized(_))));

        // 4. Status change for a nonexistent payroll errors with NotFound
        let res = PayrollService::set_payroll_status(
            &conn,
            "test-finalize",
            "2026-09",
            BordroStatus::CALCULATED,
        );
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
            "donemler": [{ "id": "2026-05", "yil": 2026, "ay": 5, "baslangicTarihi": "2026-05-15", "bitisTarihi": "2026-06-14", "donemAdi": "Mayıs 2026", "taxYear": 2026, "taxMonth": 6 }],
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
        kurum.isPrimiGruplari = Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(10),
            aktif: true,
        }]);
        kurum.geceCalismaPrimiYuzde = Some(dec!(8));

        let (gelirler, _is_primi_detay) =
            auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, Some("1. Grup"))
                .expect("grup cozumlenmeli");

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
        kurum.isPrimiGruplari = Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(10),
            aktif: true,
        }]);
        kurum.geceCalismaPrimiYuzde = Some(dec!(8));
        kurum.geceCalismaTatiliPrimiYuzde = Some(dec!(10));

        let (gelirler, _is_primi_detay) =
            auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, Some("1. Grup"))
                .expect("grup cozumlenmeli");

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

        let premium =
            NightWorkPolicy::calculate_gece_calismasi_primi(daily_base, rate_percent, days);
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

        let (gelirler, _is_primi_detay) =
            auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, Some("1. Grup"))
                .expect("grup cozumlenmeli");

        // 25 + 5 = 30 Hakediş günü -> 60,000 TL taban ücret (günler kaybolmaz)
        assert_eq!(gelirler.tabanBrutAylik, Some(dec!(60000)));
        // Ek gece primi: 0 TL
        assert_eq!(gelirler.geceCalismasiUcreti, Some(dec!(0)));
        assert_eq!(gelirler.geceCalismasiTatiliUcreti, Some(dec!(0)));
    }

    #[test]
    fn test_is_prime_group_is_authoritative_not_fallback() {
        use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;

        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 26;
        puantaj.gc = 4;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukTabanUcret = dec!(1000);
        // Even if the institution-wide single rate is 50, the authoritative
        // source is the personnel group (2. Grup = 8%).
        kurum.isPrimiYuzde = Some(dec!(50));
        kurum.isPrimiGruplari = Some(vec![IsPrimiGrupItem {
            id: "2. Grup".into(),
            ad: "2. Grup".into(),
            oran: dec!(8),
            aktif: true,
        }]);

        let (gelirler, detay) =
            auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, Some("2. Grup"))
                .expect("grup cozumlenmeli");
        // Hak gunu = C + GC = 26 + 4 = 30; 1000 * 0.08 * 30 = 2400
        assert_eq!(gelirler.isPrimi, Some(dec!(2400)));
        assert_eq!(detay.tutar, dec!(2400));
        assert_eq!(detay.hakGunu, 30);
    }

    #[test]
    fn test_is_prime_single_final_math() {
        use bordro_programi_lib::domain::calculations::calculate_is_primi_detayi;

        let gruplar = vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(3),
            aktif: true,
        }];

        let detay = calculate_is_primi_detayi(dec!(27.55), 3, Some("1. Grup"), Some(&gruplar))
            .expect("grup cozumlenmeli");

        // Hak gunu = 3, taban 27.55, %3:
        // 27.55 * 0.03 = 0.8265 ; * 3 = 2.4795 -> round2 = 2.48 (single rounding)
        assert_eq!(detay.tutar, dec!(2.48));
        // Gunluk deger display-only: round2(0.8265) = 0.83
        assert_eq!(detay.gunlukIsPrimi, dec!(0.83));
        // Double rounding would give round2(0.8265)=0.83, then 0.83*3 = 2.49.
        assert_ne!(detay.tutar, dec!(2.49));
    }

    #[test]
    fn test_is_prime_tanimsiz_grup_err() {
        use bordro_programi_lib::domain::calculations::calculate_is_primi_detayi;
        use bordro_programi_lib::domain::DomainError;

        let gruplar = vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(9),
            aktif: true,
        }];

        // Personnel group missing -> ValidationError
        let e1 = calculate_is_primi_detayi(dec!(1000), 30, None, Some(&gruplar)).unwrap_err();
        assert!(matches!(e1, DomainError::ValidationError(_)));

        // Group list empty/absent -> ValidationError
        let e2 = calculate_is_primi_detayi(dec!(1000), 30, Some("1. Grup"), None).unwrap_err();
        assert!(matches!(e2, DomainError::ValidationError(_)));

        // Group not found in list -> ValidationError
        let e3 =
            calculate_is_primi_detayi(dec!(1000), 30, Some("5. Grup"), Some(&gruplar)).unwrap_err();
        assert!(matches!(e3, DomainError::ValidationError(_)));

        // Inactive group -> ValidationError
        let pasif = vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(9),
            aktif: false,
        }];
        let e4 =
            calculate_is_primi_detayi(dec!(1000), 30, Some("1. Grup"), Some(&pasif)).unwrap_err();
        assert!(matches!(e4, DomainError::ValidationError(_)));
    }

    #[test]
    fn test_is_prime_hak_gunu_c_gc_only() {
        use bordro_programi_lib::domain::calculations::auto_fill_gelirler_from_puantaj;

        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 24;
        puantaj.gc = 2;
        puantaj.gct = 2;
        puantaj.t = 2;

        let mut kurum = DonemselKurumDegerleri::default();
        kurum.gunlukTabanUcret = dec!(1000);
        kurum.isPrimiGruplari = Some(vec![IsPrimiGrupItem {
            id: "1. Grup".into(),
            ad: "1. Grup".into(),
            oran: dec!(10),
            aktif: true,
        }]);

        let (gelirler, detay) =
            auto_fill_gelirler_from_puantaj(&puantaj, &kurum, 0, Some("1. Grup")).unwrap();
        // Is primi hak gunu: only C + GC = 26. GCT and T are NOT included.
        assert_eq!(detay.hakGunu, 26);
        // 1000 * 0.10 * 26 = 2600
        assert_eq!(gelirler.isPrimi, Some(dec!(2600)));
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
            taxYear: 2026,
            taxMonth: 6,
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
            taxYear: 2026,
            taxMonth: 6,
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
            taxYear: 2026,
            taxMonth: 7,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        // 6th episode must yield 0 paid days
        let paid_days = SickLeaveService::calculate_paid_sick_days_from_records(&records, &period);
        assert_eq!(paid_days, 0);

        Ok(())
    }

    #[test]
    fn test_h_split_period_single_sick_leave_no_duplicate() -> Result<(), Box<dyn std::error::Error>>
    {
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
            taxYear: 2026,
            taxMonth: 5,
        };

        let mayis_period = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
            taxYear: 2026,
            taxMonth: 6,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-test")?;
        let nisan_paid =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &nisan_period);
        let mayis_paid =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &mayis_period);

        // Nisan period pays the 2 days (May 13, May 14)
        assert_eq!(nisan_paid, 2);

        // Mayıs period pays 0 days (May 15-18 are days 3..6 of the SAME single episode -> no duplicate payment)
        assert_eq!(mayis_paid, 0);

        Ok(())
    }

    // ==========================================
    // CALENDAR YEAR BOUNDARY SICK LEAVE TESTS (A - E + PERSISTENCE)
    // ==========================================

    #[test]
    fn test_year_crossing_sick_leave_test_a_dec_29_start() -> Result<(), Box<dyn std::error::Error>>
    {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-year-a");
        PersonnelRepository::save(&conn, &person)?;

        // 29.12.2026–03.01.2027 (1st episode of 2026)
        let rec = SickLeaveRecord {
            id: "sick_year_a".into(),
            personnelId: "p-year-a".into(),
            startDate: "2026-12-29".into(),
            endDate: "2027-01-03".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let donem_2026_12 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let donem_2027_01 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-year-a")?;
        let paid_2026_12 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2026_12);
        let paid_2027_01 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2027_01);

        assert_eq!(paid_2026_12, 2);
        assert_eq!(paid_2027_01, 0);
        assert_eq!(paid_2026_12 + paid_2027_01, 2);

        Ok(())
    }

    #[test]
    fn test_year_crossing_sick_leave_test_b_dec_31_start() -> Result<(), Box<dyn std::error::Error>>
    {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-year-b");
        PersonnelRepository::save(&conn, &person)?;

        // 31.12.2026–03.01.2027 (1st episode of 2026)
        // Global paid dates: 31.12.2026 and 01.01.2027
        let rec = SickLeaveRecord {
            id: "sick_year_b".into(),
            personnelId: "p-year-b".into(),
            startDate: "2026-12-31".into(),
            endDate: "2027-01-03".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let donem_2026_12 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let donem_2027_01 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-year-b")?;
        let paid_2026_12 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2026_12);
        let paid_2027_01 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2027_01);

        // 31.12.2026 and 01.01.2027 both fall into 2026-12 period (15.12.2026 - 14.01.2027)
        assert_eq!(paid_2026_12, 2);
        assert_eq!(paid_2027_01, 0);
        assert_eq!(paid_2026_12 + paid_2027_01, 2);

        Ok(())
    }

    #[test]
    fn test_year_crossing_sick_leave_test_c_jan_01_start() -> Result<(), Box<dyn std::error::Error>>
    {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-year-c");
        PersonnelRepository::save(&conn, &person)?;

        // 01.01.2027–04.01.2027 (1st episode of 2027)
        // Global paid dates: 01.01.2027 and 02.01.2027
        let rec = SickLeaveRecord {
            id: "sick_year_c".into(),
            personnelId: "p-year-c".into(),
            startDate: "2027-01-01".into(),
            endDate: "2027-01-04".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let donem_2026_12 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let donem_2027_01 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-year-c")?;
        let paid_2026_12 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2026_12);
        let paid_2027_01 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2027_01);

        // 01.01.2027 and 02.01.2027 fall into 2026-12 period range (15.12.2026 - 14.01.2027)
        assert_eq!(paid_2026_12, 2);
        assert_eq!(paid_2027_01, 0);

        Ok(())
    }

    #[test]
    fn test_year_crossing_sick_leave_test_d_sixth_episode_unpaid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-year-d");
        PersonnelRepository::save(&conn, &person)?;

        // Add 5 prior episodes in 2026
        for i in 1..=5 {
            let rec = SickLeaveRecord {
                id: format!("sick_year_d_{}", i),
                personnelId: "p-year-d".into(),
                startDate: format!("2026-0{}-01", i),
                endDate: format!("2026-0{}-03", i),
                createdAt: None,
                updatedAt: None,
            };
            SickLeaveRepository::save(&conn, &rec)?;
        }

        // 6th episode starting 31.12.2026 to 02.01.2027
        let rec6 = SickLeaveRecord {
            id: "sick_year_d_6".into(),
            personnelId: "p-year-d".into(),
            startDate: "2026-12-31".into(),
            endDate: "2027-01-02".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec6)?;

        let donem_2026_12 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let donem_2027_01 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-year-d")?;
        let paid_2026_12 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2026_12);
        let paid_2027_01 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2027_01);

        assert_eq!(paid_2026_12, 0);
        assert_eq!(paid_2027_01, 0);

        Ok(())
    }

    #[test]
    fn test_year_crossing_sick_leave_test_e_single_day_year_end(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::sick_leave_service::SickLeaveService;
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-year-e");
        PersonnelRepository::save(&conn, &person)?;

        // 31.12.2026–31.12.2026 (1-day episode)
        let rec = SickLeaveRecord {
            id: "sick_year_e".into(),
            personnelId: "p-year-e".into(),
            startDate: "2026-12-31".into(),
            endDate: "2026-12-31".into(),
            createdAt: None,
            updatedAt: None,
        };
        SickLeaveRepository::save(&conn, &rec)?;

        let donem_2026_12 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let records = SickLeaveRepository::get_by_personnel(&conn, "p-year-e")?;
        let paid_2026_12 =
            SickLeaveService::calculate_paid_sick_days_from_records(&records, &donem_2026_12);

        assert_eq!(paid_2026_12, 1);

        Ok(())
    }

    #[test]
    fn test_year_crossing_sick_leave_persistence_and_finalized_reload(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::db::create_connection;
        use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::payroll_service::PayrollService;
        use std::collections::HashMap;
        use std::fs;
        use std::path::PathBuf;

        let temp_dir = std::env::temp_dir();
        let db_path: PathBuf = temp_dir.join(format!(
            "year_crossing_rapor_test_{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(98765)
        ));

        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        // 1. Calculate, save, and finalize payroll for year-crossing sick leave
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let person = setup_test_person("p-yc-persistence");
            PersonnelRepository::save(&conn, &person)?;

            let donem = BordroDonemi {
                id: "2026-12".into(),
                yil: 2026,
                ay: 12,
                baslangicTarihi: "2026-12-15".into(),
                bitisTarihi: "2027-01-14".into(),
                donemAdi: "Aralık 2026".into(),
                taxYear: 2026,
                taxMonth: 12,
            };
            PeriodRepository::save(&conn, &donem)?;
            ensure_test_institution_settings(&conn, &["2026-12"])?;

            let rec = SickLeaveRecord {
                id: "sick_yc_p".into(),
                personnelId: "p-yc-persistence".into(),
                startDate: "2026-12-31".into(),
                endDate: "2027-01-03".into(),
                createdAt: None,
                updatedAt: None,
            };
            SickLeaveRepository::save(&conn, &rec)?;

            let mut gunler = HashMap::new();
            for d in 15..=30 {
                gunler.insert(format!("2026-12-{:02}", d), "Ç".to_string());
            }
            gunler.insert("2026-12-31".to_string(), "R".to_string());
            gunler.insert("2027-01-01".to_string(), "R".to_string());
            gunler.insert("2027-01-02".to_string(), "R".to_string());
            gunler.insert("2027-01-03".to_string(), "R".to_string());

            let puantaj = PersonelPuantaj {
                id: "p-yc-persistence_2026-12".into(),
                personelId: "p-yc-persistence".into(),
                donemId: "2026-12".into(),
                gunler,
            };
            AttendanceRepository::save(&conn, &puantaj)?;

            let bordro = PayrollService::calculate_payroll_for_personnel(
                &conn,
                "p-yc-persistence",
                "2026-12",
            )?;
            assert_eq!(bordro.odenenRaporluGun, Some(2));
            assert_eq!(bordro.raporluGun, Some(4));

            PayrollRepository::save(&conn, &bordro)?;
            PayrollService::set_payroll_status(
                &conn,
                "p-yc-persistence",
                "2026-12",
                BordroStatus::FINALIZED,
            )?;
        }

        // 2. Re-open database connection and reload finalized payroll
        {
            let conn = create_connection(Some(db_path.clone()))?;
            let payrolls = PayrollRepository::get_all(&conn)?;
            assert_eq!(payrolls.len(), 1);
            let loaded = &payrolls[0];

            assert_eq!(loaded.status, BordroStatus::FINALIZED);
            assert_eq!(loaded.odenenRaporluGun, Some(2));
            assert_eq!(loaded.raporluGun, Some(4));
        }

        let _ = fs::remove_file(&db_path);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.isverenPrimToplami, Some(dec!(23750.00)));
    }

    #[test]
    fn test_net_odeme_degismez_isveren_maliyeti_etkisizdir_test_f() {
        use bordro_programi_lib::domain::calculations::{
            calculate_gelir_toplam, calculate_kesinti_toplam, calculate_statutory_deductions,
        };
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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum_ozel), &[]);

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

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.fiiliYemekGunu, 22);
        assert_eq!(pek_detay.yemekIstisnasiTutar, dec!(6600.00));
        assert_eq!(pek_detay.hesaplananPek, dec!(16.50));
    }

    #[test]
    fn test_raporlu_gun_persistence_and_finalized_reload() -> Result<(), Box<dyn std::error::Error>>
    {
        use bordro_programi_lib::db::create_connection;
        use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
        use bordro_programi_lib::repositories::sick_leave_repo::SickLeaveRepository;
        use bordro_programi_lib::services::payroll_service::PayrollService;
        use std::fs;
        use std::path::PathBuf;

        let temp_dir = std::env::temp_dir();
        let db_path: PathBuf = temp_dir.join(format!(
            "raporlu_test_{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(54321)
        ));

        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        // 1. Calculate and save payroll with Raporlu Gün = 6, Ödeme Yapılan Raporlu Gün = 2
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let person = setup_test_person("p-rapor-test");
            PersonnelRepository::save(&conn, &person)?;

            let donem = BordroDonemi {
                id: "2026-05".into(),
                yil: 2026,
                ay: 5,
                baslangicTarihi: "2026-05-15".into(),
                bitisTarihi: "2026-06-14".into(),
                donemAdi: "Mayıs 2026".into(),
                taxYear: 2026,
                taxMonth: 6,
            };
            PeriodRepository::save(&conn, &donem)?;
            ensure_test_institution_settings(&conn, &["2026-05"])?;

            // 6 days of "R" + 24 days of "Ç" = 30 days
            let mut gunler = thirty_work_days(&donem);
            for d in 25..=30 {
                gunler.insert(format!("2026-05-{d:02}"), "R".to_string());
            }

            let puantaj = PersonelPuantaj {
                id: "p-rapor-test_2026-05".into(),
                personelId: "p-rapor-test".into(),
                donemId: "2026-05".into(),
                gunler,
            };
            AttendanceRepository::save(&conn, &puantaj)?;

            // Sick leave record: 2026-05-25 to 2026-05-30 (6 days episode) -> first 2 days paid by employer
            let sick = SickLeaveRecord {
                id: "sick_rapor_1".into(),
                personnelId: "p-rapor-test".into(),
                startDate: "2026-05-25".into(),
                endDate: "2026-05-30".into(),
                createdAt: None,
                updatedAt: None,
            };
            SickLeaveRepository::save(&conn, &sick)?;

            // Calculate payroll
            let calculated =
                PayrollService::calculate_payroll_for_personnel(&conn, "p-rapor-test", "2026-05")?;
            assert_eq!(calculated.raporluGun, Some(6));
            assert_eq!(calculated.odenenRaporluGun, Some(2));

            // Connection is closed when conn drops
            drop(conn);
        }

        // 2. Reopen same SQLite file from disk and verify persistence
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let payrolls = PayrollRepository::get_all(&conn)?;
            assert_eq!(payrolls.len(), 1);
            let restored = &payrolls[0];

            assert_eq!(
                restored.raporluGun,
                Some(6),
                "raporluGun must be 6 after reloading from SQLite"
            );
            assert_eq!(
                restored.odenenRaporluGun,
                Some(2),
                "odenenRaporluGun must be 2 after reloading from SQLite"
            );

            // Finalize payroll
            PayrollService::set_payroll_status(
                &conn,
                "p-rapor-test",
                "2026-05",
                BordroStatus::FINALIZED,
            )?;

            drop(conn);
        }

        // 3. Reopen again and verify FINALIZED payroll reload keeps exact values unchanged
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let payrolls = PayrollRepository::get_all(&conn)?;
            assert_eq!(payrolls.len(), 1);
            let finalized = &payrolls[0];

            assert_eq!(finalized.status, BordroStatus::FINALIZED);
            assert_eq!(
                finalized.raporluGun,
                Some(6),
                "FINALIZED payroll must keep raporluGun = 6"
            );
            assert_eq!(
                finalized.odenenRaporluGun,
                Some(2),
                "FINALIZED payroll must keep odenenRaporluGun = 2"
            );

            drop(conn);
        }

        // Cleanup
        let _ = fs::remove_file(&db_path);

        Ok(())
    }

    #[test]
    fn test_legacy_db_migration_payroll_records_nullable_columns(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use rusqlite::Connection;
        use rusqlite_migration::{Migrations, M};

        let mut conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // Run ONLY migration 1 (the initial schema before migration 2)
        let v1_migration = Migrations::new(vec![M::up(
            r#"
                CREATE TABLE IF NOT EXISTS personnel (
                    id TEXT PRIMARY KEY,
                    tc_no TEXT UNIQUE NOT NULL,
                    ad TEXT NOT NULL,
                    soyad TEXT NOT NULL,
                    grup TEXT NOT NULL,
                    unvan TEXT,
                    sgk_sicil_no TEXT NOT NULL DEFAULT '',
                    iban TEXT NOT NULL DEFAULT '',
                    hizmet_yili INTEGER NOT NULL DEFAULT 0,
                    aciklama TEXT,
                    sendika_uyesi INTEGER NOT NULL DEFAULT 0,
                    sabit_sendika_aidati INTEGER DEFAULT 0,
                    bes_uyesi INTEGER NOT NULL DEFAULT 0,
                    oks_orani_yuzde INTEGER DEFAULT 0,
                    sabit_bes_tutar INTEGER DEFAULT 0,
                    icra_tutar INTEGER DEFAULT 0,
                    kisi_borcu_tutar INTEGER DEFAULT 0,
                    dogum_askerlik_borclanmasi_tutar INTEGER DEFAULT 0,
                    hayat_saglik_sigortasi_tutar INTEGER DEFAULT 0,
                    diger_kesinti_tutar INTEGER DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS payroll_periods (
                    id TEXT PRIMARY KEY,
                    yil INTEGER NOT NULL,
                    ay INTEGER NOT NULL,
                    baslangic_tarihi TEXT NOT NULL,
                    bitis_tarihi TEXT NOT NULL,
                    donem_adi TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS attendance_records (
                    id TEXT PRIMARY KEY,
                    personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                    period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                    attendance_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    CONSTRAINT unique_personnel_period_attendance UNIQUE(personnel_id, period_id)
                );

                CREATE TABLE IF NOT EXISTS personnel_tax_opening (
                    id TEXT PRIMARY KEY,
                    personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                    year INTEGER NOT NULL,
                    gv_cumulative_opening INTEGER NOT NULL,
                    effective_from_period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    CONSTRAINT unique_personnel_tax_opening_year UNIQUE(personnel_id, year)
                );

                CREATE TABLE IF NOT EXISTS payroll_records (
                    id TEXT PRIMARY KEY,
                    personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                    period_id TEXT NOT NULL REFERENCES payroll_periods(id) ON DELETE CASCADE,
                    gross_total INTEGER NOT NULL,
                    sgk_base INTEGER NOT NULL,
                    gv_base INTEGER NOT NULL,
                    previous_cumulative_gv INTEGER NOT NULL,
                    new_cumulative_gv INTEGER NOT NULL,
                    income_tax INTEGER NOT NULL,
                    stamp_tax INTEGER NOT NULL,
                    total_deductions INTEGER NOT NULL,
                    net_payment INTEGER NOT NULL,
                    status TEXT NOT NULL DEFAULT 'CALCULATED',
                    puantaj_summary_json TEXT NOT NULL,
                    pek_detail_json TEXT,
                    devreden_pek_gelen_json TEXT,
                    sonraki_devreden_pek_json TEXT,
                    calculated_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    CONSTRAINT unique_personnel_period_payroll UNIQUE(personnel_id, period_id)
                );

                CREATE TABLE IF NOT EXISTS payroll_income_items (
                    id TEXT PRIMARY KEY,
                    payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
                    item_type TEXT NOT NULL,
                    description TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    source TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS payroll_deduction_items (
                    id TEXT PRIMARY KEY,
                    payroll_id TEXT NOT NULL REFERENCES payroll_records(id) ON DELETE CASCADE,
                    item_type TEXT NOT NULL,
                    description TEXT NOT NULL,
                    amount INTEGER NOT NULL,
                    source TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS institution_settings (
                    period_id TEXT PRIMARY KEY REFERENCES payroll_periods(id) ON DELETE CASCADE,
                    settings_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS annual_payroll_parameters (
                    year INTEGER PRIMARY KEY,
                    params_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS app_settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS sick_leave_records (
                    id TEXT PRIMARY KEY,
                    personnel_id TEXT NOT NULL REFERENCES personnel(id) ON DELETE CASCADE,
                    start_date TEXT NOT NULL,
                    end_date TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                "#,
        )]);
        v1_migration.to_latest(&mut conn)?;

        // Insert dummy personnel & period
        let person = setup_test_person("p-old");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-15".into(),
            bitisTarihi: "2026-02-14".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        // v1 (legacy) schema has no tax_year/tax_month columns yet: insert raw, legacy-style.
        // Full migrations (migration 5) will ALTER + backfill these columns afterwards.
        conn.execute(
            "INSERT INTO payroll_periods (id, yil, ay, baslangic_tarihi, bitis_tarihi, donem_adi, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                donem.id, donem.yil, donem.ay, donem.baslangicTarihi, donem.bitisTarihi, donem.donemAdi,
                "2026-01-15T00:00:00Z"
            ],
        )?;

        // Insert legacy record into payroll_records with old columns (no raporlu_gun / odenen_raporlu_gun)
        conn.execute(
            "INSERT INTO payroll_records (
                id, personnel_id, period_id, gross_total, sgk_base, gv_base, previous_cumulative_gv,
                new_cumulative_gv, income_tax, stamp_tax, total_deductions, net_payment, status,
                puantaj_summary_json, calculated_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            rusqlite::params![
                "p-old_2026-01",
                "p-old",
                "2026-01",
                5000000,
                5000000,
                4250000,
                0,
                4250000,
                637500,
                37950,
                1425450,
                3574550,
                "CALCULATED",
                "{}",
                "2026-01-15T00:00:00Z",
                "2026-01-15T00:00:00Z"
            ],
        )?;

        // Now run full migrations (applying migration 2 ALTER TABLE)
        let all_migrations = bordro_programi_lib::db::migrations::get_migrations();
        all_migrations.to_latest(&mut conn)?;

        // Read all payroll records
        let records = PayrollRepository::get_all(&conn)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].raporluGun, None);
        assert_eq!(records[0].odenenRaporluGun, None);

        // Now save a record with values
        let mut rec = records[0].clone();
        rec.raporluGun = Some(6);
        rec.odenenRaporluGun = Some(2);
        PayrollRepository::save(&conn, &rec)?;

        let updated = PayrollRepository::get_all(&conn)?;
        assert_eq!(updated[0].raporluGun, Some(6));
        assert_eq!(updated[0].odenenRaporluGun, Some(2));

        Ok(())
    }

    #[test]
    fn test_legacy_db_missing_sick_leave_table_is_repaired(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = create_in_memory_connection()?;

        // Simulate a database that reached migration 5 before the table was
        // added to the initial schema definition.
        conn.execute("DROP TABLE sick_leave_records", [])?;
        for column in [
            "devir_kumulatif_gv_matrahi",
            "devir_kumulatif_gv_matrahi_yili",
            "devir_kumulatif_gv_matrahi_baslangic_ayi",
            "devir_kumulatif_asgari_gv_matrahi",
            "devir_kumulatif_asgari_gv_matrahi_yili",
        ] {
            conn.execute(&format!("ALTER TABLE personnel DROP COLUMN {column}"), [])?;
        }
        conn.execute("ALTER TABLE payroll_records DROP COLUMN notlar", [])?;
        conn.pragma_update(None, "user_version", 5u32)?;

        bordro_programi_lib::db::migrations::initialize_db(&mut conn)?;

        let table_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'sick_leave_records'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(table_count, 1);

        Ok(())
    }

    #[test]
    fn test_pek_alt_sinir_tamamlama_isveren_prim_ayrimi() {
        use bordro_programi_lib::domain::calculations::{
            calculate_prime_esas_kazanc, calculate_statutory_deductions,
        };

        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30; // 30 prim günü -> Alt Sınır = 33.030 TL

        let kurum = DonemselKurumDegerleri::default(); // Gunluk asgari = 1101 TL, SGK isci = %14, Issizlik isci = %1, SGK isveren = %21.75, Issizlik isveren = %2

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(25000.00)), // Ham PEK = 25.000 TL
            ..Default::default()
        };

        // 1. PEK Hesaplaması
        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.hesaplananPek, dec!(25000.00)); // ham PEK
        assert_eq!(pek_detay.pekAltSinir, dec!(33030.00)); // 30 * 1101 = 33.030 TL
        assert_eq!(pek_detay.finalPek, dec!(33030.00)); // nihai PEK
        assert_eq!(pek_detay.altSinirTamamlamaFarki, dec!(8030.00)); // 33.030 - 25.000 = 8.030 TL

        // Normal İşveren Primi:
        // SGK İşveren (%21,75): 33.030 * %21,75 = 7.184,025 -> 7.184,03 TL (MidpointAwayFromZero)
        // İşsizlik İşveren (%2): 33.030 * %2 = 660,60 TL
        assert_eq!(pek_detay.isverenSgkPrimi, Some(dec!(7184.03)));
        assert_eq!(pek_detay.isverenIssizlikPrimi, Some(dec!(660.60)));

        // Alt Sınır Tamamlama İşveren Primi:
        // 8.030 * %14 (SGK işçi payı farkı) = 1.124,20 TL
        // 8.030 * %1 (İşsizlik işçi payı farkı) = 80,30 TL
        // Toplam Alt Sınır İşveren Yükü = 1.204,50 TL
        assert_eq!(
            pek_detay.pekAltSinirTamamlamaIsverenPrimi,
            Some(dec!(1204.50))
        );

        // Toplam İşveren Primi = 7.184,03 + 660,60 + 1.204,50 = 9.049,13 TL
        assert_eq!(pek_detay.isverenPrimToplami, Some(dec!(9049.13)));

        // 2. Yasal Kesintiler (İşçi Payları)
        let (kesintiler, _, _) = calculate_statutory_deductions(
            &gelirler,
            Some(&kurum),
            None,
            Some(&puantaj),
            dec!(0),
            &[],
            dec!(0),
        );

        // İşçi SGK kesintisi 33.030 üzerinden DEĞİL, 25.000 (ham PEK) üzerinden yapılmalı (5510 m.82)
        // 25.000 * %14 = 3.500,00 TL
        assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(3500.00)));

        // İşçi İşsizlik kesintisi 33.030 üzerinden DEĞİL, 25.000 (ham PEK) üzerinden yapılmalı (4447 m.49 / Model A)
        // 25.000 * %1 = 250,00 TL
        assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(250.00)));

        // 3. Toplam SGK Tahakkuk Bütünlüğü Kontrolü:
        // İşçi Kesintisi (3.500,00 SGK + 250,00 İşsizlik = 3.750,00 TL)
        // + Normal İşveren Primi (7.184,03 SGK + 660,60 İşsizlik = 7.844,63 TL)
        // + İşveren Alt Sınır Yükü (1.204,50 TL)
        // = 12.799,13 TL
        // SGK'ya Bildirilecek Toplam Prim: 33.030 * %38,75 (%14+%1+%21.75+%2) = 12.799,125 -> 12.799,13 TL
        let isci_toplam_prim =
            kesintiler.isciSgkPrimi.unwrap() + kesintiler.isciIssizlikPrimi.unwrap();
        let isveren_toplam_prim = pek_detay.isverenPrimToplami.unwrap();
        let genel_sgk_tahakkuk = isci_toplam_prim + isveren_toplam_prim;

        assert_eq!(genel_sgk_tahakkuk, dec!(12799.13));
    }

    #[test]
    fn test_sgk_resmi_2026_ornegi_midpoint_away_from_zero() {
        use bordro_programi_lib::domain::calculations::{
            calculate_prime_esas_kazanc, calculate_statutory_deductions,
        };
        use rust_decimal::RoundingStrategy;

        // Rounding policy: 33.030 × %21,75 = 7.184,025 -> 7.184,03 (MidpointAwayFromZero, SGK resmî örneği)
        let topla = dec!(33030.00) * (dec!(21.75) / dec!(100));
        assert_eq!(topla, dec!(7184.025));
        assert_eq!(
            topla.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero),
            dec!(7184.03)
        );

        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30; // 30 prim günü, PEK = 33.030 TL

        let kurum = DonemselKurumDegerleri::default();

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(33030.00)), // PEK = 33.030 TL
            ..Default::default()
        };

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);
        let (kesintiler, _, _) = calculate_statutory_deductions(
            &gelirler,
            Some(&kurum),
            None,
            Some(&puantaj),
            dec!(0),
            &[],
            dec!(0),
        );

        // İşveren SGK (%21,75): 33.030 × %21,75 = 7.184,025 -> 7.184,03 TL
        assert_eq!(pek_detay.isverenSgkPrimi, Some(dec!(7184.03)));
        // İşveren İşsizlik (%2): 33.030 × %2 = 660,60 TL
        assert_eq!(pek_detay.isverenIssizlikPrimi, Some(dec!(660.60)));

        // İşçi SGK (%14): 33.030 × %14 = 4.624,20 TL
        assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(4624.20)));
        // İşçi İşsizlik (%1): 33.030 × %1 = 330,30 TL
        assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(330.30)));

        // Toplam değerlendirme: %38,75 toplam tahakkuk = 12.799,13 TL
        let isci_toplam = kesintiler.isciSgkPrimi.unwrap() + kesintiler.isciIssizlikPrimi.unwrap();
        let isveren_toplam =
            pek_detay.isverenSgkPrimi.unwrap() + pek_detay.isverenIssizlikPrimi.unwrap();
        let toplam_tahakkuk = isci_toplam + isveren_toplam;
        assert_eq!(toplam_tahakkuk, dec!(12799.13));
    }

    #[test]
    fn test_pek_alt_sinir_ustunde_normal_bordro() {
        use bordro_programi_lib::domain::calculations::{
            calculate_prime_esas_kazanc, calculate_statutory_deductions,
        };

        let mut puantaj = PuantajOzeti::default();
        puantaj.c = 30;

        let kurum = DonemselKurumDegerleri::default();

        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(100000.00)), // Ham PEK = 100.000 TL (Alt sınırın üzerinde)
            ..Default::default()
        };

        let (pek_detay, _) =
            calculate_prime_esas_kazanc(&gelirler, Some(&puantaj), Some(&kurum), &[]);

        assert_eq!(pek_detay.hesaplananPek, dec!(100000.00));
        assert_eq!(pek_detay.finalPek, dec!(100000.00));
        assert_eq!(pek_detay.altSinirTamamlamaFarki, dec!(0.00));
        assert_eq!(pek_detay.pekAltSinirTamamlamaIsverenPrimi, Some(dec!(0.00)));

        let (kesintiler, _, _) = calculate_statutory_deductions(
            &gelirler,
            Some(&kurum),
            None,
            Some(&puantaj),
            dec!(0),
            &[],
            dec!(0),
        );

        // 100.000 * %14 = 14.000 TL
        assert_eq!(kesintiler.isciSgkPrimi, Some(dec!(14000.00)));
        // 100.000 * %1 = 1.000 TL
        assert_eq!(kesintiler.isciIssizlikPrimi, Some(dec!(1000.00)));
        // 100.000 * %21,75 = 21.750 TL
        assert_eq!(pek_detay.isverenSgkPrimi, Some(dec!(21750.00)));
        // 100.000 * %2 = 2.000 TL
        assert_eq!(pek_detay.isverenIssizlikPrimi, Some(dec!(2000.00)));
    }

    // ===== Test A: Ocak — asgari ücret GV istisnası = 4.211,33 TL + aylık matrah = 28.075,50 TL
    #[test]
    fn test_gv_istisna_ocak() {
        let aylik_asgari_gv_matrah =
            bordro_programi_lib::domain::calculations::calculate_aylik_asgari_ucret_gv_matrahi(
                dec!(1101.00),
                dec!(0.14),
                dec!(0.01),
            );
        assert_eq!(aylik_asgari_gv_matrah, dec!(28075.50));

        let det = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            dec!(28075.50),
            dec!(0),
            aylik_asgari_gv_matrah,
            dec!(0),
        );
        assert_eq!(det.asgariUcretGvMatrahi, dec!(28075.50));
        assert_eq!(det.asgariUcretReferansKumulatifMatrahi, dec!(28075.50));
        assert_eq!(det.asgariUcretGvIstisnasi, dec!(4211.33));
        // Ocak: brüt GV = 4211,33 olduğu için istisna tam kullanılır, kesilen = 0
        assert_eq!(det.uygulananGvIstisnasi, dec!(4211.33));
        assert_eq!(det.kesilenGelirVergisi, dec!(0));
    }

    // ===== Test B: Temmuz — referans kümülatif takvimden (Oca-Tem) gelir, istisna = 4.537,75
    #[test]
    fn test_gv_istisna_temmuz() {
        let aylik = dec!(28075.50);
        let ref_prev = aylik * dec!(6); // Ocak..Haziran
        let det = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            aylik,
            dec!(0),
            aylik,
            ref_prev,
        );
        assert_eq!(det.asgariUcretReferansKumulatifMatrahi, dec!(196528.50));
        assert_eq!(det.asgariUcretGvIstisnasi, dec!(4537.75));
    }

    // ===== Test C: Ağustos — istisna = 5.615,10 TL (takvim konumu arttıkça istisna büyür)
    #[test]
    fn test_gv_istisna_agustos() {
        let aylik = dec!(28075.50);
        let ref_prev = aylik * dec!(7); // Ocak..Temmuz
        let det = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            aylik,
            dec!(0),
            aylik,
            ref_prev,
        );
        assert_eq!(det.asgariUcretReferansKumulatifMatrahi, dec!(224604.00));
        assert_eq!(det.asgariUcretGvIstisnasi, dec!(5615.10));
    }

    // ===== Test D: Gerçek kümülatif aynı ay istisnayı değiştirmez; referans takvimden gelir
    #[test]
    fn test_gv_istisna_gercek_kumulatif_bagimsiz() {
        let aylik = dec!(28075.50);
        let ref_prev = aylik * dec!(0); // Ocak
        let det_low = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            dec!(65000),
            dec!(5000),
            aylik,
            ref_prev,
        );
        let det_high = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            dec!(65000),
            dec!(300000),
            aylik,
            ref_prev,
        );
        // İstisna, gerçek kümülatiften matematiksel olarak bağımsızdır (referans takvim matrahı):
        assert_eq!(
            det_low.asgariUcretGvIstisnasi,
            det_high.asgariUcretGvIstisnasi
        );
        assert!(det_low.brutGelirVergisi < det_high.brutGelirVergisi);
    }

    // ===== Test E: Gerçek kümülatif / devir — açılış 120.000 + 65.000 = 185.000, istisna Mayıs 4.211,33
    #[test]
    fn test_gv_istisna_devir_gercek_kumulatif() {
        let aylik = dec!(28075.50);
        // Mayıs takvim konumu 4 → önceki referans = 4 ay
        let ref_prev_def = aylik * dec!(4);
        let det = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            dec!(65000),
            dec!(120000),
            aylik,
            ref_prev_def,
        );
        assert_eq!(det.yeniKumulatifGvMatrahi, dec!(185000));
        assert_eq!(det.asgariUcretGvIstisnasi, dec!(4211.33));
    }

    // ===== Test F: brüt GV < istisna hakkı → uygulanan = brüt, kesilen = 0
    #[test]
    fn test_gv_istisna_brut_alti() {
        let aylik = dec!(28075.50);
        let det = bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi(
            dec!(10000),
            dec!(0),
            aylik,
            dec!(0),
        );
        // brüt GV = 10.000 * %15 = 1.500; istisna hakkı 4.211,33
        assert_eq!(det.brutGelirVergisi, dec!(1500));
        assert_eq!(det.uygulananGvIstisnasi, dec!(1500));
        assert_eq!(det.kesilenGelirVergisi, dec!(0));
    }

    // ===== Test G: Çoklu gelir kalemi — istisna BİR kez uygulanır
    #[test]
    fn test_gv_istisna_coklu_gelir_bir_kez() {
        use bordro_programi_lib::domain::calculations::calculate_statutory_deductions;
        let gelirler = GelirKalemleri {
            tabanBrutAylik: Some(dec!(50000)),
            isPrimi: Some(dec!(10000)),
            digerGelir: Some(dec!(5000)),
            ..Default::default()
        };
        let puantaj = PuantajOzeti {
            c: 30,
            ..Default::default()
        };
        let (kesintiler, _, _) = calculate_statutory_deductions(
            &gelirler,
            None,
            None,
            Some(&puantaj),
            dec!(0),
            &[],
            dec!(0),
        );
        // matrah = 65.000 - SGK(9.100) - işsizlik(650) = 55.250 → brüt GV 8.287,50
        // istisna (Ocak) 4.211,33 → kesilen = 8.287,50 - 4.211,33 = 4.076,17
        assert_eq!(kesintiler.gelirVergisi, Some(dec!(4076.17)));
    }

    // ===== Test H: FINALIZED bordronun GV snapshot'ı kaydedilip geri yüklenir, tutarı korunur
    #[test]
    fn test_gv_snapshot_finalized_persistence() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;
        let person = setup_test_person("test-gv");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-01".into(),
            yil: 2026,
            ay: 1,
            baslangicTarihi: "2026-01-01".into(),
            bitisTarihi: "2026-01-31".into(),
            donemAdi: "Ocak 2026".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &donem)?;

        let gv_detay = GvHesapDetayi {
            cariGvMatrahi: dec!(28075.50),
            yeniKumulatifGvMatrahi: dec!(28075.50),
            brutGelirVergisi: dec!(4211.33),
            asgariUcretGvMatrahi: dec!(28075.50),
            asgariUcretReferansKumulatifMatrahi: dec!(28075.50),
            asgariUcretGvIstisnasi: dec!(4211.33),
            uygulananGvIstisnasi: dec!(4211.33),
            kesilenGelirVergisi: dec!(0),
        };

        let kall = BordroKaydi {
            id: "test-gv_2026-01".into(),
            personelId: "test-gv".into(),
            donemId: "2026-01".into(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(dec!(28075.50)),
                ..Default::default()
            },
            gelirToplam: dec!(28075.50),
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: dec!(0),
            netOdeme: dec!(28075.50),
            status: BordroStatus::FINALIZED,
            olusturulmaTarihi: "".into(),
            sonGuncellemeTarihi: "".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: Some(dec!(0)),
            oncekiKumulatifAsgariGvMatrahi: Some(dec!(0)),
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: None,
            pekDetay: None,
            isPrimiDetay: None,
            gvDetay: Some(gv_detay),
            odenenRaporluGun: None,
            raporluGun: None,
        };
        PayrollRepository::save(&conn, &kall)?;

        let reloaded = PayrollRepository::get_all(&conn)?
            .into_iter()
            .find(|b| b.donemId == "2026-01")
            .expect("kayit yok");
        let gv = reloaded.gvDetay.expect("gvDetay persist edilmedi");
        assert_eq!(gv.brutGelirVergisi, dec!(4211.33));
        assert_eq!(gv.asgariUcretGvIstisnasi, dec!(4211.33));
        assert_eq!(gv.kesilenGelirVergisi, dec!(0));
        assert_eq!(gv.yeniKumulatifGvMatrahi, dec!(28075.50));
        Ok(())
    }

    // ===== Kabul kriteri 3: 15.06–14.07 dönemi taxMonth=7 → 196.528,50 / 4.537,75;
    //     taxMonth=6 → 168.453,00 / 4.211,33 (regresyon)
    #[test]
    fn test_gv_istisna_tax_month_accept_3() {
        use bordro_programi_lib::domain::calculations::calculate_gv_hesap_detayi;
        let aylik = dec!(28075.50);

        // 15.06–14.07 dönemi, taxMonth=7: referans kümülatif = 7 × 28.075,50, istisna Temmuz = 4.537,75
        let d7 = calculate_gv_hesap_detayi(aylik, dec!(0), aylik, aylik * dec!(6));
        assert_eq!(d7.asgariUcretReferansKumulatifMatrahi, dec!(196528.50));
        assert_eq!(d7.asgariUcretGvIstisnasi, dec!(4537.75));

        // taxMonth=6 (eski davranış) referans = 6 × 28.075,50, istisna Haziran = 4.211,33
        let d6 = calculate_gv_hesap_detayi(aylik, dec!(0), aylik, aylik * dec!(5));
        assert_eq!(d6.asgariUcretReferansKumulatifMatrahi, dec!(168453.00));
        assert_eq!(d6.asgariUcretGvIstisnasi, dec!(4211.33));
    }

    // ===== Kabul kriteri 3 (servis katmanı): referans kümülatif vergi yılı/ayından gelir
    #[test]
    fn test_previous_asgari_gv_uses_tax_year_month() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        // Aralık 2025 dönemi takvim referansı 2026 vergi yılına aittir (taxYear=2026, taxMonth=1).
        let aralik2025 = BordroDonemi {
            id: "2025-12".into(),
            yil: 2025,
            ay: 12,
            baslangicTarihi: "2025-12-15".into(),
            bitisTarihi: "2026-01-14".into(),
            donemAdi: "Aralık 2025".into(),
            taxYear: 2026,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &aralik2025)?;

        // Ocak..Mayıs 2026 → taxMonth 2..6
        for m in 1..=5 {
            let donem = BordroDonemi {
                id: format!("2026-{:02}", m),
                yil: 2026,
                ay: m,
                baslangicTarihi: format!("2026-{:02}-15", m),
                bitisTarihi: format!("2026-{:02}-14", m + 1),
                donemAdi: format!("{} 2026", m),
                taxYear: 2026,
                taxMonth: m + 1,
            };
            PeriodRepository::save(&conn, &donem)?;
        }

        // Aktif dönem: 15.06–14.07 (ay=6). Vergi yılı/ayı seçimine göre referans kümülatif:
        //  - taxMonth=7 → önceki takvim ayları taxYear=2026 ve taxMonth<7:
        //    Aralık(1) + Ocak..Mayıs(2..6) = 6 ay = 6 × 28.075,50
        //  - taxMonth=6 → Aralık(1) + Ocak..Nisan(2..5) = 5 ay = 5 × 28.075,50
        let aktif_tax7 = BordroDonemi {
            id: "2026-06".into(),
            yil: 2026,
            ay: 6,
            baslangicTarihi: "2026-06-15".into(),
            bitisTarihi: "2026-07-14".into(),
            donemAdi: "Haziran 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };
        let prev_7 =
            CumulativeTaxService::get_previous_cumulative_asgari_gv(&conn, "x", &aktif_tax7)?;
        assert_eq!(prev_7, dec!(168453.00)); // 196.528,50 − 28.075,50

        let aktif_tax6 = BordroDonemi {
            taxYear: 2026,
            taxMonth: 6,
            ..aktif_tax7
        };
        let prev_6 =
            CumulativeTaxService::get_previous_cumulative_asgari_gv(&conn, "x", &aktif_tax6)?;
        assert_eq!(prev_6, dec!(140377.50)); // 168.453,00 − 28.075,50 (Aralık + Ocak..Nisan = 5 ay)

        Ok(())
    }

    // ===== Test A: Yıl geçişi — 15.12.2026–14.01.2027 (taxYear=2027/taxMonth=1)
    //     2026 vergi yılı kümülatifi 2027'ye taşınmamalı.
    #[test]
    fn test_a_yil_gecisi_2026_kumulatif_tasinmaz() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-yil-gecis");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2027,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &aralik2026)?;

        // 2026 vergi yılı açılışı 300.000 TL → 2027 dönemine taşınmamalı
        let opening2026 = PersonelTaxOpening {
            id: "opt_2026".into(),
            personnelId: "test-yil-gecis".into(),
            year: 2026,
            gvCumulativeOpening: dec!(300000),
            effectiveFromPeriodId: "2026-12".into(),
            createdAt: None,
            updatedAt: None,
        };
        TaxOpeningRepository::save(&conn, &opening2026)?;

        let prev =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-yil-gecis", &aralik2026)?;
        assert_eq!(prev, dec!(0));

        Ok(())
    }

    // ===== Test B: 2027 opening varsa 2027 açılışı kullanılır
    #[test]
    fn test_b_2027_opening_kullanilir() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-2027-opening");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2027,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &aralik2026)?;

        // 2027 açılışı 120.000 TL, effective_from aynı Aralık dönemi (taxYear=2027, taxMonth=1)
        let opening2027 = PersonelTaxOpening {
            id: "opt_2027".into(),
            personnelId: "test-2027-opening".into(),
            year: 2027,
            gvCumulativeOpening: dec!(120000),
            effectiveFromPeriodId: "2026-12".into(),
            createdAt: None,
            updatedAt: None,
        };
        TaxOpeningRepository::save(&conn, &opening2027)?;

        let prev = CumulativeTaxService::get_previous_cumulative_gv(
            &conn,
            "test-2027-opening",
            &aralik2026,
        )?;
        assert_eq!(prev, dec!(120000));

        Ok(())
    }

    // ===== Test C: Aynı vergi yılında taxMonth sıralaması authoritative
    #[test]
    fn test_c_ayni_vergi_yili_tax_month_siralamasi() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-tax-month");
        PersonnelRepository::save(&conn, &person)?;

        let tm1 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };
        let tm2 = BordroDonemi {
            id: "2027-02".into(),
            yil: 2027,
            ay: 2,
            baslangicTarihi: "2027-02-15".into(),
            bitisTarihi: "2027-03-14".into(),
            donemAdi: "Şubat 2027".into(),
            taxYear: 2027,
            taxMonth: 2,
        };
        let tm3 = BordroDonemi {
            id: "2027-03".into(),
            yil: 2027,
            ay: 3,
            baslangicTarihi: "2027-03-15".into(),
            bitisTarihi: "2027-04-14".into(),
            donemAdi: "Mart 2027".into(),
            taxYear: 2027,
            taxMonth: 3,
        };
        PeriodRepository::save(&conn, &tm1)?;
        PeriodRepository::save(&conn, &tm2)?;
        PeriodRepository::save(&conn, &tm3)?;

        PayrollRepository::save(
            &conn,
            &bordro_kaydi("test-tax-month", "2027-01", dec!(50000)),
        )?;
        PayrollRepository::save(
            &conn,
            &bordro_kaydi("test-tax-month", "2027-02", dec!(60000)),
        )?;

        // taxMonth 3'ün önceki kümülatifi = 50.000 + 60.000 = 110.000
        let prev = CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-tax-month", &tm3)?;
        assert_eq!(prev, dec!(110000));

        Ok(())
    }

    // ===== Test D: çalışma ayı farklı olsa bile taxMonth authoritative
    //     ay=12, taxYear=2027, taxMonth=1 → geçmiş araması 2027/1 semantiği kullanır.
    #[test]
    fn test_d_calisma_ayi_12_tax_month_authoritative() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-cal-12");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2027,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &aralik2026)?;

        // 2026 vergi yılına ait gerçek bordro (taxMonth=12) — 2027/1 dönemine sayılmamalı
        let kasim2026 = BordroDonemi {
            id: "2026-11".into(),
            yil: 2026,
            ay: 11,
            baslangicTarihi: "2026-11-15".into(),
            bitisTarihi: "2026-12-14".into(),
            donemAdi: "Kasım 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };
        PeriodRepository::save(&conn, &kasim2026)?;
        PayrollRepository::save(&conn, &bordro_kaydi("test-cal-12", "2026-11", dec!(40000)))?;

        let prev =
            CumulativeTaxService::get_previous_cumulative_gv(&conn, "test-cal-12", &aralik2026)?;
        assert_eq!(prev, dec!(0));

        Ok(())
    }

    // ===== Test E: FINALIZED bordro varken taxMonth değişikliği ERROR
    #[test]
    fn test_e_finalized_payroll_metadata_locked() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::services::period_service::PeriodService;

        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-meta-finalized");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-07".into(),
            yil: 2026,
            ay: 7,
            baslangicTarihi: "2026-07-15".into(),
            bitisTarihi: "2026-08-14".into(),
            donemAdi: "Temmuz 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };
        PeriodRepository::save(&conn, &donem)?;

        let bordro = bordro_kaydi("test-meta-finalized", "2026-07", dec!(0));
        let bordro = BordroKaydi {
            status: BordroStatus::FINALIZED,
            ..bordro
        };
        PayrollRepository::save(&conn, &bordro)?;

        // taxMonth 7 → 6 değişikliği ERROR olmalı
        let changed = BordroDonemi {
            taxMonth: 6,
            ..donem
        };
        let res = PeriodService::save_period(&conn, &changed);
        assert!(matches!(res, Err(DomainError::ValidationError(_))));

        // DB'deki period taxYear/taxMonth değişmemeli
        let stored = PeriodRepository::get_by_id(&conn, "2026-07")?.unwrap();
        assert_eq!(stored.taxYear, 2026);
        assert_eq!(stored.taxMonth, 7);

        Ok(())
    }

    // ===== Test F: CALCULATED bordro varken taxMonth değişikliği ERROR
    #[test]
    fn test_f_calculated_payroll_metadata_locked() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::services::period_service::PeriodService;

        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-meta-calculated");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-07".into(),
            yil: 2026,
            ay: 7,
            baslangicTarihi: "2026-07-15".into(),
            bitisTarihi: "2026-08-14".into(),
            donemAdi: "Temmuz 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };
        PeriodRepository::save(&conn, &donem)?;

        let bordro = bordro_kaydi("test-meta-calculated", "2026-07", dec!(0));
        PayrollRepository::save(&conn, &bordro)?;

        let changed = BordroDonemi {
            taxMonth: 6,
            ..donem
        };
        let res = PeriodService::save_period(&conn, &changed);
        assert!(matches!(res, Err(DomainError::ValidationError(_))));

        let stored = PeriodRepository::get_by_id(&conn, "2026-07")?.unwrap();
        assert_eq!(stored.taxMonth, 7);

        Ok(())
    }

    // ===== Test G: Bordrosuz dönemde taxYear/taxMonth değişikliği başarılı
    #[test]
    fn test_g_no_payroll_tax_update_allowed() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::services::period_service::PeriodService;

        let conn = create_in_memory_connection()?;

        let donem = BordroDonemi {
            id: "2026-07".into(),
            yil: 2026,
            ay: 7,
            baslangicTarihi: "2026-07-15".into(),
            bitisTarihi: "2026-08-14".into(),
            donemAdi: "Temmuz 2026".into(),
            taxYear: 2026,
            taxMonth: 7,
        };
        PeriodRepository::save(&conn, &donem)?;

        // Bordro kaydı yok → taxYear/taxMonth güncellemesi başarılı
        let changed = BordroDonemi {
            taxYear: 2026,
            taxMonth: 8,
            ..donem
        };
        assert!(PeriodService::save_period(&conn, &changed).is_ok());

        let stored = PeriodRepository::get_by_id(&conn, "2026-07")?.unwrap();
        assert_eq!(stored.taxYear, 2026);
        assert_eq!(stored.taxMonth, 8);

        Ok(())
    }

    // ===== Test H: GV snapshot save → DB close/open → kümülatif/istisna alanları aynı
    //     oncekiKumulatifAsgariGvMatrahi snapshot'tan kayıpsız geri türetilir.
    #[test]
    fn test_h_gv_snapshot_reload_kumulatif_korunur() -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::db::create_connection;
        use std::fs;
        use std::path::PathBuf;

        let temp_dir = std::env::temp_dir();
        let db_path: PathBuf = temp_dir.join(format!(
            "gv_snapshot_reload_{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(54321)
        ));
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        // 1. Kaydet
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let person = setup_test_person("p-gv-reload");
            PersonnelRepository::save(&conn, &person)?;

            let donem = BordroDonemi {
                id: "2026-05".into(),
                yil: 2026,
                ay: 5,
                baslangicTarihi: "2026-05-15".into(),
                bitisTarihi: "2026-06-14".into(),
                donemAdi: "Mayıs 2026".into(),
                taxYear: 2026,
                taxMonth: 6,
            };
            PeriodRepository::save(&conn, &donem)?;

            let gv_detay = GvHesapDetayi {
                cariGvMatrahi: dec!(28075.50),
                yeniKumulatifGvMatrahi: dec!(336906.00),
                brutGelirVergisi: dec!(4211.33),
                asgariUcretGvMatrahi: dec!(28075.50),
                asgariUcretReferansKumulatifMatrahi: dec!(168453.00),
                asgariUcretGvIstisnasi: dec!(4211.33),
                uygulananGvIstisnasi: dec!(4211.33),
                kesilenGelirVergisi: dec!(0),
            };

            let bordro = BordroKaydi {
                id: "p-gv-reload_2026-05".into(),
                personelId: "p-gv-reload".into(),
                donemId: "2026-05".into(),
                puantajOzeti: PuantajOzeti::default(),
                gelirler: GelirKalemleri {
                    tabanBrutAylik: Some(dec!(28075.50)),
                    ..Default::default()
                },
                gelirToplam: dec!(28075.50),
                kesintiler: KesintiKalemleri::default(),
                kesintiToplam: dec!(0),
                netOdeme: dec!(28075.50),
                status: BordroStatus::FINALIZED,
                olusturulmaTarihi: "".into(),
                sonGuncellemeTarihi: "".into(),
                notlar: None,
                oncekiKumulatifGvMatrahi: Some(dec!(0)),
                oncekiKumulatifAsgariGvMatrahi: Some(dec!(140377.50)),
                manuelKumulatifGvMatrahi: None,
                devredenPekGelen: None,
                sonrakiDevredenPek: None,
                pekDetay: None,
                isPrimiDetay: None,
                gvDetay: Some(gv_detay),
                odenenRaporluGun: None,
                raporluGun: None,
            };
            PayrollRepository::save(&conn, &bordro)?;

            drop(conn);
        }

        // 2. DB close/open sonrası alanlar aynı olmalı
        {
            let conn = create_connection(Some(db_path.clone()))?;
            let payrolls = PayrollRepository::get_all(&conn)?;
            assert_eq!(payrolls.len(), 1);
            let restored = &payrolls[0];

            let gv = restored.gvDetay.as_ref().expect("gvDetay persist edilmedi");
            assert_eq!(gv.brutGelirVergisi, dec!(4211.33));
            assert_eq!(gv.asgariUcretGvIstisnasi, dec!(4211.33));
            assert_eq!(gv.kesilenGelirVergisi, dec!(0));
            assert_eq!(gv.asgariUcretReferansKumulatifMatrahi, dec!(168453.00));

            // oncekiKumulatifAsgariGvMatrahi = referans - cari aylık matrah (kayıpsız)
            assert_eq!(
                restored.oncekiKumulatifAsgariGvMatrahi,
                Some(dec!(140377.50))
            );

            drop(conn);
        }

        let _ = fs::remove_file(&db_path);
        Ok(())
    }

    fn bordro_kaydi(
        personel_id: &str,
        donem_id: &str,
        gelir_toplam: rust_decimal::Decimal,
    ) -> BordroKaydi {
        BordroKaydi {
            id: format!("{}_{}", personel_id, donem_id),
            personelId: personel_id.to_string(),
            donemId: donem_id.to_string(),
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri {
                tabanBrutAylik: Some(gelir_toplam),
                ..Default::default()
            },
            gelirToplam: gelir_toplam,
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: dec!(0),
            netOdeme: gelir_toplam,
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
            isPrimiDetay: None,
            gvDetay: None,
            odenenRaporluGun: None,
            raporluGun: None,
        }
    }

    // ==========================================
    // Devreden PEK Yıl Geçişi Tests (5510 m.80/d)
    // ==========================================

    fn make_test_kurum_degerleri(
        donem_id: &str,
        gunluk_taban_ucret: Decimal,
        ek_odeme: Decimal,
        gunluk_asgari_ucret: Decimal,
        pek_tavan_katsayisi: Decimal,
    ) -> DonemselKurumDegerleri {
        DonemselKurumDegerleri {
            donemId: donem_id.to_string(),
            gunlukTabanUcret: gunluk_taban_ucret,
            gunlukYemek: dec!(0),
            birlestirilmisSosyalYardim: dec!(0),
            gunlukVasitaYol: dec!(0),
            giyimYardimi: dec!(0),
            hizmetZammiBirimi: dec!(0),
            isPrimiGruplari: Some(vec![
                IsPrimiGrupItem {
                    id: "g1".into(),
                    ad: "1. Grup".into(),
                    oran: dec!(0),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "g2".into(),
                    ad: "2. Grup".into(),
                    oran: dec!(0),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "g3".into(),
                    ad: "3. Grup".into(),
                    oran: dec!(0),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "g4".into(),
                    ad: "4. Grup".into(),
                    oran: dec!(0),
                    aktif: true,
                },
                IsPrimiGrupItem {
                    id: "g5".into(),
                    ad: "5. Grup".into(),
                    oran: dec!(0),
                    aktif: true,
                },
            ]),
            ekOdeme: Some(ek_odeme),
            gunlukAsgariUcret: Some(gunluk_asgari_ucret),
            pekTavanKatsayisi: Some(pek_tavan_katsayisi),
            ..Default::default()
        }
    }

    #[test]
    fn test_devreden_pek_aralik_ocak_gecisi() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;
        ensure_test_annual_parameters(&conn, &[2027])?;

        let person = setup_test_person("p-pek-a");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };

        let ocak2027 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        PeriodRepository::save(&conn, &aralik2026)?;
        PeriodRepository::save(&conn, &ocak2027)?;

        let aralik_settings = make_test_kurum_degerleri(
            "2026-12",
            dec!(3333.33333333),
            dec!(250000.00),
            dec!(1101.00),
            dec!(9.00),
        );
        SettingsRepository::save_institution_settings(&conn, &aralik_settings)?;

        let ocak_settings = make_test_kurum_degerleri(
            "2027-01",
            dec!(3333.33333333),
            dec!(0),
            dec!(1101.00),
            dec!(9.00),
        );
        SettingsRepository::save_institution_settings(&conn, &ocak_settings)?;

        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-a_2026-12".into(),
                personelId: "p-pek-a".into(),
                donemId: "2026-12".into(),
                gunler: thirty_work_days(&aralik2026),
            },
        )?;

        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-a_2027-01".into(),
                personelId: "p-pek-a".into(),
                donemId: "2027-01".into(),
                gunler: thirty_work_days(&ocak2027),
            },
        )?;

        let bordro_aralik =
            PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-a", "2026-12")?;
        let sonraki_aralik = bordro_aralik.sonrakiDevredenPek.as_ref().unwrap();
        assert_eq!(sonraki_aralik.len(), 1);
        assert_eq!(sonraki_aralik[0].tutar, dec!(52730.00));
        assert_eq!(sonraki_aralik[0].kalanAySayisi, 2);

        let bordro_ocak =
            PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-a", "2027-01")?;
        let gelen_ocak = bordro_ocak.devredenPekGelen.as_ref().unwrap();
        assert_eq!(gelen_ocak.len(), 1);
        assert_eq!(gelen_ocak[0].tutar, dec!(52730.00));

        let pek_ocak = bordro_ocak.pekDetay.as_ref().unwrap();
        assert_eq!(pek_ocak.finalPek, dec!(152730.00));

        let sonraki_ocak = bordro_ocak.sonrakiDevredenPek.as_ref().unwrap();
        assert!(sonraki_ocak.is_empty());

        Ok(())
    }

    #[test]
    fn test_devreden_pek_aralik_ocak_subat_iki_ay_omur() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;
        ensure_test_annual_parameters(&conn, &[2027])?;

        let person = setup_test_person("p-pek-b");
        PersonnelRepository::save(&conn, &person)?;

        let periods = vec![
            BordroDonemi {
                id: "2026-12".into(),
                yil: 2026,
                ay: 12,
                baslangicTarihi: "2026-12-15".into(),
                bitisTarihi: "2027-01-14".into(),
                donemAdi: "Aralık 2026".into(),
                taxYear: 2026,
                taxMonth: 12,
            },
            BordroDonemi {
                id: "2027-01".into(),
                yil: 2027,
                ay: 1,
                baslangicTarihi: "2027-01-15".into(),
                bitisTarihi: "2027-02-14".into(),
                donemAdi: "Ocak 2027".into(),
                taxYear: 2027,
                taxMonth: 1,
            },
            BordroDonemi {
                id: "2027-02".into(),
                yil: 2027,
                ay: 2,
                baslangicTarihi: "2027-02-15".into(),
                bitisTarihi: "2027-03-14".into(),
                donemAdi: "Şubat 2027".into(),
                taxYear: 2027,
                taxMonth: 2,
            },
            BordroDonemi {
                id: "2027-03".into(),
                yil: 2027,
                ay: 3,
                baslangicTarihi: "2027-03-15".into(),
                bitisTarihi: "2027-04-14".into(),
                donemAdi: "Mart 2027".into(),
                taxYear: 2027,
                taxMonth: 3,
            },
        ];
        for p in &periods {
            PeriodRepository::save(&conn, p)?;
        }

        for p in &periods {
            AttendanceRepository::save(
                &conn,
                &PersonelPuantaj {
                    id: format!("p-pek-b_{}", p.id),
                    personelId: "p-pek-b".into(),
                    donemId: p.id.clone(),
                    gunler: thirty_work_days(p),
                },
            )?;
        }

        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-12",
                dec!(8333.33333333),
                dec!(100000.00),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2027-01",
                dec!(9333.33333333),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2027-02",
                dec!(9500.00),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2027-03",
                dec!(6666.66666667),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;

        let b1 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-b", "2026-12")?;
        assert_eq!(
            b1.sonrakiDevredenPek.as_ref().unwrap()[0].tutar,
            dec!(52730.00)
        );
        assert_eq!(b1.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 2);

        let b2 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-b", "2027-01")?;
        assert_eq!(
            b2.devredenPekGelen.as_ref().unwrap()[0].tutar,
            dec!(52730.00)
        );
        assert_eq!(b2.pekDetay.as_ref().unwrap().finalPek, dec!(297270.00));
        assert_eq!(
            b2.sonrakiDevredenPek.as_ref().unwrap()[0].tutar,
            dec!(35460.00)
        );
        assert_eq!(b2.sonrakiDevredenPek.as_ref().unwrap()[0].kalanAySayisi, 1);

        let b3 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-b", "2027-02")?;
        assert_eq!(
            b3.devredenPekGelen.as_ref().unwrap()[0].tutar,
            dec!(35460.00)
        );
        assert_eq!(b3.pekDetay.as_ref().unwrap().finalPek, dec!(297270.00));
        assert!(b3.sonrakiDevredenPek.as_ref().unwrap().is_empty());

        let b4 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-b", "2027-03")?;
        assert!(b4.devredenPekGelen.as_ref().unwrap().is_empty());
        assert_eq!(b4.pekDetay.as_ref().unwrap().finalPek, dec!(200000.00));

        Ok(())
    }

    #[test]
    fn test_devreden_pek_yeni_yil_ocak_tavani() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;
        ensure_test_annual_parameters(&conn, &[2027])?;

        let person = setup_test_person("p-pek-c");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2026,
            taxMonth: 12,
        };
        let ocak2027 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 1,
        };
        PeriodRepository::save(&conn, &aralik2026)?;
        PeriodRepository::save(&conn, &ocak2027)?;

        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-c_2026-12".into(),
                personelId: "p-pek-c".into(),
                donemId: "2026-12".into(),
                gunler: thirty_work_days(&aralik2026),
            },
        )?;
        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-c_2027-01".into(),
                personelId: "p-pek-c".into(),
                donemId: "2027-01".into(),
                gunler: thirty_work_days(&ocak2027),
            },
        )?;

        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-12",
                dec!(6666.66666667),
                dec!(120000.00),
                dec!(1000.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2027-01",
                dec!(10000.00),
                dec!(0),
                dec!(1200.00),
                dec!(9.00),
            ),
        )?;

        PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-c", "2026-12")?;
        let b_ocak = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-c", "2027-01")?;

        let pek = b_ocak.pekDetay.as_ref().unwrap();
        assert_eq!(pek.finalPek, dec!(324000.00));

        let sonraki = b_ocak.sonrakiDevredenPek.as_ref().unwrap();
        assert_eq!(sonraki.len(), 1);
        assert_eq!(sonraki[0].tutar, dec!(26000.00));
        assert_eq!(sonraki[0].kalanAySayisi, 1);

        Ok(())
    }

    #[test]
    fn test_devreden_pek_yil_ici_haziran_temmuz_agustos() -> Result<(), Box<dyn std::error::Error>>
    {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("p-pek-d");
        PersonnelRepository::save(&conn, &person)?;

        let periods = vec![
            BordroDonemi {
                id: "2026-06".into(),
                yil: 2026,
                ay: 6,
                baslangicTarihi: "2026-06-15".into(),
                bitisTarihi: "2026-07-14".into(),
                donemAdi: "Haziran 2026".into(),
                taxYear: 2026,
                taxMonth: 7,
            },
            BordroDonemi {
                id: "2026-07".into(),
                yil: 2026,
                ay: 7,
                baslangicTarihi: "2026-07-15".into(),
                bitisTarihi: "2026-08-14".into(),
                donemAdi: "Temmuz 2026".into(),
                taxYear: 2026,
                taxMonth: 8,
            },
            BordroDonemi {
                id: "2026-08".into(),
                yil: 2026,
                ay: 8,
                baslangicTarihi: "2026-08-15".into(),
                bitisTarihi: "2026-09-14".into(),
                donemAdi: "Ağustos 2026".into(),
                taxYear: 2026,
                taxMonth: 9,
            },
        ];
        for p in &periods {
            PeriodRepository::save(&conn, p)?;
        }

        for p in &periods {
            AttendanceRepository::save(
                &conn,
                &PersonelPuantaj {
                    id: format!("p-pek-d_{}", p.id),
                    personelId: "p-pek-d".into(),
                    donemId: p.id.clone(),
                    gunler: thirty_work_days(p),
                },
            )?;
        }

        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-06",
                dec!(8333.33333333),
                dec!(100000.00),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-07",
                dec!(9333.33333333),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-08",
                dec!(9500.00),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;

        let b1 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-d", "2026-06")?;
        assert_eq!(
            b1.sonrakiDevredenPek.as_ref().unwrap()[0].tutar,
            dec!(52730.00)
        );

        let b2 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-d", "2026-07")?;
        assert_eq!(
            b2.devredenPekGelen.as_ref().unwrap()[0].tutar,
            dec!(52730.00)
        );
        assert_eq!(
            b2.sonrakiDevredenPek.as_ref().unwrap()[0].tutar,
            dec!(35460.00)
        );

        let b3 = PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-d", "2026-08")?;
        assert_eq!(
            b3.devredenPekGelen.as_ref().unwrap()[0].tutar,
            dec!(35460.00)
        );
        assert!(b3.sonrakiDevredenPek.as_ref().unwrap().is_empty());

        Ok(())
    }

    #[test]
    fn test_devreden_pek_tax_year_bagimsizligi() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;
        ensure_test_annual_parameters(&conn, &[2027])?;

        let person = setup_test_person("p-pek-e");
        PersonnelRepository::save(&conn, &person)?;

        let aralik2026 = BordroDonemi {
            id: "2026-12".into(),
            yil: 2026,
            ay: 12,
            baslangicTarihi: "2026-12-15".into(),
            bitisTarihi: "2027-01-14".into(),
            donemAdi: "Aralık 2026".into(),
            taxYear: 2027,
            taxMonth: 1,
        };

        let ocak2027 = BordroDonemi {
            id: "2027-01".into(),
            yil: 2027,
            ay: 1,
            baslangicTarihi: "2027-01-15".into(),
            bitisTarihi: "2027-02-14".into(),
            donemAdi: "Ocak 2027".into(),
            taxYear: 2027,
            taxMonth: 2,
        };

        PeriodRepository::save(&conn, &aralik2026)?;
        PeriodRepository::save(&conn, &ocak2027)?;

        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-e_2026-12".into(),
                personelId: "p-pek-e".into(),
                donemId: "2026-12".into(),
                gunler: thirty_work_days(&aralik2026),
            },
        )?;
        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "p-pek-e_2027-01".into(),
                personelId: "p-pek-e".into(),
                donemId: "2027-01".into(),
                gunler: thirty_work_days(&ocak2027),
            },
        )?;

        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2026-12",
                dec!(3333.33333333),
                dec!(250000.00),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;
        SettingsRepository::save_institution_settings(
            &conn,
            &make_test_kurum_degerleri(
                "2027-01",
                dec!(3333.33333333),
                dec!(0),
                dec!(1101.00),
                dec!(9.00),
            ),
        )?;

        PayrollService::calculate_payroll_for_personnel(&conn, "p-pek-e", "2026-12")?;

        let devreden_gelen =
            PayrollService::calculate_incoming_devreden_pek(&conn, "p-pek-e", &ocak2027)?;

        assert_eq!(devreden_gelen.len(), 1);
        assert_eq!(devreden_gelen[0].tutar, dec!(52730.00));
        assert_eq!(devreden_gelen[0].kalanAySayisi, 2);

        Ok(())
    }

    // ==========================================
    // PUANTAJ (ATTENDANCE) SOURCE-OF-TRUTH REGRESSION TESTS A-D
    // Zincir: UI edit -> save_attendance -> attendance_records
    //         -> get_by_personnel_and_period -> calculate_payroll_for_personnel.
    // Invariant: "Puantaj Girildi" rozeti ile hesaplama katmanı AYNI
    // (personnel_id, period_id) anahtarını kullanmalıdır.
    // ==========================================

    // Test A — save -> calculate: yeni personel + dönem oluştur, puantaj kaydet,
    // aynı personnel_id + period_id ile hesapla -> attendance bulunur, hesaplama başlar.
    #[test]
    fn test_a_save_attendance_then_calculate_same_personnel_period(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-att-a");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
            taxYear: 2026,
            taxMonth: 6,
        };
        PeriodRepository::save(&conn, &donem)?;
        ensure_test_institution_settings(&conn, &["2026-05"])?;

        let gunler = thirty_work_days(&donem);
        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "test-att-a_2026-05".into(),
                personelId: "test-att-a".into(),
                donemId: "2026-05".into(),
                gunler,
            },
        )?;

        // Aynı personnel_id + period_id ile bordro hesaplama puantajı BULMALI.
        let bordro =
            PayrollService::calculate_payroll_for_personnel(&conn, "test-att-a", "2026-05")?;
        assert_eq!(bordro.personelId, "test-att-a");
        assert_eq!(bordro.donemId, "2026-05");
        assert_eq!(bordro.puantajOzeti.c, 30, "30 gün Ç özetlenmeli");

        Ok(())
    }

    // Test B — reconnect: puantajı kaydet, DB kapat/aç, aynı dönem/personel için
    // hesapla -> attendance hâlâ bulunur (frontend memory yanılsamasını engeller).
    #[test]
    fn test_b_attendance_survives_db_close_reopen_and_calculation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use bordro_programi_lib::db::create_connection;
        use std::fs;
        use std::path::PathBuf;

        let temp_dir = std::env::temp_dir();
        let db_path: PathBuf = temp_dir.join(format!(
            "att_reconnect_{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(11111)
        ));
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        // 1. Puantajı kaydet, sonra bağlantıyı kapat (uygulama kapanışı).
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let person = setup_test_person("test-att-b");
            PersonnelRepository::save(&conn, &person)?;

            let donem = BordroDonemi {
                id: "2026-06".into(),
                yil: 2026,
                ay: 6,
                baslangicTarihi: "2026-06-15".into(),
                bitisTarihi: "2026-07-14".into(),
                donemAdi: "Haziran 2026".into(),
                taxYear: 2026,
                taxMonth: 7,
            };
            PeriodRepository::save(&conn, &donem)?;
            ensure_test_institution_settings(&conn, &["2026-06"])?;

            let gunler = thirty_work_days(&donem);
            AttendanceRepository::save(
                &conn,
                &PersonelPuantaj {
                    id: "test-att-b_2026-06".into(),
                    personelId: "test-att-b".into(),
                    donemId: "2026-06".into(),
                    gunler,
                },
            )?;

            drop(conn);
        }

        // 2. DB yeniden açılışı: attendance hâlâ mevcut ve hesaplama başarılı.
        {
            let conn = create_connection(Some(db_path.clone()))?;

            let att =
                AttendanceRepository::get_by_personnel_and_period(&conn, "test-att-b", "2026-06")?
                    .expect("attendance, DB yeniden açılışında hâlâ mevcut olmalı");
            assert!(
                !att.gunler.is_empty(),
                "kayıtlı puantaj günleri boş olmamalı"
            );

            let bordro =
                PayrollService::calculate_payroll_for_personnel(&conn, "test-att-b", "2026-06")?;
            assert_eq!(bordro.donemId, "2026-06");
            assert_eq!(bordro.puantajOzeti.c, 30);
        }

        let _ = fs::remove_file(&db_path);
        Ok(())
    }

    // Test C — yanlış period_id: aynı tarih aralığı/benzer dönem olsa bile başka
    // period_id'ye ait attendance aktif dönemin puantajı kabul edilmemeli.
    #[test]
    fn test_c_attendance_bound_to_period_id_not_date_range(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-att-c");
        PersonnelRepository::save(&conn, &person)?;

        // AYNI tarih aralığına sahip iki ayrı period ID (kopya dönem senaryosu).
        let donem_a = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
            taxYear: 2026,
            taxMonth: 6,
        };
        let donem_b = BordroDonemi {
            id: "2026-05-alt".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026 (kopya)".into(),
            taxYear: 2026,
            taxMonth: 6,
        };
        PeriodRepository::save(&conn, &donem_a)?;
        PeriodRepository::save(&conn, &donem_b)?;
        ensure_test_institution_settings(&conn, &["2026-05", "2026-05-alt"])?;

        // Puantaj yalnız A'ya kaydedildi; B aynı tarih aralığını paylaşsa bile puantajsız.
        let gunler = thirty_work_days(&donem_a);
        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "test-att-c_2026-05".into(),
                personelId: "test-att-c".into(),
                donemId: "2026-05".into(),
                gunler,
            },
        )?;

        // A için hesaplama başarılı.
        assert!(
            PayrollService::calculate_payroll_for_personnel(&conn, "test-att-c", "2026-05").is_ok()
        );

        // B için hesaplama, tarih aralığı aynı olsa bile puantaj bulamadığı için başarısız olmalı.
        let res =
            PayrollService::calculate_payroll_for_personnel(&conn, "test-att-c", "2026-05-alt");
        assert!(
            matches!(res, Err(DomainError::NotFound(msg)) if msg.contains("puantaj")),
            "Başka period_id'ye ait attendance aktif dönemin puantajı kabul edilmemeli"
        );

        Ok(())
    }

    // Test D — UI/native aynı semantik: "Puantaj Girildi" rozet koşulu
    // (personelId + donemId eşleşmesi + boş olmayan gunler) native
    // get_by_personnel_and_period lookup'ı ile birebir örtüşmelidir.
    #[test]
    fn test_d_badge_semantics_match_native_attendance_lookup(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_in_memory_connection()?;

        let person = setup_test_person("test-att-d");
        PersonnelRepository::save(&conn, &person)?;

        let donem = BordroDonemi {
            id: "2026-05".into(),
            yil: 2026,
            ay: 5,
            baslangicTarihi: "2026-05-15".into(),
            bitisTarihi: "2026-06-14".into(),
            donemAdi: "Mayıs 2026".into(),
            taxYear: 2026,
            taxMonth: 6,
        };
        PeriodRepository::save(&conn, &donem)?;

        // Rozet koşulu true (kayıt + boş olmayan gunler) -> native lookup Some döndürmeli.
        let gunler = thirty_work_days(&donem);
        AttendanceRepository::save(
            &conn,
            &PersonelPuantaj {
                id: "test-att-d_2026-05".into(),
                personelId: "test-att-d".into(),
                donemId: "2026-05".into(),
                gunler,
            },
        )?;

        let found =
            AttendanceRepository::get_by_personnel_and_period(&conn, "test-att-d", "2026-05")?
                .expect("rozet koşulu true iken native lookup da kaydı bulmalı");
        assert!(!found.gunler.is_empty());

        // Rozet koşulu false (yanlış dönem / yanlış personel) -> native lookup None.
        assert!(
            AttendanceRepository::get_by_personnel_and_period(&conn, "test-att-d", "2026-06")?
                .is_none()
        );
        assert!(AttendanceRepository::get_by_personnel_and_period(
            &conn,
            "test-att-d-x",
            "2026-05"
        )?
        .is_none());

        Ok(())
    }
}
