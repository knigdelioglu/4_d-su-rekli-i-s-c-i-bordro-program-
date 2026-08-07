use crate::domain::calculations::*;
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::SettingsRepository;
use super::cumulative_tax_service::CumulativeTaxService;
use rusqlite::Connection;
use rust_decimal::Decimal;

pub struct PayrollService;

impl PayrollService {
    pub fn calculate_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        let personel = PersonnelRepository::get_by_id(conn, personnel_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Personel bulunamadı: {}", personnel_id)))?;

        let period = PeriodRepository::get_by_id(conn, period_id)?
            .ok_or_else(|| DomainError::NotFound(format!("Dönem bulunamadı: {}", period_id)))?;

        // Check existing payroll status
        let existing = PayrollRepository::get_all(conn)?
            .into_iter()
            .find(|b| b.personelId == personnel_id && b.donemId == period_id);

        if let Some(ref b) = existing {
            if b.status == BordroStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "Kesinleştirilmiş (FINALIZED) bordro değiştirilemez.".into()
                ));
            }
        }

        // Get saved attendance
        let attendance = AttendanceRepository::get_by_personnel_and_period(conn, personnel_id, period_id)?
            .ok_or_else(|| DomainError::NotFound("Kayıtlı puantaj bulunamadı.".into()))?;

        let mut summary = PuantajOzeti::default();
        for code in attendance.gunler.values() {
            match code.as_str() {
                "Ç" => summary.c += 1,
                "T" => summary.t += 1,
                "G" => summary.g += 1,
                "İ" => summary.i += 1,
                "GÇ" => summary.gc += 1,
                "GÇT" => summary.gct += 1,
                "R" => summary.r += 1,
                _ => {}
            }
        }

        let inst_map = SettingsRepository::get_all_institution_settings(conn)?;
        let kurum_degerleri = inst_map.get(period_id).cloned().unwrap_or_default();

        let gelirler = auto_fill_gelirler_from_puantaj(
            &summary,
            &kurum_degerleri,
            personel.hizmetYili,
            Some(&personel.grup),
        );

        let prev_gv = CumulativeTaxService::get_previous_cumulative_gv(conn, personnel_id, &period)?;
        let prev_asgari_gv = CumulativeTaxService::get_previous_cumulative_asgari_gv(conn, personnel_id, &period)?;

        let devreden_pek_gelen = Self::calculate_incoming_devreden_pek(conn, personnel_id, &period)?;

        let (kesintiler, pek_detay, sonraki_devreden) = calculate_statutory_deductions(
            &gelirler,
            Some(&kurum_degerleri),
            Some(&personel),
            Some(&summary),
            prev_gv,
            &devreden_pek_gelen,
            prev_asgari_gv,
        );

        let gelir_toplam = calculate_gelir_toplam(&gelirler);
        let kesinti_toplam = calculate_kesinti_toplam(&kesintiler);
        let net_odeme = (gelir_toplam - kesinti_toplam).round_dp(2);

        let now = chrono::Utc::now().to_rfc3339();

        let new_bordro = BordroKaydi {
            id: format!("{}_{}", personnel_id, period_id),
            personelId: personnel_id.to_string(),
            donemId: period_id.to_string(),
            puantajOzeti: summary,
            gelirler,
            gelirToplam: gelir_toplam,
            kesintiler,
            kesintiToplam: kesinti_toplam,
            netOdeme: net_odeme,
            status: BordroStatus::CALCULATED,
            olusturulmaTarihi: existing.as_ref().map_or(now.clone(), |b| b.olusturulmaTarihi.clone()),
            sonGuncellemeTarihi: now,
            notlar: Some(format!("{} dönemi hesaplandı.", period.donemAdi)),
            oncekiKumulatifGvMatrahi: Some(prev_gv),
            oncekiKumulatifAsgariGvMatrahi: Some(prev_asgari_gv),
            manuelKumulatifGvMatrahi: None,
            devredenPekGelen: Some(devreden_pek_gelen),
            sonrakiDevredenPek: Some(sonraki_devreden),
            pekDetay: Some(pek_detay),
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
        let all = PayrollRepository::get_all(conn)?;
        if let Some(mut bordro) = all.into_iter().find(|b| b.personelId == personnel_id && b.donemId == period_id) {
            // A FINALIZED payroll is immutable: it can neither be re-calculated nor
            // downgraded to a mutable status. Only the FINALIZED state can ever be set again.
            if bordro.status == BordroStatus::FINALIZED && status != BordroStatus::FINALIZED {
                return Err(DomainError::PayrollFinalized(
                    "Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.".into()
                ));
            }
            bordro.status = status;
            PayrollRepository::save(conn, &bordro)?;
            Ok(())
        } else {
            Err(DomainError::NotFound("Bordro kaydı bulunamadı.".into()))
        }
    }

    fn calculate_incoming_devreden_pek(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Vec<DevredenPekKaydi>> {
        let all_periods = PeriodRepository::get_all(conn)?;
        let all_payrolls = PayrollRepository::get_all(conn)?;

        let mut prior_periods: Vec<&BordroDonemi> = all_periods
            .iter()
            .filter(|p| p.yil < active_period.yil || (p.yil == active_period.yil && p.ay < active_period.ay))
            .collect();

        prior_periods.sort_by(|a, b| (b.yil * 12 + b.ay).cmp(&(a.yil * 12 + a.ay)));

        if prior_periods.is_empty() {
            return Ok(Vec::new());
        }

        let immediately_prior = prior_periods[0];
        let prior_bordro = all_payrolls
            .iter()
            .find(|b| b.personelId == personnel_id && b.donemId == immediately_prior.id);

        Ok(prior_bordro
            .and_then(|b| b.sonrakiDevredenPek.clone())
            .unwrap_or_default())
    }
}
