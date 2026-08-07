use crate::domain::models::*;
use crate::domain::{DomainError, Result};
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::SettingsRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPayload {
    pub donemler: Option<Vec<BordroDonemi>>,
    pub aktifDonemId: Option<String>,
    pub personeller: Option<Vec<LegacyPersonel>>,
    pub kurumDegerleriMap: Option<HashMap<String, DonemselKurumDegerleri>>,
    pub puantajlar: Option<Vec<PersonelPuantaj>>,
    pub bordrolar: Option<Vec<BordroKaydi>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPersonel {
    pub id: String,
    pub tcNo: String,
    pub ad: String,
    pub soyad: String,
    pub grup: String,
    pub unvan: Option<String>,
    pub sgkSicilNo: Option<String>,
    pub iban: Option<String>,
    pub hizmetYili: Option<i32>,
    pub aciklama: Option<String>,
    pub devirKumulatifGvMatrahi: Option<rust_decimal::Decimal>,
    pub devirKumulatifGvMatrahiYili: Option<i32>,
    pub devirKumulatifGvMatrahiBaslangicAyi: Option<i32>,
    pub kesintiler: Option<PersonelKesintileri>,
}

pub struct MigrationService;

impl MigrationService {
    pub fn is_migrated(conn: &Connection) -> Result<bool> {
        let flag = SettingsRepository::get_app_setting(conn, "legacy_migrated")?;
        Ok(flag.as_deref() == Some("true"))
    }

    pub fn migrate_legacy_data(conn: &mut Connection, payload_json: &str) -> Result<()> {
        if Self::is_migrated(conn)? {
            return Ok(()); // Idempotent: already migrated
        }

        let payload: LegacyPayload = serde_json::from_str(payload_json)
            .map_err(|e| DomainError::InvalidData(format!("Geçersiz yedek payload: {}", e)))?;

        // Start SQLite Transaction
        let tx = conn.transaction()
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        // 1. Import Periods
        if let Some(periods) = payload.donemler {
            for p in periods {
                PeriodRepository::save(&tx, &p)?;
            }
        }

        // 2. Import Institution Settings
        if let Some(inst_map) = payload.kurumDegerleriMap {
            for (period_id, mut settings) in inst_map {
                settings.donemId = period_id;
                SettingsRepository::save_institution_settings(&tx, &settings)?;
            }
        }

        // 3. Import Personnel & Tax Opening
        if let Some(personnel_list) = payload.personeller {
            for lp in personnel_list {
                let p = Personel {
                    id: lp.id.clone(),
                    tcNo: lp.tcNo,
                    ad: lp.ad,
                    soyad: lp.soyad,
                    grup: lp.grup,
                    unvan: lp.unvan,
                    sgkSicilNo: lp.sgkSicilNo.unwrap_or_default(),
                    iban: lp.iban.unwrap_or_default(),
                    hizmetYili: lp.hizmetYili.unwrap_or(1),
                    aciklama: lp.aciklama,
                    kesintiler: lp.kesintiler,
                };
                PersonnelRepository::save(&tx, &p)?;

                // If tax opening devir exists in legacy payload
                if let Some(opening_val) = lp.devirKumulatifGvMatrahi {
                    if opening_val > rust_decimal_macros::dec!(0) {
                        let year = lp.devirKumulatifGvMatrahiYili.unwrap_or(2026);
                        let start_month = lp.devirKumulatifGvMatrahiBaslangicAyi.unwrap_or(5);
                        let effective_period_id = format!("{}-{:02}", year, start_month);

                        let tax_opening = PersonelTaxOpening {
                            id: format!("{}_{}", lp.id, year),
                            personnelId: lp.id.clone(),
                            year,
                            gvCumulativeOpening: opening_val,
                            effectiveFromPeriodId: effective_period_id,
                            createdAt: None,
                            updatedAt: None,
                        };
                        TaxOpeningRepository::save(&tx, &tax_opening)?;
                    }
                }
            }
        }

        // 4. Import Attendance
        if let Some(attendance_list) = payload.puantajlar {
            for att in attendance_list {
                AttendanceRepository::save(&tx, &att)?;
            }
        }

        // 5. Import Payroll Records
        if let Some(payroll_list) = payload.bordrolar {
            for b in payroll_list {
                PayrollRepository::save(&tx, &b)?;
            }
        }

        // 6. Set Active Period
        if let Some(active_id) = payload.aktifDonemId {
            SettingsRepository::set_app_setting(&tx, "active_period_id", &active_id)?;
        }

        // 7. Mark Migrated
        SettingsRepository::set_app_setting(&tx, "legacy_migrated", "true")?;

        // Commit transaction
        tx.commit().map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
