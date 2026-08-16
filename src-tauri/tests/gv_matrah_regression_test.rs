use bordro_programi_lib::db::create_in_memory_connection;
use bordro_programi_lib::domain::models::*;
use bordro_programi_lib::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use bordro_programi_lib::repositories::attendance_repo::AttendanceRepository;
use bordro_programi_lib::repositories::period_repo::PeriodRepository;
use bordro_programi_lib::repositories::personnel_repo::PersonnelRepository;
use bordro_programi_lib::repositories::settings_repo::SettingsRepository;
use bordro_programi_lib::services::cumulative_tax_service::CumulativeTaxService;
use bordro_programi_lib::services::payroll_service::PayrollService;
use chrono::{Duration, NaiveDate};
use rust_decimal_macros::dec;
use std::collections::HashMap;

#[test]
fn july_2026_gv_matrah_applies_meal_exemption_and_union_due(
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = create_in_memory_connection()?;

    let period = BordroDonemi {
        id: "2026-07".into(),
        yil: 2026,
        ay: 7,
        baslangicTarihi: "2026-07-15".into(),
        bitisTarihi: "2026-08-14".into(),
        donemAdi: "Temmuz 2026 Dönemi (15 Temmuz - 14 Ağustos)".into(),
        taxYear: 2026,
        taxMonth: 8,
    };
    PeriodRepository::save(&conn, &period)?;

    let next_period = BordroDonemi {
        id: "2026-08".into(),
        yil: 2026,
        ay: 8,
        baslangicTarihi: "2026-08-15".into(),
        bitisTarihi: "2026-09-14".into(),
        donemAdi: "Ağustos 2026 Dönemi (15 Ağustos - 14 Eylül)".into(),
        taxYear: 2026,
        taxMonth: 9,
    };
    PeriodRepository::save(&conn, &next_period)?;

    SettingsRepository::save_institution_settings(
        &conn,
        &DonemselKurumDegerleri {
            donemId: period.id.clone(),
            ..DonemselKurumDegerleri::default()
        },
    )?;
    AnnualPayrollParametersRepository::save(&conn, &AnnualPayrollParameters::default_for_2026())?;

    let person = Personel {
        id: "real-payroll-worker".into(),
        tcNo: "11111111111".into(),
        ad: "Gerçek".into(),
        soyad: "Bordro".into(),
        grup: "3. Grup".into(),
        unvan: Some("İşçi".into()),
        sgkSicilNo: "12345".into(),
        iban: "TR000000000000000000000000".into(),
        hizmetYili: 8,
        aciklama: None,
        devirKumulatifGvMatrahi: Some(dec!(400000)),
        devirKumulatifGvMatrahiYili: Some(2026),
        devirKumulatifGvMatrahiBaslangicAyi: Some(1),
        // Ağustos vergi ayına gelene kadar 7 aylık asgari GV matrahı:
        // 7 × (33.030 - %15 işçi primi) = 196.528,50 TL.
        devirKumulatifAsgariGvMatrahi: Some(dec!(196528.50)),
        devirKumulatifAsgariGvMatrahiYili: Some(2026),
        kesintiler: Some(PersonelKesintileri {
            sendikaUyesi: Some(true),
            sabitSendikaAidati: None,
            besUyesi: Some(false),
            oksOraniYuzde: None,
            sabitBesTutar: None,
            icraTutar: None,
            kisiBorcuTutar: None,
            dogumAskerlikBorclanmasiTutar: None,
            hayatSaglikSigortasiTutar: None,
            digerKesintiTutar: None,
            gvIndirimleri: None,
        }),
    };
    PersonnelRepository::save(&conn, &person)?;

    // 31 günlük hakediş: 20 fiilî çalışma + ücret hakkı doğuran 11 gün.
    let start = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
    let mut gunler = HashMap::new();
    for index in 0..31 {
        let date = start + Duration::days(index);
        let code = if index < 20 { "Ç" } else { "T" };
        gunler.insert(date.format("%Y-%m-%d").to_string(), code.to_string());
    }
    AttendanceRepository::save(
        &conn,
        &PersonelPuantaj {
            id: "real-payroll-worker_2026-07".into(),
            personelId: person.id.clone(),
            donemId: period.id.clone(),
            gunler,
        },
    )?;

    let payroll = PayrollService::calculate_payroll_for_personnel(&conn, &person.id, &period.id)?;

    // Gerçek bordrodaki 93.312,64 / 87.312,64 değerlerine motor yuvarlamasıyla 1 kuruş yaklaşır.
    assert_eq!(payroll.gelirToplam, dec!(93312.63));
    assert_eq!(
        payroll.pekDetay.as_ref().unwrap().hesaplananPek,
        dec!(87312.63)
    );
    assert_eq!(payroll.kesintiler.isciSgkPrimi, Some(dec!(12223.77)));
    assert_eq!(payroll.kesintiler.isciIssizlikPrimi, Some(dec!(873.13)));
    assert_eq!(payroll.kesintiler.sendikaAidati, Some(dec!(1588.13)));

    // 2026 GV yemek istisnası: 20 × 300 = 6.000 TL. Günlük 300,75 TL ödemenin
    // 15 TL'lik aşan kısmı GV matrahında kalır. Sendika aidatı da GVK 63 indirimi olarak düşülür.
    let gv = payroll.gvDetay.as_ref().unwrap();
    assert_eq!(gv.cariGvMatrahi, dec!(72627.60));
    assert_eq!(gv.brutGelirVergisi, dec!(19609.45));
    assert_eq!(gv.asgariUcretGvIstisnasi, dec!(5615.10));
    assert_eq!(gv.kesilenGelirVergisi, dec!(13994.35));
    assert_eq!(payroll.netOdeme, dec!(64175.71));

    // Persist edilen gv_base de aynı authoritative snapshot'ı kullanmalı; aksi halde
    // sonraki ay kümülatif matrah eski hatalı formülle yeniden yükselir.
    let next_previous =
        CumulativeTaxService::get_previous_cumulative_gv(&conn, &person.id, &next_period)?;
    assert_eq!(next_previous, dec!(472627.60));

    Ok(())
}
