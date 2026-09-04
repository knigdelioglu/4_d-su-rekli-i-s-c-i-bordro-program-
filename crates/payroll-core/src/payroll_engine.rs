//! Pure payroll orchestration over an explicit data snapshot.
//!
//! This module is deliberately unaware of SQLite, Tauri, browser storage, and
//! global state. Native code loads the snapshot from repositories and persists
//! the returned record; the WASM adapter sends the same snapshot as JSON.

#![allow(non_snake_case)]

use crate::calculations::*;
use crate::models::*;
use crate::{DomainError, Result};
use chrono::{Datelike, Duration, NaiveDate};
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
    /// Timestamp supplied by the persistence/runtime adapter. The core never
    /// reads the system clock, so identical requests are deterministic.
    pub calculatedAt: String,
    #[serde(default)]
    pub manualIncome: Option<ManualPayrollIncomeInput>,
    #[serde(default)]
    pub accrual: Option<PayrollAccrualInput>,
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
    dataset.payrolls.iter().find(|payroll| {
        payroll.personelId == personnel_id
            && payroll.donemId == period_id
            && payroll.accrualType == AccrualType::NORMAL
    })
}

fn effective_accrual_id(payroll: &BordroKaydi) -> String {
    if payroll.accrualId.trim().is_empty() {
        payroll.id.clone()
    } else {
        payroll.accrualId.clone()
    }
}

pub fn default_payment_date(period: &BordroDonemi) -> String {
    if let Ok(period_end) = NaiveDate::parse_from_str(&period.bitisTarihi, "%Y-%m-%d") {
        if period_end.year() == period.taxYear && period_end.month() as i32 == period.taxMonth {
            return period.bitisTarihi.clone();
        }
    }
    let next_month = if period.taxMonth == 12 {
        NaiveDate::from_ymd_opt(period.taxYear + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(period.taxYear, (period.taxMonth + 1) as u32, 1)
    };
    next_month
        .map(|date| (date - Duration::days(1)).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| period.bitisTarihi.clone())
}

fn effective_payment_date(payroll: &BordroKaydi, period: &BordroDonemi) -> String {
    if payroll.paymentDate.trim().is_empty() {
        default_payment_date(period)
    } else {
        payroll.paymentDate.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AccrualOrder {
    tax_ordinal: i64,
    payment_date: NaiveDate,
    sequence: i32,
    accrual_id: String,
}

#[derive(Debug, Clone)]
struct IncomingDevredenPekState {
    records: Vec<DevredenPekKaydi>,
    advances_tax_month: bool,
}

fn accrual_order_for_input(
    period: &BordroDonemi,
    input: &PayrollAccrualInput,
) -> Result<AccrualOrder> {
    let payment_date = parse_period_date(&input.paymentDate, &period.id, "ödeme/tahakkuk")?;
    Ok(AccrualOrder {
        tax_ordinal: tax_ordinal(period.taxYear, period.taxMonth),
        payment_date,
        sequence: input.sequence,
        accrual_id: input.accrualId.clone(),
    })
}

fn accrual_order_for_payroll(
    dataset: &PayrollDatasetSnapshot,
    payroll: &BordroKaydi,
) -> Result<AccrualOrder> {
    let period = period_by_id(dataset, &payroll.donemId)?;
    let payment_date = parse_period_date(
        &effective_payment_date(payroll, period),
        &period.id,
        "ödeme/tahakkuk",
    )?;
    Ok(AccrualOrder {
        tax_ordinal: tax_ordinal(period.taxYear, period.taxMonth),
        payment_date,
        sequence: payroll.sequence,
        accrual_id: effective_accrual_id(payroll),
    })
}

fn payroll_for_requested_accrual<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period_id: &str,
    requested: Option<&PayrollAccrualInput>,
) -> Option<&'a BordroKaydi> {
    if let Some(requested) = requested.filter(|input| !input.accrualId.trim().is_empty()) {
        return dataset.payrolls.iter().find(|payroll| {
            payroll.personelId == personnel_id
                && payroll.donemId == period_id
                && effective_accrual_id(payroll) == requested.accrualId
        });
    }
    existing_payroll(dataset, personnel_id, period_id)
}

fn validate_supplementary_after_normal(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    supplementary: &PayrollAccrualInput,
    supplementary_payment_date: NaiveDate,
) -> Result<()> {
    let normal = dataset
        .payrolls
        .iter()
        .find(|payroll| {
            payroll.personelId == personnel_id
                && payroll.donemId == period.id
                && payroll.accrualType == AccrualType::NORMAL
        })
        .ok_or_else(|| {
            DomainError::ValidationError(
                "Ek tahakkuk oluşturulmadan önce aynı dönemin normal maaş bordrosu hesaplanmalıdır."
                    .into(),
            )
        })?;

    match normal.status {
        BordroStatus::CALCULATED | BordroStatus::FINALIZED => {}
        BordroStatus::DRAFT | BordroStatus::STALE => {
            return Err(DomainError::ValidationError(
                "Ek tahakkuk oluşturulmadan önce aynı dönemin normal maaş bordrosu hesaplanmalıdır."
                    .into(),
            ));
        }
    }

    if normal.sequence != 0 {
        return Err(DomainError::InvalidData(
            "Normal maaş tahakkukunun sıra numarası 0 olmalıdır; ek tahakkuk zinciri çözülemez."
                .into(),
        ));
    }
    let normal_payment_date = parse_period_date(
        &effective_payment_date(normal, period),
        &period.id,
        "ödeme/tahakkuk",
    )?;
    if supplementary_payment_date < normal_payment_date {
        return Err(DomainError::ValidationError(
            "Ek tahakkukun ödeme tarihi normal maaş tahakkukundan önce olamaz.".into(),
        ));
    }
    if supplementary.sequence < 1 {
        return Err(DomainError::ValidationError(
            "Ek tahakkuk sıra numarası 1 veya daha büyük olmalıdır; NORMAL tahakkuk sıra numarası 0'dır."
                .into(),
        ));
    }
    Ok(())
}

fn resolve_accrual_input(
    request: &PayrollCalculationRequest,
    period: &BordroDonemi,
) -> Result<PayrollAccrualInput> {
    let existing = payroll_for_requested_accrual(
        &request.dataset,
        &request.personnelId,
        &request.periodId,
        request.accrual.as_ref(),
    );
    let mut input = if let Some(accrual) = request.accrual.as_ref() {
        accrual.clone()
    } else if let Some(payroll) = existing {
        PayrollAccrualInput {
            accrualId: effective_accrual_id(payroll),
            accrualType: payroll.accrualType,
            paymentDate: effective_payment_date(payroll, period),
            sequence: payroll.sequence,
            grossAmount: None,
            description: payroll.accrualDescription.clone(),
        }
    } else {
        PayrollAccrualInput {
            accrualId: format!("{}_{}", request.personnelId, request.periodId),
            accrualType: AccrualType::NORMAL,
            paymentDate: default_payment_date(period),
            sequence: 0,
            grossAmount: None,
            description: None,
        }
    };

    if input.accrualId.trim().is_empty() {
        return Err(DomainError::ValidationError(
            "Tahakkuk kimliği boş olamaz.".into(),
        ));
    }
    if input.paymentDate.trim().is_empty() {
        if request.accrual.is_some() {
            return Err(DomainError::ValidationError(
                "Tahakkuk ödeme/tahakkuk tarihi açıkça belirtilmelidir.".into(),
            ));
        }
        input.paymentDate = default_payment_date(period);
    }
    if input.sequence < 0 {
        return Err(DomainError::ValidationError(
            "Tahakkuk sıra numarası negatif olamaz.".into(),
        ));
    }
    let payment_date = parse_period_date(&input.paymentDate, &period.id, "ödeme/tahakkuk")?;
    if payment_date.year() != period.taxYear || payment_date.month() as i32 != period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "Tahakkuk ödeme tarihi {} vergi yılı/ayı {}-{:02} ile uyumlu değil.",
            input.paymentDate, period.taxYear, period.taxMonth
        )));
    }

    if input.accrualType == AccrualType::NORMAL && input.sequence != 0 {
        return Err(DomainError::ValidationError(
            "NORMAL tahakkuk sıra numarası 0 olmalıdır.".into(),
        ));
    }
    if input.accrualType != AccrualType::NORMAL && input.sequence < 1 {
        return Err(DomainError::ValidationError(
            "Ek tahakkuk sıra numarası 1 veya daha büyük olmalıdır; NORMAL tahakkuk sıra numarası 0'dır."
                .into(),
        ));
    }

    let normal_count = request
        .dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            payroll.personelId == request.personnelId
                && payroll.donemId == request.periodId
                && payroll.accrualType == AccrualType::NORMAL
        })
        .count();
    if normal_count > 1 {
        return Err(DomainError::InvalidData(
            "Personel+dönem için birden fazla NORMAL tahakkuk bulundu; hesap zinciri güvenli biçimde çözülemez."
                .into(),
        ));
    }
    let requested_order = accrual_order_for_input(period, &input)?;

    // Changing an existing node's ordering metadata would change every
    // downstream month-to-date state while retaining the old invalidation key.
    // Recalculation may change amounts, but an existing accrual must keep its
    // identity, type, date, and sequence; callers should create a new node for
    // a new payment event.
    if request.accrual.is_some() {
        if let Some(existing) = existing {
            let existing_payment_date = parse_period_date(
                &effective_payment_date(existing, period),
                &period.id,
                "ödeme/tahakkuk",
            )?;
            if existing.accrualType != input.accrualType
                || existing_payment_date != payment_date
                || existing.sequence != input.sequence
            {
                return Err(DomainError::ValidationError(
                    "Mevcut tahakkukun türü, ödeme tarihi veya sıra numarası değiştirilemez; yeni tahakkuk oluşturun."
                        .into(),
                ));
            }
        }
    }

    if input.accrualType != AccrualType::NORMAL {
        validate_supplementary_after_normal(
            &request.dataset,
            &request.personnelId,
            period,
            &input,
            payment_date,
        )?;
    } else if existing.is_none()
        && request.dataset.payrolls.iter().any(|payroll| {
            payroll.personelId == request.personnelId
                && payroll.donemId == request.periodId
                && payroll.accrualType != AccrualType::NORMAL
        })
    {
        return Err(DomainError::ValidationError(
            "Aynı dönemin normal maaş bordrosu ek tahakkuklardan önce hesaplanmalıdır.".into(),
        ));
    } else if input.accrualType == AccrualType::NORMAL {
        for payroll in request.dataset.payrolls.iter().filter(|payroll| {
            payroll.personelId == request.personnelId
                && payroll.donemId == request.periodId
                && payroll.accrualType != AccrualType::NORMAL
        }) {
            let payroll_order = accrual_order_for_payroll(&request.dataset, payroll)?;
            if payroll_order < requested_order {
                return Err(DomainError::ValidationError(
                    "Aynı dönemin normal maaş tahakkuku ek tahakkuklardan önce gelmelidir.".into(),
                ));
            }
        }
    }

    if input.accrualType != AccrualType::NORMAL {
        let amount = input.grossAmount.ok_or_else(|| {
            DomainError::ValidationError(
                "Ek tahakkuk için brüt tutar zorunludur; normal ücret yeniden üretilmeyecek."
                    .into(),
            )
        })?;
        if amount < Decimal::ZERO {
            return Err(DomainError::ValidationError(
                "Ek tahakkuk brüt tutarı negatif olamaz.".into(),
            ));
        }
        input.grossAmount = Some(round2(amount));
    }

    for payroll in request
        .dataset
        .payrolls
        .iter()
        .filter(|payroll| effective_accrual_id(payroll) == input.accrualId)
    {
        if payroll.personelId != request.personnelId || payroll.donemId != request.periodId {
            return Err(DomainError::ValidationError(format!(
                "Tahakkuk kimliği {} başka bir personel veya dönemde zaten kullanılıyor.",
                input.accrualId
            )));
        }
    }

    for payroll in request
        .dataset
        .payrolls
        .iter()
        .filter(|payroll| payroll.personelId == request.personnelId)
    {
        let same_record = effective_accrual_id(payroll) == input.accrualId;
        if same_record {
            continue;
        }
        if input.accrualType == AccrualType::NORMAL
            && payroll.donemId == request.periodId
            && payroll.accrualType == AccrualType::NORMAL
        {
            return Err(DomainError::ValidationError(
                "Bir personel ve çalışma dönemi için yalnız bir NORMAL tahakkuk olabilir.".into(),
            ));
        }
        let payroll_order = accrual_order_for_payroll(&request.dataset, payroll)?;
        if payroll_order.tax_ordinal == requested_order.tax_ordinal
            && payroll_order.payment_date == requested_order.payment_date
            && payroll_order.sequence == requested_order.sequence
        {
            return Err(DomainError::ValidationError(
                "Aynı vergi ayı/tarihi için tahakkuk sıra numarası benzersiz olmalıdır.".into(),
            ));
        }
    }
    Ok(input)
}

fn resolve_prior_accrual_state<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    personnel_id: &str,
    period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<Vec<&'a BordroKaydi>> {
    let current_order = accrual_order_for_input(period, current)?;
    let mut prior = Vec::new();
    for payroll in dataset
        .payrolls
        .iter()
        .filter(|payroll| payroll.personelId == personnel_id)
    {
        let order = accrual_order_for_payroll(dataset, payroll)?;
        if order.tax_ordinal != current_order.tax_ordinal || order >= current_order {
            continue;
        }
        match payroll.status {
            BordroStatus::CALCULATED | BordroStatus::FINALIZED => prior.push(payroll),
            BordroStatus::DRAFT | BordroStatus::STALE => {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemindeki önceki {} tahakkuk {} durumda; aynı-ay state zinciri authoritative değildir.",
                    period.id,
                    effective_accrual_id(payroll),
                    match payroll.status {
                        BordroStatus::DRAFT => "DRAFT",
                        BordroStatus::STALE => "STALE",
                        _ => "",
                    }
                )))
            }
        }
    }
    let mut ordered_prior: Vec<(AccrualOrder, &'a BordroKaydi)> = prior
        .into_iter()
        .map(|payroll| Ok((accrual_order_for_payroll(dataset, payroll)?, payroll)))
        .collect::<Result<_>>()?;
    ordered_prior.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(ordered_prior
        .into_iter()
        .map(|(_, payroll)| payroll)
        .collect())
}

fn validate_prior_accruals_finalized(
    dataset: &PayrollDatasetSnapshot,
    personnel_id: &str,
    current_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<()> {
    let current_order = accrual_order_for_input(current_period, current)?;
    for payroll in dataset
        .payrolls
        .iter()
        .filter(|payroll| payroll.personelId == personnel_id)
    {
        let order = accrual_order_for_payroll(dataset, payroll)?;
        if order >= current_order || payroll.status == BordroStatus::FINALIZED {
            continue;
        }
        return Err(DomainError::ValidationError(format!(
            "{} tahakkuk zinciri FINALIZED değil ({}). Önceki tahakkuklar kesinleştirilmeden sonraki tahakkuk FINALIZED yapılamaz.",
            effective_accrual_id(payroll),
            format_args!("{:?}", payroll.status)
        )));
    }
    Ok(())
}

fn same_month_pek_used(prior: &[&BordroKaydi]) -> Result<Decimal> {
    prior.iter().try_fold(Decimal::ZERO, |total, payroll| {
        let detail = payroll.pekDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} tahakkukunda PEK snapshot'ı eksik; aynı-ay PEK state'i çözülemez.",
                effective_accrual_id(payroll)
            ))
        })?;
        if detail.primMatrahi < Decimal::ZERO {
            return Err(DomainError::InvalidData(format!(
                "{} tahakkukunda negatif prim matrahı bulundu; aynı-ay PEK state'i çözülemez.",
                effective_accrual_id(payroll)
            )));
        }
        let total = total.checked_add(detail.primMatrahi).ok_or_else(|| {
            DomainError::InvalidData("Aynı-ay PEK toplamında Decimal taşması oluştu.".into())
        })?;
        Ok(round2(total))
    })
}

fn same_month_gv_exemption_used(prior: &[&BordroKaydi]) -> Result<Decimal> {
    prior.iter().try_fold(Decimal::ZERO, |total, payroll| {
        let detail = payroll.gvDetay.as_ref().ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} tahakkukunda GV snapshot'ı eksik; aynı-ay istisna state'i çözülemez.",
                effective_accrual_id(payroll)
            ))
        })?;
        if detail.uygulananGvIstisnasi < Decimal::ZERO {
            return Err(DomainError::InvalidData(format!(
                "{} tahakkukunda negatif GV istisnası bulundu; aynı-ay state'i çözülemez.",
                effective_accrual_id(payroll)
            )));
        }
        let total = total
            .checked_add(detail.uygulananGvIstisnasi)
            .ok_or_else(|| {
                DomainError::InvalidData(
                    "Aynı-ay GV istisna toplamında Decimal taşması oluştu.".into(),
                )
            })?;
        Ok(round2(total))
    })
}

fn same_month_stamp_exemption_used(prior: &[&BordroKaydi], stamp_rate: Decimal) -> Result<Decimal> {
    prior.iter().try_fold(Decimal::ZERO, |total, payroll| {
        if let Some(detail) = payroll.damgaDetay.as_ref() {
            if detail.uygulananDamgaIstisnasi < Decimal::ZERO {
                return Err(DomainError::InvalidData(format!(
                    "{} tahakkukunda negatif damga istisnası bulundu; aynı-ay state'i çözülemez.",
                    effective_accrual_id(payroll)
                )));
            }
            let total = total
                .checked_add(detail.uygulananDamgaIstisnasi)
                .ok_or_else(|| {
                    DomainError::InvalidData(
                        "Aynı-ay damga istisna toplamında Decimal taşması oluştu.".into(),
                    )
                })?;
            return Ok(round2(total));
        }
        // Pre-accrual databases did not persist a stamp snapshot. Reconstruct
        // the actually used amount from that immutable record's gross and tax,
        // then carry it into the first new same-month accrual.
        let gross = round2(payroll.gelirToplam * stamp_rate);
        let deducted = payroll.kesintiler.damgaVergisi.unwrap_or_default();
        let used = (gross - deducted).max(Decimal::ZERO);
        let total = total.checked_add(used).ok_or_else(|| {
            DomainError::InvalidData(
                "Aynı-ay damga istisna toplamında Decimal taşması oluştu.".into(),
            )
        })?;
        Ok(round2(total))
    })
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
    current: &PayrollAccrualInput,
) -> Result<IncomingDevredenPekState> {
    let prior_same_month =
        resolve_prior_accrual_state(dataset, personnel_id, active_period, current)?;
    if let Some(previous_accrual) = prior_same_month.last() {
        return Ok(IncomingDevredenPekState {
            records: previous_accrual
                .sonrakiDevredenPek
                .clone()
                .unwrap_or_default(),
            advances_tax_month: false,
        });
    }

    let Some(previous_period) = find_previous_work_period(dataset, active_period)? else {
        return Ok(IncomingDevredenPekState {
            records: Vec::new(),
            advances_tax_month: false,
        });
    };
    let previous_payrolls: Vec<&BordroKaydi> = dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            payroll.personelId == personnel_id && payroll.donemId == previous_period.id
        })
        .collect();
    for previous_payroll in &previous_payrolls {
        match previous_payroll.status {
            BordroStatus::CALCULATED | BordroStatus::FINALIZED => {}
            BordroStatus::DRAFT | BordroStatus::STALE => {
                return Err(DomainError::ValidationError(format!(
                    "{} dönemindeki önceki {} tahakkuk {} durumda; devreden PEK authoritative değildir.",
                    previous_period.id,
                    effective_accrual_id(previous_payroll),
                    match previous_payroll.status {
                        BordroStatus::DRAFT => "DRAFT",
                        BordroStatus::STALE => "STALE",
                        _ => "",
                    }
                )))
            }
        }
    }
    let mut ordered_previous: Vec<(AccrualOrder, &BordroKaydi)> = previous_payrolls
        .into_iter()
        .map(|payroll| Ok((accrual_order_for_payroll(dataset, payroll)?, payroll)))
        .collect::<Result<_>>()?;
    ordered_previous.sort_by(|left, right| left.0.cmp(&right.0));
    let Some((previous_order, previous_payroll)) = ordered_previous.last() else {
        return Ok(IncomingDevredenPekState {
            records: Vec::new(),
            advances_tax_month: false,
        });
    };
    let current_order = accrual_order_for_input(active_period, current)?;
    if previous_order.tax_ordinal >= current_order.tax_ordinal {
        return Err(DomainError::InvalidData(
            "Devreden PEK kaynağı mevcut vergi ayından ileri veya aynı ayda çözümlenemedi.".into(),
        ));
    }
    Ok(IncomingDevredenPekState {
        records: previous_payroll
            .sonrakiDevredenPek
            .clone()
            .unwrap_or_default(),
        advances_tax_month: true,
    })
}

fn payroll_gv_base(payroll: &BordroKaydi) -> Result<Decimal> {
    let base = payroll.gvDetay.as_ref().map_or_else(
        || {
            (calculate_gelir_toplam(&payroll.gelirler)
                - payroll.kesintiler.isciSgkPrimi.unwrap_or_default()
                - payroll.kesintiler.isciIssizlikPrimi.unwrap_or_default())
            .max(Decimal::ZERO)
        },
        |detail| detail.cariGvMatrahi,
    );
    if base < Decimal::ZERO {
        return Err(DomainError::InvalidData(format!(
            "{} tahakkukunda negatif GV matrahı bulundu; kümülatif GV zinciri çözülemez.",
            effective_accrual_id(payroll)
        )));
    }
    Ok(base)
}

fn tax_ordinal(year: i32, month: i32) -> i64 {
    i64::from(year) * 12 + i64::from(month)
}

fn previous_gv(
    dataset: &PayrollDatasetSnapshot,
    person: &Personel,
    active_period: &BordroDonemi,
    current: &PayrollAccrualInput,
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

    let active_order = accrual_order_for_input(active_period, current)?;
    let mut prior = opening_value;
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == person.id
            && matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
    }) {
        let period = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} bordrosunun dönemi bulunamadı; kümülatif GV zinciri eksik.",
                    payroll.id
                ))
            })?;
        let before_current = period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll(dataset, payroll)? < active_order));
        if before_current {
            prior = prior
                .checked_add(payroll_gv_base(payroll)?)
                .ok_or_else(|| {
                    DomainError::InvalidData("Kümülatif GV Decimal taşması oluştu.".into())
                })?;
        }
    }
    for payroll in dataset.payrolls.iter().filter(|payroll| {
        payroll.personelId == person.id
            && matches!(payroll.status, BordroStatus::DRAFT | BordroStatus::STALE)
    }) {
        let period = dataset
            .periods
            .iter()
            .find(|period| period.id == payroll.donemId)
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} bordrosunun dönemi bulunamadı; kümülatif GV zinciri eksik.",
                    payroll.id
                ))
            })?;
        let before_current = period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll(dataset, payroll)? < active_order));
        if before_current {
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

    let mut covered_tax_months = BTreeSet::new();
    for period in dataset.periods.iter().filter(|period| {
        period.taxYear == active_period.taxYear && period.taxMonth < active_period.taxMonth
    }) {
        // A tax month has one minimum-wage reference entitlement regardless
        // of how many accruals it contains.
        if !covered_tax_months.insert(period.taxMonth) {
            continue;
        }
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
    current: &PayrollAccrualInput,
) -> Result<Decimal> {
    let active_order = accrual_order_for_input(active_period, current)?;
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
        let before_current = period.taxYear == active_period.taxYear
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll(dataset, payroll)? < active_order));
        if before_current {
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
        let before_current = period.taxYear == active_period.taxYear
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll(dataset, payroll)? < active_order));
        if before_current {
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
                "{} vergi yılı asgari ücret GV referans zinciri eksik. Önce şu vergi ayına ait dönemi oluşturun veya uygun devir başlangıcını girin: {:02}.",
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
            "{} döneminde asgari ücret dönem içinde değişiyor ve seçilen vergi ayı son yasal segmentle uyuşmuyor. Yanlış GV/DV istisnası üretmemek için vergi ayını yürürlükteki asgari ücret segmentiyle uyumlu seçin.",
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
    let active_start = parse_period_date(
        &active_period.baslangicTarihi,
        &active_period.id,
        "başlangıç",
    )?;
    let prior: Vec<(&BordroKaydi, &BordroDonemi)> = dataset
        .payrolls
        .iter()
        .filter_map(|payroll| {
            if payroll.personelId != personnel_id {
                return None;
            }
            let period = dataset
                .periods
                .iter()
                .find(|period| period.id == payroll.donemId)?;
            let start = NaiveDate::parse_from_str(&period.baslangicTarihi, "%Y-%m-%d").ok()?;
            (start < active_start).then_some((payroll, period))
        })
        .collect();
    let Some(previous_period_id) = prior
        .iter()
        .max_by_key(|(_, period)| (period.baslangicTarihi.clone(), period.id.clone()))
        .map(|(_, period)| period.id.clone())
    else {
        return Ok(());
    };
    let mut ordered_previous: Vec<(AccrualOrder, &BordroKaydi, &BordroDonemi)> = prior
        .into_iter()
        .filter(|(_, period)| period.id == previous_period_id)
        .map(|(payroll, period)| {
            Ok((
                accrual_order_for_payroll(dataset, payroll)?,
                payroll,
                period,
            ))
        })
        .collect::<Result<_>>()?;
    ordered_previous.sort_by(|left, right| left.0.cmp(&right.0));
    let Some((_, previous_payroll, previous_period)) = ordered_previous.pop() else {
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
            "{} dönemindeki son önceki bordro {} durumda ve devreden PEK taşıyor. Önce bu bordroyu yeniden hesaplayın.",
            previous_period.id,
            match previous_payroll.status {
                BordroStatus::DRAFT => "DRAFT",
                BordroStatus::STALE => "STALE",
                _ => "",
            }
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
            "{} personelinde {} döneminden gelen devreden PEK hâlâ geçerli, ancak aradaki {} dönemi için bordro yok. Devreden PEK'in sessizce kaybolmaması için önce ara dönem bordrosunu tamamlayın.",
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
    let accrual = resolve_accrual_input(request, period)?;
    let normal_count = request
        .dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            payroll.personelId == request.personnelId
                && payroll.donemId == request.periodId
                && payroll.accrualType == AccrualType::NORMAL
        })
        .count();
    if normal_count > 1 {
        return Err(DomainError::InvalidData(
            "Personel+dönem için birden fazla NORMAL tahakkuk bulundu; hesap zinciri güvenli biçimde çözülemez.".into(),
        ));
    }
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
    resolve_prior_accrual_state(&request.dataset, &request.personnelId, period, &accrual)?;
    Ok(())
}

/// Runs the stricter checks required before a payroll can be made official.
///
/// A stale or draft payroll has no current authoritative result to finalize
/// and must first pass through the normal calculation flow. A CALCULATED
/// record is still recalculated from the supplied source snapshot immediately
/// before it becomes official, so the persisted result is never reused as the
/// source of truth.
pub fn validate_payroll_finalization_request(request: &PayrollCalculationRequest) -> Result<()> {
    validate_payroll_request(request)?;

    let period = period_by_id(&request.dataset, &request.periodId)?;
    let accrual = resolve_accrual_input(request, period)?;
    let existing = payroll_for_requested_accrual(
        &request.dataset,
        &request.personnelId,
        &request.periodId,
        Some(&accrual),
    )
    .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?;
    match existing.status {
        BordroStatus::FINALIZED => {
            return Err(DomainError::PayrollFinalized(
                "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
            ));
        }
        BordroStatus::DRAFT => {
            return Err(DomainError::ValidationError(
                "DRAFT bordro kesinleştirilemez. Önce bordroyu hesaplayın.".into(),
            ));
        }
        BordroStatus::STALE => {
            return Err(DomainError::ValidationError(
                "STALE bordro kesinleştirilemez. Önce bordroyu yeniden hesaplayın.".into(),
            ));
        }
        BordroStatus::CALCULATED => {}
    }

    validate_prior_accruals_finalized(&request.dataset, &request.personnelId, period, &accrual)?;

    let attendance = request
        .dataset
        .attendances
        .iter()
        .find(|attendance| {
            attendance.personelId == request.personnelId && attendance.donemId == request.periodId
        })
        .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let mut missing_dates = Vec::new();
    let mut current = start;
    while current <= end {
        let date_text = current.format("%Y-%m-%d").to_string();
        if !attendance.gunler.contains_key(&date_text) {
            missing_dates.push(date_text);
        }
        current = current
            .checked_add_signed(chrono::Duration::days(1))
            .ok_or_else(|| {
                DomainError::InvalidData("Puantaj tarih aralığı çözümlenemedi.".into())
            })?;
    }
    if !missing_dates.is_empty() {
        return Err(DomainError::ValidationError(format!(
            "{} dönemi puantajı eksik: {} takvim günü için kayıt bulunmuyor.",
            period.id,
            missing_dates.len()
        )));
    }

    // Finalization recalculates the source payroll. A later FINALIZED payroll
    // would then be an immutable downstream dependency, so the same pure
    // mutation policy must reject the operation before any adapter persists it.
    let impact = crate::policies::evaluate_payroll_invalidation(
        &request.dataset,
        &crate::policies::PayrollMutation::AccrualCalculation {
            personnelId: request.personnelId.clone(),
            periodId: request.periodId.clone(),
            accrualId: accrual.accrualId.clone(),
        },
    )?;
    if !impact.blockedByFinalized.is_empty() {
        let keys = impact
            .blockedByFinalized
            .iter()
            .map(|key| format!("{} / {}", key.personnelId, key.periodId))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(DomainError::PayrollFinalized(format!(
            "Kesinleştirme, downstream FINALIZED bordroları etkilediği için yapılamaz: {}.",
            keys
        )));
    }

    Ok(())
}

/// Recalculates the current source snapshot and returns the only supported
/// official transition. No persistence or invalidation is performed here.
pub fn finalize_payroll(request: &PayrollCalculationRequest) -> Result<BordroKaydi> {
    validate_payroll_finalization_request(request)?;
    let mut payroll = calculate_payroll(request)?;
    payroll.status = BordroStatus::FINALIZED;
    Ok(payroll)
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
    let accrual = resolve_accrual_input(request, &period)?;
    validate_period(&period)?;
    if let Some(input) = request.manualIncome.as_ref() {
        validate_manual_payroll_income_input(input)?;
    }

    let existing = payroll_for_requested_accrual(
        dataset,
        &request.personnelId,
        &request.periodId,
        Some(&accrual),
    );
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

    let is_normal_accrual = accrual.accrualType == AccrualType::NORMAL;
    let (mut income, mut is_primi_detail) = if is_normal_accrual {
        let (mut income, is_primi_detail) = auto_fill_gelirler_from_puantaj(
            &summary,
            &settings,
            person.hizmetYili,
            Some(&person.grup),
        )?;
        apply_manual_payroll_income(&mut income, request.manualIncome.as_ref())?;
        (income, Some(is_primi_detail))
    } else {
        let amount = accrual.grossAmount.unwrap_or_default();
        let mut income = GelirKalemleri::default();
        match accrual.accrualType {
            AccrualType::TEDIYE => income.tediye = Some(amount),
            AccrualType::TIS_IKRAMIYE => income.tisIkramiyesi = Some(amount),
            AccrualType::SUPPLEMENTAL => income.ekOdeme = Some(amount),
            AccrualType::NORMAL => unreachable!(),
        }
        // Supplementary accruals deliberately do not copy attendance-derived
        // salary, meal, road, premium, allowance, or service-year income.
        (income, None)
    };
    let mut effective_settings = settings.clone();

    if is_normal_accrual {
        if let Some(cutoff) = raise_date {
            let previous_period =
                find_previous_work_period(dataset, &period)?.ok_or_else(|| {
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
            let (after_income, after_is_primi) = calculate_gunluk_gelirler_from_puantaj(
                &after_summary,
                &settings,
                Some(&person.grup),
            )?;
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
            is_primi_detail = Some(merge_is_primi_details(&before_is_primi, &after_is_primi));

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
    }

    let prior_accruals = resolve_prior_accrual_state(dataset, &person.id, &period, &accrual)?;
    let month_to_date_pek = same_month_pek_used(&prior_accruals)?;
    if month_to_date_pek > statutory_snapshot.pekUstSinir {
        return Err(DomainError::InvalidData(format!(
            "Aynı vergi ayında kullanılan PEK {}, {} döneminin aylık tavanı {} değerini aşıyor; zincir güvenli biçimde devam ettirilemez.",
            month_to_date_pek, period.id, statutory_snapshot.pekUstSinir
        )));
    }
    let same_month_gv_used = same_month_gv_exemption_used(&prior_accruals)?;
    let previous_cumulative_gv = previous_gv(dataset, &person, &period, &accrual)?;
    let previous_cumulative_asgari_gv = previous_asgari_gv(dataset, &person, &period)?;
    let incoming_devreden_state = incoming_devreden_pek(dataset, &person.id, &period, &accrual)?;
    let incoming_devreden = &incoming_devreden_state.records;
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv,
        incoming_devreden_pek: incoming_devreden,
        previous_cumulative_asgari_gv,
        tax_brackets: &annual_parameters.gelirVergisiDilimleri,
    };
    let (mut deductions, pek_detail, next_devreden) = if is_normal_accrual {
        calculate_statutory_deductions_with_month_to_date_and_devreden_state(
            &income,
            Some(&effective_settings),
            Some(&person),
            Some(&summary),
            &tax_inputs,
            Some(&statutory_snapshot),
            StatutoryCalculationOptions {
                month_to_date_pek,
                advance_devreden_month: incoming_devreden_state.advances_tax_month,
                apply_lower_bound: prior_accruals.is_empty(),
            },
        )
    } else {
        let (pek_detail, next_devreden) = calculate_incremental_prime_esas_kazanc(
            &income,
            None,
            Some(&effective_settings),
            incoming_devreden,
            Some(&statutory_snapshot),
            month_to_date_pek,
        );
        let sgk_rate = effective_settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let unemployment_rate = effective_settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let worker_pek = pek_detail.primMatrahi;
        let is_oks = person
            .kesintiler
            .as_ref()
            .and_then(|deductions| deductions.besUyesi)
            .unwrap_or(false);
        let bes = if is_oks
            && person
                .kesintiler
                .as_ref()
                .and_then(|deductions| deductions.sabitBesTutar)
                .is_none()
        {
            let rate = person
                .kesintiler
                .as_ref()
                .and_then(|deductions| deductions.oksOraniYuzde)
                .or(effective_settings.besOraniYuzde)
                .unwrap_or(dec!(3))
                / dec!(100);
            Some((worker_pek * rate).floor())
        } else {
            // Fixed BES, union, enforcement, debt, insurance, and other
            // configured deductions are MONTHLY_ONCE/MANUAL. No silent second
            // application is allowed on a supplementary accrual.
            None
        };
        (
            KesintiKalemleri {
                isciSgkPrimi: Some((worker_pek * sgk_rate).round_dp_with_strategy(
                    2,
                    rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                )),
                isciIssizlikPrimi: Some((worker_pek * unemployment_rate).round_dp_with_strategy(
                    2,
                    rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                )),
                bes,
                ..KesintiKalemleri::default()
            },
            pek_detail,
            next_devreden,
        )
    };
    let income_total = calculate_gelir_toplam(&income);

    let stamp_rate = effective_settings
        .damgaVergisiOraniBinde
        .ok_or_else(|| DomainError::InvalidData("Damga vergisi oranı eksik.".into()))?
        / dec!(1000);
    let monthly_minimum = round2(statutory_snapshot.gvReferansGunlukAsgariUcret * dec!(30));
    let same_month_stamp_used = same_month_stamp_exemption_used(&prior_accruals, stamp_rate)?;
    let stamp_detail = calculate_monthly_stamp_tax_state(
        income_total,
        monthly_minimum,
        stamp_rate,
        same_month_stamp_used,
    );
    if same_month_stamp_used > stamp_detail.aylikDamgaIstisnaHakki {
        return Err(DomainError::InvalidData(format!(
            "Aynı vergi ayında kullanılan damga vergisi istisnası {}, aylık hak {} değerini aşıyor; zincir güvenli biçimde devam ettirilemez.",
            same_month_stamp_used, stamp_detail.aylikDamgaIstisnaHakki
        )));
    }
    deductions.damgaVergisi = Some(stamp_detail.kesilenDamgaVergisi);

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
    let birth_military = if is_normal_accrual {
        gv_inputs
            .and_then(|inputs| inputs.dogumAskerlikGvIndirimTutar)
            .unwrap_or_default()
    } else {
        Decimal::ZERO
    };
    let life_insurance = if is_normal_accrual {
        gv_inputs
            .and_then(|inputs| inputs.hayatSigortasiPrimiTutar)
            .unwrap_or_default()
    } else {
        Decimal::ZERO
    };
    let health_insurance = if is_normal_accrual {
        gv_inputs
            .and_then(|inputs| inputs.saglikSigortasiPrimiTutar)
            .unwrap_or_default()
    } else {
        Decimal::ZERO
    };
    let insurance_cap = annual_parameters
        .sigortaGvYillikBrutAsgariUcretTavani
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} vergi yılı sigorta GV yıllık brüt asgari ücret tavanı eksik.",
                period.taxYear
            ))
        })?;
    let insurance_used = previous_insurance_gv(dataset, &person.id, &period, &accrual)?;
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
    let mut gv_detail = calculate_gv_hesap_detayi_with_monthly_exemption_state(
        gv_base,
        previous_cumulative_gv,
        monthly_asgari_gv,
        previous_cumulative_asgari_gv,
        same_month_gv_used,
        &annual_parameters.gelirVergisiDilimleri,
    );
    if same_month_gv_used > gv_detail.asgariUcretGvIstisnasi {
        return Err(DomainError::InvalidData(format!(
            "Aynı vergi ayında kullanılan GV istisnası {}, aylık hak {} değerini aşıyor; zincir güvenli biçimde devam ettirilemez.",
            same_month_gv_used, gv_detail.asgariUcretGvIstisnasi
        )));
    }
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

    Ok(BordroKaydi {
        // Recalculating a legacy or imported NORMAL record must preserve its
        // primary key; the accrual identity is the separate stable chain key.
        id: existing
            .map(|payroll| payroll.id.clone())
            .unwrap_or_else(|| accrual.accrualId.clone()),
        personelId: request.personnelId.clone(),
        donemId: request.periodId.clone(),
        accrualId: accrual.accrualId.clone(),
        accrualType: accrual.accrualType,
        paymentDate: accrual.paymentDate.clone(),
        sequence: accrual.sequence,
        accrualDescription: accrual.description.clone(),
        puantajOzeti: summary.clone(),
        gelirler: income,
        gelirToplam: income_total,
        kesintiler: deductions,
        kesintiToplam: deduction_total,
        netOdeme: net_payment,
        status: BordroStatus::CALCULATED,
        olusturulmaTarihi: existing
            .map(|payroll| payroll.olusturulmaTarihi.clone())
            .unwrap_or_else(|| request.calculatedAt.clone()),
        sonGuncellemeTarihi: request.calculatedAt.clone(),
        notlar: Some(
            accrual
                .description
                .clone()
                .unwrap_or_else(|| format!("{} dönemi hesaplandı.", period.donemAdi)),
        ),
        oncekiKumulatifGvMatrahi: Some(previous_cumulative_gv),
        oncekiKumulatifAsgariGvMatrahi: Some(previous_cumulative_asgari_gv),
        manuelKumulatifGvMatrahi: None,
        devredenPekGelen: Some(incoming_devreden.clone()),
        sonrakiDevredenPek: Some(next_devreden),
        pekDetay: Some(pek_detail),
        isPrimiDetay: is_primi_detail,
        gvDetay: Some(gv_detail),
        damgaDetay: Some(stamp_detail),
        statutorySnapshot: Some(statutory_snapshot),
        odenenRaporluGun: Some(paid_sick_days),
        raporluGun: Some(summary.r),
    })
}
