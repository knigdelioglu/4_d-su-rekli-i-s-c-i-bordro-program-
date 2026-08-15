#![allow(non_snake_case)]

use crate::domain::calculations::{calculate_gelir_toplam, calculate_gv_matrah, calculate_kesinti_toplam};
use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::{SettingsRepository, ZAM_AYLARI_SETTING_KEY};
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};

pub const CURRENT_BACKUP_VERSION: u32 = 2;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPayload {
    #[serde(default)]
    pub backupVersion: Option<u32>,
    #[serde(default)]
    pub exportedAt: Option<String>,
    #[serde(default)]
    pub donemler: Option<Vec<BordroDonemi>>,
    #[serde(default)]
    pub aktifDonemId: Option<String>,
    #[serde(default)]
    pub personeller: Option<Vec<LegacyPersonel>>,
    #[serde(default)]
    pub kurumDegerleriMap: Option<HashMap<String, DonemselKurumDegerleri>>,
    #[serde(default)]
    pub puantajlar: Option<Vec<PersonelPuantaj>>,
    #[serde(default)]
    pub bordrolar: Option<Vec<BordroKaydi>>,
    #[serde(default)]
    pub taxOpenings: Option<Vec<PersonelTaxOpening>>,
    #[serde(default)]
    pub sickLeaveRecords: Option<Vec<SickLeaveRecord>>,
    #[serde(default)]
    pub annualPayrollParameters: Option<Vec<AnnualPayrollParameters>>,
    #[serde(default)]
    pub zamAylari: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPersonel {
    pub id: String,
    pub tcNo: String,
    pub ad: String,
    pub soyad: String,
    pub grup: String,
    #[serde(default)]
    pub unvan: Option<String>,
    #[serde(default)]
    pub sgkSicilNo: Option<String>,
    #[serde(default)]
    pub iban: Option<String>,
    #[serde(default)]
    pub hizmetYili: Option<i32>,
    #[serde(default)]
    pub aciklama: Option<String>,
    #[serde(default)]
    pub devirKumulatifGvMatrahi: Option<Decimal>,
    #[serde(default)]
    pub devirKumulatifGvMatrahiYili: Option<i32>,
    #[serde(default)]
    pub devirKumulatifGvMatrahiBaslangicAyi: Option<i32>,
    #[serde(default)]
    pub devirKumulatifAsgariGvMatrahi: Option<Decimal>,
    #[serde(default)]
    pub devirKumulatifAsgariGvMatrahiYili: Option<i32>,
    #[serde(default)]
    pub kesintiler: Option<PersonelKesintileri>,
}

pub struct MigrationService;

impl MigrationService {
    pub fn is_migrated(conn: &Connection) -> Result<bool> {
        let flag = SettingsRepository::get_app_setting(conn, "legacy_migrated")?;
        Ok(flag.as_deref() == Some("true"))
    }

    fn parse_payload(payload_json: &str) -> Result<LegacyPayload> {
        let payload: LegacyPayload = serde_json::from_str(payload_json)
            .map_err(|e| DomainError::InvalidData(format!("Geçersiz yedek payload: {}", e)))?;

        if let Some(version) = payload.backupVersion {
            if version == 0 || version > CURRENT_BACKUP_VERSION {
                return Err(DomainError::InvalidData(format!(
                    "Desteklenmeyen yedek sürümü: {} (desteklenen en yeni sürüm: {}).",
                    version, CURRENT_BACKUP_VERSION
                )));
            }

            if version >= 2
                && (payload.taxOpenings.is_none()
                    || payload.sickLeaveRecords.is_none()
                    || payload.annualPayrollParameters.is_none())
            {
                return Err(DomainError::InvalidData(
                    "V2 yedek payload'ı vergi açılışları, rapor kayıtları ve yıllık bordro parametrelerini içermelidir."
                        .into(),
                ));
            }
        }
        Ok(payload)
    }

    fn same_money(left: Decimal, right: Decimal) -> bool {
        left.round_dp(2) == right.round_dp(2)
    }

    fn validate_imported_payroll_snapshot(conn: &Connection, payroll: &BordroKaydi) -> Result<()> {
        if PersonnelRepository::get_by_id(conn, &payroll.personelId)?.is_none() {
            return Err(DomainError::InvalidData(format!(
                "{} bordrosunun personeli bulunamadı: {}",
                payroll.id, payroll.personelId
            )));
        }
        if PeriodRepository::get_by_id(conn, &payroll.donemId)?.is_none() {
            return Err(DomainError::InvalidData(format!(
                "{} bordrosunun dönemi bulunamadı: {}",
                payroll.id, payroll.donemId
            )));
        }

        let expected_income = calculate_gelir_toplam(&payroll.gelirler);
        if !Self::same_money(expected_income, payroll.gelirToplam) {
            return Err(DomainError::InvalidData(format!(
                "{} bordro snapshot'ında gelir toplamı tutarsız: kalemler {} TL, kayıt {} TL.",
                payroll.id, expected_income, payroll.gelirToplam
            )));
        }

        let expected_deductions = calculate_kesinti_toplam(&payroll.kesintiler);
        if !Self::same_money(expected_deductions, payroll.kesintiToplam) {
            return Err(DomainError::InvalidData(format!(
                "{} bordro snapshot'ında kesinti toplamı tutarsız: kalemler {} TL, kayıt {} TL.",
                payroll.id, expected_deductions, payroll.kesintiToplam
            )));
        }

        let expected_net = (expected_income - expected_deductions).round_dp(2);
        if expected_net < Decimal::ZERO {
            return Err(DomainError::InvalidData(format!(
                "{} bordro snapshot'ında negatif net ödeme var: {} TL.",
                payroll.id, expected_net
            )));
        }
        if !Self::same_money(expected_net, payroll.netOdeme) {
            return Err(DomainError::InvalidData(format!(
                "{} bordro snapshot'ında net ödeme tutarsız: beklenen {} TL, kayıt {} TL.",
                payroll.id, expected_net, payroll.netOdeme
            )));
        }

        if payroll.raporluGun.is_some_and(|days| days != payroll.puantajOzeti.r) {
            return Err(DomainError::InvalidData(format!(
                "{} bordro snapshot'ında raporlu gün puantaj özetiyle uyuşmuyor.",
                payroll.id
            )));
        }

        if let Some(detail) = payroll.isPrimiDetay.as_ref() {
            let income_item = payroll.gelirler.isPrimi.unwrap_or_default();
            if !Self::same_money(detail.tutar, income_item) {
                return Err(DomainError::InvalidData(format!(
                    "{} bordro snapshot'ında iş primi detayı gelir kalemiyle uyuşmuyor.",
                    payroll.id
                )));
            }
        }

        if let Some(pek) = payroll.pekDetay.as_ref() {
            if pek.pekAltSinir < Decimal::ZERO
                || pek.pekUstSinir < pek.pekAltSinir
                || pek.finalPek < Decimal::ZERO
                || pek.finalPek > pek.pekUstSinir
                || pek.hesaplananPek < Decimal::ZERO
                || pek.fiiliYemekGunu < 0
            {
                return Err(DomainError::InvalidData(format!(
                    "{} bordro snapshot'ında PEK detayı geçersiz sınır/değer içeriyor.",
                    payroll.id
                )));
            }
            if pek.hesaplananPek > Decimal::ZERO && pek.finalPek < pek.pekAltSinir {
                return Err(DomainError::InvalidData(format!(
                    "{} bordro snapshot'ında nihai PEK alt sınırın altında.",
                    payroll.id
                )));
            }
        }

        let sgk = payroll.kesintiler.isciSgkPrimi.unwrap_or_default();
        let unemployment = payroll.kesintiler.isciIssizlikPrimi.unwrap_or_default();
        let expected_gv_base = calculate_gv_matrah(expected_income, sgk, unemployment);
        if let Some(gv) = payroll.gvDetay.as_ref() {
            if !Self::same_money(gv.cariGvMatrahi, expected_gv_base)
                || !Self::same_money(
                    gv.kesilenGelirVergisi,
                    payroll.kesintiler.gelirVergisi.unwrap_or_default(),
                )
                || gv.kesilenGelirVergisi < Decimal::ZERO
                || gv.uygulananGvIstisnasi < Decimal::ZERO
            {
                return Err(DomainError::InvalidData(format!(
                    "{} bordro snapshot'ında gelir vergisi detayı kesinti/matrah ile uyuşmuyor.",
                    payroll.id
                )));
            }
            if let Some(previous) = payroll.oncekiKumulatifGvMatrahi {
                if !Self::same_money(previous + expected_gv_base, gv.yeniKumulatifGvMatrahi) {
                    return Err(DomainError::InvalidData(format!(
                        "{} bordro snapshot'ında kümülatif GV zinciri tutarsız.",
                        payroll.id
                    )));
                }
            }
        }

        Ok(())
    }

    fn import_payload(conn: &Connection, payload: LegacyPayload) -> Result<()> {
        let LegacyPayload {
            backupVersion: _,
            exportedAt: _,
            donemler,
            aktifDonemId,
            personeller,
            kurumDegerleriMap,
            puantajlar,
            bordrolar,
            taxOpenings,
            sickLeaveRecords,
            annualPayrollParameters,
            zamAylari,
        } = payload;

        if let Some(periods) = donemler {
            for period in periods {
                PeriodRepository::save(conn, &period)?;
            }
        }

        if let Some(inst_map) = kurumDegerleriMap {
            for (period_id, mut settings) in inst_map {
                settings.donemId = period_id;
                SettingsRepository::save_institution_settings(conn, &settings)?;
            }
        }

        if let Some(personnel_list) = personeller {
            for legacy_personel in personnel_list {
                let personel = Personel {
                    id: legacy_personel.id.clone(),
                    tcNo: legacy_personel.tcNo,
                    ad: legacy_personel.ad,
                    soyad: legacy_personel.soyad,
                    grup: legacy_personel.grup,
                    unvan: legacy_personel.unvan,
                    sgkSicilNo: legacy_personel.sgkSicilNo.unwrap_or_default(),
                    iban: legacy_personel.iban.unwrap_or_default(),
                    hizmetYili: legacy_personel.hizmetYili.unwrap_or(1),
                    aciklama: legacy_personel.aciklama,
                    devirKumulatifGvMatrahi: legacy_personel.devirKumulatifGvMatrahi,
                    devirKumulatifGvMatrahiYili: legacy_personel.devirKumulatifGvMatrahiYili,
                    devirKumulatifGvMatrahiBaslangicAyi: legacy_personel
                        .devirKumulatifGvMatrahiBaslangicAyi,
                    devirKumulatifAsgariGvMatrahi: legacy_personel.devirKumulatifAsgariGvMatrahi,
                    devirKumulatifAsgariGvMatrahiYili: legacy_personel
                        .devirKumulatifAsgariGvMatrahiYili,
                    kesintiler: legacy_personel.kesintiler,
                };
                PersonnelRepository::save(conn, &personel)?;

                if let Some(opening_value) = personel.devirKumulatifGvMatrahi {
                    if opening_value > Decimal::ZERO {
                        let year = personel.devirKumulatifGvMatrahiYili.unwrap_or(2026);
                        let start_month = personel.devirKumulatifGvMatrahiBaslangicAyi.unwrap_or(1);
                        let tax_opening = PersonelTaxOpening {
                            id: format!("{}_{}", personel.id, year),
                            personnelId: personel.id.clone(),
                            year,
                            gvCumulativeOpening: opening_value,
                            effectiveFromPeriodId: format!("{}-{:02}", year, start_month),
                            createdAt: None,
                            updatedAt: None,
                        };
                        if PeriodRepository::get_by_id(conn, &tax_opening.effectiveFromPeriodId)?
                            .is_some()
                        {
                            TaxOpeningRepository::save(conn, &tax_opening)?;
                        }
                    }
                }
            }
        }

        if let Some(openings) = taxOpenings {
            for opening in openings {
                TaxOpeningRepository::save(conn, &opening)?;
            }
        }

        if let Some(attendance_list) = puantajlar {
            for attendance in attendance_list {
                AttendanceRepository::save(conn, &attendance)?;
            }
        }

        if let Some(payroll_list) = bordrolar {
            for payroll in payroll_list {
                Self::validate_imported_payroll_snapshot(conn, &payroll)?;
                PayrollRepository::save_in_transaction(conn, &payroll)?;
            }
        }

        if let Some(sick_records) = sickLeaveRecords {
            for record in sick_records {
                SickLeaveRepository::save(conn, &record)?;
            }
        }

        if let Some(parameters) = annualPayrollParameters {
            for parameter in parameters {
                AnnualPayrollParametersRepository::save(conn, &parameter)?;
            }
        }

        if let Some(months) = zamAylari {
            let normalized_months = SettingsRepository::normalize_zam_aylari(&months)?;
            let value = serde_json::to_string(&normalized_months)
                .map_err(|e| DomainError::InvalidData(e.to_string()))?;
            SettingsRepository::set_app_setting(conn, ZAM_AYLARI_SETTING_KEY, &value)?;
        }

        let imported_tax_years: BTreeSet<i32> = PeriodRepository::get_all(conn)?
            .into_iter()
            .map(|period| period.taxYear)
            .collect();
        for year in imported_tax_years {
            if AnnualPayrollParametersRepository::get_by_year(conn, year)?.is_none() {
                let mut defaults = AnnualPayrollParameters::default_for_2026();
                defaults.year = year;
                AnnualPayrollParametersRepository::save(conn, &defaults)?;
            }
        }

        if let Some(active_id) = aktifDonemId {
            if PeriodRepository::get_by_id(conn, &active_id)?.is_none() {
                return Err(DomainError::InvalidData(format!(
                    "Yedekteki aktif dönem bulunamadı: {}",
                    active_id
                )));
            }
            SettingsRepository::set_app_setting(conn, "active_period_id", &active_id)?;
        }

        Ok(())
    }

    pub fn migrate_legacy_data(conn: &mut Connection, payload_json: &str) -> Result<()> {
        if Self::is_migrated(conn)? {
            return Ok(());
        }

        let payload = Self::parse_payload(payload_json)?;
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Self::import_payload(&tx, payload)?;
        SettingsRepository::set_app_setting(&tx, "legacy_migrated", "true")?;
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn replace_backup_data(conn: &mut Connection, payload_json: &str) -> Result<()> {
        let payload = Self::parse_payload(payload_json)?;
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        tx.execute_batch(
            "DELETE FROM payroll_income_items;
             DELETE FROM payroll_deduction_items;
             DELETE FROM payroll_records;
             DELETE FROM attendance_records;
             DELETE FROM sick_leave_records;
             DELETE FROM personnel_tax_opening;
             DELETE FROM institution_settings;
             DELETE FROM personnel;
             DELETE FROM payroll_periods;
             DELETE FROM annual_payroll_parameters;
             DELETE FROM app_settings;",
        )
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Self::import_payload(&tx, payload)?;
        SettingsRepository::set_app_setting(&tx, "legacy_migrated", "true")?;
        tx.commit()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}
