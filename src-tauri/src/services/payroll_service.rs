use super::cumulative_tax_service::CumulativeTaxService;
use crate::domain::calculations::*;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::{SettingsRepository, ZAM_AYLARI_SETTING_KEY};
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn add_puantaj_kodu(summary: &mut PuantajOzeti, code: &str, period_id: &str) -> Result<()> {
    match code {
        "Ç" => summary.c += 1,
        "T" => summary.t += 1,
        "G" => summary.g += 1,
        "İ" => summary.i += 1,
        "GÇ" => summary.gc += 1,
        "GÇT" => summary.gct += 1,
        "R" => summary.r += 1,
        _ => {
            return Err(DomainError::InvalidData(format!(
                "{} döneminde desteklenmeyen puantaj kodu: {}",
                period_id, code
            )))
        }
    }
    Ok(())
}

fn hakedis_gun(ozet: &PuantajOzeti) -> i32 {
    ozet.c + ozet.t + ozet.g + ozet.i + ozet.gc + ozet.gct
}

fn parse_period_date(value: &str, period_id: &str, field_name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| {
        DomainError::InvalidData(format!(
            "{} dönemi {} tarihi geçersiz: {}",
            period_id, field_name, e
        ))
    })
}

fn parse_attendance_date(value: &str, period_id: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| {
        DomainError::InvalidData(format!(
            "{} döneminde puantaj tarihi geçersiz: {} ({})",
            period_id, value, e
        ))
    })
}

fn find_zam_tarihi(period: &BordroDonemi, zam_aylari: &[i32]) -> Result<Option<NaiveDate>> {
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let mut result = None;

    for year in (start.year() - 1)..=(end.year() + 1) {
        for month in zam_aylari {
            let Some(candidate) = NaiveDate::from_ymd_opt(year, *month as u32, 1) else {
                continue;
            };
            if candidate >= start && candidate <= end {
                result =
                    Some(result.map_or(candidate, |current: NaiveDate| current.min(candidate)));
            }
        }
    }

    Ok(result)
}

fn find_2026_sgk_yemek_istisnasi_gecis_tarihi(
    period: &BordroDonemi,
) -> Result<Option<NaiveDate>> {
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let effective_date = NaiveDate::from_ymd_opt(2026, 4, 17)
        .expect("2026-04-17 geçerli sabit mevzuat tarihi olmalı");
    Ok((start < effective_date && effective_date <= end).then_some(effective_date))
}

fn split_puantaj_by_zam_tarihi(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    zam_aylari: &[i32],
) -> Result<(PuantajOzeti, PuantajOzeti, Option<NaiveDate>)> {
    let zam_tarihi = find_zam_tarihi(period, zam_aylari)?;
    let mut zam_oncesi = PuantajOzeti::default();
    let mut zam_sonrasi = PuantajOzeti::default();

    if zam_tarihi.is_none() {
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut zam_sonrasi, code, &period.id)?;
        }
        return Ok((zam_oncesi, zam_sonrasi, zam_tarihi));
    }

    let cutoff = zam_tarihi.expect("zam tarihi kontrolü sonrası tarih mevcut olmalı");
    for (date_text, code) in &attendance.gunler {
        let date = parse_attendance_date(date_text, &period.id)?;
        let summary = if date < cutoff {
            &mut zam_oncesi
        } else {
            &mut zam_sonrasi
        };
        add_puantaj_kodu(summary, code, &period.id)?;
    }

    Ok((zam_oncesi, zam_sonrasi, zam_tarihi))
}

fn calculate_effective_daily_meal_exemption(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    transition_date: NaiveDate,
    zam_tarihi: Option<NaiveDate>,
    previous_settings: &DonemselKurumDegerleri,
    current_settings: &DonemselKurumDegerleri,
) -> Result<Decimal> {
    let old_limit = previous_settings.gunlukYemekIstisnasiSGK.ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} dönemi SGK yemek istisnası eksik; 17.04.2026 öncesi tutar belirlenemiyor.",
            previous_settings.donemId
        ))
    })?;
    let new_limit = current_settings.gunlukYemekIstisnasiSGK.ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} dönemi SGK yemek istisnası eksik.",
            current_settings.donemId
        ))
    })?;

    let mut eligible_days = 0i32;
    let mut total_exemption = Decimal::ZERO;

    for (date_text, code) in &attendance.gunler {
        if code != "Ç" && code != "GÇ" {
            continue;
        }
        let date = parse_attendance_date(date_text, &period.id)?;
        let meal_amount = if zam_tarihi.is_some_and(|cutoff| date < cutoff) {
            previous_settings.gunlukYemek
        } else {
            current_settings.gunlukYemek
        };
        let daily_limit = if date < transition_date {
            old_limit
        } else {
            new_limit
        };
        total_exemption += meal_amount.min(daily_limit);
        eligible_days += 1;
    }

    if eligible_days == 0 {
        return Ok(new_limit);
    }

    Ok((total_exemption / Decimal::from(eligible_days)).round_dp(6))
}

fn sum_income_field(before: Option<Decimal>, after: Option<Decimal>) -> Option<Decimal> {
    Some((before.unwrap_or_default() + after.unwrap_or_default()).round_dp(2))
}

fn merge_is_primi_details(
    before: &IsPrimiHesapDetayi,
    after: &IsPrimiHesapDetayi,
) -> IsPrimiHesapDetayi {
    let hak_gunu = before.hakGunu + after.hakGunu;
    let tutar = (before.tutar + after.tutar).round_dp(2);
    let reference = if after.hakGunu > 0 { after } else { before };
    let gunluk_is_primi = if hak_gunu > 0 {
        (tutar / Decimal::from(hak_gunu)).round_dp(2)
    } else {
        Decimal::ZERO
    };

    IsPrimiHesapDetayi {
        grupId: reference.grupId.clone(),
        grupAd: reference.grupAd.clone(),
        oran: reference.oran,
        hakGunu: hak_gunu,
        gunlukIsPrimi: gunluk_is_primi,
        tutar,
    }
}

fn get_zam_aylari(conn: &Connection) -> Result<Vec<i32>> {
    let Some(raw) = SettingsRepository::get_app_setting(conn, ZAM_AYLARI_SETTING_KEY)? else {
        return Ok(Vec::new());
    };
    let months: Vec<i32> = serde_json::from_str(&raw).map_err(|e| {
        DomainError::InvalidData(format!("Kurum zam ayarı bozuk JSON içeriyor: {}", e))
    })?;
    SettingsRepository::normalize_zam_aylari(&months)
}

pub struct PayrollService;

impl PayrollService {
    pub fn calculate_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?.ok_or_else(|| {
            DomainError::NotFound(format!("Personel bulunamadı: {}", personnel_id))
        })?;

        let period = PeriodRepository::get_by_id(conn, period_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", period_id)))?;

        let existing = PayrollRepository::get_status_and_created_at(conn, personnel_id, period_id)?;
        if let Some((status, _)) = existing.as_ref() {
            if *status == BordroStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
                ));
            }
        }

        let attendance =
            AttendanceRepository::get_by_personnel_and_period(conn, personnel_id, period_id)?
                .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?;

        let mut summary = PuantajOzeti::default();
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut summary, code, period_id)?;
        }

        let kurum_degerleri = SettingsRepository::get_institution_settings(conn, period_id)?
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                    period_id
                ))
            })?;
        validate_kurum_degerleri_for_payroll(&kurum_degerleri)?;

        let zam_aylari = get_zam_aylari(conn)?;
        let (zam_oncesi_ozet, zam_sonrasi_ozet, zam_tarihi) =
            split_puantaj_by_zam_tarihi(&attendance, &period, &zam_aylari)?;
        let sgk_yemek_gecis_tarihi = find_2026_sgk_yemek_istisnasi_gecis_tarihi(&period)?;
        let needs_previous_settings = zam_tarihi.is_some() || sgk_yemek_gecis_tarihi.is_some();

        let previous_kurum_degerleri = if needs_previous_settings {
            let previous_period = PeriodRepository::get_previous_by_work_period(conn, &period)?
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} dönemi için önceki kurum ayarları bulunamadı.",
                        period.id
                    ))
                })?;
            let settings = SettingsRepository::get_institution_settings(conn, &previous_period.id)?
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} dönemi önceki kurum ayarları bulunamadı.",
                        previous_period.id
                    ))
                })?;
            validate_kurum_degerleri_for_payroll(&settings)?;
            Some(settings)
        } else {
            None
        };

        let mut effective_kurum_degerleri = kurum_degerleri.clone();

        let annual_parameters =
            AnnualPayrollParametersRepository::get_by_year(conn, period.taxYear)?.ok_or_else(
                || {
                    DomainError::InvalidData(format!(
                        "{} vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.",
                        period.taxYear
                    ))
                },
            )?;

        let (mut gelirler, mut is_primi_detay) = auto_fill_gelirler_from_puantaj(
            &summary,
            &kurum_degerleri,
            personel.hizmetYili,
            Some(&personel.grup),
        )?;

        if zam_tarihi.is_some() {
            let previous = previous_kurum_degerleri.as_ref().ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi zam öncesi kurum ayarları bulunamadı.",
                    period.id
                ))
            })?;

            let (zam_oncesi_gelirler, zam_oncesi_is_primi) =
                calculate_gunluk_gelirler_from_puantaj(
                    &zam_oncesi_ozet,
                    previous,
                    Some(&personel.grup),
                )?;
            let (zam_sonrasi_gelirler, zam_sonrasi_is_primi) =
                calculate_gunluk_gelirler_from_puantaj(
                    &zam_sonrasi_ozet,
                    &kurum_degerleri,
                    Some(&personel.grup),
                )?;

            gelirler.tabanBrutAylik = sum_income_field(
                zam_oncesi_gelirler.tabanBrutAylik,
                zam_sonrasi_gelirler.tabanBrutAylik,
            );
            gelirler.yemek =
                sum_income_field(zam_oncesi_gelirler.yemek, zam_sonrasi_gelirler.yemek);
            gelirler.vasitaYol = sum_income_field(
                zam_oncesi_gelirler.vasitaYol,
                zam_sonrasi_gelirler.vasitaYol,
            );
            gelirler.isPrimi =
                sum_income_field(zam_oncesi_gelirler.isPrimi, zam_sonrasi_gelirler.isPrimi);
            gelirler.geceCalismasiUcreti = sum_income_field(
                zam_oncesi_gelirler.geceCalismasiUcreti,
                zam_sonrasi_gelirler.geceCalismasiUcreti,
            );
            gelirler.geceCalismasiTatiliUcreti = sum_income_field(
                zam_oncesi_gelirler.geceCalismasiTatiliUcreti,
                zam_sonrasi_gelirler.geceCalismasiTatiliUcreti,
            );
            is_primi_detay = merge_is_primi_details(&zam_oncesi_is_primi, &zam_sonrasi_is_primi);

            let zam_oncesi_hakedis = hakedis_gun(&zam_oncesi_ozet);
            let zam_sonrasi_hakedis = hakedis_gun(&zam_sonrasi_ozet);
            let toplam_hakedis = zam_oncesi_hakedis + zam_sonrasi_hakedis;
            if toplam_hakedis > 0 {
                effective_kurum_degerleri.gunlukTabanUcret =
                    (previous.gunlukTabanUcret * Decimal::from(zam_oncesi_hakedis)
                        + kurum_degerleri.gunlukTabanUcret * Decimal::from(zam_sonrasi_hakedis))
                    .checked_div(Decimal::from(toplam_hakedis))
                    .unwrap_or(kurum_degerleri.gunlukTabanUcret)
                    .round_dp(6);
            }
        }

        if let Some(transition_date) = sgk_yemek_gecis_tarihi {
            let previous = previous_kurum_degerleri.as_ref().ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi 17.04.2026 öncesi SGK yemek istisnası ayarı bulunamadı.",
                    period.id
                ))
            })?;
            effective_kurum_degerleri.gunlukYemekIstisnasiSGK =
                Some(calculate_effective_daily_meal_exemption(
                    &attendance,
                    &period,
                    transition_date,
                    zam_tarihi,
                    previous,
                    &kurum_degerleri,
                )?);
        }

        let prev_gv =
            CumulativeTaxService::get_previous_cumulative_gv(conn, personnel_id, &period)?;
        let prev_asgari_gv = CumulativeTaxService::get_previous_cumulative_asgari_gv_strict(
            conn,
            personnel_id,
            &period,
        )?;

        let devreden_pek_gelen =
            Self::calculate_incoming_devreden_pek(conn, personnel_id, &period)?;
        let tax_inputs = StatutoryDeductionTaxInputs {
            previous_cumulative_gv: prev_gv,
            incoming_devreden_pek: &devreden_pek_gelen,
            previous_cumulative_asgari_gv: prev_asgari_gv,
            tax_brackets: &annual_parameters.gelirVergisiDilimleri,
        };

        let (kesintiler, pek_detay, sonraki_devreden) =
            calculate_statutory_deductions_with_tax_brackets(
                &gelirler,
                Some(&effective_kurum_degerleri),
                Some(&personel),
                Some(&summary),
                &tax_inputs,
            );

        let gelir_toplam = calculate_gelir_toplam(&gelirler);

        let sgk_orani = kurum_degerleri
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let issizlik_orani = kurum_degerleri
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let gunluk_asgari = kurum_degerleri
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::InvalidData("Günlük asgari ücret eksik.".into()))?;
        let cari_gv_matrah = calculate_gv_matrah(
            gelir_toplam,
            kesintiler.isciSgkPrimi.unwrap_or_default(),
            kesintiler.isciIssizlikPrimi.unwrap_or_default(),
        );
        let asgari_ucret_aylik_matrah =
            calculate_aylik_asgari_ucret_gv_matrahi(gunluk_asgari, sgk_orani, issizlik_orani);
        let gv_detay = calculate_gv_hesap_detayi_with_brackets(
            cari_gv_matrah,
            prev_gv,
            asgari_ucret_aylik_matrah,
            prev_asgari_gv,
            &annual_parameters.gelirVergisiDilimleri,
        );

        let odenen_raporlu_gun =
            super::sick_leave_service::SickLeaveService::calculate_paid_sick_days_for_period(
                conn,
                personnel_id,
                &period,
            )?;

        let kesinti_toplam = calculate_kesinti_toplam(&kesintiler);
        let net_odeme = (gelir_toplam - kesinti_toplam).round_dp(2);

        let now = chrono::Utc::now().to_rfc3339();

        let new_bordro = BordroKaydi {
            id: format!("{}_{}", personnel_id, period_id),
            personelId: personnel_id.to_string(),
            donemId: period_id.to_string(),
            puantajOzeti: summary.clone(),
            gelirler,
            gelirToplam: gelir_toplam,
            kesintiler,
            kesintiToplam: kesinti_toplam,
            netOdeme: net_odeme,
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: existing
                .as_ref()
                .map_or(now.clone(), |(_, created_at)| created_at.clone()),
            sonGuncellemeTarihi: now,
            notlar: Some(format!("{} dönemi hesaplandı.", period.donemAdi)),
            oncekiKumulatifGvMatrahi: Some(prev_gv),
            oncekiKumulatifAsgariGvMatrahi: Some(prev_asgari_gv),
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: Some(devreden_pek_gelen),
            sonrakiDevredenPek: Some(sonraki_devreden),
            pekDetay: Some(pek_detay),
            isPrimiDetay: Some(is_primi_detay),
            gvDetay: Some(gv_detay),
            odenenRaporluGun: Some(odenen_raporlu_gun),
            raporluGun: Some(summary.r),
        };

        PayrollRepository::save(conn, &new_bordro)?;

        Ok(new_bordro)
    }

    pub fn set_payroll_status(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        status: BordroStatus,
    ) -> Result<()> {
        let current = PayrollRepository::get_status_and_created_at(conn, personnel_id, period_id)?;
        let Some((current_status, _)) = current else {
            return Err(DomainError::NotFound("Bordro kaydı bulunamadı.".into()));
        };

        if current_status == BordroStatus::FINALIZED && status != BordroStatus::FINALIZED {
            return Err(DomainError::PayrollFinalized(
                "Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.".into(),
            ));
        }
        PayrollRepository::update_status(conn, personnel_id, period_id, status)
    }

    pub fn calculate_incoming_devreden_pek(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Vec<DevredenPekKaydi>> {
        let Some(immediately_prior) =
            PeriodRepository::get_previous_by_work_period(conn, active_period)?
        else {
            return Ok(Vec::new());
        };

        Ok(
            PayrollRepository::get_next_devreden_pek(conn, personnel_id, &immediately_prior.id)?
                .unwrap_or_default(),
        )
    }
}
