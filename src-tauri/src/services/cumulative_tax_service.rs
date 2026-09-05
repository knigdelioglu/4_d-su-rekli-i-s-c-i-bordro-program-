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
        let tax_opening = explicit_tax_opening.or_else(|| {
            personel.as_ref().and_then(|p| {
                let opening = p.devirKumulatifGvMatrahi?;
                if opening <= dec!(0) {
                    return None;
                }
                let year = p
                    .devirKumulatifGvMatrahiYili
                    .unwrap_or(active_period.taxYear);
                if year != active_period.taxYear {
                    return None;
                }
                Some(PersonelTaxOpening {
                    id: format!("{}_{}", personnel_id, year),
                    personnelId: personnel_id.to_string(),
                    year,
                    gvCumulativeOpening: opening,
                    effectiveFromPeriodId: format!(
                        "{}-{:02}",
                        year,
                        p.devirKumulatifGvMatrahiBaslangicAyi.unwrap_or(1)
                    ),
                    createdAt: None,
                    updatedAt: None,
                })
            })
        });

        if let Some(opening) = tax_opening {
            if opening.gvCumulativeOpening > dec!(0) {
                // effective_from_period: devrin uygulanmaya başladığı dönem.
                // Başlangıç anahtarı vergi yılı/ayı üzerinden alınır; dönem
                // bulunamazsa eski davranış olan vergi yılı başlangıcı korunur.
                let effective_period =
                    PeriodRepository::get_by_id(conn, &opening.effectiveFromPeriodId)?;
                let start_tax_month = match effective_period {
                    Some(period) if period.taxYear == opening.year => period.taxMonth,
                    _ => 1,
                };

                // Collision ve kümülatif toplamı SQL tarafında dar aralıkta
                // sorgula; tüm dönem/bordro tablolarını her personel için
                // belleğe taşıma.
                if PayrollRepository::has_personnel_tax_month_before(
                    conn,
                    personnel_id,
                    opening.year,
                    start_tax_month,
                )? {
                    return Err(DomainError::TaxOpeningConflict(
                        "Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır. Mükerrer vergi matrahını önlemek için devir tutarını veya devir başlangıç dönemini düzeltin.".into(),
                    ));
                }

                let prior_gv = PayrollRepository::sum_gv_base_for_tax_month_range(
                    conn,
                    personnel_id,
                    opening.year,
                    start_tax_month,
                    active_period.taxMonth,
                )?;
                return Ok((opening.gvCumulativeOpening + prior_gv).round_dp(2));
            }
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
                &period, &payment_date.format("%Y-%m-%d").to_string(), record.sequence, record_id,
            )? < payroll_core::payroll_engine::payment_event_order(
                active_period, &accrual.paymentDate, accrual.sequence, current_id,
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
                .unwrap_or_else(|| {
                    (record.gelirToplam
                        - record.kesintiler.isciSgkPrimi.unwrap_or_default()
                        - record.kesintiler.isciIssizlikPrimi.unwrap_or_default())
                    .max(Decimal::ZERO)
                });
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
        let prior_periods = PeriodRepository::get_by_tax_year_before_month(
            conn,
            active_period.taxYear,
            active_period.taxMonth,
        )?;
        let period_ids: Vec<String> = prior_periods
            .iter()
            .map(|period| period.id.clone())
            .collect();
        let inst_map = crate::repositories::settings_repo::SettingsRepository::get_for_periods(
            conn,
            &period_ids,
        )?;

        let mut cumulative_asgari = personel_devir_asgari_gv(conn, personnel_id, active_period)?;

        for p in prior_periods {
            let k_degerleri = if require_settings {
                inst_map.get(&p.id).cloned().ok_or_else(|| {
                    DomainError::InvalidData(format!(
                        "{} dönemi kurum ayarları bulunamadı; asgari GV kümülatifi hesaplanamaz.",
                        p.id
                    ))
                })?
            } else {
                inst_map.get(&p.id).cloned().unwrap_or_default()
            };
            if require_settings {
                crate::domain::calculations::validate_kurum_degerleri_for_payroll(&k_degerleri)?;
            }
            let gunluk_asgari = k_degerleri.gunlukAsgariUcret.unwrap_or(dec!(1101.00));
            let sgk_rate = k_degerleri.sgkIsciOraniYuzde.unwrap_or(dec!(14)) / dec!(100);
            let issizlik_rate = k_degerleri.issizlikIsciOraniYuzde.unwrap_or(dec!(1)) / dec!(100);

            let aylik_brut_asgari = (gunluk_asgari * dec!(30)).round_dp(2);
            let aylik_asgari_sgk = (aylik_brut_asgari * (sgk_rate + issizlik_rate)).round_dp(2);
            let aylik_asgari_gv_matrah = (aylik_brut_asgari - aylik_asgari_sgk).max(dec!(0));

            cumulative_asgari += aylik_asgari_gv_matrah;
        }

        Ok(cumulative_asgari.round_dp(2))
    }
}

fn personel_devir_asgari_gv(
    conn: &Connection,
    personnel_id: &str,
    active_period: &BordroDonemi,
) -> Result<Decimal> {
    let personel = PersonnelRepository::get_by_id(conn, personnel_id)?;
    Ok(personel
        .and_then(|personel| {
            let value = personel.devirKumulatifAsgariGvMatrahi?;
            let year = personel
                .devirKumulatifAsgariGvMatrahiYili
                .unwrap_or(active_period.taxYear);
            (year == active_period.taxYear).then_some(value)
        })
        .unwrap_or(dec!(0)))
}
