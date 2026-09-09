use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

pub struct CumulativeTaxService;

impl CumulativeTaxService {
    /// Çalışanın gerçek kümülatif GV matrahı vergi yılı/ayı (taxYear/taxMonth)
    /// domaini üzerinden hesaplanır:
    ///   = ilgili vergi yılına ait geçerli personnel_tax_opening
    ///     + aynı vergi yılı içinde taxMonth < aktif.taxMonth olan gerçek bordrolar
    ///
    /// Çalışma yılı/ayı (yil/ay) bu sıralamada authoritative değildir. Aralık dönemi
    /// (15.12.2026–14.01.2027, taxYear=2027/taxMonth=1) 2027 vergi yılının Ocak
    /// bordrosu olarak ele alınır; önceki takvim yılının kümülatifi otomatik taşınmaz.
    pub fn get_previous_cumulative_gv(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Decimal> {
        let explicit_tax_opening = TaxOpeningRepository::get_by_personnel_and_year(
            conn,
            personnel_id,
            active_period.taxYear,
        )?;

        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?;
        if let Some((opening_value, start_tax_month)) = resolve_normal_opening(
            conn,
            explicit_tax_opening.as_ref(),
            personel.as_ref(),
            active_period,
        )? {
            if start_tax_month > active_period.taxMonth {
                return Err(DomainError::ValidationError(
                    "GV opening başlangıç vergi ayı aktif vergi ayı sonrasında olamaz.".into(),
                ));
            }

            // Collision ve kümülatif toplamı SQL tarafında dar aralıkta
            // sorgula; tüm dönem/bordro tablolarını her personel için
            // belleğe taşıma.
            if PayrollRepository::has_personnel_tax_month_before(
                conn,
                personnel_id,
                active_period.taxYear,
                start_tax_month,
            )? {
                return Err(DomainError::TaxOpeningConflict(
                    "Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır. Mükerrer vergi matrahını önlemek için devir tutarını veya devir başlangıç dönemini düzeltin.".into(),
                ));
            }

            let prior_gv = PayrollRepository::sum_gv_base_for_tax_month_range(
                conn,
                personnel_id,
                active_period.taxYear,
                start_tax_month,
                active_period.taxMonth,
            )?;
            return Ok((opening_value + prior_gv).round_dp(2));
        }

        // Aktif vergi yılına ait açılış yoksa: aynı vergi yılı içinde
        // taxMonth < aktif.taxMonth olan gerçek bordroları toplu SQL sorgusuyla al.
        let prior_gv = PayrollRepository::sum_gv_base_for_tax_month_range(
            conn,
            personnel_id,
            active_period.taxYear,
            1,
            active_period.taxMonth,
        )?;
        Ok(prior_gv.round_dp(2))
    }

    /// Resolves the cumulative GV immediately before one explicit accrual.
    /// The period-level API above remains the opening value for a NORMAL
    /// payroll; this API adds earlier same-tax-month accrual snapshots in the
    /// same deterministic paymentDate/sequence/accrualId order as payroll-core.
    pub fn get_previous_cumulative_gv_for_accrual(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
        accrual: &PayrollAccrualInput,
    ) -> Result<Decimal> {
        let mut cumulative = Self::get_previous_cumulative_gv(conn, personnel_id, active_period)?;
        let current_date =
            NaiveDate::parse_from_str(&accrual.paymentDate, "%Y-%m-%d").map_err(|error| {
                DomainError::ValidationError(format!(
                    "Tahakkuk ödeme tarihi geçersiz: {} ({})",
                    accrual.paymentDate, error
                ))
            })?;
        if current_date.year() != active_period.taxYear
            || current_date.month() as i32 != active_period.taxMonth
        {
            return Err(DomainError::ValidationError(
                "Tahakkuk ödeme tarihi aktif vergi yılı/ayı ile uyumlu değil.".into(),
            ));
        }

        let current_id = if accrual.accrualId.trim().is_empty() {
            return Err(DomainError::ValidationError(
                "Tahakkuk kimliği boş olamaz.".into(),
            ));
        } else {
            accrual.accrualId.as_str()
        };
        let records = PayrollRepository::get_all(conn)?;
        for record in records
            .iter()
            .filter(|record| record.personelId == personnel_id)
        {
            let Some(period) = PeriodRepository::get_by_id(conn, &record.donemId)? else {
                return Err(DomainError::NotFound(format!(
                    "{} bordrosunun dönemi bulunamadı.",
                    record.id
                )));
            };
            if period.taxYear != active_period.taxYear || period.taxMonth != active_period.taxMonth
            {
                continue;
            }
            let payment_date = if record.paymentDate.trim().is_empty() {
                payroll_core::payroll_engine::default_payment_date(&period)
            } else {
                record.paymentDate.clone()
            };
            let payment_date =
                NaiveDate::parse_from_str(&payment_date, "%Y-%m-%d").map_err(|error| {
                    DomainError::InvalidData(format!(
                        "{} tahakkuk ödeme tarihi geçersiz: {}",
                        record.id, error
                    ))
                })?;
            let record_id = if record.accrualId.trim().is_empty() {
                record.id.as_str()
            } else {
                record.accrualId.as_str()
            };
            let is_before = payroll_core::payroll_engine::payment_event_order(
                &period,
                &payment_date.format("%Y-%m-%d").to_string(),
                record.sequence,
                record_id,
            )? < payroll_core::payroll_engine::payment_event_order(
                active_period,
                &accrual.paymentDate,
                accrual.sequence,
                current_id,
            )?;
            if !is_before {
                continue;
            }
            if matches!(record.status, BordroStatus::DRAFT | BordroStatus::STALE) {
                return Err(DomainError::ValidationError(format!(
                    "{} tahakkuku {} durumda; kümülatif GV zinciri authoritative değildir.",
                    record_id,
                    match record.status {
                        BordroStatus::DRAFT => "DRAFT",
                        BordroStatus::STALE => "STALE",
                        _ => "",
                    }
                )));
            }
            let gv_base = record
                .gvDetay
                .as_ref()
                .map(|detail| detail.cariGvMatrahi)
                .or(record.persistedGvBase)
                .ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} tahakkukunda authoritative GV matrahı eksik; gross-SGK tahmini kümülatif zincire alınamaz.",
                        record_id
                    ))
                })?;
            cumulative = cumulative.checked_add(gv_base).ok_or_else(|| {
                DomainError::InvalidData("Kümülatif GV Decimal taşması oluştu.".into())
            })?;
        }
        Ok(cumulative.round_dp(2))
    }

    pub fn get_previous_cumulative_asgari_gv(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Decimal> {
        Self::get_previous_cumulative_asgari_gv_with_settings_policy(
            conn,
            personnel_id,
            active_period,
            false,
        )
    }

    /// Production payroll path: a missing prior-period institution setting is
    /// invalid data, not permission to silently calculate with a default rate.
    pub fn get_previous_cumulative_asgari_gv_strict(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Decimal> {
        Self::get_previous_cumulative_asgari_gv_with_settings_policy(
            conn,
            personnel_id,
            active_period,
            true,
        )
    }

    fn get_previous_cumulative_asgari_gv_with_settings_policy(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
        require_settings: bool,
    ) -> Result<Decimal> {
        let explicit_opening = TaxOpeningRepository::get_by_personnel_and_year(
            conn,
            personnel_id,
            active_period.taxYear,
        )?;
        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?;
        let (mut cumulative_asgari, start_tax_month) = resolve_asgari_opening(
            conn,
            explicit_opening.as_ref(),
            personel.as_ref(),
            active_period,
        )?
        .unwrap_or((dec!(0), 1));

        if start_tax_month > active_period.taxMonth {
            return Err(DomainError::ValidationError(
                "Asgari GV opening başlangıç vergi ayı aktif vergi ayı sonrasında olamaz.".into(),
            ));
        }

        let prior_periods = PeriodRepository::get_by_tax_year_before_month(
            conn,
            active_period.taxYear,
            active_period.taxMonth,
        )?;
        let period_by_tax_month = prior_periods
            .into_iter()
            .map(|period| (period.taxMonth, period))
            .collect::<std::collections::BTreeMap<_, _>>();

        for tax_month in start_tax_month..active_period.taxMonth {
            let Some(period) = period_by_tax_month.get(&tax_month) else {
                if require_settings {
                    return Err(DomainError::ValidationError(format!(
                        "{} vergi yılı {} vergi ayı için asgari GV referans dönemi bulunamadı.",
                        active_period.taxYear, tax_month
                    )));
                }
                // The non-strict adapter remains usable for historical
                // scoped fixtures; production callers use the strict path.
                continue;
            };
            let settings = if let Some(settings) =
                crate::repositories::settings_repo::SettingsRepository::get_institution_settings(
                    conn, &period.id,
                )? {
                settings
            } else if require_settings {
                return Err(DomainError::InvalidData(format!(
                    "{} dönemi kurum ayarları bulunamadı; asgari GV kümülatifi hesaplanamaz.",
                    period.id
                )));
            } else {
                DonemselKurumDegerleri {
                    donemId: period.id.clone(),
                    ..DonemselKurumDegerleri::default()
                }
            };
            let settings = historical_statutory_settings(conn, period, &settings)?;
            if require_settings {
                crate::domain::calculations::validate_kurum_degerleri_for_payroll(&settings)?;
            }
            let statutory_snapshot =
                payroll_core::resolve_statutory_snapshot_for_payment_month(period, &settings)?;
            let statutory_parameters = payroll_core::snapshot_statutory_parameters(&settings)?;
            let monthly_reference = payroll_core::calculate_aylik_asgari_ucret_gv_matrahi(
                statutory_snapshot.gvReferansGunlukAsgariUcret,
                statutory_parameters.sgkIsciOraniYuzde / dec!(100),
                statutory_parameters.issizlikIsciOraniYuzde / dec!(100),
            );
            cumulative_asgari = cumulative_asgari
                .checked_add(monthly_reference)
                .ok_or_else(|| {
                    DomainError::InvalidData("Kümülatif asgari GV Decimal taşması oluştu.".into())
                })?;
        }

        Ok(cumulative_asgari.round_dp(2))
    }
}

fn historical_statutory_settings(
    conn: &Connection,
    period: &BordroDonemi,
    settings: &DonemselKurumDegerleri,
) -> Result<DonemselKurumDegerleri> {
    if settings.statutoryParameterSnapshot.is_some() {
        return Ok(settings.clone());
    }

    let payrolls = PayrollRepository::get_all(conn)?;
    let historical_snapshots = payrolls
        .into_iter()
        .filter(|payroll| {
            payroll.donemId == period.id
                && matches!(
                    payroll.status,
                    BordroStatus::CALCULATED | BordroStatus::FINALIZED
                )
        })
        .filter_map(|payroll| payroll.statutorySnapshot)
        .collect::<Vec<_>>();

    let Some(historical_snapshot) = historical_snapshots.first() else {
        // Old scoped fixtures may have no persisted statutory snapshot. The
        // checked production path freezes the period settings before saving;
        // preserve the compatibility adapter's existing behavior here.
        return Ok(settings.clone());
    };
    if historical_snapshots
        .iter()
        .any(|snapshot| snapshot != historical_snapshot)
    {
        return Err(DomainError::InvalidData(format!(
            "{} dönemi bordrolarında uyuşmayan historical statutory snapshot'lar bulundu; asgari GV referansı güvenilir biçimde çözülemez.",
            period.id
        )));
    }

    payroll_core::settings_with_resolved_statutory_snapshot(settings, historical_snapshot).map_err(
        |error| {
            DomainError::InvalidData(format!(
                "{} tarihsel statutory snapshot'ı kullanılamadı: {}",
                period.id, error
            ))
        },
    )
}

fn resolve_explicit_start_tax_month(
    conn: &Connection,
    year: i32,
    period_id: &str,
    label: &str,
) -> Result<i32> {
    let period = PeriodRepository::get_by_id(conn, period_id)?.ok_or_else(|| {
        DomainError::ValidationError(format!(
            "{} effective başlangıç dönemi bulunamadı: {}.",
            label, period_id
        ))
    })?;
    if period.taxYear != year {
        return Err(DomainError::ValidationError(format!(
            "{} effective dönemi {} vergi yılı {}, açılış yılı {} ile uyuşmuyor.",
            label, period.id, period.taxYear, year
        )));
    }
    Ok(period.taxMonth)
}

fn resolve_legacy_start_tax_month(
    conn: &Connection,
    year: i32,
    raw_month: Option<i32>,
    label: &str,
) -> Result<i32> {
    let raw_month = raw_month.ok_or_else(|| {
        DomainError::ValidationError(format!(
            "{} için legacy başlangıç ayı period ID ile çözülemiyor; explicit effectiveFromPeriodId girin.",
            label
        ))
    })?;
    if !(1..=12).contains(&raw_month) {
        return Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı 1-12 arasında olmalıdır.",
            label
        )));
    }
    let mut candidates = PeriodRepository::get_all(conn)?
        .into_iter()
        .filter(|period| {
            period.taxYear == year && (period.ay == raw_month || period.taxMonth == raw_month)
        })
        .map(|period| (period.id, period.taxMonth))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.dedup_by(|left, right| left.0 == right.0);
    match candidates.as_slice() {
        [(_, tax_month)] => Ok(*tax_month),
        [] => Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı {} için {} vergi yılında dönem bulunamadı; explicit effectiveFromPeriodId girin.",
            label, raw_month, year
        ))),
        _ => Err(DomainError::ValidationError(format!(
            "{} legacy başlangıç ayı {} çalışma/vergi ayı olarak belirsiz; explicit effectiveFromPeriodId girin.",
            label, raw_month
        ))),
    }
}

fn resolve_normal_opening(
    conn: &Connection,
    explicit_opening: Option<&PersonelTaxOpening>,
    personel: Option<&Personel>,
    active_period: &BordroDonemi,
) -> Result<Option<(Decimal, i32)>> {
    if let Some(opening) = explicit_opening {
        match (
            opening.gvCumulativeOpening,
            opening.effectiveFromPeriodId.as_deref(),
        ) {
            // A modern row without a normal component leaves the legacy
            // normal component eligible for fallback.
            (None, None) => {}
            (Some(value), Some(period_id)) => {
                if value < dec!(0) {
                    return Err(DomainError::ValidationError(
                        "GV opening matrahı negatif olamaz.".into(),
                    ));
                }
                let start_tax_month =
                    resolve_explicit_start_tax_month(conn, opening.year, period_id, "GV opening")?;
                return Ok(
                    (opening.year == active_period.taxYear).then_some((value, start_tax_month))
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(DomainError::ValidationError(
                    "Normal GV opening değeri ile effectiveFromPeriodId birlikte tanımlanmalıdır."
                        .into(),
                ));
            }
        }
    }

    let legacy_value = personel.and_then(|person| person.devirKumulatifGvMatrahi);
    if legacy_value.is_some_and(|value| value < dec!(0)) {
        return Err(DomainError::ValidationError(
            "Legacy GV devir matrahı negatif olamaz.".into(),
        ));
    }
    let Some(value) = legacy_value.filter(|value| *value > dec!(0)) else {
        return Ok(None);
    };
    let year = personel
        .and_then(|person| person.devirKumulatifGvMatrahiYili)
        .unwrap_or(active_period.taxYear);
    if year != active_period.taxYear {
        return Ok(None);
    }
    let start_tax_month = resolve_legacy_start_tax_month(
        conn,
        year,
        personel.and_then(|person| person.devirKumulatifGvMatrahiBaslangicAyi),
        "Legacy GV opening",
    )?;
    Ok(Some((value, start_tax_month)))
}

fn resolve_asgari_opening(
    conn: &Connection,
    explicit_opening: Option<&PersonelTaxOpening>,
    personel: Option<&Personel>,
    active_period: &BordroDonemi,
) -> Result<Option<(Decimal, i32)>> {
    let (explicit_value, explicit_period_id) = if let Some(opening) = explicit_opening {
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
        if value < dec!(0) {
            return Err(DomainError::ValidationError(
                "Asgari GV opening matrahı negatif olamaz.".into(),
            ));
        }
        let opening = explicit_opening.ok_or_else(|| {
            DomainError::InvalidData("Asgari GV explicit opening kaydı çözülemedi.".into())
        })?;
        let period_id = explicit_period_id.ok_or_else(|| {
            DomainError::InvalidData("Asgari GV explicit opening dönemi çözülemedi.".into())
        })?;
        let start_tax_month =
            resolve_explicit_start_tax_month(conn, opening.year, period_id, "Asgari GV opening")?;
        return Ok((opening.year == active_period.taxYear).then_some((value, start_tax_month)));
    }

    let legacy_value = personel.and_then(|person| person.devirKumulatifAsgariGvMatrahi);
    if legacy_value.is_some_and(|value| value < dec!(0)) {
        return Err(DomainError::ValidationError(
            "Legacy asgari GV devir matrahı negatif olamaz.".into(),
        ));
    }
    // Older personnel rows commonly contain a default zero. It is not a
    // legacy opening and must not force legacy month resolution.
    let Some(value) = legacy_value.filter(|value| *value > dec!(0)) else {
        return Ok(None);
    };
    let year = personel
        .and_then(|person| person.devirKumulatifAsgariGvMatrahiYili)
        .unwrap_or(active_period.taxYear);
    if year != active_period.taxYear {
        return Ok(None);
    }
    let start_tax_month = resolve_legacy_start_tax_month(
        conn,
        year,
        personel.and_then(|person| person.devirKumulatifGvMatrahiBaslangicAyi),
        "Legacy asgari GV opening",
    )?;
    Ok(Some((value, start_tax_month)))
}
