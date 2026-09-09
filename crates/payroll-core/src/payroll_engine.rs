//! Pure payroll orchestration over an explicit data snapshot.
//!
//! This module is deliberately unaware of SQLite, Tauri, browser storage, and
//! global state. Native code loads the snapshot from repositories and persists
//! the returned record; the WASM adapter sends the same snapshot as JSON.

#![allow(non_snake_case)]

use crate::calculations::*;
use crate::index::PayrollDatasetIndex;
use crate::models::*;
use crate::retro::{
    retro_payable_allocation_amount, retro_payable_settlement_amount,
    retro_payment_income_with_index,
};
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
    #[serde(default)]
    pub compensationRevisions: Vec<CompensationRevision>,
    #[serde(default)]
    pub compensationRevisionOverrides: Vec<CompensationRevisionOverride>,
    #[serde(default)]
    pub retroBatches: Vec<RetroAdjustmentBatch>,
    #[serde(default)]
    pub retroAllocations: Vec<RetroAllocation>,
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

/// Validates the canonical 15–14 work-period definition. Persistence adapters
/// call this function instead of maintaining a second copy of the invariant.
pub fn validate_period(period: &BordroDonemi) -> Result<()> {
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

pub fn validate_tax_month_overlap(period: &BordroDonemi) -> Result<()> {
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

/// Validates period-local statutory overrides before they are persisted or
/// resolved into a payroll snapshot.
pub fn validate_statutory_segments_for_period(
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

/// Captures only the period-level legal inputs needed to reproduce the
/// statutory reference. It intentionally excludes attendance, personnel and
/// payment-event state so the same snapshot can serve every employee.
pub fn snapshot_statutory_parameters(
    settings: &DonemselKurumDegerleri,
) -> Result<StatutoryParameterSnapshot> {
    if let Some(snapshot) = settings.statutoryParameterSnapshot.as_ref() {
        return Ok(snapshot.clone());
    }

    current_statutory_parameter_definition(settings)
}

/// Returns the statutory definition currently present in the mutable settings
/// envelope. Unlike `snapshot_statutory_parameters`, this deliberately ignores
/// any already-persisted snapshot so settings mutation can compare the new
/// legal definition with the old one.
pub fn current_statutory_parameter_definition(
    settings: &DonemselKurumDegerleri,
) -> Result<StatutoryParameterSnapshot> {
    Ok(StatutoryParameterSnapshot {
        gunlukAsgariUcret: settings
            .gunlukAsgariUcret
            .ok_or_else(|| DomainError::ValidationError("Günlük asgari ücret eksik.".into()))?,
        sgkIsciOraniYuzde: settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("SGK işçi oranı eksik.".into()))?,
        issizlikIsciOraniYuzde: settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("İşsizlik işçi oranı eksik.".into()))?,
        pekTavanKatsayisi: settings
            .pekTavanKatsayisi
            .ok_or_else(|| DomainError::ValidationError("PEK tavan katsayısı eksik.".into()))?,
        gunlukYemekIstisnasiSGK: settings.gunlukYemekIstisnasiSGK.ok_or_else(|| {
            DomainError::ValidationError("Günlük SGK yemek istisnası eksik.".into())
        })?,
        gunlukYemekIstisnasiGV: settings
            .gunlukYemekIstisnasiGV
            .or(settings.gunlukYemekIstisnasiSGK)
            .ok_or_else(|| {
                DomainError::ValidationError("Günlük GV yemek istisnası eksik.".into())
            })?,
        statutoryParameterSegments: settings
            .statutoryParameterSegments
            .clone()
            .unwrap_or_default(),
    })
}

/// Applies the immutable period-level legal snapshot to the mutable settings
/// envelope. Non-statutory earning/deduction settings remain untouched.
fn effective_statutory_settings(
    settings: &DonemselKurumDegerleri,
) -> Result<DonemselKurumDegerleri> {
    let Some(snapshot) = settings.statutoryParameterSnapshot.as_ref() else {
        return Ok(settings.clone());
    };

    let mut effective = settings.clone();
    effective.gunlukAsgariUcret = Some(snapshot.gunlukAsgariUcret);
    effective.sgkIsciOraniYuzde = Some(snapshot.sgkIsciOraniYuzde);
    effective.issizlikIsciOraniYuzde = Some(snapshot.issizlikIsciOraniYuzde);
    effective.pekTavanKatsayisi = Some(snapshot.pekTavanKatsayisi);
    effective.gunlukYemekIstisnasiSGK = Some(snapshot.gunlukYemekIstisnasiSGK);
    effective.gunlukYemekIstisnasiGV = Some(snapshot.gunlukYemekIstisnasiGV);
    effective.statutoryParameterSegments = Some(snapshot.statutoryParameterSegments.clone());
    Ok(effective)
}

/// Reconstructs the legal settings envelope from an older persisted payroll
/// snapshot. This is a compatibility fallback for periods created before the
/// period-level immutable snapshot existed; the period-level snapshot remains
/// the primary source whenever it is available.
pub fn settings_with_resolved_statutory_snapshot(
    settings: &DonemselKurumDegerleri,
    snapshot: &ResolvedStatutorySnapshot,
) -> Result<DonemselKurumDegerleri> {
    let Some(first_segment) = snapshot.segments.first() else {
        return Err(DomainError::InvalidData(
            "Tarihsel statutory snapshot'ı boş; yasal ayarlar yeniden üretilemez.".into(),
        ));
    };

    let mut historical = settings.clone();
    historical.gunlukAsgariUcret = Some(first_segment.gunlukAsgariUcret);
    historical.pekTavanKatsayisi = Some(first_segment.pekTavanKatsayisi);
    historical.gunlukYemekIstisnasiSGK = Some(first_segment.gunlukYemekIstisnasiSGK);
    historical.gunlukYemekIstisnasiGV = Some(first_segment.gunlukYemekIstisnasiGV);
    if let Some(rate) = snapshot.sgkIsciOraniYuzde {
        historical.sgkIsciOraniYuzde = Some(rate);
    }
    if let Some(rate) = snapshot.issizlikIsciOraniYuzde {
        historical.issizlikIsciOraniYuzde = Some(rate);
    }
    historical.statutoryParameterSegments = Some(
        snapshot
            .segments
            .iter()
            .skip(1)
            .map(|segment| StatutoryParameterSegment {
                effectiveFrom: segment.effectiveFrom.clone(),
                gunlukAsgariUcret: Some(segment.gunlukAsgariUcret),
                pekTavanKatsayisi: Some(segment.pekTavanKatsayisi),
                gunlukYemekIstisnasiSGK: Some(segment.gunlukYemekIstisnasiSGK),
                gunlukYemekIstisnasiGV: Some(segment.gunlukYemekIstisnasiGV),
            })
            .collect(),
    );
    effective_statutory_settings(&historical)
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
    resolve_statutory_snapshot_internal(Some(attendance), period, settings, paid_sick_dates)
}

/// Resolves the provisional statutory payment-month reference for a
/// supplementary event that has no authoritative attendance input. The event
/// still receives the complete 15–14 period statutory SGK/PEK capacity (30
/// normalized days), while attendance-derived meal entitlements and
/// sick-leave inputs remain empty.
pub fn resolve_statutory_snapshot_for_payment_month(
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<ResolvedStatutorySnapshot> {
    resolve_statutory_snapshot_internal(None, period, settings, &[])
}

fn resolve_statutory_snapshot_internal(
    attendance: Option<&PersonelPuantaj>,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
    paid_sick_dates: &[NaiveDate],
) -> Result<ResolvedStatutorySnapshot> {
    let settings = effective_statutory_settings(settings)?;
    validate_statutory_segments_for_period(period, &settings)?;
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let full_calendar_coverage = attendance
        .map(|attendance| attendance_has_full_calendar_coverage(attendance, period))
        .transpose()?
        .unwrap_or(false);
    let full_calendar_has_non_prim_day = if full_calendar_coverage {
        attendance.is_some_and(|attendance| {
            attendance.gunler.iter().any(|(date_text, code)| {
                let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
                    return true;
                };
                !is_prim_bearing_code(code, date, paid_sick_dates)
            })
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
        if let Some(attendance) = attendance {
            for (date_text, code) in &attendance.gunler {
                let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| {
                    DomainError::InvalidData(format!("Puantaj tarihi geçersiz: {}", date_text))
                })?;
                if date < *range_start || date > range_end {
                    continue;
                }
                if is_prim_bearing_code(code, date, paid_sick_dates) {
                    segment_sgk_days += if full_calendar_coverage && !full_calendar_has_non_prim_day
                    {
                        full_period_sgk_day_weight(date, start)?
                    } else {
                        1
                    };
                }
                if matches!(code.as_str(), "Ç" | "GÇ") {
                    worked_days += 1;
                }
            }
        } else {
            // A supplementary event is not allowed to turn missing attendance
            // into a zero PEK ceiling. Use the statutory 30-day normalized
            // capacity and keep attendance-derived meal days at zero.
            let mut date = *range_start;
            while date <= range_end {
                segment_sgk_days += full_period_sgk_day_weight(date, start)?;
                date = date.succ_opt().ok_or_else(|| {
                    DomainError::InvalidData("Yasal segment tarih aralığı çözülemedi.".into())
                })?;
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
        source: attendance
            .map(|_| StatutorySnapshotSource::AttendanceBacked)
            .unwrap_or(StatutorySnapshotSource::ProvisionalPaymentMonth),
        segments: snapshots,
        sgkPrimGunSayisi: total_sgk_days,
        pekAltSinir: pek_alt_sinir.round_dp(2),
        pekUstSinir: pek_ust_sinir.round_dp(2),
        sgkYemekIstisnasiToplam: sgk_meal_total.round_dp(2),
        gvYemekIstisnasiToplam: gv_meal_total.round_dp(2),
        gvReferansGunlukAsgariUcret: gv_reference,
        sgkIsciOraniYuzde: settings.sgkIsciOraniYuzde,
        issizlikIsciOraniYuzde: settings.issizlikIsciOraniYuzde,
    })
}

pub fn calculate_paid_sick_dates_from_records(
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

pub(crate) fn find_zam_tarihi(period: &BordroDonemi, zam_aylari: &[i32]) -> Result<Option<NaiveDate>> {
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

/// Builds the person-specific meal earning slices that correspond to the
/// already-resolved statutory segments.  A wage change can split one legal
/// segment into two earning slices; both slices retain the legal capacity of
/// their dates.  The resulting totals are the only meal-exemption inputs used
/// by normal SGK, GV, and stamp-tax calculations.
fn calculate_normal_meal_exemptions(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
    statutory_snapshot: &ResolvedStatutorySnapshot,
    current_settings: &DonemselKurumDegerleri,
    previous_settings: Option<&DonemselKurumDegerleri>,
    raise_date: Option<NaiveDate>,
) -> Result<MealExemptionTotals> {
    let mut earning_segments = Vec::new();

    for statutory_segment in &statutory_snapshot.segments {
        let segment_start = parse_period_date(
            &statutory_segment.effectiveFrom,
            &period.id,
            "statutory snapshot başlangıç",
        )?;
        let segment_end = parse_period_date(
            &statutory_segment.effectiveTo,
            &period.id,
            "statutory snapshot bitiş",
        )?;
        let mut before_raise_days = 0;
        let mut after_raise_days = 0;

        for (date_text, code) in &attendance.gunler {
            if !matches!(code.as_str(), "Ç" | "GÇ") {
                continue;
            }
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|error| {
                DomainError::InvalidData(format!(
                    "{} döneminde puantaj tarihi geçersiz: {} ({})",
                    period.id, date_text, error
                ))
            })?;
            if date < segment_start || date > segment_end {
                continue;
            }
            if raise_date.is_some_and(|cutoff| date < cutoff) {
                before_raise_days += 1;
            } else {
                after_raise_days += 1;
            }
        }

        if before_raise_days + after_raise_days != statutory_segment.fiiliYemekGunu {
            return Err(DomainError::InvalidData(format!(
                "{} döneminde statutory yemek günü ile puantaj yemek günü eşleşmiyor ({} / {}).",
                period.id,
                statutory_segment.fiiliYemekGunu,
                before_raise_days + after_raise_days
            )));
        }

        if before_raise_days > 0 {
            let settings = previous_settings.ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} döneminde zam öncesi yemek ayarı çözümlenemedi.",
                    period.id
                ))
            })?;
            earning_segments.push(MealExemptionSegment {
                actual: calculate_meal_income(before_raise_days, settings.gunlukYemek),
                sgk_capacity: statutory_segment.gunlukYemekIstisnasiSGK
                    * Decimal::from(before_raise_days),
                gv_capacity: statutory_segment.gunlukYemekIstisnasiGV
                    * Decimal::from(before_raise_days),
            });
        }
        if after_raise_days > 0 {
            earning_segments.push(MealExemptionSegment {
                actual: calculate_meal_income(after_raise_days, current_settings.gunlukYemek),
                sgk_capacity: statutory_segment.gunlukYemekIstisnasiSGK
                    * Decimal::from(after_raise_days),
                gv_capacity: statutory_segment.gunlukYemekIstisnasiGV
                    * Decimal::from(after_raise_days),
            });
        }
    }

    Ok(calculate_segmented_meal_exemptions(&earning_segments))
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

fn period_by_id<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    index: &'a PayrollDatasetIndex,
    id: &str,
) -> Result<&'a BordroDonemi> {
    index
        .period(dataset, id)
        .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", id)))
}

fn normal_attendance<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    index: &'a PayrollDatasetIndex,
    personnel_id: &str,
    period_id: &str,
) -> Result<&'a PersonelPuantaj> {
    index
        .attendances(dataset, personnel_id, period_id)
        .into_iter()
        .find(|attendance| !attendance.gunler.is_empty())
        .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))
}

fn attendance_missing_calendar_days(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
) -> Result<Vec<String>> {
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    let mut missing = Vec::new();
    let mut current = start;
    while current <= end {
        let date_text = current.format("%Y-%m-%d").to_string();
        if !attendance.gunler.contains_key(&date_text) {
            missing.push(date_text);
        }
        current = current
            .checked_add_signed(Duration::days(1))
            .ok_or_else(|| DomainError::InvalidData("Puantaj tarih aralığı çözülemedi.".into()))?;
    }
    Ok(missing)
}

fn attendance_has_full_calendar_coverage(
    attendance: &PersonelPuantaj,
    period: &BordroDonemi,
) -> Result<bool> {
    let start = parse_period_date(&period.baslangicTarihi, &period.id, "başlangıç")?;
    let end = parse_period_date(&period.bitisTarihi, &period.id, "bitiş")?;
    Ok(
        attendance.gunler.len() as i64 == (end - start).num_days() + 1
            && attendance_missing_calendar_days(attendance, period)?.is_empty(),
    )
}

fn existing_payroll<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    index: &'a PayrollDatasetIndex,
    personnel_id: &str,
    period_id: &str,
) -> Option<&'a BordroKaydi> {
    index
        .payrolls_for_person_period(dataset, personnel_id, period_id)
        .into_iter()
        .find(|payroll| payroll.accrualType == AccrualType::NORMAL)
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
pub struct AccrualOrder {
    tax_ordinal: i64,
    payment_date: NaiveDate,
    sequence: i32,
    accrual_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct IncomingDevredenPekState {
    pub(crate) records: Vec<DevredenPekKaydi>,
    pub(crate) tax_months_elapsed: i32,
}

#[derive(Debug, Clone)]
struct RetroSourceCarryOverride {
    batch_id: String,
    source_period_id: String,
    source_tax_ordinal: i64,
    original_carry: Vec<DevredenPekKaydi>,
    target_carry: Vec<DevredenPekKaydi>,
    payment_date: NaiveDate,
}

fn retro_source_carry_overrides_for_event(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    current_order: &AccrualOrder,
) -> Result<Vec<RetroSourceCarryOverride>> {
    let mut by_source = BTreeMap::<String, RetroSourceCarryOverride>::new();
    for batch in index
        .retro_batches_for_person(dataset, personnel_id)
        .filter(|batch| {
            matches!(
                batch.status,
                CompensationRevisionStatus::CALCULATED | CompensationRevisionStatus::FINALIZED
            )
        })
    {
        let payment_date = parse_period_date(&batch.paymentDate, &batch.id, "retro ödeme")?;
        if payment_date > current_order.payment_date {
            continue;
        }
        for allocation in index.retro_allocations_for_batch(dataset, &batch.id) {
            let Some(target_carry) = allocation.targetSourceCarry.clone() else {
                continue;
            };
            let period = period_by_id(dataset, index, &allocation.sourcePeriodId)?;
            let source_tax_ordinal = tax_ordinal(period.taxYear, period.taxMonth);
            if source_tax_ordinal > current_order.tax_ordinal {
                continue;
            }
            let candidate = RetroSourceCarryOverride {
                batch_id: batch.id.clone(),
                source_period_id: allocation.sourcePeriodId.clone(),
                source_tax_ordinal,
                original_carry: allocation.originalSourceCarry.clone().unwrap_or_default(),
                target_carry,
                payment_date,
            };
            validate_retro_source_carry(
                &candidate.original_carry,
                &format!("{} original source carry", allocation.id),
            )?;
            validate_retro_source_carry(
                &candidate.target_carry,
                &format!("{} target source carry", allocation.id),
            )?;
            if let Some(existing) = by_source.get(&candidate.source_period_id) {
                if existing.batch_id == candidate.batch_id {
                    return Err(DomainError::InvalidData(format!(
                        "{} batch'inde aynı source period için birden fazla carry snapshot'ı var.",
                        candidate.batch_id
                    )));
                }
                if (candidate.payment_date, candidate.batch_id.clone())
                    <= (existing.payment_date, existing.batch_id.clone())
                {
                    continue;
                }
            }
            by_source.insert(candidate.source_period_id.clone(), candidate);
        }
    }
    let mut overrides = by_source.into_values().collect::<Vec<_>>();
    overrides.sort_by(|left, right| {
        left.source_tax_ordinal
            .cmp(&right.source_tax_ordinal)
            .then_with(|| left.source_period_id.cmp(&right.source_period_id))
            .then_with(|| left.payment_date.cmp(&right.payment_date))
            .then_with(|| left.batch_id.cmp(&right.batch_id))
    });
    Ok(overrides)
}

fn carry_override_changed(override_state: &RetroSourceCarryOverride) -> bool {
    override_state.original_carry != override_state.target_carry
}

fn validate_retro_source_carry(records: &[DevredenPekKaydi], field: &str) -> Result<()> {
    if records
        .iter()
        .any(|record| record.tutar < Decimal::ZERO || record.kalanAySayisi < 0)
    {
        return Err(DomainError::InvalidData(format!(
            "{} negatif tutar veya negatif kalan ay içeriyor.",
            field
        )));
    }
    Ok(())
}

fn carry_override_for_source_period<'a>(
    overrides: &'a [RetroSourceCarryOverride],
    source_period_id: &str,
) -> Option<&'a RetroSourceCarryOverride> {
    overrides
        .iter()
        .find(|item| item.source_period_id == source_period_id && carry_override_changed(item))
}

fn incoming_from_retro_source_carry(
    override_state: &RetroSourceCarryOverride,
    current_order: &AccrualOrder,
) -> Result<IncomingDevredenPekState> {
    let tax_months_elapsed =
        tax_month_distance(current_order.tax_ordinal, override_state.source_tax_ordinal)?;
    if tax_months_elapsed < 0 {
        return Err(DomainError::InvalidData(
            "Retro source carry mevcut vergi ayından ileri olamaz.".into(),
        ));
    }
    Ok(IncomingDevredenPekState {
        records: override_state.target_carry.clone(),
        tax_months_elapsed,
    })
}

fn retro_event_contains_carry_override(
    overrides: &[RetroSourceCarryOverride],
    accrual_id: &str,
) -> bool {
    overrides
        .iter()
        .any(|item| item.batch_id == accrual_id && carry_override_changed(item))
}

pub(crate) fn accrual_order_for_input(
    period: &BordroDonemi,
    input: &PayrollAccrualInput,
) -> Result<AccrualOrder> {
    payment_event_order(period, &input.paymentDate, input.sequence, &input.accrualId)
}

pub fn payment_event_order(
    period: &BordroDonemi,
    payment_date: &str,
    sequence: i32,
    accrual_id: &str,
) -> Result<AccrualOrder> {
    let parsed_payment_date = parse_period_date(payment_date, &period.id, "ödeme/tahakkuk")?;
    Ok(AccrualOrder {
        // Payment date is the event-level source of truth for new records.
        // Legacy records retain their period-compatible date and therefore
        // produce the same ordinal as before.
        tax_ordinal: tax_ordinal(
            parsed_payment_date.year(),
            parsed_payment_date.month() as i32,
        ),
        payment_date: parsed_payment_date,
        sequence,
        accrual_id: accrual_id.into(),
    })
}

pub fn accrual_order_for_payroll(
    dataset: &PayrollDatasetSnapshot,
    payroll: &BordroKaydi,
) -> Result<AccrualOrder> {
    let index = PayrollDatasetIndex::build(dataset);
    accrual_order_for_payroll_with_index(dataset, &index, payroll)
}

/// Resolves one payment event using a caller-owned dataset index. Adapters
/// that order several records can build the index once and preserve the same
/// explicit payment-event ordering semantics as the calculation engine.
pub fn accrual_order_for_payroll_with_index(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    payroll: &BordroKaydi,
) -> Result<AccrualOrder> {
    let period = period_by_id(dataset, index, &payroll.donemId)?;
    payment_event_order(
        period,
        &effective_payment_date(payroll, period),
        payroll.sequence,
        &effective_accrual_id(payroll),
    )
}

fn ordered_prior_payment_events<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    index: &'a PayrollDatasetIndex,
    personnel_id: &str,
    current_order: &AccrualOrder,
) -> Result<Vec<(AccrualOrder, &'a BordroKaydi)>> {
    let mut events = index
        .payrolls_for_person(dataset, personnel_id)
        .map(|payroll| {
            Ok((
                accrual_order_for_payroll_with_index(dataset, index, payroll)?,
                payroll,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    events.retain(|(order, _)| order < current_order);
    events.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(events)
}

pub(crate) fn ensure_authoritative_payment_event(payroll: &BordroKaydi) -> Result<()> {
    if payroll.status == BordroStatus::FINALIZED && is_provisional_supplementary_payroll(payroll) {
        return Err(DomainError::InvalidData(format!(
            "{} tahakkuku FINALIZED durumda ancak geçici/legacy statutory snapshot taşıyor; kayıt sessizce değiştirilemez. Migration veya veri kalitesi incelemesi gerekir.",
            effective_accrual_id(payroll)
        )));
    }
    match payroll.status {
        BordroStatus::CALCULATED | BordroStatus::FINALIZED => Ok(()),
        BordroStatus::DRAFT | BordroStatus::STALE => Err(DomainError::ValidationError(format!(
            "Payment-event/PEK zinciri çözülemez: {} tahakkuku {} durumda; authoritative state belirlenemiyor.",
            effective_accrual_id(payroll),
            match payroll.status {
                BordroStatus::DRAFT => "DRAFT",
                BordroStatus::STALE => "STALE",
                _ => unreachable!(),
            }
        ))),
    }
}

pub fn is_provisional_supplementary_payroll(payroll: &BordroKaydi) -> bool {
    if payroll.accrualType == AccrualType::RETRO_ADJUSTMENT {
        // A retro event carries source-month SGK snapshots in its allocations;
        // its payment-month statutory snapshot is used only for GV/DV and any
        // explicitly payment-month non-wage PEK. It is not a provisional
        // supplementary event.
        return false;
    }
    payroll.accrualType != AccrualType::NORMAL
        && !matches!(
            payroll
                .statutorySnapshot
                .as_ref()
                .map(|snapshot| snapshot.source),
            Some(StatutorySnapshotSource::AttendanceBacked)
        )
}

fn ensure_finalizable_statutory_snapshot(payroll: &BordroKaydi) -> Result<()> {
    if is_provisional_supplementary_payroll(payroll) {
        return Err(DomainError::ValidationError(
            "Bu tahakkuk geçici 30 günlük SGK/PEK kapasitesiyle hesaplandı. Kesinleştirme için SGK prim günü bilgisinin authoritative olması gerekir.".into(),
        ));
    }
    Ok(())
}

fn payroll_for_requested_accrual<'a>(
    dataset: &'a PayrollDatasetSnapshot,
    index: &'a PayrollDatasetIndex,
    personnel_id: &str,
    period_id: &str,
    requested: Option<&PayrollAccrualInput>,
) -> Option<&'a BordroKaydi> {
    if let Some(requested) = requested.filter(|input| !input.accrualId.trim().is_empty()) {
        return index.payroll_for_accrual(dataset, personnel_id, period_id, &requested.accrualId);
    }
    existing_payroll(dataset, index, personnel_id, period_id)
}

fn resolve_accrual_input(
    request: &PayrollCalculationRequest,
    period: &BordroDonemi,
    index: &PayrollDatasetIndex,
) -> Result<PayrollAccrualInput> {
    let existing = payroll_for_requested_accrual(
        &request.dataset,
        index,
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
            sequence: index
                .payrolls_for_person(&request.dataset, &request.personnelId)
                .filter(|event| event.personelId == request.personnelId)
                .filter(|event| {
                    index
                        .period(&request.dataset, &event.donemId)
                        .is_some_and(|owner| {
                            owner.taxYear == period.taxYear
                                && owner.taxMonth == period.taxMonth
                                && effective_payment_date(event, owner)
                                    == default_payment_date(period)
                        })
                })
                .map(|event| {
                    event.sequence.checked_add(1).ok_or_else(|| {
                        DomainError::InvalidData("Tahakkuk sıra numarası taştı.".into())
                    })
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .max()
                .unwrap_or(0),
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
    if input.accrualType == AccrualType::RETRO_ADJUSTMENT && input.grossAmount.is_none() {
        let batch = index
            .retro_batch(&request.dataset, &input.accrualId)
            .ok_or_else(|| {
                DomainError::NotFound(format!(
                    "Retro adjustment batch bulunamadı: {}",
                    input.accrualId
                ))
            })?;
        input.grossAmount = Some(retro_payable_settlement_amount(batch));
    }
    let payment_date = parse_period_date(&input.paymentDate, &period.id, "ödeme/tahakkuk")?;
    if payment_date.year() != period.taxYear || payment_date.month() as i32 != period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "Tahakkuk ödeme tarihi {} vergi yılı/ayı {}-{:02} ile uyumlu değil.",
            input.paymentDate, period.taxYear, period.taxMonth
        )));
    }

    let normal_count = index
        .payrolls_for_person_period(&request.dataset, &request.personnelId, &request.periodId)
        .filter(|payroll| payroll.accrualType == AccrualType::NORMAL)
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

    for payroll in index.payrolls_for_accrual(&request.dataset, &input.accrualId) {
        if payroll.personelId != request.personnelId || payroll.donemId != request.periodId {
            return Err(DomainError::ValidationError(format!(
                "Tahakkuk kimliği {} başka bir personel veya dönemde zaten kullanılıyor.",
                input.accrualId
            )));
        }
    }

    for payroll in index.payrolls_for_person(&request.dataset, &request.personnelId) {
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
        let payroll_order = accrual_order_for_payroll_with_index(&request.dataset, index, payroll)?;
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
    index: &'a PayrollDatasetIndex,
    personnel_id: &str,
    period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<Vec<&'a BordroKaydi>> {
    let current_order = accrual_order_for_input(period, current)?;
    ordered_prior_payment_events(dataset, index, personnel_id, &current_order)?
        .into_iter()
        .filter(|(order, _)| order.tax_ordinal == current_order.tax_ordinal)
        // DRAFT/STALE rows are retained for audit, but they are not payment
        // events in the authoritative tax/PEK chain. Counting one here would
        // consume the same-month GV/DV/PEK state without a valid entitlement.
        .filter(|(_, payroll)| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        })
        .map(|(_, payroll)| {
            ensure_authoritative_payment_event(payroll)?;
            Ok(payroll)
        })
        .collect()
}

fn validate_prior_accruals_finalized(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    current_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<()> {
    let current_order = accrual_order_for_input(current_period, current)?;
    for payroll in index.payrolls_for_person(dataset, personnel_id) {
        let order = accrual_order_for_payroll_with_index(dataset, index, payroll)?;
        if order >= current_order {
            continue;
        }
        if payroll.status == BordroStatus::FINALIZED {
            ensure_authoritative_payment_event(payroll)?;
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
        // New records carry the reconciled same-month state explicitly.  This
        // prevents a later authoritative NORMAL calculation from summing an
        // earlier provisional 30-day PEK snapshot as if it were final.
        let next = match detail.aylikSonrasiPekTuketimi {
            Some(value) => value,
            None => total.checked_add(detail.primMatrahi).ok_or_else(|| {
                DomainError::InvalidData("Aynı-ay PEK toplamında Decimal taşması oluştu.".into())
            })?,
        };
        let total = total.checked_add(next - total).ok_or_else(|| {
            DomainError::InvalidData("Aynı-ay PEK toplamında Decimal taşması oluştu.".into())
        })?;
        Ok(round2(total))
    })
}

#[derive(Debug, Default)]
struct SameMonthPekReconciliation {
    month_to_date_for_calculation: Decimal,
    month_used_after: Option<Decimal>,
    outgoing_non_wage: Vec<DevredenPekKaydi>,
    active: bool,
}

fn canonical_event_pek_components(payroll: &BordroKaydi) -> (Decimal, Decimal) {
    let meal = payroll.gelirler.yemek.unwrap_or_default();
    let meal_exemption = payroll
        .pekDetay
        .as_ref()
        .map(|detail| detail.yemekIstisnasiTutar)
        .unwrap_or_default();
    let sgk_tabi_yemek = (meal - meal_exemption).max(Decimal::ZERO);
    let (wage, non_wage) = canonical_sgk_income_components(&payroll.gelirler, sgk_tabi_yemek);
    (wage.max(Decimal::ZERO), non_wage.max(Decimal::ZERO))
}

/// Reconciles a prior provisional payment-month PEK allocation when an
/// attendance-backed event reveals a lower monthly ceiling.  Wage earnings
/// are allocated before non-wage earnings; only the canonical non-wage excess
/// becomes carry.  Prior FINALIZED rows are never rewritten.
fn reconcile_same_month_pek(
    prior: &[&BordroKaydi],
    current_income: &GelirKalemleri,
    current_is_normal: bool,
    current_meal_exemption: Decimal,
    statutory_snapshot: &ResolvedStatutorySnapshot,
    persisted_month_to_date: Decimal,
) -> SameMonthPekReconciliation {
    let has_provisional_prior = prior.iter().any(|payroll| {
        payroll.accrualType != AccrualType::NORMAL
            && (is_provisional_supplementary_payroll(payroll)
                || payroll.pekDetay.as_ref().is_some_and(|detail| {
                    detail.pekUstSinir > statutory_snapshot.pekUstSinir
                }))
    });
    if !current_is_normal
        || persisted_month_to_date <= statutory_snapshot.pekUstSinir
        || !has_provisional_prior
    {
        return SameMonthPekReconciliation {
            month_to_date_for_calculation: persisted_month_to_date,
            ..SameMonthPekReconciliation::default()
        };
    }

    let (prior_wage, prior_non_wage) = prior.iter().fold(
        (Decimal::ZERO, Decimal::ZERO),
        |(wage_total, non_wage_total), payroll| {
            let (wage, non_wage) = canonical_event_pek_components(payroll);
            (wage_total + wage, non_wage_total + non_wage)
        },
    );
    let sgk_tabi_yemek = (current_income.yemek.unwrap_or_default() - current_meal_exemption)
        .max(Decimal::ZERO);
    let (current_wage, current_non_wage) =
        canonical_sgk_income_components(current_income, sgk_tabi_yemek);
    let current_wage = current_wage.max(Decimal::ZERO);
    let current_non_wage = current_non_wage.max(Decimal::ZERO);
    let capacity = statutory_snapshot.pekUstSinir.max(Decimal::ZERO);
    let prior_wage_used = prior_wage.min(capacity);
    let month_to_date_for_calculation = if current_wage > Decimal::ZERO {
        prior_wage_used
    } else {
        (prior_wage + prior_non_wage).min(capacity)
    };
    let wage_used = (prior_wage + current_wage).min(capacity);
    let non_wage_capacity = (capacity - wage_used).max(Decimal::ZERO);
    let non_wage_used = (prior_non_wage + current_non_wage).min(non_wage_capacity);
    let month_used_after = round2(wage_used + non_wage_used);
    let excess_non_wage = round2(
        (prior_non_wage + current_non_wage - non_wage_used).max(Decimal::ZERO),
    );
    let outgoing_non_wage = if excess_non_wage > Decimal::ZERO {
        vec![DevredenPekKaydi {
            tutar: excess_non_wage,
            kalanAySayisi: 2,
            kaynakDonemId: None,
        }]
    } else {
        Vec::new()
    };

    SameMonthPekReconciliation {
        month_to_date_for_calculation: round2(month_to_date_for_calculation),
        month_used_after: Some(month_used_after),
        outgoing_non_wage,
        active: true,
    }
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

pub(crate) fn find_previous_work_period<'a>(
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
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    active_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<IncomingDevredenPekState> {
    let current_order = accrual_order_for_input(active_period, current)?;
    let ordered_prior = ordered_prior_payment_events(dataset, index, personnel_id, &current_order)?;
    let retro_carry_overrides =
        retro_source_carry_overrides_for_event(dataset, index, personnel_id, &current_order)?;
    let changed_overrides = retro_carry_overrides
        .iter()
        .filter(|item| carry_override_changed(item))
        .collect::<Vec<_>>();
    let prior_same_month: Vec<&BordroKaydi> = ordered_prior
        .iter()
        .filter(|(order, _)| order.tax_ordinal == current_order.tax_ordinal)
        .map(|(_, payroll)| *payroll)
        .collect();
    for payroll in &prior_same_month {
        ensure_authoritative_payment_event(payroll)?;
    }
    if let Some(previous_accrual) = prior_same_month.last() {
        if let Some(override_state) =
            carry_override_for_source_period(&retro_carry_overrides, &previous_accrual.donemId)
        {
            return incoming_from_retro_source_carry(override_state, &current_order);
        }
        if changed_overrides.iter().any(|override_state| {
            override_state.source_tax_ordinal <= current_order.tax_ordinal
                && !retro_event_contains_carry_override(
                    &retro_carry_overrides,
                    &effective_accrual_id(previous_accrual),
                )
        }) {
            return Err(DomainError::ValidationError(
                "Retro source carry downstream aynı vergi ayındaki mevcut payment-event zinciriyle çakışıyor; tarihçe sessizce yeniden yazılamaz, downstream replay gerekir."
                    .into(),
            ));
        }
        return Ok(IncomingDevredenPekState {
            records: previous_accrual
                .sonrakiDevredenPek
                .clone()
                .unwrap_or_default(),
            tax_months_elapsed: 0,
        });
    };

    let Some(previous_tax_ordinal) = ordered_prior
        .iter()
        .map(|(order, _)| order.tax_ordinal)
        .max()
    else {
        if let Some(override_state) = changed_overrides.last() {
            return incoming_from_retro_source_carry(override_state, &current_order);
        }
        return Ok(IncomingDevredenPekState {
            records: Vec::new(),
            tax_months_elapsed: 0,
        });
    };
    let previous_month: Vec<(AccrualOrder, &BordroKaydi)> = ordered_prior
        .iter()
        .map(|(order, payroll)| (order.clone(), *payroll))
        .filter(|(order, _)| order.tax_ordinal == previous_tax_ordinal)
        .collect();
    for (_, previous_payroll) in &previous_month {
        ensure_authoritative_payment_event(previous_payroll)?;
    }
    let Some((previous_order, previous_payroll)) = previous_month.last() else {
        if let Some(override_state) = changed_overrides.last() {
            return incoming_from_retro_source_carry(override_state, &current_order);
        }
        return Ok(IncomingDevredenPekState {
            records: Vec::new(),
            tax_months_elapsed: 0,
        });
    };
    let tax_months_elapsed =
        tax_month_distance(current_order.tax_ordinal, previous_order.tax_ordinal)?;
    if tax_months_elapsed <= 0 {
        return Err(DomainError::InvalidData(
            "Devreden PEK kaynağı mevcut vergi ayından ileri veya aynı ayda çözümlenemedi.".into(),
        ));
    }
    if let Some(override_state) =
        carry_override_for_source_period(&retro_carry_overrides, &previous_payroll.donemId)
    {
        return incoming_from_retro_source_carry(override_state, &current_order);
    }
    if let Some(override_state) = changed_overrides.last() {
        if override_state.source_tax_ordinal > previous_order.tax_ordinal {
            return incoming_from_retro_source_carry(override_state, &current_order);
        }
        if !retro_event_contains_carry_override(
            &retro_carry_overrides,
            &effective_accrual_id(previous_payroll),
        ) {
            return Err(DomainError::ValidationError(
                "Retro source carry downstream payment-event zincirini etkiliyor; historical event sessizce değiştirilmeden önce downstream replay yapılmalıdır."
                    .into(),
            ));
        }
    }
    Ok(IncomingDevredenPekState {
        records: previous_payroll
            .sonrakiDevredenPek
            .clone()
            .unwrap_or_default(),
        tax_months_elapsed,
    })
}

/// Replays the incoming carry for a historical payment event while retaining
/// the legacy opening-carry case where no earlier event exists.  The boolean
/// is true when the canonical chain (or a retro carry override) is authoritative
/// for this event; false means the persisted opening carry is the only input.
pub(crate) fn incoming_devreden_pek_for_replay(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    period: &BordroDonemi,
    payroll: &BordroKaydi,
) -> Result<(IncomingDevredenPekState, bool)> {
    let current_order = accrual_order_for_payroll_with_index(dataset, index, payroll)?;
    let has_prior_events =
        !ordered_prior_payment_events(dataset, index, personnel_id, &current_order)?.is_empty();
    let has_retro_carry_override =
        retro_source_carry_overrides_for_event(dataset, index, personnel_id, &current_order)?
            .iter()
            .any(carry_override_changed);
    let state = incoming_devreden_pek(
        dataset,
        index,
        personnel_id,
        period,
        &PayrollAccrualInput {
            accrualId: effective_accrual_id(payroll),
            accrualType: payroll.accrualType,
            paymentDate: effective_payment_date(payroll, period),
            sequence: payroll.sequence,
            grossAmount: None,
            description: payroll.accrualDescription.clone(),
        },
    )?;
    Ok((state, has_prior_events || has_retro_carry_override))
}

fn payroll_gv_base(payroll: &BordroKaydi) -> Result<Decimal> {
    crate::validate_gv_base_reconciliation(payroll)?;
    let base = payroll
        .gvDetay
        .as_ref()
        .map(|detail| detail.cariGvMatrahi)
        .or(payroll.persistedGvBase)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} tahakkukunda authoritative GV matrahı eksik; legacy kayıt açık migration olmadan kümülatif zincire alınamaz.",
                effective_accrual_id(payroll)
            ))
        })?;
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

fn tax_month_distance(current_tax_ordinal: i64, source_tax_ordinal: i64) -> Result<i32> {
    let distance = current_tax_ordinal
        .checked_sub(source_tax_ordinal)
        .ok_or_else(|| DomainError::InvalidData("Vergi ayı farkı çözülemedi.".into()))?;
    if distance < 0 {
        return Err(DomainError::InvalidData(
            "Vergi ayı kaynağı mevcut vergi ayından ileri olamaz.".into(),
        ));
    }
    i32::try_from(distance)
        .map_err(|_| DomainError::InvalidData("Vergi ayı farkı desteklenen aralığı aşıyor.".into()))
}

fn previous_gv(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    active_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<Decimal> {
    let openings = resolve_tax_openings(dataset, index, person, active_period)?;
    let (start_month, opening_value) = openings
        .normal
        .map(|opening| (opening.start_tax_month, opening.value))
        .unwrap_or((1, Decimal::ZERO));
    if start_month > active_period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "GV opening başlangıç vergi ayı {} aktif vergi ayı {} sonrasında olamaz.",
            start_month, active_period.taxMonth
        )));
    }

    if openings.normal.is_some()
        && index
            .payrolls_for_person(dataset, &person.id)
            .any(|payroll| {
                matches!(
                    payroll.status,
                    BordroStatus::CALCULATED | BordroStatus::FINALIZED
                ) && index
                    .period(dataset, &payroll.donemId)
                    .is_some_and(|period| {
                        period.taxYear == active_period.taxYear && period.taxMonth < start_month
                    })
            })
    {
        return Err(DomainError::TaxOpeningConflict(
            "Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır.".into(),
        ));
    }

    let active_order = accrual_order_for_input(active_period, current)?;
    let mut prior = opening_value;
    for payroll in index
        .payrolls_for_person(dataset, &person.id)
        .filter(|payroll| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        })
    {
        let period = index.period(dataset, &payroll.donemId).ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} bordrosunun dönemi bulunamadı; kümülatif GV zinciri eksik.",
                payroll.id
            ))
        })?;
        let before_current = period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll_with_index(dataset, index, payroll)?
                        < active_order));
        if before_current {
            prior = prior
                .checked_add(payroll_gv_base(payroll)?)
                .ok_or_else(|| {
                    DomainError::InvalidData("Kümülatif GV Decimal taşması oluştu.".into())
                })?;
        }
    }
    for payroll in index
        .payrolls_for_person(dataset, &person.id)
        .filter(|payroll| matches!(payroll.status, BordroStatus::DRAFT | BordroStatus::STALE))
    {
        let period = index.period(dataset, &payroll.donemId).ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} bordrosunun dönemi bulunamadı; kümülatif GV zinciri eksik.",
                payroll.id
            ))
        })?;
        let before_current = period.taxYear == active_period.taxYear
            && period.taxMonth >= start_month
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll_with_index(dataset, index, payroll)?
                        < active_order));
        if before_current {
            return Err(DomainError::ValidationError(
                "Önceki vergi zincirinde DRAFT/STALE bordro var. Kümülatif GV hesabına devam etmeden önce bu bordroları yeniden hesaplayın.".into(),
            ));
        }
    }
    Ok(round2(prior))
}

#[derive(Debug, Clone, Copy)]
struct ResolvedTaxOpening {
    value: Decimal,
    start_tax_month: i32,
}

#[derive(Debug, Clone, Default)]
struct ResolvedTaxOpenings {
    normal: Option<ResolvedTaxOpening>,
    asgari: Option<ResolvedTaxOpening>,
}

fn resolve_effective_period_tax_month(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    year: i32,
    period_id: &str,
    opening_label: &str,
) -> Result<i32> {
    let period = index.period(dataset, period_id).ok_or_else(|| {
        DomainError::ValidationError(format!(
            "{} effective başlangıç dönemi bulunamadı: {}.",
            opening_label, period_id
        ))
    })?;
    if period.taxYear != year {
        return Err(DomainError::ValidationError(format!(
            "{} effective dönemi {} vergi yılına ait değil; bulunan vergi yılı {}, beklenen vergi yılı {}.",
            opening_label, period.id, period.taxYear, year
        )));
    }
    Ok(period.taxMonth)
}

/// Resolves an old month-only value only when the loaded period set proves a
/// single interpretation. Work-month and tax-month candidates are both
/// considered; two different candidates are deliberately rejected.
fn resolve_legacy_effective_tax_month(
    dataset: &PayrollDatasetSnapshot,
    year: i32,
    raw_month: Option<i32>,
    opening_label: &str,
) -> Result<i32> {
    let raw_month = raw_month.ok_or_else(|| {
        DomainError::ValidationError(format!(
            "{} için legacy başlangıç ayı period ID ile çözülemiyor; explicit effectiveFromPeriodId girin.",
            opening_label
        ))
    })?;
    if !(1..=12).contains(&raw_month) {
        return Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı 1-12 arasında olmalıdır.",
            opening_label
        )));
    }

    let mut candidates = dataset
        .periods
        .iter()
        .filter(|period| {
            period.taxYear == year && (period.ay == raw_month || period.taxMonth == raw_month)
        })
        .map(|period| (period.id.as_str(), period.taxMonth))
        .collect::<Vec<_>>();
    candidates.sort_unstable_by(|left, right| left.0.cmp(right.0));
    candidates.dedup_by(|left, right| left.0 == right.0);

    match candidates.as_slice() {
        [(_, tax_month)] => Ok(*tax_month),
        [] => Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı {} için {} vergi yılında bir bordro dönemi bulunamadı; explicit effectiveFromPeriodId girin.",
            opening_label, raw_month, year
        ))),
        _ => Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı {} çalışma ayı/vergi ayı olarak birden fazla döneme karşılık geliyor; sessiz dönüşüm yapılmadı, explicit effectiveFromPeriodId girin.",
            opening_label, raw_month
        ))),
    }
}

fn resolve_normal_opening(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    active_period: &BordroDonemi,
    explicit: Option<&PersonelTaxOpening>,
) -> Result<Option<ResolvedTaxOpening>> {
    if let Some(opening) = explicit {
        match (
            opening.gvCumulativeOpening,
            opening.effectiveFromPeriodId.as_deref(),
        ) {
            // A modern row without a normal component leaves the legacy
            // normal component eligible for fallback.
            (None, None) => {}
            (Some(value), Some(period_id)) => {
                if value < Decimal::ZERO {
                    return Err(DomainError::ValidationError(
                        "GV opening matrahı negatif olamaz.".into(),
                    ));
                }
                // Zero is still an explicit opening. Its period boundary
                // determines which historical payrolls are in scope.
                return Ok(Some(ResolvedTaxOpening {
                    value,
                    start_tax_month: resolve_effective_period_tax_month(
                        dataset,
                        index,
                        opening.year,
                        period_id,
                        "GV opening",
                    )?,
                }));
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(DomainError::ValidationError(
                    "Normal GV opening değeri ile effectiveFromPeriodId birlikte tanımlanmalıdır."
                        .into(),
                ));
            }
        }
    }

    let value = person.devirKumulatifGvMatrahi.unwrap_or_default();
    if value < Decimal::ZERO {
        return Err(DomainError::ValidationError(
            "Legacy GV devir matrahı negatif olamaz.".into(),
        ));
    }
    let year = person
        .devirKumulatifGvMatrahiYili
        .unwrap_or(active_period.taxYear);
    if value > Decimal::ZERO && year == active_period.taxYear {
        Ok(Some(ResolvedTaxOpening {
            value,
            start_tax_month: resolve_legacy_effective_tax_month(
                dataset,
                year,
                person.devirKumulatifGvMatrahiBaslangicAyi,
                "Legacy GV opening",
            )?,
        }))
    } else {
        Ok(None)
    }
}

fn resolve_asgari_opening(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    active_period: &BordroDonemi,
    explicit: Option<&PersonelTaxOpening>,
) -> Result<Option<ResolvedTaxOpening>> {
    let (explicit_value, explicit_period_id) = if let Some(opening) = explicit {
        match (
            opening.asgariGvCumulativeOpening,
            opening.asgariGvEffectiveFromPeriodId.as_deref(),
        ) {
            // A modern row without an asgari component leaves the legacy
            // asgari component eligible for fallback.
            (None, None) => (None, None),
            (Some(value), Some(period_id)) => (Some(value), Some(period_id)),
            (Some(_), None) | (None, Some(_)) => {
                return Err(DomainError::ValidationError(
                    "Asgari GV opening değeri ile effectiveFromPeriodId birlikte tanımlanmalıdır."
                        .into(),
                ));
            }
        }
    } else {
        (None, None)
    };

    if let Some(value) = explicit_value {
        if value < Decimal::ZERO {
            return Err(DomainError::ValidationError(
                "Asgari GV opening matrahı negatif olamaz.".into(),
            ));
        }
        let opening = explicit.ok_or_else(|| {
            DomainError::InvalidData("Asgari GV explicit opening kaydı çözülemedi.".into())
        })?;
        let start_tax_month = resolve_effective_period_tax_month(
            dataset,
            index,
            opening.year,
            explicit_period_id.ok_or_else(|| {
                DomainError::InvalidData("Asgari GV explicit opening dönemi çözülemedi.".into())
            })?,
            "Asgari GV opening",
        )?;
        return Ok(
            (opening.year == active_period.taxYear).then_some(ResolvedTaxOpening {
                value,
                start_tax_month,
            }),
        );
    }

    let legacy_value = person.devirKumulatifAsgariGvMatrahi;
    if legacy_value.is_some_and(|value| value < Decimal::ZERO) {
        return Err(DomainError::ValidationError(
            "Legacy asgari GV devir matrahı negatif olamaz.".into(),
        ));
    }
    // Legacy personnel rows commonly persisted a default zero. That is not
    // an asgari opening and must not force legacy month resolution.
    let Some(value) = legacy_value.filter(|value| *value > Decimal::ZERO) else {
        return Ok(None);
    };
    let year = person
        .devirKumulatifAsgariGvMatrahiYili
        .unwrap_or(active_period.taxYear);
    if year != active_period.taxYear {
        return Ok(None);
    }
    Ok(Some(ResolvedTaxOpening {
        value,
        start_tax_month: resolve_legacy_effective_tax_month(
            dataset,
            year,
            person.devirKumulatifGvMatrahiBaslangicAyi,
            "Legacy asgari GV opening",
        )?,
    }))
}

fn resolve_tax_openings(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    active_period: &BordroDonemi,
) -> Result<ResolvedTaxOpenings> {
    let explicit = index.tax_opening(dataset, &person.id, active_period.taxYear);
    Ok(ResolvedTaxOpenings {
        normal: resolve_normal_opening(dataset, index, person, active_period, explicit)?,
        asgari: resolve_asgari_opening(dataset, index, person, active_period, explicit)?,
    })
}

fn previous_asgari_gv(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    person: &Personel,
    active_period: &BordroDonemi,
) -> Result<Decimal> {
    let openings = resolve_tax_openings(dataset, index, person, active_period)?;
    let (start_tax_month, mut cumulative) = openings
        .asgari
        .map(|opening| (opening.start_tax_month, opening.value))
        .unwrap_or((1, Decimal::ZERO));
    if start_tax_month > active_period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "Asgari GV opening başlangıç vergi ayı {} aktif vergi ayı {} sonrasında olamaz.",
            start_tax_month, active_period.taxMonth
        )));
    }

    // Opening, start tax month'tan önceki kümülatif referansı zaten içerir.
    // Yalnızca opening sonrasındaki eksik vergi ayları bir kez oluşturulur.
    for tax_month in start_tax_month..active_period.taxMonth {
        // A tax month has one minimum-wage reference entitlement regardless
        // of how many accruals it contains. The period-level statutory source
        // is shared by every employee and payment event.
        let Some(period) = index
            .periods_for_tax_month(dataset, active_period.taxYear, tax_month)
            .next()
        else {
            // `calculate_payroll` is the historical low-level formula API and
            // intentionally accepts a scoped fixture without every preceding
            // period. Production/checked calculation has already run
            // `validate_tax_month_chain`, which uses the same opening start
            // resolver and fails closed before reaching this compatibility
            // path.
            continue;
        };
        let settings = historical_statutory_settings(dataset, period)?;
        validate_kurum_degerleri_for_payroll(&settings)?;
        let statutory_snapshot = resolve_statutory_snapshot_for_payment_month(period, &settings)?;
        let sgk_rate = settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let unemployment_rate = settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::ValidationError("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let monthly_reference = calculate_aylik_asgari_ucret_gv_matrahi(
            statutory_snapshot.gvReferansGunlukAsgariUcret,
            sgk_rate,
            unemployment_rate,
        );
        cumulative = cumulative.checked_add(monthly_reference).ok_or_else(|| {
            DomainError::InvalidData("Kümülatif asgari GV Decimal taşması oluştu.".into())
        })?;
    }
    Ok(round2(cumulative))
}

fn historical_statutory_settings(
    dataset: &PayrollDatasetSnapshot,
    period: &BordroDonemi,
) -> Result<DonemselKurumDegerleri> {
    let settings = dataset.institutionSettings.get(&period.id).ok_or_else(|| {
        DomainError::InvalidData(format!(
            "{} dönemi kurum ayarları bulunamadı; asgari GV kümülatifi hesaplanamaz.",
            period.id
        ))
    })?;

    // The period-level immutable snapshot is the canonical source. It is
    // shared by every employee/event and therefore remains valid even when a
    // particular employee has no historical payroll row.
    if settings.statutoryParameterSnapshot.is_some() {
        return effective_statutory_settings(settings);
    }

    // Older databases may predate the period-level field. A persisted,
    // authoritative payroll statutory snapshot is a compatibility fallback
    // only; it is never preferred over the period-level legal source.
    let historical_snapshots = dataset
        .payrolls
        .iter()
        .filter(|payroll| {
            payroll.donemId == period.id
                && matches!(
                    payroll.status,
                    BordroStatus::CALCULATED | BordroStatus::FINALIZED
                )
        })
        .filter_map(|payroll| payroll.statutorySnapshot.as_ref())
        .collect::<Vec<_>>();
    let Some(historical_snapshot) = historical_snapshots.first() else {
        return effective_statutory_settings(settings);
    };
    if historical_snapshots
        .iter()
        .any(|snapshot| *snapshot != *historical_snapshot)
    {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi bordrolarında birbiriyle uyuşmayan historical statutory snapshot'lar bulundu; asgari GV referansı güvenilir biçimde çözülemez.",
            period.id
        )));
    }
    settings_with_resolved_statutory_snapshot(settings, historical_snapshot).map_err(|error| {
        DomainError::InvalidData(format!(
            "{} tarihsel statutory snapshot'ı kullanılamadı: {}",
            period.id, error
        ))
    })
}

fn previous_insurance_gv(
    dataset: &PayrollDatasetSnapshot,
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    active_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<Decimal> {
    let active_order = accrual_order_for_input(active_period, current)?;
    let mut total = Decimal::ZERO;
    for payroll in index
        .payrolls_for_person(dataset, personnel_id)
        .filter(|payroll| {
            matches!(
                payroll.status,
                BordroStatus::CALCULATED | BordroStatus::FINALIZED
            )
        })
    {
        let Some(period) = index.period(dataset, &payroll.donemId) else {
            continue;
        };
        let before_current = period.taxYear == active_period.taxYear
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll_with_index(dataset, index, payroll)?
                        < active_order));
        if before_current {
            if let Some(detail) = payroll.gvDetay.as_ref() {
                total += detail.uygulanabilirSigortaGvIndirimi;
            }
        }
    }
    for payroll in index
        .payrolls_for_person(dataset, personnel_id)
        .filter(|payroll| matches!(payroll.status, BordroStatus::DRAFT | BordroStatus::STALE))
    {
        let Some(period) = index.period(dataset, &payroll.donemId) else {
            continue;
        };
        let before_current = period.taxYear == active_period.taxYear
            && (period.taxMonth < active_period.taxMonth
                || (period.taxMonth == active_period.taxMonth
                    && accrual_order_for_payroll_with_index(dataset, index, payroll)?
                        < active_order));
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
    index: &PayrollDatasetIndex,
    person: &Personel,
    period: &BordroDonemi,
) -> Result<()> {
    let openings = resolve_tax_openings(dataset, index, person, period)?;
    let start_month = openings
        .asgari
        .map(|opening| opening.start_tax_month)
        .unwrap_or(1);
    if start_month > period.taxMonth {
        return Err(DomainError::ValidationError(format!(
            "Asgari GV opening başlangıç vergi ayı {} aktif vergi ayı {} sonrasında olamaz.",
            start_month, period.taxMonth
        )));
    }
    for tax_month in start_month..period.taxMonth {
        if index
            .periods_for_tax_month(dataset, period.taxYear, tax_month)
            .next()
            .is_none()
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
    index: &PayrollDatasetIndex,
    personnel_id: &str,
    active_period: &BordroDonemi,
    current: &PayrollAccrualInput,
) -> Result<()> {
    let current_order = accrual_order_for_input(active_period, current)?;
    let ordered_prior = ordered_prior_payment_events(dataset, index, personnel_id, &current_order)?;
    let retro_carry_overrides =
        retro_source_carry_overrides_for_event(dataset, index, personnel_id, &current_order)?;
    let changed_overrides = retro_carry_overrides
        .iter()
        .filter(|item| carry_override_changed(item))
        .collect::<Vec<_>>();
    let prior_same_month: Vec<&BordroKaydi> = ordered_prior
        .iter()
        .filter(|(order, _)| order.tax_ordinal == current_order.tax_ordinal)
        .map(|(_, payroll)| *payroll)
        .collect();
    for payroll in &prior_same_month {
        ensure_authoritative_payment_event(payroll)?;
    }
    if let Some(previous_accrual) = prior_same_month.last() {
        if carry_override_for_source_period(&retro_carry_overrides, &previous_accrual.donemId)
            .is_some()
        {
            return Ok(());
        }
        if changed_overrides.iter().any(|override_state| {
            override_state.source_tax_ordinal <= current_order.tax_ordinal
                && !retro_event_contains_carry_override(
                    &retro_carry_overrides,
                    &effective_accrual_id(previous_accrual),
                )
        }) {
            return Err(DomainError::ValidationError(
                "Retro source carry downstream aynı vergi ayındaki mevcut payment-event zinciriyle çakışıyor; downstream replay gerekir."
                    .into(),
            ));
        }
        return Ok(());
    }

    let Some(previous_tax_ordinal) = ordered_prior
        .iter()
        .map(|(order, _)| order.tax_ordinal)
        .max()
    else {
        return Ok(());
    };
    let previous_month: Vec<(AccrualOrder, &BordroKaydi)> = ordered_prior
        .iter()
        .map(|(order, payroll)| (order.clone(), *payroll))
        .filter(|(order, _)| order.tax_ordinal == previous_tax_ordinal)
        .collect();
    for (_, payroll) in &previous_month {
        ensure_authoritative_payment_event(payroll)?;
    }
    let Some((previous_order, previous_payroll)) = previous_month.last() else {
        return Ok(());
    };
    let tax_months_elapsed =
        tax_month_distance(current_order.tax_ordinal, previous_order.tax_ordinal)?;
    if tax_months_elapsed <= 0 {
        return Err(DomainError::InvalidData(
            "Devreden PEK vergi ayı kronolojisi geçersiz.".into(),
        ));
    }
    if carry_override_for_source_period(&retro_carry_overrides, &previous_payroll.donemId).is_some()
    {
        return Ok(());
    }
    if let Some(override_state) = changed_overrides.last() {
        if override_state.source_tax_ordinal > previous_order.tax_ordinal {
            return Ok(());
        }
        if !retro_event_contains_carry_override(
            &retro_carry_overrides,
            &effective_accrual_id(previous_payroll),
        ) {
            return Err(DomainError::ValidationError(
                "Retro source carry downstream payment-event zincirini etkiliyor; downstream replay gerekir."
                    .into(),
            ));
        }
    }
    let positive: Vec<&DevredenPekKaydi> = previous_payroll
        .sonrakiDevredenPek
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|item| item.tutar > Decimal::ZERO && item.kalanAySayisi > 0)
        .collect();
    if positive.is_empty() || tax_months_elapsed == 1 {
        return Ok(());
    }

    // A direct source-to-current tax-month distance is deterministic. If every
    // carry record expires by the current event, no artificial intermediate
    // period/event is required and no PEK is silently retained.
    if !positive
        .iter()
        .any(|item| i64::from(item.kalanAySayisi) > i64::from(tax_months_elapsed))
    {
        return Ok(());
    }

    // When a carry would still be alive at the current event, every skipped
    // tax month in its active window must have an authoritative payment-event
    // state. The event type is intentionally irrelevant.
    let first_missing_tax_ordinal = previous_order.tax_ordinal + 1;
    for tax_ordinal in first_missing_tax_ordinal..current_order.tax_ordinal {
        let events: Vec<&BordroKaydi> = ordered_prior
            .iter()
            .filter(|(order, _)| order.tax_ordinal == tax_ordinal)
            .map(|(_, payroll)| *payroll)
            .collect();
        if events.is_empty() {
            return Err(DomainError::ValidationError(format!(
                "Payment-event/PEK zinciri çözülemez: {} personelinde {} vergi ayındaki ara authoritative payment event bulunamadı; devreden PEK current event'te hâlâ geçerli.",
                personnel_id, tax_ordinal
            )));
        }
        for payroll in events {
            ensure_authoritative_payment_event(payroll)?;
        }
    }
    Ok(())
}

/// Runs the fail-closed, cross-record checks required before browser or native
/// production calculation. It does not mutate the supplied snapshot.
pub fn validate_payroll_request(request: &PayrollCalculationRequest) -> Result<()> {
    let index = PayrollDatasetIndex::build(&request.dataset);
    validate_payroll_request_with_index(request, &index)
}

fn validate_payroll_request_with_index(
    request: &PayrollCalculationRequest,
    index: &PayrollDatasetIndex,
) -> Result<()> {
    let period = index
        .period(&request.dataset, &request.periodId)
        .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", request.periodId)))?;
    let person = index
        .personnel(&request.dataset, &request.personnelId)
        .ok_or_else(|| {
            DomainError::NotFound(format!("Personel bulunamadı: {}", request.personnelId))
        })?;
    crate::validate_personnel_for_payroll(person)?;
    for payroll in &request.dataset.payrolls {
        crate::validate_payroll_snapshot_authority(payroll)?;
    }
    let annual = index.annual_parameters(&request.dataset, period.taxYear).ok_or_else(|| {
        DomainError::InvalidData(format!("{} vergi yılı yıllık bordro parametreleri eksik.", period.taxYear))
    })?;
    crate::validate_annual_payroll_parameters(annual)?;
    let accrual = resolve_accrual_input(request, period, index)?;
    let normal_count = index
        .payrolls_for_person_period(&request.dataset, &request.personnelId, &request.periodId)
        .filter(|payroll| payroll.accrualType == AccrualType::NORMAL)
        .count();
    if normal_count > 1 {
        return Err(DomainError::InvalidData(
            "Personel+dönem için birden fazla NORMAL tahakkuk bulundu; hesap zinciri güvenli biçimde çözülemez.".into(),
        ));
    }
    validate_period(period)?;
    validate_tax_month_overlap(period)?;
    validate_tax_chronology(&request.dataset, period)?;
    validate_tax_month_chain(&request.dataset, index, person, period)?;
    let raw_settings = request
        .dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                period.id
            ))
        })?;
    let settings = effective_statutory_settings(raw_settings)?;
    validate_kurum_degerleri_for_payroll(&settings)?;
    validate_statutory_tax_month_reference(period, &settings)?;
    validate_devreden_pek_gap(
        &request.dataset,
        index,
        &request.personnelId,
        period,
        &accrual,
    )?;
    resolve_prior_accrual_state(
        &request.dataset,
        index,
        &request.personnelId,
        period,
        &accrual,
    )?;
    if accrual.accrualType == AccrualType::NORMAL {
        normal_attendance(
            &request.dataset,
            index,
            &request.personnelId,
            &request.periodId,
        )?;
    }
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
    let index = PayrollDatasetIndex::build(&request.dataset);
    validate_payroll_finalization_request_with_index(request, &index)
}

fn validate_payroll_finalization_request_with_index(
    request: &PayrollCalculationRequest,
    index: &PayrollDatasetIndex,
) -> Result<()> {
    validate_payroll_request_with_index(request, index)?;

    let period = index
        .period(&request.dataset, &request.periodId)
        .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", request.periodId)))?;
    let accrual = resolve_accrual_input(request, period, index)?;
    let existing = payroll_for_requested_accrual(
        &request.dataset,
        index,
        &request.personnelId,
        &request.periodId,
        Some(&accrual),
    )
    .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?;
    match existing.status {
        BordroStatus::FINALIZED => {
            if is_provisional_supplementary_payroll(existing) {
                return Err(DomainError::InvalidData(
                    "Legacy/imported FINALIZED supplementary tahakkuk geçici statutory snapshot taşıyor; kayıt otomatik olarak yeniden yazılamaz. Migration veya veri kalitesi incelemesi gerekir.".into(),
                ));
            }
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

    validate_prior_accruals_finalized(
        &request.dataset,
        index,
        &request.personnelId,
        period,
        &accrual,
    )?;

    if accrual.accrualType == AccrualType::NORMAL {
        let attendance = normal_attendance(
            &request.dataset,
            index,
            &request.personnelId,
            &request.periodId,
        )?;
        let missing_dates = attendance_missing_calendar_days(attendance, period)?;
        if !missing_dates.is_empty() {
            return Err(DomainError::ValidationError(format!(
                "{} dönemi puantajı eksik: {} takvim günü için kayıt bulunmuyor.",
                period.id,
                missing_dates.len()
            )));
        }
    }

    // Finalization recalculates the source payroll. A later FINALIZED payroll
    // would then be an immutable downstream dependency, so the same pure
    // mutation policy must reject the operation before any adapter persists it.
    let impact = crate::policies::evaluate_payroll_invalidation_with_index(
        &request.dataset,
        &crate::policies::PayrollMutation::AccrualCalculation {
            personnelId: request.personnelId.clone(),
            periodId: request.periodId.clone(),
            accrualId: accrual.accrualId.clone(),
        },
        index,
    )?;
    if !impact.blockedByFinalized.is_empty() || !impact.blockedByFinalizedRetroBatches.is_empty() {
        let keys = impact
            .blockedByFinalized
            .iter()
            .map(|key| format!("{} / {}", key.personnelId, key.periodId))
            .collect::<Vec<_>>()
            .join(", ");
        let retro_batches = impact.blockedByFinalizedRetroBatches.join(", ");
        let detail = [
            (!keys.is_empty()).then_some(keys),
            (!retro_batches.is_empty()).then_some(format!("retro batch: {retro_batches}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        return Err(DomainError::PayrollFinalized(format!(
            "Kesinleştirme, downstream FINALIZED bordro/retro tarihçesini etkilediği için yapılamaz: {}.",
            detail
        )));
    }

    Ok(())
}

/// Recalculates the current source snapshot and returns the only supported
/// official transition. No persistence or invalidation is performed here.
pub fn finalize_payroll(request: &PayrollCalculationRequest) -> Result<BordroKaydi> {
    let index = PayrollDatasetIndex::build(&request.dataset);
    validate_payroll_finalization_request_with_index(request, &index)?;
    let mut payroll = calculate_payroll_with_index(request, &index)?;
    ensure_finalizable_statutory_snapshot(&payroll)?;
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
    let index = PayrollDatasetIndex::build(&request.dataset);
    calculate_payroll_with_index(request, &index)
}

/// Validates and calculates a payroll with one immutable index for the full
/// request boundary. Browser/WASM callers use this entry point so validation
/// and formula evaluation do not rebuild the same lookup structures.
pub fn calculate_payroll_checked(request: &PayrollCalculationRequest) -> Result<BordroKaydi> {
    let index = PayrollDatasetIndex::build(&request.dataset);
    validate_payroll_request_with_index(request, &index)?;
    calculate_payroll_with_index(request, &index)
}

fn calculate_payroll_with_index(
    request: &PayrollCalculationRequest,
    index: &PayrollDatasetIndex,
) -> Result<BordroKaydi> {
    let dataset = &request.dataset;
    let person = index
        .personnel(dataset, &request.personnelId)
        .ok_or_else(|| {
            DomainError::NotFound(format!("Personel bulunamadı: {}", request.personnelId))
        })?
        .clone();
    let period = index
        .period(dataset, &request.periodId)
        .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", request.periodId)))?
        .clone();
    let accrual = resolve_accrual_input(request, &period, index)?;
    validate_period(&period)?;
    if let Some(input) = request.manualIncome.as_ref() {
        validate_manual_payroll_income_input(input)?;
    }

    let existing = payroll_for_requested_accrual(
        dataset,
        index,
        &request.personnelId,
        &request.periodId,
        Some(&accrual),
    );
    if existing.is_some_and(|payroll| payroll.status == BordroStatus::FINALIZED) {
        return Err(DomainError::PayrollFinalized(
            "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into(),
        ));
    }

    crate::validate_personnel_for_payroll(&person)?;
    let is_normal_accrual = accrual.accrualType == AccrualType::NORMAL;
    let is_retro_accrual = accrual.accrualType == AccrualType::RETRO_ADJUSTMENT;
    let retro_payment = if is_retro_accrual {
        Some(retro_payment_income_with_index(
            dataset,
            index,
            &accrual.accrualId,
        )?)
    } else {
        None
    };
    let (retro_income_tax_exempt, retro_stamp_tax_exempt) = retro_payment
        .as_ref()
        .map(|(batch, allocations, _, _)| {
            allocations.iter().fold(
                (Decimal::ZERO, Decimal::ZERO),
                |(income_tax_exempt, stamp_tax_exempt), allocation| {
                    (
                        income_tax_exempt
                            + if allocation.incomeTaxTreatment == RetroTaxTreatment::EXEMPT {
                                retro_payable_allocation_amount(batch, allocation)
                            } else {
                                Decimal::ZERO
                            },
                        stamp_tax_exempt
                            + if allocation.stampTaxTreatment == RetroTaxTreatment::EXEMPT {
                                retro_payable_allocation_amount(batch, allocation)
                            } else {
                                Decimal::ZERO
                            },
                    )
                },
            )
        })
        .unwrap_or((Decimal::ZERO, Decimal::ZERO));
    let attendance = if is_normal_accrual {
        Some(normal_attendance(
            dataset,
            index,
            &request.personnelId,
            &request.periodId,
        )?)
    } else {
        None
    };
    let supplementary_attendance = if is_normal_accrual {
        None
    } else {
        index
            .attendances(dataset, &request.personnelId, &request.periodId)
            .next()
    };
    let attendance_for_snapshot = if is_normal_accrual {
        attendance
    } else if let Some(candidate) = supplementary_attendance {
        attendance_has_full_calendar_coverage(candidate, &period)?.then_some(candidate)
    } else {
        None
    };
    let supplementary_attendance_incomplete = !is_normal_accrual
        && supplementary_attendance.is_some()
        && attendance_for_snapshot.is_none();
    let mut summary = PuantajOzeti::default();
    if let Some(attendance) = attendance {
        for code in attendance.gunler.values() {
            add_puantaj_kodu(&mut summary, code, &period.id)?;
        }
    }

    let paid_sick_dates = if let Some(attendance) = attendance_for_snapshot {
        let sick_records: Vec<SickLeaveRecord> = index
            .sick_leave_for_person(dataset, &request.personnelId)
            .cloned()
            .collect();
        let paid_sick_dates = calculate_paid_sick_dates_from_records(&sick_records, &period);
        validate_paid_sick_dates_against_attendance(attendance, &paid_sick_dates, &period.id)?;
        paid_sick_dates
    } else {
        Vec::new()
    };
    let paid_sick_days = if is_normal_accrual {
        paid_sick_dates.len() as i32
    } else {
        0
    };

    let raw_settings = dataset
        .institutionSettings
        .get(&period.id)
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} dönemi kurum ayarları bulunamadı; bordro hesaplanamaz.",
                period.id
            ))
        })?
        .clone();
    let settings = effective_statutory_settings(&raw_settings)?;
    validate_kurum_degerleri_for_payroll(&settings)?;
    validate_statutory_segments_for_period(&period, &settings)?;

    // Resolve prior payment-event state before calculating tax. This keeps a
    // DRAFT/STALE chain fail-closed with the generic payment-event error and
    // makes the same-month authoritative snapshot available to a supplementary
    // event that has no attendance input.
    let prior_accruals =
        resolve_prior_accrual_state(dataset, index, &person.id, &period, &accrual)?;
    let incoming_devreden_state =
        incoming_devreden_pek(dataset, index, &person.id, &period, &accrual)?;
    let statutory_snapshot = if is_normal_accrual {
        resolve_statutory_snapshot_for_period_with_paid_sick_dates(
            attendance
                .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?,
            &period,
            &settings,
            &paid_sick_dates,
        )?
    } else if let Some(attendance) = attendance_for_snapshot {
        resolve_statutory_snapshot_for_period_with_paid_sick_dates(
            attendance,
            &period,
            &settings,
            &paid_sick_dates,
        )?
    } else if supplementary_attendance_incomplete {
        resolve_statutory_snapshot_for_payment_month(&period, &settings)?
    } else if let Some(snapshot) = prior_accruals
        .last()
        .and_then(|payroll| payroll.statutorySnapshot.clone())
    {
        snapshot
    } else {
        resolve_statutory_snapshot_for_payment_month(&period, &settings)?
    };
    validate_pek_bounds(
        statutory_snapshot.pekAltSinir,
        statutory_snapshot.pekUstSinir,
    )?;

    let annual_parameters = index
        .annual_parameters(dataset, period.taxYear)
        .cloned()
        .ok_or_else(|| {
            DomainError::InvalidData(format!(
                "{} vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.",
                period.taxYear
            ))
        })?;
    crate::validate_annual_payroll_parameters(&annual_parameters)?;
    let (before_summary, after_summary, raise_date) = if let Some(attendance) = attendance {
        split_puantaj_by_zam_tarihi(attendance, &period, &dataset.zamAylari)?
    } else {
        (PuantajOzeti::default(), PuantajOzeti::default(), None)
    };
    let mut retro_payment_pek_income = GelirKalemleri::default();
    let (mut income, mut is_primi_detail) = if is_normal_accrual {
        let (mut income, is_primi_detail) = auto_fill_gelirler_from_puantaj(
            &summary,
            &settings,
            person.hizmetYili,
            Some(&person.grup),
        )?;
        apply_manual_payroll_income(&mut income, request.manualIncome.as_ref())?;
        (income, Some(is_primi_detail))
    } else if let Some((_, _, retro_income, payment_month_pek_income)) = retro_payment.as_ref() {
        retro_payment_pek_income = payment_month_pek_income.clone();
        (retro_income.clone(), None)
    } else {
        let amount = accrual.grossAmount.unwrap_or_default();
        let mut income = GelirKalemleri::default();
        match accrual.accrualType {
            AccrualType::TEDIYE => income.tediye = Some(amount),
            AccrualType::TIS_IKRAMIYE => income.tisIkramiyesi = Some(amount),
            AccrualType::SUPPLEMENTAL => income.ekOdeme = Some(amount),
            AccrualType::RETRO_ADJUSTMENT => {
                // The actual earning-code breakdown is resolved from the
                // authoritative RetroAdjustmentBatch below.
                income.ekOdeme = Some(amount);
            }
            AccrualType::NORMAL => unreachable!(),
        }
        // Supplementary accruals deliberately do not copy attendance-derived
        // salary, meal, road, premium, allowance, or service-year income.
        (income, None)
    };
    let mut effective_settings = settings.clone();
    let mut previous_settings_for_meal: Option<DonemselKurumDegerleri> = None;

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
            previous_settings_for_meal = Some(previous_settings);
        } else {
            add_paid_sick_wage(
                &mut income.tabanBrutAylik,
                paid_sick_days,
                settings.gunlukTabanUcret,
            );
        }
    }

    let normal_meal_exemptions = if is_normal_accrual {
        calculate_normal_meal_exemptions(
            attendance.ok_or_else(|| {
                DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into())
            })?,
            &period,
            &statutory_snapshot,
            &settings,
            previous_settings_for_meal.as_ref(),
            raise_date,
        )?
    } else {
        MealExemptionTotals::default()
    };

    let persisted_month_to_date_pek = same_month_pek_used(&prior_accruals)?;
    let pek_reconciliation = reconcile_same_month_pek(
        &prior_accruals,
        &income,
        is_normal_accrual,
        normal_meal_exemptions.sgk,
        &statutory_snapshot,
        persisted_month_to_date_pek,
    );
    let month_to_date_pek = pek_reconciliation.month_to_date_for_calculation;
    let same_month_gv_used = same_month_gv_exemption_used(&prior_accruals)?;
    let previous_cumulative_gv = previous_gv(dataset, index, &person, &period, &accrual)?;
    let previous_cumulative_asgari_gv = previous_asgari_gv(dataset, index, &person, &period)?;
    let incoming_devreden = &incoming_devreden_state.records;
    let tax_inputs = StatutoryDeductionTaxInputs {
        previous_cumulative_gv,
        incoming_devreden_pek: incoming_devreden,
        previous_cumulative_asgari_gv,
        tax_brackets: &annual_parameters.gelirVergisiDilimleri,
    };
    let (mut deductions, mut pek_detail, mut next_devreden) = if is_normal_accrual {
        calculate_statutory_contributions_with_month_to_date_and_devreden_state(
            &income,
            Some(&effective_settings),
            Some(&person),
            Some(&summary),
            &tax_inputs,
            Some(&statutory_snapshot),
            StatutoryCalculationOptions {
                month_to_date_pek,
                tax_months_elapsed: incoming_devreden_state.tax_months_elapsed,
                apply_lower_bound: true,
                meal_exemption: Some(normal_meal_exemptions),
            },
        )?
    } else if is_retro_accrual {
        let (_, allocations, _, _) = retro_payment.as_ref().ok_or_else(|| {
            DomainError::InvalidData("Retro payment allocation state çözülemedi.".into())
        })?;
        let (pek_detail, next_devreden) =
            calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
                &retro_payment_pek_income,
                None,
                Some(&effective_settings),
                incoming_devreden,
                Some(&statutory_snapshot),
                month_to_date_pek,
                PekCalculationOptions {
                    tax_months_elapsed: incoming_devreden_state.tax_months_elapsed,
                    apply_lower_bound: false,
                    meal_exemption: None,
                },
            )?;
        let source_worker_sgk = allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
            sum + allocation.workerSgkDelta
        });
        let source_worker_unemployment =
            allocations.iter().fold(Decimal::ZERO, |sum, allocation| {
                sum + allocation.workerUnemploymentDelta
            });
        let sgk_rate = effective_settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let unemployment_rate = effective_settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let payment_month_worker_pek = pek_detail.primMatrahi;
        (
            KesintiKalemleri {
                isciSgkPrimi: Some(round_sgk_amount(
                    source_worker_sgk + payment_month_worker_pek * sgk_rate,
                )),
                isciIssizlikPrimi: Some(round_sgk_amount(
                    source_worker_unemployment + payment_month_worker_pek * unemployment_rate,
                )),
                bes: calculate_oks_deduction(
                    payment_month_worker_pek
                        + allocations.iter().map(|allocation| allocation.retroPekDelta).sum::<Decimal>(),
                    &effective_settings,
                    Some(&person),
                    false,
                ),
                ..KesintiKalemleri::default()
            },
            pek_detail,
            next_devreden,
        )
    } else {
        let (pek_detail, next_devreden) =
            calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
                &income,
                None,
                Some(&effective_settings),
                incoming_devreden,
                Some(&statutory_snapshot),
                month_to_date_pek,
                PekCalculationOptions {
                    tax_months_elapsed: incoming_devreden_state.tax_months_elapsed,
                    apply_lower_bound: false,
                    meal_exemption: None,
                },
            )?;
        let sgk_rate = effective_settings
            .sgkIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("SGK işçi oranı eksik.".into()))?
            / dec!(100);
        let unemployment_rate = effective_settings
            .issizlikIsciOraniYuzde
            .ok_or_else(|| DomainError::InvalidData("İşsizlik işçi oranı eksik.".into()))?
            / dec!(100);
        let worker_pek = pek_detail.primMatrahi;
        let bes = calculate_oks_deduction(worker_pek, &effective_settings, Some(&person), false);
        (
            KesintiKalemleri {
                isciSgkPrimi: Some(round_sgk_amount(worker_pek * sgk_rate)),
                isciIssizlikPrimi: Some(round_sgk_amount(worker_pek * unemployment_rate)),
                bes,
                ..KesintiKalemleri::default()
            },
            pek_detail,
            next_devreden,
        )
    };
    if pek_reconciliation.active {
        // The old provisional snapshot remains immutable.  The current
        // attendance-backed event records the reconciled monthly state and
        // carries only the canonical non-wage excess forward.
        next_devreden = pek_reconciliation.outgoing_non_wage;
    }
    pek_detail.aylikOncekiPekTuketimi = Some(month_to_date_pek);
    pek_detail.aylikSonrasiPekTuketimi = Some(
        pek_reconciliation
            .month_used_after
            .unwrap_or_else(|| round2(month_to_date_pek + pek_detail.primMatrahi)),
    );
    let income_total = calculate_gelir_toplam(&income);

    let stamp_rate = effective_settings
        .damgaVergisiOraniBinde
        .ok_or_else(|| DomainError::InvalidData("Damga vergisi oranı eksik.".into()))?
        / dec!(1000);
    let monthly_minimum = round2(statutory_snapshot.gvReferansGunlukAsgariUcret * dec!(30));
    let same_month_stamp_used = same_month_stamp_exemption_used(&prior_accruals, stamp_rate)?;
    // GVK 23/8 meal exemption also excludes this amount from stamp tax
    // (322 numbered Income Tax Communiqué, article 4/6). Keep it separate
    // from the shared monthly minimum-wage exemption balance.
    let normal_meal_tax_exemption = if is_normal_accrual {
        normal_meal_exemptions.gv.min(income.yemek.unwrap_or_default())
    } else {
        Decimal::ZERO
    };

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
    let insurance_used = previous_insurance_gv(dataset, index, &person.id, &period, &accrual)?;
    let insurance_wage_base =
        (income_total - income.yemek.unwrap_or_default() - income.vasitaYol.unwrap_or_default())
            .max(Decimal::ZERO);
    let daily_minimum = statutory_snapshot.gvReferansGunlukAsgariUcret;
    let monthly_asgari_gv =
        calculate_aylik_asgari_ucret_gv_matrahi(daily_minimum, sgk_rate, unemployment_rate);
    let canonical_tax = calculate_canonical_tax_state(&CanonicalTaxCalculationInput {
        income_total,
        retro_income_tax_exempt,
        retro_stamp_tax_exempt,
        normal_meal_tax_exemption,
        worker_sgk: deductions.isciSgkPrimi.unwrap_or_default(),
        worker_unemployment: deductions.isciIssizlikPrimi.unwrap_or_default(),
        union_deduction: deductions.sendikaAidati.unwrap_or_default(),
        insurance_wage_base,
        birth_military_gv_input: birth_military,
        life_insurance_premium: life_insurance,
        health_insurance_premium: health_insurance,
        insurance_annual_cap: insurance_cap,
        insurance_used_before: insurance_used,
        previous_cumulative_gv,
        monthly_asgari_gv_matrahi: monthly_asgari_gv,
        previous_cumulative_asgari_gv,
        same_month_gv_used,
        tax_brackets: &annual_parameters.gelirVergisiDilimleri,
        monthly_minimum_gross: monthly_minimum,
        stamp_rate,
        same_month_stamp_used,
    });
    if same_month_gv_used > canonical_tax.gv.asgariUcretGvIstisnasi {
        return Err(DomainError::InvalidData(format!(
            "Aynı vergi ayında kullanılan GV istisnası {}, aylık hak {} değerini aşıyor; zincir güvenli biçimde devam ettirilemez.",
            same_month_gv_used, canonical_tax.gv.asgariUcretGvIstisnasi
        )));
    }
    if same_month_stamp_used > canonical_tax.stamp.aylikDamgaIstisnaHakki {
        return Err(DomainError::InvalidData(format!(
            "Aynı vergi ayında kullanılan damga vergisi istisnası {}, aylık hak {} değerini aşıyor; zincir güvenli biçimde devam ettirilemez.",
            same_month_stamp_used, canonical_tax.stamp.aylikDamgaIstisnaHakki
        )));
    }
    deductions.gelirVergisi = Some(canonical_tax.gv.kesilenGelirVergisi);
    deductions.damgaVergisi = Some(canonical_tax.stamp.kesilenDamgaVergisi);
    let gv_base = canonical_tax.gv_base;
    let gv_detail = canonical_tax.gv;
    let stamp_detail = canonical_tax.stamp;
    crate::validate_ordinary_payroll_line_items(&income, &deductions)?;

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
        persistedGvBase: Some(gv_base),
        damgaDetay: Some(stamp_detail),
        statutorySnapshot: Some(statutory_snapshot),
        odenenRaporluGun: Some(paid_sick_days),
        raporluGun: Some(summary.r),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn tax_period(id: &str, tax_month: i32) -> BordroDonemi {
        BordroDonemi {
            id: id.into(),
            yil: 2026,
            ay: tax_month,
            baslangicTarihi: format!("2026-{tax_month:02}-01"),
            bitisTarihi: format!("2026-{tax_month:02}-28"),
            donemAdi: id.into(),
            taxYear: 2026,
            taxMonth: tax_month,
        }
    }

    fn event(
        period_id: &str,
        tax_month: i32,
        accrual_id: &str,
        accrual_type: AccrualType,
        status: BordroStatus,
        remaining_months: i32,
    ) -> BordroKaydi {
        BordroKaydi {
            id: accrual_id.into(),
            personelId: "person-1".into(),
            donemId: period_id.into(),
            accrualId: accrual_id.into(),
            accrualType: accrual_type,
            paymentDate: format!("2026-{tax_month:02}-10"),
            sequence: 0,
            accrualDescription: None,
            puantajOzeti: PuantajOzeti::default(),
            gelirler: GelirKalemleri::default(),
            gelirToplam: Decimal::ZERO,
            kesintiler: KesintiKalemleri::default(),
            kesintiToplam: Decimal::ZERO,
            netOdeme: Decimal::ZERO,
            status,
            olusturulmaTarihi: "2026-09-05T00:00:00Z".into(),
            sonGuncellemeTarihi: "2026-09-05T00:00:00Z".into(),
            notlar: None,
            oncekiKumulatifGvMatrahi: None,
            oncekiKumulatifAsgariGvMatrahi: None,
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: None,
            sonrakiDevredenPek: (remaining_months > 0).then(|| {
                vec![DevredenPekKaydi {
                    tutar: dec!(1000),
                    kalanAySayisi: remaining_months,
                    kaynakDonemId: Some(period_id.into()),
                }]
            }),
            pekDetay: None,
            isPrimiDetay: None,
            gvDetay: None,
            persistedGvBase: None,
            damgaDetay: None,
            statutorySnapshot: None,
            odenenRaporluGun: None,
            raporluGun: None,
        }
    }

    fn valid_tax_period(
        id: &str,
        work_year: i32,
        work_month: i32,
        tax_year: i32,
        tax_month: i32,
    ) -> BordroDonemi {
        let (end_year, end_month) = if work_month == 12 {
            (work_year + 1, 1)
        } else {
            (work_year, work_month + 1)
        };
        BordroDonemi {
            id: id.into(),
            yil: work_year,
            ay: work_month,
            baslangicTarihi: format!("{work_year}-{work_month:02}-15"),
            bitisTarihi: format!("{end_year}-{end_month:02}-14"),
            donemAdi: id.into(),
            taxYear: tax_year,
            taxMonth: tax_month,
        }
    }

    fn test_person(id: &str) -> Personel {
        Personel {
            id: id.into(),
            tcNo: "1".into(),
            ad: "Test".into(),
            soyad: "Personel".into(),
            grup: "A".into(),
            unvan: None,
            sgkSicilNo: String::new(),
            iban: String::new(),
            hizmetYili: 1,
            aciklama: None,
            devirKumulatifGvMatrahi: None,
            devirKumulatifGvMatrahiYili: None,
            devirKumulatifGvMatrahiBaslangicAyi: None,
            devirKumulatifAsgariGvMatrahi: None,
            devirKumulatifAsgariGvMatrahiYili: None,
            kesintiler: None,
        }
    }

    fn test_settings(period_id: &str, daily_minimum: Decimal) -> DonemselKurumDegerleri {
        DonemselKurumDegerleri {
            donemId: period_id.into(),
            gunlukAsgariUcret: Some(daily_minimum),
            ..DonemselKurumDegerleri::default()
        }
    }

    fn test_asgari_opening(
        personnel_id: &str,
        period: &BordroDonemi,
        value: Decimal,
    ) -> PersonelTaxOpening {
        PersonelTaxOpening {
            id: format!("{}_{}", personnel_id, period.taxYear),
            personnelId: personnel_id.into(),
            year: period.taxYear,
            gvCumulativeOpening: None,
            effectiveFromPeriodId: None,
            asgariGvCumulativeOpening: Some(value),
            asgariGvEffectiveFromPeriodId: Some(period.id.clone()),
            createdAt: None,
            updatedAt: None,
        }
    }

    fn test_normal_opening(
        personnel_id: &str,
        period: &BordroDonemi,
        value: Decimal,
    ) -> PersonelTaxOpening {
        PersonelTaxOpening {
            id: format!("{}_{}", personnel_id, period.taxYear),
            personnelId: personnel_id.into(),
            year: period.taxYear,
            gvCumulativeOpening: Some(value),
            effectiveFromPeriodId: Some(period.id.clone()),
            asgariGvCumulativeOpening: None,
            asgariGvEffectiveFromPeriodId: None,
            createdAt: None,
            updatedAt: None,
        }
    }

    fn test_dataset(
        periods: Vec<BordroDonemi>,
        person: Personel,
        settings: Vec<DonemselKurumDegerleri>,
        tax_openings: Vec<PersonelTaxOpening>,
    ) -> PayrollDatasetSnapshot {
        PayrollDatasetSnapshot {
            personnel: vec![person],
            periods,
            institutionSettings: settings
                .into_iter()
                .map(|settings| (settings.donemId.clone(), settings))
                .collect::<HashMap<_, _>>(),
            taxOpenings: tax_openings,
            ..PayrollDatasetSnapshot::default()
        }
    }

    #[test]
    fn legacy_persisted_gv_base_survives_reload_and_starts_next_cumulative_chain() {
        let prior_period = valid_tax_period("2026-04", 2026, 4, 2026, 5);
        let active_period = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let person = test_person("person-1");
        let mut prior = event(
            &prior_period.id,
            prior_period.taxMonth,
            "legacy-gv-event",
            AccrualType::NORMAL,
            BordroStatus::CALCULATED,
            0,
        );
        prior.gvDetay = None;
        prior.persistedGvBase = Some(dec!(27500));
        let mut dataset = test_dataset(
            vec![prior_period.clone(), active_period.clone()],
            person.clone(),
            vec![
                test_settings(&prior_period.id, dec!(1000)),
                test_settings(&active_period.id, dec!(1000)),
            ],
            Vec::new(),
        );
        dataset.payrolls.push(prior.clone());
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(payroll_gv_base(&prior).unwrap(), dec!(27500));
        assert_eq!(
            previous_gv(&dataset, &index, &person, &active_period, &PayrollAccrualInput {
                accrualId: "next-event".into(),
                accrualType: AccrualType::NORMAL,
                paymentDate: "2026-06-10".into(),
                sequence: 0,
                grossAmount: None,
                description: None,
            })
            .unwrap(),
            dec!(27500)
        );
    }

    #[test]
    fn asgari_gv_opening_at_active_tax_month_is_not_double_counted() {
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let person = test_person("person-a");
        let opening = test_asgari_opening(&person.id, &active, dec!(100000));
        let dataset = test_dataset(
            vec![active.clone()],
            person.clone(),
            vec![test_settings(&active.id, dec!(1000))],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(100000)
        );
    }

    #[test]
    fn asgari_gv_opening_adds_only_months_after_effective_tax_month() {
        let april = valid_tax_period("2026-03", 2026, 3, 2026, 4);
        let may = valid_tax_period("2026-04", 2026, 4, 2026, 5);
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let person = test_person("person-b");
        let opening = test_asgari_opening(&person.id, &april, dec!(100000));
        let dataset = test_dataset(
            vec![april.clone(), may.clone(), active.clone()],
            person.clone(),
            vec![
                test_settings(&april.id, dec!(1000)),
                test_settings(&may.id, dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(151000)
        );
        validate_tax_month_chain(&dataset, &index, &person, &active).unwrap();
    }

    #[test]
    fn asgari_gv_without_opening_keeps_january_to_previous_month_behavior() {
        let periods = vec![
            valid_tax_period("2025-12", 2025, 12, 2026, 1),
            valid_tax_period("2026-01", 2026, 1, 2026, 2),
            valid_tax_period("2026-02", 2026, 2, 2026, 3),
            valid_tax_period("2026-03", 2026, 3, 2026, 4),
            valid_tax_period("2026-04", 2026, 4, 2026, 5),
            valid_tax_period("2026-05", 2026, 5, 2026, 6),
        ];
        let active = periods.last().cloned().unwrap();
        let person = test_person("person-c");
        let settings = periods
            .iter()
            .map(|period| test_settings(&period.id, dec!(1000)))
            .collect();
        let dataset = test_dataset(periods, person.clone(), settings, Vec::new());
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(127500)
        );
    }

    #[test]
    fn cross_year_period_uses_tax_month_from_period_not_work_month() {
        let active = valid_tax_period("2025-12", 2025, 12, 2026, 1);
        let person = test_person("person-d");
        let opening = test_asgari_opening(&person.id, &active, dec!(50000));
        let dataset = test_dataset(
            vec![active.clone()],
            person.clone(),
            vec![test_settings(&active.id, dec!(1000))],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(active.ay, 12);
        assert_eq!(active.taxYear, 2026);
        assert_eq!(active.taxMonth, 1);
        validate_tax_month_chain(&dataset, &index, &person, &active).unwrap();
        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(50000)
        );
    }

    #[test]
    fn historical_asgari_gv_uses_statutory_segment_for_cross_year_period() {
        let prior = valid_tax_period("2025-12", 2025, 12, 2026, 1);
        let active = valid_tax_period("2026-01", 2026, 1, 2026, 2);
        let person = test_person("person-e");
        let mut prior_settings = test_settings(&prior.id, dec!(1000));
        prior_settings.statutoryParameterSegments = Some(vec![StatutoryParameterSegment {
            effectiveFrom: "2026-01-01".into(),
            gunlukAsgariUcret: Some(dec!(1100)),
            ..StatutoryParameterSegment::default()
        }]);
        let dataset = test_dataset(
            vec![prior.clone(), active.clone()],
            person.clone(),
            vec![prior_settings, test_settings(&active.id, dec!(1100))],
            Vec::new(),
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(28050)
        );
    }

    #[test]
    fn historical_asgari_gv_ignores_later_mutable_settings_when_snapshot_exists() {
        let prior = valid_tax_period("2025-12", 2025, 12, 2026, 1);
        let active = valid_tax_period("2026-01", 2026, 1, 2026, 2);
        let person = test_person("person-f");
        let mut prior_settings = test_settings(&prior.id, dec!(2000));
        prior_settings.statutoryParameterSnapshot = Some(StatutoryParameterSnapshot {
            gunlukAsgariUcret: dec!(1000),
            sgkIsciOraniYuzde: dec!(14),
            issizlikIsciOraniYuzde: dec!(1),
            pekTavanKatsayisi: dec!(9),
            gunlukYemekIstisnasiSGK: dec!(300),
            gunlukYemekIstisnasiGV: dec!(300),
            statutoryParameterSegments: Vec::new(),
        });
        let dataset = test_dataset(
            vec![prior.clone(), active.clone()],
            person.clone(),
            vec![prior_settings, test_settings(&active.id, dec!(2000))],
            Vec::new(),
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(25500)
        );
    }

    #[test]
    fn same_tax_month_multiple_events_advance_asgari_reference_once() {
        let tax_one = valid_tax_period("2025-12", 2025, 12, 2026, 1);
        let tax_two = valid_tax_period("2026-01", 2026, 1, 2026, 2);
        let active = valid_tax_period("2026-02", 2026, 2, 2026, 3);
        let person = test_person("person-1");
        let dataset = PayrollDatasetSnapshot {
            personnel: vec![person.clone()],
            periods: vec![tax_one.clone(), tax_two.clone(), active.clone()],
            institutionSettings: [
                (tax_one.id.clone(), test_settings(&tax_one.id, dec!(1000))),
                (tax_two.id.clone(), test_settings(&tax_two.id, dec!(1000))),
                (active.id.clone(), test_settings(&active.id, dec!(1000))),
            ]
            .into_iter()
            .collect(),
            payrolls: vec![
                event(
                    &tax_two.id,
                    2,
                    "normal-2",
                    AccrualType::NORMAL,
                    BordroStatus::CALCULATED,
                    0,
                ),
                event(
                    &tax_two.id,
                    2,
                    "supplemental-2",
                    AccrualType::SUPPLEMENTAL,
                    BordroStatus::CALCULATED,
                    0,
                ),
            ],
            ..PayrollDatasetSnapshot::default()
        };
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(51000)
        );
    }

    #[test]
    fn asgari_only_opening_does_not_depend_on_normal_opening() {
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let person = test_person("person-h");
        let opening = test_asgari_opening(&person.id, &active, dec!(50000));
        let dataset = test_dataset(
            vec![active.clone()],
            person.clone(),
            vec![test_settings(&active.id, dec!(1000))],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(50000)
        );
    }

    #[test]
    fn asgari_only_modern_opening_keeps_legacy_normal_opening() {
        let legacy_start = valid_tax_period("2026-04", 2026, 4, 2026, 4);
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let mut person = test_person("person-h-legacy-normal");
        person.devirKumulatifGvMatrahi = Some(dec!(300000));
        person.devirKumulatifGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(4);
        let opening = test_asgari_opening(&person.id, &active, dec!(100000));
        let dataset = test_dataset(
            vec![legacy_start, active.clone()],
            person.clone(),
            vec![
                test_settings("2026-04", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);
        let current = PayrollAccrualInput {
            accrualId: "current-legacy-normal".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-06-10".into(),
            sequence: 0,
            grossAmount: None,
            description: None,
        };

        let resolved = resolve_tax_openings(&dataset, &index, &person, &active).unwrap();
        assert_eq!(
            resolved.normal.map(|opening| opening.value),
            Some(dec!(300000))
        );
        assert_eq!(
            resolved.asgari.map(|opening| opening.value),
            Some(dec!(100000))
        );
        assert_eq!(
            previous_gv(&dataset, &index, &person, &active, &current).unwrap(),
            dec!(300000)
        );
        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(100000)
        );
    }

    #[test]
    fn modern_normal_only_opening_keeps_legacy_asgari_opening() {
        let normal_start = valid_tax_period("2026-05", 2026, 5, 2026, 5);
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let mut person = test_person("person-h-legacy-asgari");
        person.devirKumulatifAsgariGvMatrahi = Some(dec!(120000));
        person.devirKumulatifAsgariGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(6);
        let opening = test_normal_opening(&person.id, &normal_start, dec!(250000));
        let dataset = test_dataset(
            vec![normal_start, active.clone()],
            person.clone(),
            vec![
                test_settings("2026-05", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);
        let current = PayrollAccrualInput {
            accrualId: "current-legacy-asgari".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-06-10".into(),
            sequence: 0,
            grossAmount: None,
            description: None,
        };

        let resolved = resolve_tax_openings(&dataset, &index, &person, &active).unwrap();
        assert_eq!(
            resolved.normal.map(|opening| opening.value),
            Some(dec!(250000))
        );
        assert_eq!(
            resolved.asgari.map(|opening| opening.value),
            Some(dec!(120000))
        );
        assert_eq!(
            previous_gv(&dataset, &index, &person, &active, &current).unwrap(),
            dec!(250000)
        );
        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(120000)
        );
    }

    #[test]
    fn explicit_zero_normal_opening_overrides_legacy_normal_opening() {
        let legacy_start = valid_tax_period("2026-04", 2026, 4, 2026, 4);
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let mut person = test_person("person-h-explicit-zero");
        person.devirKumulatifGvMatrahi = Some(dec!(300000));
        person.devirKumulatifGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(4);
        let mut opening = test_normal_opening(&person.id, &active, Decimal::ZERO);
        opening.asgariGvCumulativeOpening = None;
        let dataset = test_dataset(
            vec![legacy_start, active.clone()],
            person.clone(),
            vec![
                test_settings("2026-04", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);
        let current = PayrollAccrualInput {
            accrualId: "current-explicit-zero".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-06-10".into(),
            sequence: 0,
            grossAmount: None,
            description: None,
        };

        let resolved = resolve_tax_openings(&dataset, &index, &person, &active).unwrap();
        assert_eq!(
            resolved.normal.map(|opening| opening.value),
            Some(Decimal::ZERO)
        );
        assert_eq!(
            previous_gv(&dataset, &index, &person, &active, &current).unwrap(),
            Decimal::ZERO
        );
    }

    #[test]
    fn no_modern_or_legacy_opening_resolves_to_none() {
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let person = test_person("person-h-no-opening");
        let dataset = test_dataset(
            vec![active.clone()],
            person.clone(),
            vec![test_settings(&active.id, dec!(1000))],
            Vec::new(),
        );
        let index = PayrollDatasetIndex::build(&dataset);

        let resolved = resolve_tax_openings(&dataset, &index, &person, &active).unwrap();
        assert!(resolved.normal.is_none());
        assert!(resolved.asgari.is_none());
    }

    #[test]
    fn legacy_normal_opening_with_ambiguous_start_fails_closed() {
        let work_month_candidate = valid_tax_period("work-04", 2026, 4, 2026, 5);
        let tax_month_candidate = valid_tax_period("tax-04", 2026, 3, 2026, 4);
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let mut person = test_person("person-h-ambiguous");
        person.devirKumulatifGvMatrahi = Some(dec!(300000));
        person.devirKumulatifGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(4);
        let opening = test_asgari_opening(&person.id, &active, dec!(100000));
        let dataset = test_dataset(
            vec![work_month_candidate, tax_month_candidate, active.clone()],
            person.clone(),
            vec![
                test_settings("work-04", dec!(1000)),
                test_settings("tax-04", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert!(matches!(
            resolve_tax_openings(&dataset, &index, &person, &active),
            Err(DomainError::ValidationError(message)) if message.contains("birden fazla döneme")
        ));
    }

    #[test]
    fn asgari_only_opening_keeps_legacy_normal_through_historical_payrolls() {
        let april = valid_tax_period("2026-04", 2026, 4, 2026, 4);
        let may = valid_tax_period("2026-05", 2026, 5, 2026, 5);
        let active = valid_tax_period("2026-06", 2026, 6, 2026, 6);
        let mut person = test_person("person-1");
        person.devirKumulatifGvMatrahi = Some(dec!(100000));
        person.devirKumulatifGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(4);
        let opening = test_asgari_opening(&person.id, &active, dec!(100000));
        let mut april_payroll = event(
            &april.id,
            april.taxMonth,
            "historical-april",
            AccrualType::NORMAL,
            BordroStatus::CALCULATED,
            0,
        );
        april_payroll.gelirler.tabanBrutAylik = Some(dec!(10000));
        april_payroll.gelirToplam = dec!(10000);
        let mut may_payroll = event(
            &may.id,
            may.taxMonth,
            "historical-may",
            AccrualType::NORMAL,
            BordroStatus::CALCULATED,
            0,
        );
        may_payroll.gelirler.tabanBrutAylik = Some(dec!(20000));
        may_payroll.gelirToplam = dec!(20000);
        let dataset = PayrollDatasetSnapshot {
            personnel: vec![person.clone()],
            periods: vec![april, may, active.clone()],
            institutionSettings: vec![
                test_settings("2026-04", dec!(1000)),
                test_settings("2026-05", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ]
            .into_iter()
            .map(|settings| (settings.donemId.clone(), settings))
            .collect(),
            payrolls: vec![april_payroll, may_payroll],
            taxOpenings: vec![opening],
            ..PayrollDatasetSnapshot::default()
        };
        let index = PayrollDatasetIndex::build(&dataset);
        let current = PayrollAccrualInput {
            accrualId: "current-historical".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-06-10".into(),
            sequence: 0,
            grossAmount: None,
            description: None,
        };

        assert_eq!(
            previous_gv(&dataset, &index, &person, &active, &current).unwrap(),
            dec!(130000)
        );
        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            dec!(100000)
        );
    }

    #[test]
    fn legacy_zero_asgari_value_does_not_require_legacy_month_resolution() {
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let mut person = test_person("person-h-zero");
        person.devirKumulatifAsgariGvMatrahi = Some(Decimal::ZERO);
        person.devirKumulatifAsgariGvMatrahiYili = Some(2026);
        let opening = PersonelTaxOpening {
            id: "person-h-zero_2026".into(),
            personnelId: person.id.clone(),
            year: 2026,
            gvCumulativeOpening: Some(dec!(1234)),
            effectiveFromPeriodId: Some(active.id.clone()),
            asgariGvCumulativeOpening: None,
            asgariGvEffectiveFromPeriodId: None,
            createdAt: None,
            updatedAt: None,
        };
        let dataset = test_dataset(
            vec![active.clone()],
            person.clone(),
            vec![test_settings(&active.id, dec!(1000))],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_asgari_gv(&dataset, &index, &person, &active).unwrap(),
            Decimal::ZERO
        );
    }

    #[test]
    fn modern_normal_opening_is_authoritative_over_legacy_month_and_value() {
        let effective = valid_tax_period("2026-03", 2026, 3, 2026, 4);
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let mut person = test_person("person-i");
        person.devirKumulatifGvMatrahi = Some(dec!(999999));
        person.devirKumulatifGvMatrahiYili = Some(2026);
        person.devirKumulatifGvMatrahiBaslangicAyi = Some(1);
        let opening = PersonelTaxOpening {
            id: "person-i_2026".into(),
            personnelId: person.id.clone(),
            year: 2026,
            gvCumulativeOpening: Some(dec!(1234)),
            effectiveFromPeriodId: Some(effective.id.clone()),
            asgariGvCumulativeOpening: None,
            asgariGvEffectiveFromPeriodId: None,
            createdAt: None,
            updatedAt: None,
        };
        let dataset = test_dataset(
            vec![effective, active.clone()],
            person.clone(),
            vec![
                test_settings("2026-03", dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ],
            vec![opening],
        );
        let index = PayrollDatasetIndex::build(&dataset);

        assert_eq!(
            previous_gv(
                &dataset,
                &index,
                &person,
                &active,
                &PayrollAccrualInput {
                    accrualId: "normal-i".into(),
                    accrualType: AccrualType::NORMAL,
                    paymentDate: "2026-06-10".into(),
                    sequence: 0,
                    grossAmount: None,
                    description: None,
                },
            )
            .unwrap(),
            dec!(1234)
        );
    }

    #[test]
    fn explicit_zero_normal_opening_is_not_the_same_as_no_opening() {
        let prior = valid_tax_period("2026-01", 2026, 1, 2026, 2);
        let effective = valid_tax_period("2026-03", 2026, 3, 2026, 4);
        let active = valid_tax_period("2026-05", 2026, 5, 2026, 6);
        let person = test_person("person-1");
        let prior_payroll = event(
            &prior.id,
            prior.taxMonth,
            "prior-gv",
            AccrualType::NORMAL,
            BordroStatus::CALCULATED,
            0,
        );
        let opening = PersonelTaxOpening {
            id: "person-1_2026".into(),
            personnelId: person.id.clone(),
            year: 2026,
            gvCumulativeOpening: Some(Decimal::ZERO),
            effectiveFromPeriodId: Some(effective.id.clone()),
            asgariGvCumulativeOpening: None,
            asgariGvEffectiveFromPeriodId: None,
            createdAt: None,
            updatedAt: None,
        };
        let current = PayrollAccrualInput {
            accrualId: "current-gv".into(),
            accrualType: AccrualType::NORMAL,
            paymentDate: "2026-06-10".into(),
            sequence: 0,
            grossAmount: None,
            description: None,
        };
        let explicit_dataset = PayrollDatasetSnapshot {
            personnel: vec![person.clone()],
            periods: vec![prior.clone(), effective.clone(), active.clone()],
            institutionSettings: vec![
                test_settings(&prior.id, dec!(1000)),
                test_settings(&effective.id, dec!(1000)),
                test_settings(&active.id, dec!(1000)),
            ]
            .into_iter()
            .map(|settings| (settings.donemId.clone(), settings))
            .collect(),
            payrolls: vec![prior_payroll.clone()],
            taxOpenings: vec![opening],
            ..PayrollDatasetSnapshot::default()
        };
        let explicit_index = PayrollDatasetIndex::build(&explicit_dataset);
        assert!(matches!(
            previous_gv(
                &explicit_dataset,
                &explicit_index,
                &person,
                &active,
                &current
            ),
            Err(DomainError::TaxOpeningConflict(_))
        ));

        let no_opening_dataset = PayrollDatasetSnapshot {
            personnel: vec![person.clone()],
            periods: vec![prior, effective, active.clone()],
            institutionSettings: vec![
                test_settings("2026-01", dec!(1000)),
                test_settings("2026-03", dec!(1000)),
                test_settings("2026-05", dec!(1000)),
            ]
            .into_iter()
            .map(|settings| (settings.donemId.clone(), settings))
            .collect(),
            payrolls: vec![prior_payroll],
            ..PayrollDatasetSnapshot::default()
        };
        let no_opening_index = PayrollDatasetIndex::build(&no_opening_dataset);
        assert_eq!(
            previous_gv(
                &no_opening_dataset,
                &no_opening_index,
                &person,
                &active,
                &current,
            )
            .unwrap(),
            Decimal::ZERO
        );
    }

    #[test]
    fn devreden_gap_accepts_authoritative_tediye_without_normal_and_rejects_stale() {
        let source_period = tax_period("source", 1);
        let intermediate_period = tax_period("intermediate", 2);
        let current_period = tax_period("current", 3);
        let source = event(
            &source_period.id,
            1,
            "source-event",
            AccrualType::NORMAL,
            BordroStatus::CALCULATED,
            3,
        );
        let intermediate = event(
            &intermediate_period.id,
            2,
            "intermediate-tediye",
            AccrualType::TEDIYE,
            BordroStatus::CALCULATED,
            2,
        );
        let current = PayrollAccrualInput {
            accrualId: "current-event".into(),
            accrualType: AccrualType::SUPPLEMENTAL,
            paymentDate: "2026-03-10".into(),
            sequence: 0,
            grossAmount: Some(dec!(1000)),
            description: None,
        };
        let mut dataset = PayrollDatasetSnapshot {
            periods: vec![source_period, intermediate_period, current_period.clone()],
            payrolls: vec![source, intermediate],
            ..PayrollDatasetSnapshot::default()
        };
        let index = PayrollDatasetIndex::build(&dataset);

        validate_devreden_pek_gap(&dataset, &index, "person-1", &current_period, &current)
            .expect("authoritative intermediate TEDIYE is a valid chain state");

        for status in [BordroStatus::DRAFT, BordroStatus::STALE] {
            dataset.payrolls[1].status = status;
            let error =
                validate_devreden_pek_gap(&dataset, &index, "person-1", &current_period, &current)
                    .expect_err("non-authoritative intermediate event must fail closed");
            assert!(error.to_string().contains("Payment-event/PEK"));
        }
    }

    #[test]
    fn devreden_carry_expires_after_the_exact_two_tax_month_distance() {
        let settings = DonemselKurumDegerleri {
            gunlukAsgariUcret: Some(dec!(1000)),
            pekTavanKatsayisi: Some(Decimal::ONE),
            gunlukYemekIstisnasiSGK: Some(Decimal::ZERO),
            ..DonemselKurumDegerleri::default()
        };
        let attendance = PuantajOzeti {
            c: 30,
            ..PuantajOzeti::default()
        };
        let income = GelirKalemleri {
            tabanBrutAylik: Some(dec!(29000)),
            ..GelirKalemleri::default()
        };
        let incoming = vec![DevredenPekKaydi {
            tutar: dec!(30000),
            kalanAySayisi: 2,
            kaynakDonemId: Some("source".into()),
        }];

        let (pek, carried) = calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
            &income,
            Some(&attendance),
            Some(&settings),
            &incoming,
            None,
            Decimal::ZERO,
            PekCalculationOptions {
                tax_months_elapsed: 2,
                apply_lower_bound: false,
                meal_exemption: None,
            },
        )
        .expect("distance=2 should be valid");

        assert_eq!(pek.devredenPekKullanilan, dec!(1000));
        assert_eq!(pek.primMatrahi, dec!(30000));
        assert!(carried.is_empty());
    }

    #[test]
    fn devreden_carry_is_not_used_after_its_tax_month_lifetime() {
        let settings = DonemselKurumDegerleri {
            gunlukAsgariUcret: Some(dec!(1000)),
            pekTavanKatsayisi: Some(Decimal::ONE),
            gunlukYemekIstisnasiSGK: Some(Decimal::ZERO),
            ..DonemselKurumDegerleri::default()
        };
        let attendance = PuantajOzeti {
            c: 30,
            ..PuantajOzeti::default()
        };
        let income = GelirKalemleri {
            tabanBrutAylik: Some(dec!(29000)),
            ..GelirKalemleri::default()
        };
        let incoming = vec![DevredenPekKaydi {
            tutar: dec!(30000),
            kalanAySayisi: 2,
            kaynakDonemId: Some("source".into()),
        }];

        let (pek, carried) = calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
            &income,
            Some(&attendance),
            Some(&settings),
            &incoming,
            None,
            Decimal::ZERO,
            PekCalculationOptions {
                tax_months_elapsed: 3,
                apply_lower_bound: false,
                meal_exemption: None,
            },
        )
        .expect("distance=3 should be valid");

        assert_eq!(pek.devredenPekKullanilan, Decimal::ZERO);
        assert!(carried.is_empty());
    }

    #[test]
    fn negative_tax_month_distance_fails_closed() {
        let error = calculate_prime_esas_kazanc_with_month_to_date_and_devreden_state(
            &GelirKalemleri::default(),
            None,
            None,
            &[],
            None,
            Decimal::ZERO,
            PekCalculationOptions {
                tax_months_elapsed: -1,
                apply_lower_bound: false,
                meal_exemption: None,
            },
        )
        .expect_err("negative tax-month distance must be rejected");

        assert!(error.to_string().contains("negatif"));
    }
}
