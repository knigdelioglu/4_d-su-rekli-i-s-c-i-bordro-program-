use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

pub struct CumulativeTaxService;

impl CumulativeTaxService {
    pub fn get_previous_cumulative_gv(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Decimal> {
        let tax_opening = TaxOpeningRepository::get_by_personnel_and_year(
            conn,
            personnel_id,
            active_period.yil,
        )?;

        let all_periods = PeriodRepository::get_all(conn)?;
        let all_payrolls = PayrollRepository::get_all(conn)?;

        let mut cumulative_matrah = dec!(0);

        if let Some(opening) = tax_opening {
            if opening.gvCumulativeOpening > dec!(0) {
                // Find effective_from_period
                let effective_period = all_periods
                    .iter()
                    .find(|p| p.id == opening.effectiveFromPeriodId);

                let start_month = effective_period.map_or(1, |p| p.ay);

                // Collision Check: Find if there are saved payroll records in the SAME year for months PRIOR to start_month
                let prior_period_ids: Vec<String> = all_periods
                    .iter()
                    .filter(|p| p.yil == active_period.yil && p.ay < start_month)
                    .map(|p| p.id.clone())
                    .collect();

                let has_conflict = all_payrolls.iter().any(|b| {
                    b.personelId == personnel_id && prior_period_ids.contains(&b.donemId)
                });

                if has_conflict {
                    return Err(DomainError::TaxOpeningConflict(
                        "Bu devir matrahı sistemde mevcut geçmiş bordrolarla aynı dönemi kapsamaktadır. Mükerrer vergi matrahını önlemek için devir tutarını veya devir başlangıç dönemini düzeltin.".into()
                    ));
                }

                cumulative_matrah = opening.gvCumulativeOpening;

                // Add payrolls in same year with month >= start_month AND < active_period.ay
                let valid_prior_period_ids: Vec<String> = all_periods
                    .iter()
                    .filter(|p| p.yil == active_period.yil && p.ay >= start_month && p.ay < active_period.ay)
                    .map(|p| p.id.clone())
                    .collect();

                for b in &all_payrolls {
                    if b.personelId == personnel_id && valid_prior_period_ids.contains(&b.donemId) {
                        let isci_sgk = b.kesintiler.isciSgkPrimi.unwrap_or_default();
                        let isci_issizlik = b.kesintiler.isciIssizlikPrimi.unwrap_or_default();
                        let gv_base = (b.gelirToplam - isci_sgk - isci_issizlik).max(dec!(0));
                        cumulative_matrah += gv_base;
                    }
                }

                return Ok(cumulative_matrah.round_dp(2));
            }
        }

        // If no tax opening for active_period.yil: add payrolls in same year < active_period.ay
        let prior_period_ids: Vec<String> = all_periods
            .iter()
            .filter(|p| p.yil == active_period.yil && p.ay < active_period.ay)
            .map(|p| p.id.clone())
            .collect();

        for b in &all_payrolls {
            if b.personelId == personnel_id && prior_period_ids.contains(&b.donemId) {
                let isci_sgk = b.kesintiler.isciSgkPrimi.unwrap_or_default();
                let isci_issizlik = b.kesintiler.isciIssizlikPrimi.unwrap_or_default();
                let gv_base = (b.gelirToplam - isci_sgk - isci_issizlik).max(dec!(0));
                cumulative_matrah += gv_base;
            }
        }

        Ok(cumulative_matrah.round_dp(2))
    }

    pub fn get_previous_cumulative_asgari_gv(
        conn: &Connection,
        _personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Decimal> {
        let all_periods = PeriodRepository::get_all(conn)?;
        let inst_map = crate::repositories::settings_repo::SettingsRepository::get_all_institution_settings(conn)?;

        let prior_periods: Vec<&BordroDonemi> = all_periods
            .iter()
            .filter(|p| p.yil == active_period.yil && p.ay < active_period.ay)
            .collect();

        let mut cumulative_asgari = dec!(0);

        for p in prior_periods {
            let k_degerleri = inst_map.get(&p.id).cloned().unwrap_or_default();
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
