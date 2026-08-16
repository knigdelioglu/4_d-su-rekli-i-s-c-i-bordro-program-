#[cfg(test)]
mod smoke_tests {
    use bordro_programi_lib::db::create_connection;
    use bordro_programi_lib::domain::models::*;
    use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
    use bordro_programi_lib::repositories::payroll_repo::PayrollRepository;
    use bordro_programi_lib::repositories::period_repo::PeriodRepository;
    use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
    use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
    use bordro_programi_lib::repositories::tax_opening_repo::TaxOpeningRepository;
    use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
    use bordro_programi_lib::services::payroll_service::PayrollService;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_full_payroll_smoke_flow_on_clean_sqlite() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Setup temporary disk SQLite file path
        let temp_dir = std::env::temp_dir();
        let db_path: PathBuf = temp_dir.join(format!(
            "smoke_test_{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(12345)
        ));

        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        println!("==> 1. Clean SQLite DB path created: {:?}", db_path);

        // Scope 1: Initial App Launch, Setup & May Payroll Calculation
        {
            let conn = create_connection(Some(db_path.clone()))?;

            // Verify clean DB has 0 periods, 0 personnel
            let initial_periods = PeriodRepository::get_all(&conn)?;
            let initial_personnel = PersonnelRepository::get_all(&conn)?;
            assert_eq!(
                initial_periods.len(),
                0,
                "Clean DB must start with 0 periods!"
            );
            assert_eq!(
                initial_personnel.len(),
                0,
                "Clean DB must start with 0 personnel!"
            );

            println!("==> Verified clean SQLite DB initialized with 0 periods & 0 personnel.");

            // 2. Add Personnel (1. Grup, 5 years seniority)
            let person = Personel {
                id: "p-smoke-1".to_string(),
                tcNo: "12345678901".to_string(),
                ad: "Ahmet".to_string(),
                soyad: "Yılmaz".to_string(),
                grup: "1. Grup".to_string(),
                unvan: Some("1. Grup (%9 İş Primi)".to_string()),
                sgkSicilNo: "48201938201".to_string(),
                iban: "TR120006200000012345678901".to_string(),
                hizmetYili: 5,
                aciklama: Some("Destek Hizmetleri".to_string()),
                devirKumulatifGvMatrahi: None,
                devirKumulatifGvMatrahiYili: None,
                devirKumulatifGvMatrahiBaslangicAyi: None,
                devirKumulatifAsgariGvMatrahi: None,
                devirKumulatifAsgariGvMatrahiYili: None,
                kesintiler: Some(PersonelKesintileri {
                    sendikaUyesi: Some(true),
                    besUyesi: Some(true),
                    sabitSendikaAidati: None,
                    oksOraniYuzde: None,
                    sabitBesTutar: None,
                    icraTutar: None,
                    kisiBorcuTutar: None,
                    dogumAskerlikBorclanmasiTutar: None,
                    hayatSaglikSigortasiTutar: None,
                    digerKesintiTutar: None,
                }),
            };
            PersonnelRepository::save(&conn, &person)?;

            // 3. Add Periods: May 2026 & June 2026
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

            PeriodRepository::save(&conn, &mayis2026)?;
            PeriodRepository::save(&conn, &haziran2026)?;

            // Save institution settings for May & June (set daily base rate so May GV base = 65,000 TL)
            // Gross for 30 days = 76,470.588 TL -> Worker SGK (15%) = 11,470.588 TL -> GV Base = 65,000.00 TL
            let mayis_settings = DonemselKurumDegerleri {
                donemId: "2026-05".to_string(),
                gunlukTabanUcret: dec!(2549.019607843137),
                gunlukYemek: dec!(0),
                birlestirilmisSosyalYardim: dec!(0),
                gunlukVasitaYol: dec!(0),
                giyimYardimi: dec!(0),
                hizmetZammiBirimi: dec!(0),
                isPrimiYuzde: Some(dec!(0)),
                ekOdeme: Some(dec!(0)),
                ..Default::default()
            };
            SettingsRepository::save_institution_settings(&conn, &mayis_settings)?;

            let haziran_settings = DonemselKurumDegerleri {
                donemId: "2026-06".to_string(),
                gunlukTabanUcret: dec!(2549.019607843137),
                gunlukYemek: dec!(0),
                birlestirilmisSosyalYardim: dec!(0),
                gunlukVasitaYol: dec!(0),
                giyimYardimi: dec!(0),
                hizmetZammiBirimi: dec!(0),
                isPrimiYuzde: Some(dec!(0)),
                ekOdeme: Some(dec!(0)),
                ..Default::default()
            };
            SettingsRepository::save_institution_settings(&conn, &haziran_settings)?;

            // 4. Add Attendance (Puantaj) for May 2026 (30 days "Ç") & June 2026 (30 days "Ç")
            // Attendance keys must be real calendar dates inside the exact 15-14 payroll period.
            let mayis_start =
                chrono::NaiveDate::parse_from_str(&mayis2026.baslangicTarihi, "%Y-%m-%d")?;
            let haziran_start =
                chrono::NaiveDate::parse_from_str(&haziran2026.baslangicTarihi, "%Y-%m-%d")?;
            let mut mayis_gunler = HashMap::new();
            let mut haziran_gunler = HashMap::new();
            for offset in 0..30 {
                let mayis_date = mayis_start + chrono::Duration::days(offset);
                let haziran_date = haziran_start + chrono::Duration::days(offset);
                mayis_gunler.insert(mayis_date.format("%Y-%m-%d").to_string(), "Ç".to_string());
                haziran_gunler.insert(haziran_date.format("%Y-%m-%d").to_string(), "Ç".to_string());
            }

            let mayis_puantaj = PersonelPuantaj {
                id: "p-smoke-1_2026-05".into(),
                personelId: "p-smoke-1".into(),
                donemId: "2026-05".into(),
                gunler: mayis_gunler,
            };
            AttendanceRepository::save(&conn, &mayis_puantaj)?;

            let haziran_puantaj = PersonelPuantaj {
                id: "p-smoke-1_2026-06".into(),
                personelId: "p-smoke-1".into(),
                donemId: "2026-06".into(),
                gunler: haziran_gunler,
            };
            AttendanceRepository::save(&conn, &haziran_puantaj)?;

            // 5. Add 120.000 TL Tax Opening balance for 2026 effective from May 2026
            let tax_opening = PersonelTaxOpening {
                id: "opt_smoke_1".into(),
                personnelId: "p-smoke-1".into(),
                year: 2026,
                gvCumulativeOpening: dec!(120000),
                effectiveFromPeriodId: "2026-05".into(),
                createdAt: None,
                updatedAt: None,
            };
            TaxOpeningRepository::save(&conn, &tax_opening)?;

            println!("==> 2. Setup personnel, periods, puantaj and 120,000 TL tax opening.");

            // 6. Calculate May 2026 Payroll
            let mayis_bordro =
                PayrollService::calculate_payroll_for_personnel(&conn, "p-smoke-1", "2026-05")?;

            println!("==> 3. Calculated May 2026 Payroll:");
            println!("     Gelir Toplam: {} TL", mayis_bordro.gelirToplam);
            println!(
                "     Önceki Küm. GV: {:?} TL",
                mayis_bordro.oncekiKumulatifGvMatrahi
            );
            println!("     Kesintiler Toplam: {} TL", mayis_bordro.kesintiToplam);
            println!("     Net Ödeme: {} TL", mayis_bordro.netOdeme);

            // May previous cumulative GV must equal tax opening (120,000 TL)
            assert_eq!(mayis_bordro.oncekiKumulatifGvMatrahi, Some(dec!(120000)));

            // Verify employer premiums in May payroll
            let mayis_pek = mayis_bordro
                .pekDetay
                .as_ref()
                .expect("May payroll must have pekDetay");
            assert!(mayis_pek.isverenSgkPrimi.is_some());
            assert!(mayis_pek.isverenIssizlikPrimi.is_some());
            assert!(mayis_pek.isverenPrimToplami.is_some());
            let mayis_isveren_sgk = mayis_pek.isverenSgkPrimi.unwrap();
            let mayis_isveren_issizlik = mayis_pek.isverenIssizlikPrimi.unwrap();
            let mayis_isveren_toplam = mayis_pek.isverenPrimToplami.unwrap();
            assert_eq!(
                mayis_isveren_toplam,
                mayis_isveren_sgk + mayis_isveren_issizlik
            );

            // Explicitly drop connection to simulate shutting down the app
            drop(conn);
            println!("==> 4. Application process fully closed. SQLite connection dropped.");
        }

        // Scope 2: App Restart & June 2026 Payroll Calculation from SQLite
        {
            println!("==> 5. Re-opening app and reading SQLite database file from disk...");
            let conn = create_connection(Some(db_path.clone()))?;

            // Verify data recovered from SQLite DB
            let restored_personnel = PersonnelRepository::get_all(&conn)?;
            let restored_payrolls = PayrollRepository::get_all(&conn)?;
            let restored_openings = TaxOpeningRepository::get_all(&conn)?;

            assert_eq!(restored_personnel.len(), 1);
            assert_eq!(restored_payrolls.len(), 1);
            assert_eq!(restored_openings.len(), 1);
            assert_eq!(restored_payrolls[0].donemId, "2026-05");

            // Verify snapshot of pekDetay, employer costs, raporluGun and odenenRaporluGun persisted across SQLite restart
            assert_eq!(restored_payrolls[0].raporluGun, Some(0));
            assert_eq!(restored_payrolls[0].odenenRaporluGun, Some(0));

            let restored_mayis_pek = restored_payrolls[0]
                .pekDetay
                .as_ref()
                .expect("Restored payroll must have pekDetay");
            assert!(restored_mayis_pek.isverenSgkPrimi.is_some());
            assert!(restored_mayis_pek.isverenIssizlikPrimi.is_some());
            assert!(restored_mayis_pek.isverenPrimToplami.is_some());

            // Extract May GV matrah from saved May payroll snapshot (authoritative cariGvMatrahi accounting for union due / GVK 63/4)
            let mayis_saved = &restored_payrolls[0];
            let mayis_gv_base = mayis_saved
                .gvDetay
                .as_ref()
                .expect("Restored May payroll must have gvDetay")
                .cariGvMatrahi;
            assert_eq!(mayis_gv_base, dec!(69193.14));

            println!("     Restored May GV Base: {} TL", mayis_gv_base);

            // 7. Calculate June 2026 Payroll
            let haziran_bordro =
                PayrollService::calculate_payroll_for_personnel(&conn, "p-smoke-1", "2026-06")?;

            println!("==> 6. Calculated June 2026 Payroll after app restart:");
            println!(
                "     Önceki Küm. GV Matrahı: {:?} TL",
                haziran_bordro.oncekiKumulatifGvMatrahi
            );
            println!("     June Net Payment: {} TL", haziran_bordro.netOdeme);

            // Verify June employer costs
            let haziran_pek = haziran_bordro
                .pekDetay
                .as_ref()
                .expect("June payroll must have pekDetay");
            assert!(haziran_pek.isverenSgkPrimi.is_some());
            assert!(haziran_pek.isverenIssizlikPrimi.is_some());
            assert!(haziran_pek.isverenPrimToplami.is_some());

            // Calculate cumulative GV for June: Opening (120,000 TL) + May GV Base
            let expected_june_prev_gv = dec!(120000) + mayis_gv_base;
            println!(
                "     Expected June Previous Cumulative GV: {} TL",
                expected_june_prev_gv
            );

            // Verify previous cumulative GV for June matches expectations (120,000 + May GV Base)
            assert_eq!(
                haziran_bordro.oncekiKumulatifGvMatrahi,
                Some(expected_june_prev_gv),
                "June previous cumulative GV must match 120,000 + May GV base!"
            );

            let service_prev_gv = CumulativeTaxService::get_previous_cumulative_gv(
                &conn,
                "p-smoke-1",
                &BordroDonemi {
                    id: "2026-06".into(),
                    yil: 2026,
                    ay: 6,
                    baslangicTarihi: "2026-06-15".into(),
                    bitisTarihi: "2026-07-14".into(),
                    donemAdi: "Haziran 2026".into(),
                    taxYear: 2026,
                    taxMonth: 7,
                },
            )?;

            assert_eq!(service_prev_gv, expected_june_prev_gv);
        }

        // Cleanup temp file
        let _ = fs::remove_file(&db_path);
        println!("==> Smoke test completed SUCCESSFULLY with clean SQLite DB!");

        Ok(())
    }
}
