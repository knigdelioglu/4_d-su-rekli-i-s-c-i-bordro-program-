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
        let tax_opening = TaxOpeningRepository::get_by_personnel_and_year(
            conn,
            personnel_id,
            active_period.taxYear,
        )?;

        let all_periods = PeriodRepository::get_all(conn)?;
        let all_payrolls = PayrollRepository::get_all(conn)?;

        let mut cumulative_matrah = dec!(0);

        if let Some(opening) = tax_opening {
            if opening.gvCumulativeOpening > dec!(0) {
                // effective_from_period: devrin uygulanmaya başladığı dönem.
                // Başlangıç anahtarı vergi yılı/ayı (taxYear, taxMonth) üzerinden alınır;
                // devir açılış yılından farklı bir vergi yılına aitse yıl başından başlar.
                let effective_period = all_periods
                    .iter()
                    .find(|p| p.id == opening.effectiveFromPeriodId);

                let start_tax_month = match effective_period {
                    Some(p) if p.taxYear == opening.year => p.taxMonth,
                    _ => 1,
                };

                // Collision Check: devir başlangıcından ÖNCE aynı vergi yılında kayıtlı
                // bordro varsa mükerrer matrah oluşur → açık hata.
                let prior_period_ids: Vec<String> = all_periods
                    .iter()
                    .filter(|p| p.taxYear == opening.year && p.taxMonth < start_tax_month)
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

                // Aynı vergi yılında devir başlangıcından aktif döneme kadar olan gerçek bordrolar.
                let valid_prior_period_ids: Vec<String> = all_periods
                    .iter()
                    .filter(|p| {
                        p.taxYear == opening.year
                            && p.taxMonth >= start_tax_month
                            && p.taxMonth < active_period.taxMonth
                    })
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

        // Aktif vergi yılına ait açılış yoksa: aynı vergi yılı içinde
        // taxMonth < aktif.taxMonth olan gerçek bordroları topla.
        let prior_period_ids: Vec<String> = all_periods
            .iter()
            .filter(|p| p.taxYear == active_period.taxYear && p.taxMonth < active_period.taxMonth)
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
            .filter(|p| p.taxYear == active_period.taxYear && p.taxMonth < active_period.taxMonth)
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
