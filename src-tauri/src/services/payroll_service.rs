use super::cumulative_tax_service::CumulativeTaxService;
use super::sick_leave_service::SickLeaveService;
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

#[derive(Clone)]
struct ResolvedStatutoryValues {
    gunluk_asgari_ucret: Decimal,
    pek_tavan_katsayisi: Decimal,
    gunluk_yemek_istisnasi_sgk: Decimal,
    gunluk_yemek_istisnasi_gv: Decimal,
}

fn apply_statutory_segment(
    values: &mut ResolvedStatutoryValues,
    segment: &StatutoryParameterSegment,
) {
    if let Some(value) = segment.gunlukAsgariUcret {
        values.gunluk_asgari_ucret = value;
    }
    if let Some(value) = segment.pekTavanKatsayisi {
        values.pek_tavan_katsayisi = value;
    }
    if let Some(value) = segment.gunlukYemekIstisnasiSGK {
        values.gunluk_yemek_istisnasi_sgk = value;
    }
    if let Some(value) = segment.gunlukYemekIstisnasiGV {
        values.gunluk_yemek_istisnasi_gv = value;
    }
}

fn days_in_month(date: NaiveDate) -> Result<u32> {
    let next_month = if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
    }
    .ok_or_else(|| DomainError::InvalidData("Ay sonu çözümlenemedi.".into()))?;
    next_month
        .pred_opt()
        .map(|day| day.day())
        .ok_or_else(|| DomainError::InvalidData("Ay sonu çözümlenemedi.".into()))
}

/// Kamu 15-14 bildiriminde tam dönem 30 SGK günüdür. Ayın 15'inden ay
/// sonuna düşen kısım 16, takip eden ayın 1-14 kısmı 14 SGK günü taşır.
/// Böylece 31 günlük ayda 31. gün 0 ek SGK günü, Şubat sonunda ise eksik
/// takvim günleri aynı ilk parçaya sanal SGK günleri olarak eklenir.
fn full_period_sgk_day_weight(date: NaiveDate, period_start: NaiveDate) -> Result<i32> {
    if date.year() == period_start.year() && date.month() == period_start.month() {
        let month_days = days_in_month(period_start)?;
        if date.day() == month_days {
            let actual_second_half_days = (month_days - 14) as i32;
            return Ok(1 + (16 - actual_second_half_days));
        }
    }
    Ok(1)
}

fn is_prim_bearing_code(code: &str) -> bool {
    matches!(code, "Ç" | "T" | "G" | "İ" | "GÇ" | "GÇT" | "R")
}

pub fn resolve_statutory_snapshot_for_period(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    k: &DonemselKurumDegerleri,
) -> Result<ResolvedStatutorySnapshot> {
    SettingsRepository::validate_statutory_segments_for_period(period, k)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let calendar_day_count = (end - start).num_days() + 1;
    let full_calendar_coverage = attendance.gunler.len() as i64 == calendar_day_count;

    let base_gv_meal = k
        .gunlukYemekIstisnasiGV
        .or(k.gunlukYemekIstisnasiSGK)
        .ok_or_else(|| DomainError::ValidationError("Günlük GV yemek istisnası eksik.".into()))?;
    let base = ResolvedStatutoryValues {
        gunluk_asgari_ucret: k
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?,
        pek_tavan_katsayisi: k
            .pekTavanKatsayisi
            .ok_or_else(|| DomainError::ValidationError("PEK tavan katsayısı eksik.".into()))?,
        gunluk_yemek_istisnasi_sgk: k.gunlukYemekIstisnasiSGK.ok_or_else(|| {
            DomainError::ValidationError("Günlük SGK yemek istisnası eksik.".into())
        })?,
        gunluk_yemek_istisnasi_gv: base_gv_meal,
    };

    let mut points: Vec<(NaiveDate, ResolvedStatutoryValues)> = vec![(start, base.clone())];
    for segment in k.statutoryParameterSegments.as_deref().unwrap_or(&[]) {
        let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
            .map_err(|_| DomainError::ValidationError("Yasal segment tarihi geçersiz.".into()))?;
        if effective == start {
            apply_statutory_segment(&mut points[0].1, segment);
        } else {
            let mut next_values =
                points
                    .last()
                    .map(|(_, values)| values.clone())
                    .ok_or_else(|| {
                        DomainError::InvalidData("Yasal parametre baseline eksik.".into())
                    })?;
            apply_statutory_segment(&mut next_values, segment);
            points.push((effective, next_values));
        }
    }

    let mut snapshots = Vec::new();
    let mut total_sgk_days = 0i32;
    let mut pek_alt_sinir = Decimal::ZERO;
    let mut pek_ust_sinir = Decimal::ZERO;
    let mut sgk_meal_total = Decimal::ZERO;
    let mut gv_meal_total = Decimal::ZERO;

    for (index, (range_start, values)) in points.iter().enumerate() {
        let range_end = if let Some((next_start, _)) = points.get(index + 1) {
            next_start.pred_opt().ok_or_else(|| {
                DomainError::InvalidData("Yasal segment bitiş tarihi çözümlenemedi.".into())
            })?
        } else {
            end
        };

        let mut segment_sgk_days = 0i32;
        let mut worked_days = 0i32;
        for (date_text, code) in &attendance.gunler {
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
                DomainError::InvalidData(format!("Puantaj tarihi geçersiz: {}", date_text))
            })?;
            if date < *range_start || date > range_end {
                continue;
            }
            if is_prim_bearing_code(code) {
                segment_sgk_days += if full_calendar_coverage {
                    full_period_sgk_day_weight(date, start)?
                } else {
                    1
                };
            }
            if matches!(code.as_str(), "Ç" | "GÇ") {
                worked_days += 1;
            }
        }

        total_sgk_days += segment_sgk_days;
        pek_alt_sinir += values.gunluk_asgari_ucret * Decimal::from(segment_sgk_days);
        pek_ust_sinir += values.gunluk_asgari_ucret
            * values.pek_tavan_katsayisi
            * Decimal::from(segment_sgk_days);
        sgk_meal_total += values.gunluk_yemek_istisnasi_sgk * Decimal::from(worked_days);
        gv_meal_total += values.gunluk_yemek_istisnasi_gv * Decimal::from(worked_days);

        snapshots.push(ResolvedStatutorySegmentSnapshot {
            effectiveFrom: range_start.format("%Y-%m-%d").to_string(),
            effectiveTo: range_end.format("%Y-%m-%d").to_string(),
            sgkPrimGunSayisi: segment_sgk_days,
            fiiliYemekGunu: worked_days,
            gunlukAsgariUcret: values.gunluk_asgari_ucret,
            pekTavanKatsayisi: values.pek_tavan_katsayisi,
            gunlukYemekIstisnasiSGK: values.gunluk_yemek_istisnasi_sgk,
            gunlukYemekIstisnasiGV: values.gunluk_yemek_istisnasi_gv,
        });
    }

    if total_sgk_days > 30 {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi çözümlenen SGK prim günü 30'u aşıyor: {}.",
            period.id, total_sgk_days
        )));
    }
    let gv_reference = snapshots
        .last()
        .map(|segment| segment.gunlukAsgariUcret)
        .ok_or_else(|| DomainError::InvalidData("Yasal parametre snapshot'ı boş.".into()))?;

    Ok(ResolvedStatutorySnapshot {
        segments: snapshots,
        sgkPrimGunSayisi: total_sgk_days,
        pekAltSinir: pek_alt_sinir.round_dp(2),
        pekUstSinir: pek_ust_sinir.round_dp(2),
        sgkYemekIstisnasiToplam: sgk_meal_total.round_dp(2),
        gvYemekIstisnasiToplam: gv_meal_total.round_dp(2),
        gvReferansGunlukAsgariUcret: gv_reference,
    })
}

fn parse_period_date(value: &str, period_id: &str, field_name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| {
        DomainError::InvalidData(format!(
            "{} dönemi {} tarihi geçersiz: {}",
            period_id, field_name, e
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
        let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|e| {
            DomainError::InvalidData(format!(
                "{} döneminde puantaj tarihi geçersiz: {} ({})",
                period.id, date_text, e
            ))
        })?;
        let summary = if date < cutoff {
            &mut zam_oncesi
        } else {
            &mut zam_sonrasi
        };
        add_puantaj_kodu(summary, code, &period.id)?;
    }

    Ok((zam_oncesi, zam_sonrasi, zam_tarihi))
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

fn validate_paid_sick_dates_against_attendance(
    attendance: &PersonelPuantaj,
    paid_sick_dates: &[NaiveDate],
    period_id: &str,
) -> Result<()> {
    for date in paid_sick_dates {
        let date_text = date.format("%Y-%m-%d").to_string();
        match attendance.gunler.get(&date_text).map(String::as_str) {
            Some("R") => {}
            Some(code) => {
                return Err(DomainError::InvalidData(format!(
                    "{} döneminde kurumca ödenecek rapor günü {} puantajda '{}' olarak kayıtlı. Aynı gün hem başka bir puantaj koduyla hem rapor ücretiyle ödenemez.",
                    period_id, date_text, code
                )))
            }
            None => {
                return Err(DomainError::InvalidData(format!(
                    "{} döneminde kurumca ödenecek rapor günü {} puantajda bulunmuyor. Rapor kaydı ile puantajı eşleştirin.",
                    period_id, date_text
                )))
            }
        }
    }
    Ok(())
}

fn add_paid_sick_wage(field: &mut Option<Decimal>, paid_days: i32, daily_wage: Decimal) {
    if paid_days <= 0 {
        return;
    }
    let current = (*field).unwrap_or_default();
    let sick_wage = (daily_wage * Decimal::from(paid_days)).round_dp(2);
    *field = Some((current + sick_wage).round_dp(2));
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

        // Check existing payroll status
        let existing = PayrollRepository::get_status_and_created_at(conn, personnel_id, period_id)?;

        if let Some((status, _)) = existing.as_ref() {
            if *status == BordroStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
                ));
            }
        }

        // Get saved attendance
        let attendance =
            AttendanceRepository::get_by_personnel_and_period(conn, personnel_id, period_id)?
                .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?;

        let mut summary = PuantajOzeti::default();
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut summary, code, period_id)?;
        }
        // Calculate exact payable sick dates before any income is produced. A payable date must
        // be represented as R in attendance; otherwise payroll fails closed to prevent double pay.
        let paid_sick_dates =
            SickLeaveService::calculate_paid_sick_dates_for_period(conn, personnel_id, &period)?;
        validate_paid_sick_dates_against_attendance(&attendance, &paid_sick_dates, period_id)?;
        let odenen_raporlu_gun = paid_sick_dates.len() as i32;

        let kurum_degerleri =
            crate::repositories::settings_repo::SettingsRepository::get_institution_settings(
                conn, period_id,
            )?
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                    period_id
                ))
            })?;
        validate_kurum_degerleri_for_payroll(&kurum_degerleri)?;
        SettingsRepository::validate_statutory_segments_for_period(&period, &kurum_degerleri)?;
        let statutory_snapshot =
            resolve_statutory_snapshot_for_period(&attendance, &period, &kurum_degerleri)?;
        validate_pek_bounds(
            statutory_snapshot.pekAltSinir,
            statutory_snapshot.pekUstSinir,
        )?;

        let zam_aylari = get_zam_aylari(conn)?;
        let (zam_oncesi_ozet, zam_sonrasi_ozet, zam_tarihi) =
            split_puantaj_by_zam_tarihi(&attendance, &period, &zam_aylari)?;
        let mut effective_kurum_degerleri = kurum_degerleri.clone();

        let annual_parameters =
            AnnualPayrollParametersRepository::get_by_year(conn, period.taxYear)?;
        let annual_parameters = annual_parameters.ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.",
                period.taxYear
            ))
        })?;

        let (mut gelirler, mut is_primi_detay) = auto_fill_gelirler_from_puantaj(
            &summary,
            &kurum_degerleri,
            personel.hizmetYili,
            Some(&personel.grup),
        )?;

        if let Some(cutoff) = zam_tarihi {
            let previous_period = PeriodRepository::get_previous_by_work_period(conn, &period)?
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} dönemi zam öncesi dönem ayarı bulunamadı.",
                        period.id
                    ))
                })?;
            let previous_kurum_degerleri =
                SettingsRepository::get_institution_settings(conn, &previous_period.id)?
                    .ok_or_else(|| {
                        DomainError::InvalidData(format!(
                            "{} dönemi zam öncesi kurum ayarları bulunamadı.",
                            previous_period.id
                        ))
                    })?;
            validate_kurum_degerleri_for_payroll(&previous_kurum_degerleri)?;

            let (zam_oncesi_gelirler, zam_oncesi_is_primi) =
                calculate_gunluk_gelirler_from_puantaj(
                    &zam_oncesi_ozet,
                    &previous_kurum_degerleri,
                    Some(&personel.grup),
                )?;
            let (zam_sonrasi_gelirler, zam_sonrasi_is_primi) =
                calculate_gunluk_gelirler_from_puantaj(
                    &zam_sonrasi_ozet,
                    &kurum_degerleri,
                    Some(&personel.grup),
                )?;

            let paid_before = paid_sick_dates
                .iter()
                .filter(|date| **date < cutoff)
                .count() as i32;
            let paid_after = odenen_raporlu_gun - paid_before;

            let mut taban_brut = sum_income_field(
                zam_oncesi_gelirler.tabanBrutAylik,
                zam_sonrasi_gelirler.tabanBrutAylik,
            );
            add_paid_sick_wage(
                &mut taban_brut,
                paid_before,
                previous_kurum_degerleri.gunlukTabanUcret,
            );
            add_paid_sick_wage(
                &mut taban_brut,
                paid_after,
                kurum_degerleri.gunlukTabanUcret,
            );
            gelirler.tabanBrutAylik = taban_brut;

            // Rapor günü ücretlidir ancak fiili çalışma değildir: yemek, yol, iş primi ve gece
            // kalemlerine eklenmez. Bu alanlar yalnız mevcut puantaj özetlerinden birleşir.
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

            // Effective daily base must represent every wage-bearing day, including institution-paid
            // sick dates, otherwise percentage-based fallback deductions can use a distorted rate.
            let zam_oncesi_ucret_gunu = hakedis_gun(&zam_oncesi_ozet) + paid_before;
            let zam_sonrasi_ucret_gunu = hakedis_gun(&zam_sonrasi_ozet) + paid_after;
            let toplam_ucret_gunu = zam_oncesi_ucret_gunu + zam_sonrasi_ucret_gunu;
            if toplam_ucret_gunu > 0 {
                effective_kurum_degerleri.gunlukTabanUcret = (previous_kurum_degerleri
                    .gunlukTabanUcret
                    * Decimal::from(zam_oncesi_ucret_gunu)
                    + kurum_degerleri.gunlukTabanUcret * Decimal::from(zam_sonrasi_ucret_gunu))
                .checked_div(Decimal::from(toplam_ucret_gunu))
                .unwrap_or(kurum_degerleri.gunlukTabanUcret)
                .round_dp(6);
            }
        } else {
            // Institution-paid sick days restore only the base wage. Attendance remains unchanged,
            // therefore meal/road/is-primi stay based on actual worked days.
            add_paid_sick_wage(
                &mut gelirler.tabanBrutAylik,
                odenen_raporlu_gun,
                kurum_degerleri.gunlukTabanUcret,
            );
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

        let (mut kesintiler, pek_detay, sonraki_devreden) =
            calculate_statutory_deductions_with_tax_brackets(
                &gelirler,
                Some(&effective_kurum_degerleri),
                Some(&personel),
                Some(&summary),
                &tax_inputs,
                Some(&statutory_snapshot),
            );

        let gelir_toplam = calculate_gelir_toplam(&gelirler);

        // GV detay snapshot: gerçek kümülatif ile asgari takvim referansı açıkça ayrılır.
        let sgk_orani = kurum_degerleri
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let issizlik_orani = kurum_degerleri
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let gunluk_asgari = statutory_snapshot.gvReferansGunlukAsgariUcret;

        // SGK ve gelir vergisi yemek istisnaları bağımsız parametrelerdir.
        // Period-local segment çözümü fiilen çalışılan her tarihi doğru limite bağlar.
        let yemek_gv_istisnasi = gelirler
            .yemek
            .unwrap_or_default()
            .min(statutory_snapshot.gvYemekIstisnasiToplam);
        let sendika_aidati = kesintiler.sendikaAidati.unwrap_or_default();

        let gv_inputs = personel
            .kesintiler
            .as_ref()
            .and_then(|k| k.gvIndirimleri.as_ref());
        let dogum_askerlik_gv = gv_inputs
            .and_then(|g| g.dogumAskerlikGvIndirimTutar)
            .unwrap_or_default();
        let hayat_sigortasi_primi = gv_inputs
            .and_then(|g| g.hayatSigortasiPrimiTutar)
            .unwrap_or_default();
        let saglik_sigortasi_primi = gv_inputs
            .and_then(|g| g.saglikSigortasiPrimiTutar)
            .unwrap_or_default();
        let sigorta_yillik_tavan = annual_parameters
            .sigortaGvYillikBrutAsgariUcretTavani
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} vergi yılı sigorta GV yıllık brüt asgari ücret tavanı eksik.",
                    period.taxYear
                ))
            })?;
        let sigorta_yil_once_kullanilan =
            PayrollRepository::sum_insurance_gv_deduction_before_tax_month(
                conn,
                personnel_id,
                period.taxYear,
                period.taxMonth,
            )?;

        // GİB 63/3 sınırı ücret üzerinden çalışır. Yemek ve yol burada masraf
        // karşılığı niteliğinde olduğundan aylık %15 limit bazına eklenmez.
        let sigorta_limit_brut_ucret = (gelir_toplam
            - gelirler.yemek.unwrap_or_default()
            - gelirler.vasitaYol.unwrap_or_default())
        .max(Decimal::ZERO);
        let gv_indirim = calculate_gv_indirimleri(
            sigorta_limit_brut_ucret,
            dogum_askerlik_gv,
            hayat_sigortasi_primi,
            saglik_sigortasi_primi,
            sigorta_yillik_tavan,
            sigorta_yil_once_kullanilan,
        );

        // GVK 63: işçi SGK/işsizlik primleri, çalışan sendika aidatı, belgeye
        // dayalı doğum/askerlik borçlanması ve sınırlar içindeki sigorta primi
        // indirimleri matrahtan düşer. Yemek GV istisnası ayrıca uygulanır.
        let cari_gv_matrah = (gelir_toplam
            - kesintiler.isciSgkPrimi.unwrap_or_default()
            - kesintiler.isciIssizlikPrimi.unwrap_or_default()
            - yemek_gv_istisnasi
            - sendika_aidati
            - gv_indirim.dogum_askerlik_indirimi
            - gv_indirim.uygulanabilir_sigorta_indirimi)
            .max(dec!(0));
        let asgari_ucret_aylik_matrah =
            calculate_aylik_asgari_ucret_gv_matrahi(gunluk_asgari, sgk_orani, issizlik_orani);
        let mut gv_detay = calculate_gv_hesap_detayi_with_brackets(
            cari_gv_matrah,
            prev_gv,
            asgari_ucret_aylik_matrah,
            prev_asgari_gv,
            &annual_parameters.gelirVergisiDilimleri,
        );
        gv_detay.dogumAskerlikGvIndirimi = gv_indirim.dogum_askerlik_indirimi;
        gv_detay.sigortaGvIndirimAdayi = gv_indirim.sigorta_adayi;
        gv_detay.sigortaGvAylikLimiti = gv_indirim.sigorta_aylik_limiti;
        gv_detay.sigortaGvYillikKalanLimiti = gv_indirim.sigorta_yillik_kalan_limiti;
        gv_detay.uygulanabilirSigortaGvIndirimi = gv_indirim.uygulanabilir_sigorta_indirimi;

        // Alt seviye statutory helper eski yalın GV matrah formülünü kullanıyor.
        // Production bordro yolu authoritative GV snapshot üzerinden doğru vergiyi uygular.
        kesintiler.gelirVergisi = Some(gv_detay.kesilenGelirVergisi);

        let kesinti_toplam = calculate_kesinti_toplam(&kesintiler);
        let net_odeme = (gelir_toplam - kesinti_toplam).round_dp(2);

        if net_odeme < Decimal::ZERO {
            return Err(DomainError::NegativeNetPayment {
                gelir: gelir_toplam,
                kesinti: kesinti_toplam,
                fark: (kesinti_toplam - gelir_toplam).round_dp(2),
            });
        }

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
            statutorySnapshot: Some(statutory_snapshot),
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

        // A FINALIZED payroll is immutable: it can neither be re-calculated nor
        // downgraded to a mutable status. Only the FINALIZED state can ever be set again.
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
