//! Pure payroll orchestration over an explicit data snapshot.
//!
//! This module is deliberately unaware of SQLite, Tauri, browser storage, and
//! global state. Native code loads the snapshot from repositories and persists
//! the returned record; the WASM adapter sends the same snapshot as JSON.

#![allow(non_snake_case)]

use crate::calculations::*;
use crate::models::*;
use crate::{DomainError, Result};
use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayrollDatasetSnapshot {
    pub personnel: Vec<Personel>,
    pub periods: Vec<BordroDonemi>,
    pub institutionSettings: HashMap<String, DonemselKurumDegerleri>,
    pub attendances: Vec<PersonelPuantaj>,
    pub payrolls: Vec<BordroKaydi>,
    pub taxOpenings: Vec<PersonelTaxOpening>,
    pub sickLeaveRecords: Vec<SickLeaveRecord>,
    pub annualPayrollParameters: Vec<AnnualPayrollParameters>,
    #[serde(default)]
    pub zamAylari: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayrollCalculationRequest {
    pub personnelId: String,
    pub periodId: String,
    #[serde(default)]
    pub manualIncome: Option<ManualPayrollIncomeInput>,
    pub dataset: PayrollDatasetSnapshot,
}

fn round2(value: Decimal) -> Decimal {
    value.round_dp(2)
}

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

fn hakedis_gun(summary: &PuantajOzeti) -> i32 {
    summary.c + summary.t + summary.g + summary.i + summary.gc + summary.gct
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

fn is_prim_bearing_code(code: &str, date: NaiveDate, paid_sick_dates: &[NaiveDate]) -> bool {
    matches!(code, "Ç" | "T" | "G" | "İ" | "GÇ" | "GÇT")
        || (code == "R" && paid_sick_dates.contains(&date))
}

fn parse_period_date(value: &str, period_id: &str, field_name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "{} dönemi {} tarihi geçersiz: {}",
            period_id, field_name, error
        ))
    })
}

fn validate_period(period: &BordroDonemi) -> Result<()> {
    if period.yil <= 0 || !(1..=12).contains(&period.ay) {
        return Err(DomainError::ValidationError(
            "Dönem yılı geçerli olmalı ve ayı 1-12 arasında olmalıdır.".into(),
        ));
    }
    if period.taxYear <= 0 || !(1..=12).contains(&period.taxMonth) {
        return Err(DomainError::ValidationError(
            "Vergi yılı geçerli olmalı ve vergi ayı 1-12 arasında olmalıdır.".into(),
        ));
    }

    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    if start > end {
        return Err(DomainError::ValidationError(
            "Dönem başlangıç tarihi bitiş tarihinden sonra olamaz.".into(),
        ));
    }
    if start.day() != 15 || end.day() != 14 {
        return Err(DomainError::ValidationError(format!(
            "Bordro dönemi 15-14 olmalıdır: {} - {}.",
            period.baslangicTarihi, period.bitisTarihi
        )));
    }
    let (expected_end_year, expected_end_month) = if start.month() == 12 {
        (start.year() + 1, 1)
    } else {
        (start.year(), start.month() + 1)
    };
    if end.year() != expected_end_year || end.month() != expected_end_month {
        return Err(DomainError::ValidationError(format!(
            "Bordro dönemi başlangıç ayını izleyen ayın 14'ünde bitmelidir: {} - {}.",
            period.baslangicTarihi, period.bitisTarihi
        )));
    }
    if period.yil != start.year() || period.ay != start.month() as i32 {
        return Err(DomainError::ValidationError(format!(
            "Dönem yıl/ay metadata'sı başlangıç tarihiyle uyuşmuyor: {}-{:02} / {}.",
            period.yil, period.ay, period.baslangicTarihi
        )));
    }
    Ok(())
}

fn validate_tax_month_overlap(period: &BordroDonemi) -> Result<()> {
    validate_period(period)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let matches_start = period.taxYear == start.year() && period.taxMonth == start.month() as i32;
    let matches_end = period.taxYear == end.year() && period.taxMonth == end.month() as i32;
    if !matches_start && !matches_end {
        return Err(DomainError::ValidationError(format!(
            "Vergi yılı/ayı {}-{:02}, {}–{} çalışma dönemiyle örtüşmüyor.",
            period.taxYear, period.taxMonth, period.baslangicTarihi, period.bitisTarihi
        )));
    }
    Ok(())
}

fn validate_statutory_segments_for_period(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<()> {
    validate_period(period)?;
    if settings.donemId != period.id {
        return Err(DomainError::ValidationError(format!(
            "Kurum ayarı dönem kimliği eşleşmiyor: {} / {}.",
            settings.donemId, period.id
        )));
    }
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let mut previous = None;
    for segment in settings
        .statutoryParameterSegments
        .as_deref()
        .unwrap_or(&[])
    {
        let effective =
            NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d").map_err(|_| {
                DomainError::ValidationError(format!(
                    "Yasal parametre segment tarihi geçersiz: {}.",
                    segment.effectiveFrom
                ))
            })?;
        if effective < start || effective > end {
            return Err(DomainError::ValidationError(format!(
                "Yasal parametre segment tarihi {} dönemin dışında ({}–{}).",
                segment.effectiveFrom, period.baslangicTarihi, period.bitisTarihi
            )));
        }
        if previous.is_some_and(|date| effective <= date) {
            return Err(DomainError::ValidationError(
                "Yasal parametre segmentleri artan tarihte ve tekrarsız olmalıdır.".into(),
            ));
        }
        previous = Some(effective);
        if segment.gunlukAsgariUcret.is_none()
            && segment.pekTavanKatsayisi.is_none()
            && segment.gunlukYemekIstisnasiSGK.is_none()
            && segment.gunlukYemekIstisnasiGV.is_none()
        {
            return Err(DomainError::ValidationError(format!(
                "{} tarihli yasal parametre segmentinde en az bir override bulunmalıdır.",
                segment.effectiveFrom
            )));
        }
        if segment
            .gunlukAsgariUcret
            .is_some_and(|value| value <= Decimal::ZERO)
        {
            return Err(DomainError::ValidationError(
                "Segment günlük asgari ücret değeri sıfırdan büyük olmalıdır.".into(),
            ));
        }
        if segment
            .pekTavanKatsayisi
            .is_some_and(|value| value < Decimal::ONE)
        {
            return Err(DomainError::ValidationError(
                "Segment PEK tavan katsayısı en az 1 olmalıdır.".into(),
            ));
        }
        if segment
            .gunlukYemekIstisnasiSGK
            .is_some_and(|value| value < Decimal::ZERO)
            || segment
                .gunlukYemekIstisnasiGV
                .is_some_and(|value| value < Decimal::ZERO)
        {
            return Err(DomainError::ValidationError(
                "Segment yemek istisnası negatif olamaz.".into(),
            ));
        }
    }
    Ok(())
}

/// Resolves all statutory values against the dates actually present in the
/// attendance map. This is the same segment algorithm used by the native
/// application and is public for native/WASM parity tests.
pub fn resolve_statutory_snapshot_for_period(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<ResolvedStatutorySnapshot> {
    resolve_statutory_snapshot_for_period_with_paid_sick_dates(attendance, period, settings, &[])
}

pub fn resolve_statutory_snapshot_for_period_with_paid_sick_dates(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
    paid_sick_dates: &[NaiveDate],
) -> Result<ResolvedStatutorySnapshot> {
    validate_statutory_segments_for_period(period, settings)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let calendar_day_count = (end - start).num_days() + 1;
    let full_calendar_coverage = attendance.gunler.len() as i64 == calendar_day_count;
    let full_calendar_has_non_prim_day = if full_calendar_coverage {
        attendance.gunler.iter().any(|(date_text, code)| {
            let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
                return true;
            };
            !is_prim_bearing_code(code, date, paid_sick_dates)
        })
    } else {
        false
    };

    let base_gv_meal = settings
        .gunlukYemekIstisnasiGV
        .or(settings.gunlukYemekIstisnasiSGK)
        .ok_or_else(|| DomainError::ValidationError("Günlük GV yemek istisnası eksik.".into()))?;
    let base = ResolvedStatutoryValues {
        gunluk_asgari_ucret: settings
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?,
        pek_tavan_katsayisi: settings
            .pekTavanKatsayisi
            .ok_or_else(|| DomainError::ValidationError("PEK tavan katsayısı eksik.".into()))?,
        gunluk_yemek_istisnasi_sgk: settings.gunlukYemekIstisnasiSGK.ok_or_else(|| {
            DomainError::ValidationError("Günlük SGK yemek istisnası eksik.".into())
        })?,
        gunluk_yemek_istisnasi_gv: base_gv_meal,
    };

    let mut points = vec![(start, base.clone())];
    for segment in settings
        .statutoryParameterSegments
        .as_deref()
        .unwrap_or(&[])
    {
        let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
            .map_err(|_| DomainError::ValidationError("Yasal segment tarihi geçersiz.".into()))?;
        if effective == start {
            apply_statutory_segment(&mut points[0].1, segment);
        } else {
            let mut next = points
                .last()
                .map(|(_, values)| values.clone())
                .ok_or_else(|| {
                    DomainError::InvalidData("Yasal parametre baseline eksik.".into())
                })?;
            apply_statutory_segment(&mut next, segment);
            points.push((effective, next));
        }
    }

    let mut snapshots = Vec::new();
    let mut total_sgk_days = 0;
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
        let mut segment_sgk_days = 0;
        let mut worked_days = 0;
        for (date_text, code) in &attendance.gunler {
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
                DomainError::InvalidData(format!("Puantaj tarihi geçersiz: {}", date_text))
            })?;
            if date < *range_start || date > range_end {
                continue;
            }
            if is_prim_bearing_code(code, date, paid_sick_dates) {
                segment_sgk_days += if full_calendar_coverage && !full_calendar_has_non_prim_day {
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

fn calculate_paid_sick_dates_from_records(
    records: &[SickLeaveRecord],
    period: &BordroDonemi,
) -> Vec<NaiveDate> {
    let Ok(period_start) = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d") else {
        return Vec::new();
    };
    let Ok(period_end) = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") else {
        return Vec::new();
    };

    let mut year_groups: BTreeMap<i32, Vec<(NaiveDate, NaiveDate)>> = BTreeMap::new();
    for record in records {
        if let (Ok(start), Ok(end)) = (
            NaiveDate::parse_from_str(&record.startDate, "%Y-%m-%d"),
            NaiveDate::parse_from_str(&record.endDate, "%Y-%m-%d"),
        ) {
            if end >= start {
                year_groups
                    .entry(start.year())
                    .or_default()
                    .push((start, end));
            }
        }
    }

    let mut paid_dates = BTreeSet::new();
    for (_year, mut episodes) in year_groups {
        episodes.sort_by_key(|(start, end)| (*start, *end));
        episodes.dedup();
        for (index, (start, end)) in episodes.iter().enumerate() {
            if index >= 5 {
                continue;
            }
            if *start >= period_start && *start <= period_end {
                paid_dates.insert(*start);
            }
            if *end > *start {
                if let Some(day2) = start.succ_opt() {
                    if day2 <= *end && day2 >= period_start && day2 <= period_end {
                        paid_dates.insert(day2);
                    }
                }
            }
        }
    }
    paid_dates.into_iter().collect()
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
                    "{} döneminde kurumca ödenecek rapor günü {} puantajda '{}' olarak kayıtlı.",
                    period_id, date_text, code
                )))
            }
            None => {
                return Err(DomainError::InvalidData(format!(
                    "{} döneminde kurumca ödenecek rapor günü {} puantajda bulunmuyor.",
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
    let current = field.unwrap_or_default();
    *field = Some(round2(
        current + round2(daily_wage * Decimal::from(paid_days)),
    ));
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
    let mut before = PuantajOzeti::default();
    let mut after = PuantajOzeti::default();
    let Some(cutoff) = zam_tarihi else {
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut after, code, &period.id)?;
        }
        return Ok((before, after, None));
    };

    for (date_text, code) in &attendance.gunler {
        let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|error| {
            DomainError::InvalidData(format!(
                "{} döneminde puantaj tarihi geçersiz: {} ({})",
                period.id, date_text, error
            ))
        })?;
        add_puantaj_kodu(
            if date < cutoff {
                &mut before
            } else {
                &mut after
            },
            code,
            &period.id,
        )?;
    }
    Ok((before, after, Some(cutoff)))
}

fn sum_income_field(before: Option<Decimal>, after: Option<Decimal>) -> Option<Decimal> {
    Some(round2(
        before.unwrap_or_default() + after.unwrap_or_default(),
    ))
}

fn merge_is_primi_details(
    before: &IsPrimiHesapDetayi,
    after: &IsPrimiHesapDetayi,
) -> IsPrimiHesapDetayi {
    let hak_gunu = before.hakGunu + after.hakGunu;
    let tutar = round2(before.tutar + after.tutar);
    let reference = if after.hakGunu > 0 { after } else { before };
    let gunluk_is_primi = if hak_gunu > 0 {
        round2(tutar / Decimal::from(hak_gunu))
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

fn validate_manual_payroll_income_input(input: &ManualPayrollIncomeInput) -> Result<()> {
    for (field, value) in [
        ("tediye", input.tediye),
        ("tisIkramiyesi", input.tisIkramiyesi),
    ] {
        if value.is_some_and(|amount| amount < Decimal::ZERO) {
            return Err(DomainError::ValidationError(format!(
                "Manuel {} tutarı negatif olamaz.",
                field
            )));
        }
    }
    Ok(())
}

fn apply_manual_payroll_income(
    income: &mut GelirKalemleri,
    input: Option<&ManualPayrollIncomeInput>,
) -> Result<()> {
    if let Some(input) = input {
        validate_manual_payroll_income_input(input)?;
        income.tediye = input.tediye.map(round2);
        income.tisIkramiyesi = input.tisIkramiyesi.map(round2);
    } else {
        income.tediye = None;
        income.tisIkramiyesi = None;
    }
    Ok(())
}

fn period_by_id<'a>(dataset: &'a PayrollDatasetSnapshot, id: &str) -> Result<&'a BordroDonemi> {
    dataset
        .periods
        .iter()
        .find(|period| period.id == id)
        .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", id)))
}

fn person_by_id<'a>(dataset: &'a PayrollDatasetSnapshot, id: &str) -> Result<&'a Personel> {
    dataset
        .personnel
        .iter()
        .find(|person| person.id == id)
        .ok_or_else(|| DomainError::NotFound(format!("Personel bulunamadı: {}", id)))
}

fn existing_payroll<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
) -> Option<&'a BordroKaydi> {
    dataset
        .payrolls
        .iter()
        .find(|payroll| payroll.personelId == personnel_id && payroll.donemId == period_id)
}

fn find_previous_work_period<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    active_period: &BordroDonemi,
) -> Result<Option<&'a BordroDonemi>> {
    let active_start = parse_period_date(
        &active_period.baslangicTarihi,
        &active_period.id,
        "başlangıç",
    )?;
    let mut prior: Vec<&BordroDonemi> = dataset
        .periods
        .iter()
        .filter(|period| {
            NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
                .map(|start| start < active_start)
                .unwrap_or(false)
        })
        .collect();
    prior.sort_by_key(|period| (period.baslangicTarihi.clone(), period.id.clone()));
    if prior.len() >= 2 {
        let last = prior[prior.len() - 1];
        let previous = prior[prior.len() - 2];
        if last.baslangicTarihi == previous.baslangicTarihi {
            return Err(DomainError::ValidationError(
                "Aynı başlangıç tarihine sahip birden fazla önceki bordro dönemi var.".into(),
            ));
        }
    }
    Ok(prior.pop())
}

fn incoming_devreden_pek(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    active_period: &BordroDonemi,
) -> Result<Vec<DevredenPekKaydi>> {
    let Some(previous_period) = find_previous_work_period(dataset, active_period)? else {
        return Ok(Vec::new());
    };
    let Some(previous_payroll) = existing_payroll(dataset, personnel_id, &previous_period.id)
    else {
        return Ok(Vec::new());
    };
    match previous_payroll.status {
        BordroStatus::CALCULATED | BordroStatus::FINALIZED => {}
        BordroStatus::DRAFT | BordroStatus::STALE => {
            return Err(DomainError::ValidationError(format!(
                "{} dönemindeki önceki bordro {} durumda; devreden PEK authoritative değildir.",
                previous_period.id,
                match previous_payroll.status {
                    BordroStatus::DRAFT => "DRAFT",
                    BordroStatus::STALE => "STALE",
                    _ => "",
                }
            )))
        }
    }
    Ok(previous_payroll
        .sonrakiDevredenPek
        .clone()
        .unwrap_or_default())
}

fn payroll_gv_base(payroll: &BordroKaydi) -> Decimal {
    payroll.gvDetay.as_ref().map_or_else(
        || {
            (calculate_gelir_toplam(&payroll.gelirler)
                - payroll.kesintiler.isciSgkPrimi.unwrap_or_default()
                - payroll.kesintiler.isciIssizlikPrimi.unwrap_or_default())
            .max(Decimal::ZERO)
        },
        |detail| detail.cariGvMatrahi,
    )
}

fn tax_ordinal(year: i32, month: i32) -> i64 {
    i64::from(year) * 12 + i64::from(month)
}

fn previous_gv(
    dataset: &PayrollDatasetSnapshot,
    person: &Personel,
    active_period: &BordroDonemi,
) -> Result<Decimal> {
    let explicit = dataset
        .taxOpenings
        .iter()
        .find(|opening| opening.personnelId == person.id && opening.year == active_period.taxYear)
        .cloned();
    let opening = explicit.or_else(|| {
        person.devirKumulatifGvMatrahi.and_then(|value| {
            if value <= Decimal::ZERO {
                return None;
            }
            let year = person
                .devirKumulatifGvMatrahiYili
                .unwrap_or(active_period.taxYear);
            (year == active_period.taxYear).then(|| PersonelTaxOpening {
                id: format!("{}_{}", person.id, year),
                personnelId: person.id.clone(),
                year,
                gvCumulativeOpening: value,
                effectiveFromPeriodId: format!(
                    "{}-{:02}",
                    year,
                    person.devirKumulatifGvMatrahiBaslangicAyi.unwrap_or(1)
                ),
                createdAt: None,
                updatedAt: None,
            })
        })
    });

    let (start_month, opening_value) = if let Some(opening) = opening {
        if opening.gvCumulativeOpening <= Decimal::ZERO {
            (1, Decimal::ZERO)
        } else {
            let start_month = dataset
                .periods
                .iter()
                .find(|period| period.id == opening.effectiveFromPeriodId)
                .filter(|period| period.taxYear == opening.year)
                .map(|period| period.taxMonth)
                .unwrap_or(1);
            if dataset.payrolls.iter().any(|payroll| {
                payroll.personelId == person.id
                    && matches!(
                        payroll.status,
                        BordroStatus::CALCULATED | BordroStatus::FINALIZED
                    )
                    && dataset
                        .periods
                        .iter()
                        .find(|period| period.id == payroll.donemId)
                        .is_some_and(|period| {
                            period.taxYear == opening.year && period.taxMonth < start_month
                        })
            }) {
                return Err(DomainError::TaxOpeningConflict(
                    "Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır."
                        .into(),
                ));
            }
            (start_month, opening.gvCumulativeOpening)
        }
    } else {
        (1, Decimal::ZERO)
    };

    let mut prior = opening_value;
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == person.id
            && matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
    }) {
        let Some(period) = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
        else {
            continue;
        };
        if period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && period.taxMonth < active_period.taxMonth
        {
            prior += payroll_gv_base(payroll);
        }
    }
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == person.id
            && matches!(payroll.status, BordroStatus::DRAFT | BordroStatus::STALE)
    }) {
        let Some(period) = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
        else {
            continue;
        };
        if period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && period.taxMonth < active_period.taxMonth
        {
            return Err(DomainError::ValidationError(
                "Önceki vergi zincirinde DRAFT/STALE bordro var. Kümülatif GV hesabına devam etmeden önce bu bordroları yeniden hesaplayın.".into(),
            ));
        }
    }
    Ok(round2(prior))
}

fn previous_asgari_gv(
    dataset: &PayrollDatasetSnapshot,
    person: &Personel,
    active_period: &BordroDonemi,
) -> Result<Decimal> {
    let mut cumulative = if person
        .devirKumulatifAsgariGvMatrahiYili
        .unwrap_or(active_period.taxYear)
        == active_period.taxYear
    {
        person.devirKumulatifAsgariGvMatrahi.unwrap_or_default()
    } else {
        Decimal::ZERO
    };

    for period in dataset.periods.iter().filter(|period| {
        period.taxYear == active_period.taxYear && period.taxMonth < active_period.taxMonth
    }) {
        let settings = dataset.institutionSettings.get(&period.id).ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları bulunamadı; asgari GV kümülatifi hesaplanamaz.",
                period.id
            ))
        })?;
        validate_kurum_degerleri_for_payroll(settings)?;
        let daily_minimum = settings
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?;
        let sgk_rate = settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let unemployment_rate = settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let monthly_gross = round2(daily_minimum * dec!(30));
        let monthly_sgk = round2(monthly_gross * (sgk_rate + unemployment_rate));
        cumulative += (monthly_gross - monthly_sgk).max(Decimal::ZERO);
    }
    Ok(round2(cumulative))
}

fn previous_insurance_gv(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    active_period: &BordroDonemi,
) -> Result<Decimal> {
    let mut total = Decimal::ZERO;
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == personnel_id
            && matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
    }) {
        let Some(period) = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
        else {
            continue;
        };
        if period.taxYear == active_period.taxYear && period.taxMonth < active_period.taxMonth {
            if let Some(detail) = payroll.gvDetay.as_ref() {
                total += detail.uygulanabilirSigortaGvIndirimi;
            }
        }
    }
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == personnel_id
            && matches!(payroll.status, BordroStatus::DRAFT | BordroStatus::STALE)
    }) {
        let Some(period) = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
        else {
            continue;
        };
        if period.taxYear == active_period.taxYear && period.taxMonth < active_period.taxMonth {
            return Err(DomainError::ValidationError(
                "Önceki vergi zincirinde DRAFT/STALE bordro var; sigorta GV yıllık limiti çözülemez.".into(),
            ));
        }
    }
    Ok(round2(total))
}

fn validate_tax_chronology(
    dataset: &PayrollDatasetSnapshot,
    active_period: &BordroDonemi,
) -> Result<()> {
    let active_start = parse_period_date(
        &active_period.baslangicTarihi,
        &active_period.id,
        "başlangıç",
    )?;
    let mut periods: Vec<&BordroDonemi> = dataset
        .periods
        .iter()
        .filter(|period| period.id != active_period.id)
        .collect();
    periods.sort_by_key(|period| (period.baslangicTarihi.clone(), period.id.clone()));
    let previous = periods.iter().rev().find(|period| {
        NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .map(|date| date < active_start)
            .unwrap_or(false)
    });
    if let Some(previous) = previous {
        if tax_ordinal(previous.taxYear, previous.taxMonth)
            >= tax_ordinal(active_period.taxYear, active_period.taxMonth)
        {
            return Err(DomainError::ValidationError(format!(
                "Vergi kronolojisi çalışma dönemiyle ters düşüyor: önceki {} dönemi {}-{:02}, {} dönemi ise {}-{:02}.",
                previous.id,
                previous.taxYear,
                previous.taxMonth,
                active_period.id,
                active_period.taxYear,
                active_period.taxMonth
            )));
        }
    }

    let next = periods.iter().find(|period| {
        NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d")
            .map(|date| date > active_start)
            .unwrap_or(false)
    });
    if let Some(next) = next {
        if tax_ordinal(active_period.taxYear, active_period.taxMonth)
            >= tax_ordinal(next.taxYear, next.taxMonth)
        {
            return Err(DomainError::ValidationError(format!(
                "Vergi kronolojisi çalışma dönemiyle ters düşüyor: {} dönemi {}-{:02}, sonraki {} dönemi ise {}-{:02}.",
                active_period.id,
                active_period.taxYear,
                active_period.taxMonth,
                next.id,
                next.taxYear,
                next.taxMonth
            )));
        }
    }
    Ok(())
}

fn validate_tax_month_chain(
    dataset: &PayrollDatasetSnapshot,
    person: &Personel,
    period: &BordroDonemi,
) -> Result<()> {
    if period.taxMonth <= 1 {
        return Ok(());
    }
    let has_asgari_opening = person.devirKumulatifAsgariGvMatrahi.is_some()
        && person
            .devirKumulatifAsgariGvMatrahiYili
            .unwrap_or(period.taxYear)
            == period.taxYear;
    let start_month = if has_asgari_opening {
        person.devirKumulatifGvMatrahiBaslangicAyi.unwrap_or(1)
    } else {
        1
    };
    if !(1..=12).contains(&start_month) {
        return Err(DomainError::ValidationError(
            "Asgari GV devir başlangıç ayı 1-12 arasında olmalıdır.".into(),
        ));
    }
    if start_month > period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "Asgari GV devir başlangıç ayı {} aktif vergi ayı {} sonrasında olamaz.",
            start_month, period.taxMonth
        )));
    }
    for tax_month in start_month..period.taxMonth {
        if !dataset
            .periods
            .iter()
            .any(|candidate| candidate.taxYear == period.taxYear && candidate.taxMonth == tax_month)
        {
            return Err(DomainError::ValidationError(format!(
                "{} vergi yılı asgari ücret GV referans zinciri eksik: {:02} vergi ayı.",
                period.taxYear, tax_month
            )));
        }
    }
    Ok(())
}

fn validate_statutory_tax_month_reference(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<()> {
    validate_statutory_segments_for_period(period, settings)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let target_date = if period.taxYear == start.year() && period.taxMonth == start.month() as i32 {
        start
    } else if period.taxYear == end.year() && period.taxMonth == end.month() as i32 {
        NaiveDate::from_ymd_opt(end.year(), end.month(), 1).ok_or_else(|| {
            DomainError::InvalidData("Vergi ayı referans tarihi çözümlenemedi.".into())
        })?
    } else {
        return Err(DomainError::ValidationError(format!(
            "Vergi ayı {}-{:02} çalışma dönemiyle örtüşmüyor.",
            period.taxYear, period.taxMonth
        )));
    };

    let base = settings
        .gunlukAsgariUcret
        .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?;
    let mut target_value = base;
    let mut final_value = base;
    for segment in settings
        .statutoryParameterSegments
        .as_deref()
        .unwrap_or(&[])
    {
        let effective = NaiveDate::parse_from_str(&segment.effectiveFrom, "%Y-%m-%d")
            .map_err(|_| DomainError::ValidationError("Yasal segment tarihi geçersiz.".into()))?;
        if let Some(value) = segment.gunlukAsgariUcret {
            final_value = value;
            if effective <= target_date {
                target_value = value;
            }
        }
    }
    if target_value != final_value {
        return Err(DomainError::ValidationError(format!(
            "{} döneminde asgari ücret dönem içinde değişiyor ve seçilen vergi ayı son yasal segmentle uyuşmuyor.",
            period.id
        )));
    }
    Ok(())
}

fn validate_devreden_pek_gap(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    active_period: &BordroDonemi,
) -> Result<()> {
    let Some(previous_period) = find_previous_work_period(dataset, active_period)? else {
        return Ok(());
    };
    let Some(previous_payroll) = existing_payroll(dataset, personnel_id, &previous_period.id)
    else {
        return Ok(());
    };
    let positive: Vec<&DevredenPekKaydi> = previous_payroll
        .sonrakiDevredenPek
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
        .collect();
    if positive.is_empty() {
        return Ok(());
    }
    let source_ordinal = i64::from(previous_period.yil) * 12 + i64::from(previous_period.ay);
    let active_ordinal = i64::from(active_period.yil) * 12 + i64::from(active_period.ay);
    let distance = active_ordinal - source_ordinal;
    if distance <= 0 {
        return Err(DomainError::InvalidData(
            "Devreden PEK çalışma dönemi kronolojisi geçersiz.".into(),
        ));
    }
    if distance == 1 {
        return Ok(());
    }
    let skipped_months = (distance - 1) as i32;
    if !positive
        .iter()
        .any(|item| item.kalanAySayisi - skipped_months > 0)
    {
        return Ok(());
    }
    if !matches!(
        previous_payroll.status,
        BordroStatus::CALCULATED | BordroStatus::FINALIZED
    ) {
        return Err(DomainError::ValidationError(format!(
            "{} dönemindeki son önceki bordro authoritative değil; önce yeniden hesaplayın.",
            previous_period.id
        )));
    }

    let (expected_year, expected_month) = if active_period.ay == 1 {
        (active_period.yil - 1, 12)
    } else {
        (active_period.yil, active_period.ay - 1)
    };
    let expected_period = dataset
        .periods
        .iter()
        .find(|period| period.yil == expected_year && period.ay == expected_month);
    let Some(expected_period) = expected_period else {
        return Err(DomainError::ValidationError(format!(
            "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli fakat {}-{:02} ara çalışma dönemi oluşturulmamış.",
            personnel_id, previous_period.id, expected_year, expected_month
        )));
    };
    if existing_payroll(dataset, personnel_id, &expected_period.id).is_none() {
        return Err(DomainError::ValidationError(format!(
            "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli, ancak aradaki {} dönemi için bordro yok.",
            personnel_id, previous_period.id, expected_period.id
        )));
    }
    Ok(())
}

/// Runs the fail-closed, cross-record checks required before browser or native
/// production calculation. It does not mutate the supplied snapshot.
pub fn validate_payroll_request(request: &PayrollCalculationRequest) -> Result<()> {
    let period = period_by_id(&request.dataset, &request.periodId)?;
    let person = person_by_id(&request.dataset, &request.personnelId)?;
    validate_period(period)?;
    validate_tax_month_overlap(period)?;
    validate_tax_chronology(&request.dataset, period)?;
    validate_tax_month_chain(&request.dataset, person, period)?;
    let settings = request
        .dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                period.id
            ))
        })?;
    validate_kurum_degerleri_for_payroll(settings)?;
    validate_statutory_tax_month_reference(period, settings)?;
    validate_devreden_pek_gap(&request.dataset, &request.personnelId, period)?;
    Ok(())
}

/// Calculates one authoritative payroll record from the supplied snapshot.
/// Persistence and invalidation are intentionally left to the caller.
///
/// The function keeps the historical low-level service contract: strict
/// cross-period preflight is exposed separately through
/// [`validate_payroll_request`] so existing native fixture callers can still
/// exercise the formula engine with a deliberately small snapshot.
pub fn calculate_payroll(request: &PayrollCalculationRequest) -> Result<BordroKaydi> {
    let dataset = &request.dataset;
    let person = person_by_id(dataset, &request.personnelId)?.clone();
    let period = period_by_id(dataset, &request.periodId)?.clone();
    validate_period(&period)?;
    if let Some(input) = request.manualIncome.as_ref() {
        validate_manual_payroll_income_input(input)?;
    }

    let existing = existing_payroll(dataset, &request.personnelId, &request.periodId);
    if existing.is_some_and(|payroll| payroll.status == BordroStatus::FINALIZED) {
        return Err(DomainError::PayrollFinalized(
            "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
        ));
    }

    let attendance = dataset
        .attendances
        .iter()
        .find(|attendance| {
            attendance.personelId == request.personnelId && attendance.donemId == request.periodId
        })
        .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?
        .clone();
    let mut summary = PuantajOzeti::default();
    for code in attendance.gunler.values() {
        add_puantaj_kodu(&mut summary, code, &period.id)?;
    }

    let sick_records: Vec<SickLeaveRecord> = dataset
        .sickLeaveRecords
        .iter()
        .filter(|record| record.personnelId == request.personnelId)
        .cloned()
        .collect();
    let paid_sick_dates = calculate_paid_sick_dates_from_records(&sick_records, &period);
    validate_paid_sick_dates_against_attendance(&attendance, &paid_sick_dates, &period.id)?;
    let paid_sick_days = paid_sick_dates.len() as i32;

    let settings = dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                period.id
            ))
        })?
        .clone();
    validate_kurum_degerleri_for_payroll(&settings)?;
    validate_statutory_segments_for_period(&period, &settings)?;
    let statutory_snapshot = resolve_statutory_snapshot_for_period_with_paid_sick_dates(
        &attendance,
        &period,
        &settings,
        &paid_sick_dates,
    )?;
    validate_pek_bounds(
        statutory_snapshot.pekAltSinir,
        statutory_snapshot.pekUstSinir,
    )?;

    let annual_parameters = dataset
        .annualPayrollParameters
        .iter()
        .find(|parameters| parameters.year == period.taxYear)
        .cloned()
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.",
                period.taxYear
            ))
        })?;
    let (before_summary, after_summary, raise_date) =
        split_puantaj_by_zam_tarihi(&attendance, &period, &dataset.zamAylari)?;

    let (mut income, mut is_primi_detail) = auto_fill_gelirler_from_puantaj(
        &summary,
        &settings,
        person.hizmetYili,
        Some(&person.grup),
    )?;
    apply_manual_payroll_income(&mut income, request.manualIncome.as_ref())?;
    let mut effective_settings = settings.clone();

    if let Some(cutoff) = raise_date {
        let previous_period = find_previous_work_period(dataset, &period)?.ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi zam öncesi dönem ayarı bulunamadı.",
                period.id
            ))
        })?;
        let previous_settings = dataset
            .institutionSettings
            .get(&previous_period.id)
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi zam öncesi kurum ayarları bulunamadı.",
                    previous_period.id
                ))
            })?
            .clone();
        validate_kurum_degerleri_for_payroll(&previous_settings)?;
        let (before_income, before_is_primi) = calculate_gunluk_gelirler_from_puantaj(
            &before_summary,
            &previous_settings,
            Some(&person.grup),
        )?;
        let (after_income, after_is_primi) =
            calculate_gunluk_gelirler_from_puantaj(&after_summary, &settings, Some(&person.grup))?;
        let paid_before = paid_sick_dates
            .iter()
            .filter(|date| **date < cutoff)
            .count() as i32;
        let paid_after = paid_sick_days - paid_before;

        let mut base_wage =
            sum_income_field(before_income.tabanBrutAylik, after_income.tabanBrutAylik);
        add_paid_sick_wage(
            &mut base_wage,
            paid_before,
            previous_settings.gunlukTabanUcret,
        );
        add_paid_sick_wage(&mut base_wage, paid_after, settings.gunlukTabanUcret);
        income.tabanBrutAylik = base_wage;
        income.yemek = sum_income_field(before_income.yemek, after_income.yemek);
        income.vasitaYol = sum_income_field(before_income.vasitaYol, after_income.vasitaYol);
        income.isPrimi = sum_income_field(before_income.isPrimi, after_income.isPrimi);
        income.geceCalismasiUcreti = sum_income_field(
            before_income.geceCalismasiUcreti,
            after_income.geceCalismasiUcreti,
        );
        income.geceCalismasiTatiliUcreti = sum_income_field(
            before_income.geceCalismasiTatiliUcreti,
            after_income.geceCalismasiTatiliUcreti,
        );
        is_primi_detail = merge_is_primi_details(&before_is_primi, &after_is_primi);

        let before_paid_days = hakedis_gun(&before_summary) + paid_before;
        let after_paid_days = hakedis_gun(&after_summary) + paid_after;
        let total_paid_days = before_paid_days + after_paid_days;
        if total_paid_days > 0 {
            effective_settings.gunlukTabanUcret = round2(
                (previous_settings.gunlukTabanUcret * Decimal::from(before_paid_days)
                    + settings.gunlukTabanUcret * Decimal::from(after_paid_days))
                .checked_div(Decimal::from(total_paid_days))
                .unwrap_or(settings.gunlukTabanUcret),
            );
        }
    } else {
        add_paid_sick_wage(
            &mut income.tabanBrutAylik,
            paid_sick_days,
            settings.gunlukTabanUcret,
        );
    }

    let previous_cumulative_gv = previous_gv(dataset, &person, &period)?;
    let previous_cumulative_asgari_gv = previous_asgari_gv(dataset, &person, &period)?;
    let incoming_devreden = incoming_devreden_pek(dataset, &person.id, &period)?;
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv,
        incoming_devreden_pek: &incoming_devreden,
        previous_cumulative_asgari_gv,
        tax_brackets: &annual_parameters.gelirVergisiDilimleri,
    };
    let (mut deductions, pek_detail, next_devreden) =
        calculate_statutory_deductions_with_tax_brackets(
            &income,
            Some(&effective_settings),
            Some(&person),
            Some(&summary),
            &tax_inputs,
            Some(&statutory_snapshot),
        );
    let income_total = calculate_gelir_toplam(&income);

    let stamp_rate = effective_settings
        .damgaVergisiOraniBinde
        .ok_or_else(|| DomainError::InvalidData("Damga vergisi oranı eksik.".into()))?
        / dec!(1000);
    let monthly_minimum = round2(statutory_snapshot.gvReferansGunlukAsgariUcret * dec!(30));
    let gross_stamp = round2(income_total * stamp_rate);
    let stamp_exemption = round2(monthly_minimum * stamp_rate);
    deductions.damgaVergisi = Some(round2((gross_stamp - stamp_exemption).max(Decimal::ZERO)));

    let sgk_rate = settings
        .sgkIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
        / dec!(100);
    let unemployment_rate = settings
        .issizlikIsciOraniYuzde
        .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
        / dec!(100);
    let gv_inputs = person
        .kesintiler
        .as_ref()
        .and_then(|deductions| deductions.gvIndirimleri.as_ref());
    let birth_military = gv_inputs
        .and_then(|inputs| inputs.dogumAskerlikGvIndirimTutar)
        .unwrap_or_default();
    let life_insurance = gv_inputs
        .and_then(|inputs| inputs.hayatSigortasiPrimiTutar)
        .unwrap_or_default();
    let health_insurance = gv_inputs
        .and_then(|inputs| inputs.saglikSigortasiPrimiTutar)
        .unwrap_or_default();
    let insurance_cap = annual_parameters
        .sigortaGvYillikBrutAsgariUcretTavani
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} vergi yılı sigorta GV yıllık brüt asgari ücret tavanı eksik.",
                period.taxYear
            ))
        })?;
    let insurance_used = previous_insurance_gv(dataset, &person.id, &period)?;
    let insurance_wage_base =
        (income_total - income.yemek.unwrap_or_default() - income.vasitaYol.unwrap_or_default())
            .max(Decimal::ZERO);
    let gv_discount = calculate_gv_indirimleri(
        insurance_wage_base,
        birth_military,
        life_insurance,
        health_insurance,
        insurance_cap,
        insurance_used,
    );
    let gv_base = (income_total
        - deductions.isciSgkPrimi.unwrap_or_default()
        - deductions.isciIssizlikPrimi.unwrap_or_default()
        - income
            .yemek
            .unwrap_or_default()
            .min(statutory_snapshot.gvYemekIstisnasiToplam)
        - deductions.sendikaAidati.unwrap_or_default()
        - gv_discount.dogum_askerlik_indirimi
        - gv_discount.uygulanabilir_sigorta_indirimi)
        .max(Decimal::ZERO);
    let daily_minimum = statutory_snapshot.gvReferansGunlukAsgariUcret;
    let monthly_asgari_gv =
        calculate_aylik_asgari_ucret_gv_matrahi(daily_minimum, sgk_rate, unemployment_rate);
    let mut gv_detail = calculate_gv_hesap_detayi_with_brackets(
        gv_base,
        previous_cumulative_gv,
        monthly_asgari_gv,
        previous_cumulative_asgari_gv,
        &annual_parameters.gelirVergisiDilimleri,
    );
    gv_detail.dogumAskerlikGvIndirimi = gv_discount.dogum_askerlik_indirimi;
    gv_detail.sigortaGvIndirimAdayi = gv_discount.sigorta_adayi;
    gv_detail.sigortaGvAylikLimiti = gv_discount.sigorta_aylik_limiti;
    gv_detail.sigortaGvYillikKalanLimiti = gv_discount.sigorta_yillik_kalan_limiti;
    gv_detail.uygulanabilirSigortaGvIndirimi = gv_discount.uygulanabilir_sigorta_indirimi;
    deductions.gelirVergisi = Some(gv_detail.kesilenGelirVergisi);

    let deduction_total = calculate_kesinti_toplam(&deductions);
    let net_payment = round2(income_total - deduction_total);
    if net_payment < Decimal::ZERO {
        return Err(DomainError::NegativeNetPayment {
            gelir: income_total,
            kesinti: deduction_total,
            fark: round2(deduction_total - income_total),
        });
    }

    let now = Utc::now().to_rfc3339();
    Ok(BordroKaydi {
        id: format!("{}_{}", request.personnelId, request.periodId),
        personelId: request.personnelId.clone(),
        donemId: request.periodId.clone(),
        puantajOzeti: summary.clone(),
        gelirler: income,
        gelirToplam: income_total,
        kesintiler: deductions,
        kesintiToplam: deduction_total,
        netOdeme: net_payment,
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: existing
            .map(|payroll| payroll.olusturulmaTarihi.clone())
            .unwrap_or_else(|| now.clone()),
        sonGuncellemeTarihi: now,
        notlar: Some(format!("{} dönemi hesaplandı.", period.donemAdi)),
        oncekiKumulatifGvMatrahi: Some(previous_cumulative_gv),
        oncekiKumulatifAsgariGvMatrahi: Some(previous_cumulative_asgari_gv),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: Some(incoming_devreden),
        sonrakiDevredenPek: Some(next_devreden),
        pekDetay: Some(pek_detail),
        isPrimiDetay: Some(is_primi_detail),
        gvDetay: Some(gv_detail),
        statutorySnapshot: Some(statutory_snapshot),
        odenenRaporluGun: Some(paid_sick_days),
        raporluGun: Some(summary.r),
    })
}
