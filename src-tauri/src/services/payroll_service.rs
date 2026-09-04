use crate::domain::models::{
    AccrualType, BordroDonemi, BordroKaydi, BordroStatus, DevredenPekKaydi,
    ManualPayrollIncomeInput, PayrollAccrualInput,
};
use crate::domain::{DomainError, Result};
use crate::repositories::annual_payroll_parameters_repo::AnnualPayrollParametersRepository;
use crate::repositories::attendance_repo::AttendanceRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::personnel_repo::PersonnelRepository;
use crate::repositories::settings_repo::{SettingsRepository, ZAM_AYLARI_SETTING_KEY};
use crate::repositories::sick_leave_repo::SickLeaveRepository;
use crate::repositories::tax_opening_repo::TaxOpeningRepository;
use chrono::Utc;
use rusqlite::Connection;

pub use payroll_core::payroll_engine::{
    resolve_statutory_snapshot_for_period,
    resolve_statutory_snapshot_for_period_with_paid_sick_dates,
};

fn get_zam_aylari(conn: &Connection) -> Result<Vec<i32>> {
    let Some(raw) = SettingsRepository::get_app_setting(conn, ZAM_AYLARI_SETTING_KEY)? else {
        return Ok(Vec::new());
    };
    let months: Vec<i32> = serde_json::from_str(&raw).map_err(|error| {
        DomainError::InvalidData(format!("Kurum zam ayarı bozuk JSON içeriyor: {}", error))
    })?;
    SettingsRepository::normalize_zam_aylari(&months)
}

pub struct PayrollService;

impl PayrollService {
    pub fn calculate_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        Self::calculate_payroll_for_personnel_with_manual_income(
            conn,
            personnel_id,
            period_id,
            None,
        )
    }

    pub fn validate_payroll_request(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<()> {
        Self::validate_payroll_request_for_accrual(conn, personnel_id, period_id, None)
    }

    pub fn validate_payroll_request_for_accrual(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        accrual: Option<&PayrollAccrualInput>,
    ) -> Result<()> {
        let request =
            Self::build_calculation_request(conn, personnel_id, period_id, accrual, None)?;
        payroll_core::validate_payroll_request(&request)
    }

    /// Native persistence adapter for the shared, deterministic payroll engine.
    /// SQLite access and invalidation remain outside `payroll-core`.
    pub fn calculate_payroll_for_personnel_with_manual_income(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        manual_income: Option<&ManualPayrollIncomeInput>,
    ) -> Result<BordroKaydi> {
        Self::calculate_payroll_for_accrual(conn, personnel_id, period_id, None, manual_income)
    }

    pub fn calculate_payroll_for_accrual(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        accrual: Option<&PayrollAccrualInput>,
        manual_income: Option<&ManualPayrollIncomeInput>,
    ) -> Result<BordroKaydi> {
        let request =
            Self::build_calculation_request(conn, personnel_id, period_id, accrual, manual_income)?;
        let calculated = payroll_core::calculate_payroll(&request)?;
        PayrollRepository::save(conn, &calculated)?;
        Ok(calculated)
    }

    /// Native persistence adapter for the shared finalization transaction.
    /// The request snapshot is read through the transaction, finalized by the
    /// core, and only then persisted together with dependent invalidation.
    pub fn finalize_payroll_for_personnel(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
    ) -> Result<BordroKaydi> {
        Self::finalize_payroll_for_accrual(conn, personnel_id, period_id, None)
    }

    pub fn finalize_payroll_for_accrual(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        requested_accrual_id: Option<&str>,
    ) -> Result<BordroKaydi> {
        let tx = conn
            .unchecked_transaction()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        let records = PayrollRepository::get_all(&tx)?;
        let saved = records
            .iter()
            .find(|record| {
                if record.personelId != personnel_id || record.donemId != period_id {
                    return false;
                }
                match requested_accrual_id {
                    Some(target_id) => record.accrualId == target_id || record.id == target_id,
                    None => record.accrualType == AccrualType::NORMAL,
                }
            })
            .ok_or_else(|| DomainError::NotFound("Bordro tahakkuku bulunamadı.".into()))?;
        let accrual = PayrollAccrualInput {
            accrualId: if saved.accrualId.trim().is_empty() {
                saved.id.clone()
            } else {
                saved.accrualId.clone()
            },
            accrualType: saved.accrualType,
            paymentDate: if saved.paymentDate.trim().is_empty() {
                let period = PeriodRepository::get_by_id(&tx, period_id)?
                    .ok_or_else(|| DomainError::NotFound("Dönem bulunamadı.".into()))?;
                payroll_core::payroll_engine::default_payment_date(&period)
            } else {
                saved.paymentDate.clone()
            },
            sequence: saved.sequence,
            grossAmount: match saved.accrualType {
                AccrualType::TEDIYE => saved.gelirler.tediye,
                AccrualType::TIS_IKRAMIYE => saved.gelirler.tisIkramiyesi,
                AccrualType::SUPPLEMENTAL => saved.gelirler.ekOdeme,
                AccrualType::NORMAL => None,
            },
            description: saved.accrualDescription.clone(),
        };
        let manual_income = if saved.accrualType == AccrualType::NORMAL {
            Some(ManualPayrollIncomeInput {
                tediye: saved.gelirler.tediye,
                tisIkramiyesi: saved.gelirler.tisIkramiyesi,
            })
        } else {
            None
        };
        let request = Self::build_calculation_request(
            &tx,
            personnel_id,
            period_id,
            Some(&accrual),
            manual_income.as_ref(),
        )?;
        let finalized = payroll_core::finalize_payroll(&request)?;
        let impact = crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::
            assert_mutation_allowed(
                &tx,
                &payroll_core::PayrollMutation::AccrualCalculation {
                    personnelId: personnel_id.to_string(),
                    periodId: period_id.to_string(),
                    accrualId: accrual.accrualId.clone(),
                },
            )?;
        PayrollRepository::save_in_transaction(&tx, &finalized)?;
        crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::apply_impact(
            &tx, &impact,
        )?;
        tx.commit()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        Ok(finalized)
    }

    pub fn evaluate_mutation_policy(
        conn: &Connection,
        mutation: &payroll_core::PayrollMutation,
    ) -> Result<payroll_core::MutationImpact> {
        let dataset = Self::build_dataset_snapshot(conn)?;
        payroll_core::evaluate_payroll_invalidation(&dataset, mutation)
    }

    fn build_calculation_request(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        accrual: Option<&PayrollAccrualInput>,
        manual_income: Option<&ManualPayrollIncomeInput>,
    ) -> Result<payroll_core::PayrollCalculationRequest> {
        Ok(payroll_core::PayrollCalculationRequest {
            personnelId: personnel_id.to_string(),
            periodId: period_id.to_string(),
            calculatedAt: Utc::now().to_rfc3339(),
            manualIncome: manual_income.cloned(),
            accrual: accrual.cloned(),
            dataset: Self::build_dataset_snapshot(conn)?,
        })
    }

    fn build_dataset_snapshot(conn: &Connection) -> Result<payroll_core::PayrollDatasetSnapshot> {
        Ok(payroll_core::PayrollDatasetSnapshot {
            personnel: PersonnelRepository::get_all(conn)?,
            periods: PeriodRepository::get_all(conn)?,
            institutionSettings: SettingsRepository::get_all_institution_settings(conn)?,
            attendances: AttendanceRepository::get_all(conn)?,
            payrolls: PayrollRepository::get_all(conn)?,
            taxOpenings: TaxOpeningRepository::get_all(conn)?,
            sickLeaveRecords: SickLeaveRepository::get_all(conn)?,
            annualPayrollParameters: AnnualPayrollParametersRepository::get_all(conn)?,
            zamAylari: get_zam_aylari(conn)?,
        })
    }

    pub fn set_payroll_status(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        status: BordroStatus,
    ) -> Result<()> {
        Self::set_payroll_status_for_accrual(conn, personnel_id, period_id, None, status)
    }

    pub fn set_payroll_status_for_accrual(
        conn: &Connection,
        personnel_id: &str,
        period_id: &str,
        accrual_id: Option<&str>,
        status: BordroStatus,
    ) -> Result<()> {
        if status == BordroStatus::FINALIZED {
            return Err(DomainError::ValidationError(
                "FINALIZED geçişi için finalize_payroll API'si kullanılmalıdır.".into(),
            ));
        }
        let resolved_accrual_id = match accrual_id {
            Some(value) => value.to_owned(),
            None => PayrollRepository::get_normal_accrual_id(conn, personnel_id, period_id)?
                .ok_or_else(|| DomainError::NotFound("Bordro kaydı bulunamadı.".into()))?,
        };
        let current = PayrollRepository::get_status_and_created_at_for_accrual(
            conn,
            personnel_id,
            period_id,
            &resolved_accrual_id,
        )?;
        let Some((current_status, _)) = current else {
            return Err(DomainError::NotFound("Bordro kaydı bulunamadı.".into()));
        };

        if current_status == BordroStatus::FINALIZED && status != BordroStatus::FINALIZED {
            return Err(DomainError::PayrollFinalized(
                "Kesinleştirilmiş (FINALIZED) bordronun durumu değiştirilemez.".into(),
            ));
        }
        PayrollRepository::update_status_for_accrual(
            conn,
            personnel_id,
            period_id,
            &resolved_accrual_id,
            status,
        )
    }

    pub fn calculate_incoming_devreden_pek(
        conn: &Connection,
        personnel_id: &str,
        active_period: &BordroDonemi,
    ) -> Result<Vec<DevredenPekKaydi>> {
        let Some(immediately_prior) =
            PeriodRepository::get_previous_by_work_period(conn, active_period)?
        else {
            return Ok(Vec::new());
        };

        Ok(
            PayrollRepository::get_next_devreden_pek(conn, personnel_id, &immediately_prior.id)?
                .unwrap_or_default(),
        )
    }
}
