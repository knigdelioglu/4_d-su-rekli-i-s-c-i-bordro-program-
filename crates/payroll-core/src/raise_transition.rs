//! Shared scheduled raise transition for NORMAL payroll and historical replay.
//! Stored settings remain immutable. Copied income rates carry their effective
//! raised values forward; explicitly changed amounts retain their entered value.

use crate::models::{BordroDonemi, DonemselKurumDegerleri};
use crate::payroll_engine::{find_previous_work_period, PayrollDatasetSnapshot};
use crate::{DomainError, Result};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn round2(value: Decimal) -> Decimal {
    value.round_dp(2)
}

fn period_date(value: &str, period_id: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        DomainError::InvalidData(format!(
            "{} dönemi tarihi geçersiz: {} ({})",
            period_id, value, error
        ))
    })
}

pub(crate) struct RaiseTransition {
    pub current: DonemselKurumDegerleri,
    pub previous: Option<DonemselKurumDegerleri>,
    pub cutoff: Option<NaiveDate>,
    period: BordroDonemi,
}

impl RaiseTransition {
    pub fn calendar_days(&self) -> Result<(i32, i32)> {
        let cutoff = self
            .cutoff
            .ok_or_else(|| DomainError::InvalidData("Zam tarihi bulunamadı.".into()))?;
        period_days_before_and_after_raise(&self.period, cutoff)
    }
}

/// Follow stored compensation history, comparing raw inputs to recognize
/// overrides, and effective previous amounts to preserve inferred raises.
/// History is chronological by service dates, never by tax/payment month.
pub(crate) fn resolve_raise_transition(
    dataset: &PayrollDatasetSnapshot,
    institution_settings: &std::collections::HashMap<String, DonemselKurumDegerleri>,
    raise_months: &[i32],
    period: &BordroDonemi,
) -> Result<RaiseTransition> {
    let cutoff = find_zam_tarihi(period, raise_months)?;
    let settings_for = |period: &BordroDonemi| -> Result<DonemselKurumDegerleri> {
        institution_settings
            .get(&period.id)
            .cloned()
            .ok_or_else(|| {
                DomainError::InvalidData(format!(
                    "{} dönemi zam gelir geçmişinde kurum ayarları bulunamadı.",
                    period.id
                ))
            })
    };
    let current = settings_for(period)?;
    if raise_months.is_empty() {
        return Ok(RaiseTransition {
            current,
            previous: None,
            cutoff,
            period: period.clone(),
        });
    }
    let mut history = vec![period];
    let mut cursor = period;
    while let Some(previous) = find_previous_work_period(dataset, cursor)? {
        let start = period_date(&cursor.baslangicTarihi, &cursor.id)?;
        let previous_end = period_date(&previous.bitisTarihi, &previous.id)?;
        if previous_end.succ_opt() != Some(start) {
            return Err(DomainError::InvalidData(format!("{} dönemi zam gelir geçmişi kesintili; hemen önceki dönem ve kurum ayarları gereklidir.", cursor.id)));
        }
        history.push(previous);
        cursor = previous;
    }
    if cutoff.is_some() && history.len() < 2 {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi zam öncesi dönem ayarı bulunamadı.",
            period.id
        )));
    }
    history.reverse();
    // The oldest stored period supplies the initial end-of-period rates.
    let mut previous_raw = settings_for(history[0])?;
    let mut effective = previous_raw.clone();
    let mut before_current = None;
    for next in history.into_iter().skip(1) {
        let raw = settings_for(next)?;
        let scheduled_raise = find_zam_tarihi(next, raise_months)?.is_some();
        let adjust = |current_rate, previous_raw_rate, previous_effective_rate| {
            if current_rate != previous_raw_rate {
                current_rate
            } else if scheduled_raise {
                index_income_rate(previous_effective_rate, &raw, &effective)
            } else {
                previous_effective_rate
            }
        };
        let mut raised = raw.clone();
        raised.gunlukYemek = adjust(
            raw.gunlukYemek,
            previous_raw.gunlukYemek,
            effective.gunlukYemek,
        );
        raised.birlestirilmisSosyalYardim = adjust(
            raw.birlestirilmisSosyalYardim,
            previous_raw.birlestirilmisSosyalYardim,
            effective.birlestirilmisSosyalYardim,
        );
        raised.giyimYardimi = adjust(
            raw.giyimYardimi,
            previous_raw.giyimYardimi,
            effective.giyimYardimi,
        );
        raised.hizmetZammiBirimi = adjust(
            raw.hizmetZammiBirimi,
            previous_raw.hizmetZammiBirimi,
            effective.hizmetZammiBirimi,
        );
        if next.id == period.id {
            before_current = Some(effective.clone());
        }
        previous_raw = raw;
        effective = raised;
    }
    Ok(RaiseTransition {
        current: effective,
        previous: before_current,
        cutoff,
        period: period.clone(),
    })
}

/// Preserve the existing two-decimal institutional percentage convention;
/// keep the derived rate precise until the final income amount is rounded.
fn index_income_rate(
    previous_rate: Decimal,
    current_settings: &DonemselKurumDegerleri,
    previous_settings: &DonemselKurumDegerleri,
) -> Decimal {
    if previous_settings.gunlukTabanUcret <= Decimal::ZERO
        || current_settings.gunlukTabanUcret <= previous_settings.gunlukTabanUcret
    {
        return previous_rate;
    }
    let wage_increase_percent = round2(
        (current_settings.gunlukTabanUcret - previous_settings.gunlukTabanUcret) * dec!(100)
            / previous_settings.gunlukTabanUcret,
    );
    previous_rate + previous_rate * wage_increase_percent / dec!(100)
}

pub(crate) fn weighted_period_income_amount(
    before_amount: Decimal,
    after_amount: Decimal,
    before_days: i32,
    after_days: i32,
) -> Result<Decimal> {
    let total_days = before_days + after_days;
    if before_days < 0 || after_days < 0 || total_days <= 0 {
        return Err(DomainError::InvalidData(
            "Zam gelir hesabında takvim günleri geçersiz.".into(),
        ));
    }
    Ok(crate::round_currency_amount(
        (before_amount * Decimal::from(before_days) + after_amount * Decimal::from(after_days))
            / Decimal::from(total_days),
    ))
}

fn period_days_before_and_after_raise(
    period: &BordroDonemi,
    raise_date: NaiveDate,
) -> Result<(i32, i32)> {
    let start = period_date(&period.baslangicTarihi, &period.id)?;
    let end = period_date(&period.bitisTarihi, &period.id)?;
    let before_days = (raise_date - start).num_days();
    let after_days = (end - raise_date).num_days() + 1;
    if before_days < 0 || after_days < 0 {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi zam tarihi dönem aralığının dışında.",
            period.id
        )));
    }
    Ok((before_days as i32, after_days as i32))
}

pub(crate) fn find_zam_tarihi(
    period: &BordroDonemi,
    zam_aylari: &[i32],
) -> Result<Option<NaiveDate>> {
    let start = period_date(&period.baslangicTarihi, &period.id)?;
    let end = period_date(&period.bitisTarihi, &period.id)?;
    if zam_aylari.iter().any(|month| !(1..=12).contains(month)) {
        return Err(DomainError::ValidationError(
            "Zam ayları 1–12 arasında olmalıdır.".into(),
        ));
    }
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
