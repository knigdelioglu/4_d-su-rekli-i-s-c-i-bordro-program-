use crate::domain::models::{
    AccrualType, BordroDonemi, BordroKaydi, BordroStatus, DevredenPekKaydi,
    ManualPayrollIncomeInput, PayrollAccrualInput, RetroAdjustmentBatch, RetroAllocation,
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
use payroll_core::{RetroCalculationRequest, RetroEntitlementEngine};
use rusqlite::{params, Connection, OptionalExtension};

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

    /// Atomically persists a calculated retro batch and its canonical payment
    /// event. The batch must be visible to the shared core while calculating,
    /// but neither it nor the event may survive if the calculation or
    /// dependency policy fails.
    pub fn create_retro_payment(
        conn: &Connection,
        batch: &RetroAdjustmentBatch,
        allocations: &[RetroAllocation],
        payment_period_id: &str,
        _sequence: i32,
    ) -> Result<BordroKaydi> {
        if batch.status != crate::domain::models::CompensationRevisionStatus::CALCULATED {
            return Err(DomainError::ValidationError(
                "Yalnız CALCULATED retro batch payment event'e dönüştürülebilir.".into(),
            ));
        }
        if batch.totalGrossDelta <= rust_decimal::Decimal::ZERO
            || allocations
                .iter()
                .any(|allocation| allocation.deltaAmount < rust_decimal::Decimal::ZERO)
        {
            return Err(DomainError::ValidationError(
                "Negatif veya sıfır retro delta otomatik payment event'e dönüştürülemez."
                    .into(),
            ));
        }
        let tx = conn
            .unchecked_transaction()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        let (canonical_batch, canonical_allocations) =
            Self::validate_retro_batch_input(&tx, batch, allocations)?;
        // The batch id is also the stable accrual id of its payment event. Do
        // not allow a caller to reuse it for a different person, period, or
        // accrual type; a composite person+period uniqueness rule alone would
        // otherwise permit a second payment node with the same batch id.
        let existing_batch_identity = PayrollRepository::get_all(&tx)?
            .into_iter()
            .find(|record| record.id == canonical_batch.id || record.accrualId == canonical_batch.id);
        if let Some(existing) = existing_batch_identity {
            if existing.personelId != canonical_batch.personnelId
                || existing.accrualType != AccrualType::RETRO_ADJUSTMENT
                || existing.paymentDate != canonical_batch.paymentDate
            {
                return Err(DomainError::InvalidData(
                    "Retro batch kimliği farklı bir ödeme olayına ait; yeniden bağlanamaz."
                        .into(),
                ));
            }
        }
        if canonical_batch.settlementStatus == crate::domain::models::RetroSettlementStatus::OVERPAYMENT
        {
            return Err(DomainError::ValidationError(
                "Fazla tahakkuk batch'i payment event'e dönüştürülemez.".into(),
            ));
        }
        crate::repositories::retro_repo::save_batch_in_transaction(
            &tx,
            &canonical_batch,
            &canonical_allocations,
        )?;

        let payrolls = PayrollRepository::get_all(&tx)?;
        let existing = payrolls.iter().find(|record| {
            record.personelId == canonical_batch.personnelId
                && record.donemId == payment_period_id
                && record.accrualId == canonical_batch.id
        });
        let effective_sequence = existing.map(|record| record.sequence).unwrap_or_else(|| {
            payrolls
                .iter()
                .filter(|record| {
                    record.personelId == canonical_batch.personnelId
                        && record.paymentDate == canonical_batch.paymentDate
                })
                .map(|record| record.sequence)
                .max()
                .map_or(0, |sequence| sequence + 1)
        });
        let accrual = PayrollAccrualInput {
            accrualId: canonical_batch.id.clone(),
            accrualType: AccrualType::RETRO_ADJUSTMENT,
            paymentDate: canonical_batch.paymentDate.clone(),
            // The database owns the canonical payment order. A caller-provided
            // sequence is only a UI hint and must not be able to reorder an
            // existing event or collide with a concurrent double click.
            sequence: effective_sequence,
            grossAmount: Some(canonical_batch.totalGrossDelta),
            description: canonical_batch
                .description
                .clone()
                .or_else(|| Some("Geriye dönük hakediş farkı".into())),
        };
        let request = Self::build_calculation_request(
            &tx,
            &canonical_batch.personnelId,
            payment_period_id,
            Some(&accrual),
            None,
        )?;
        // Retro settlement is a production payment event. Run the complete
        // cross-period/tax-month preflight before any canonical event can be
        // persisted; the low-level formula function intentionally remains
        // usable by small unit-test fixtures.
        payroll_core::validate_payroll_request(&request)?;
        let calculated = payroll_core::calculate_payroll(&request)?;
        let mutation = if existing.is_some() {
            payroll_core::PayrollMutation::AccrualCalculation {
                personnelId: canonical_batch.personnelId.clone(),
                periodId: payment_period_id.to_string(),
                accrualId: canonical_batch.id.clone(),
            }
        } else {
            payroll_core::PayrollMutation::AccrualInsert {
                personnelId: canonical_batch.personnelId.clone(),
                periodId: payment_period_id.to_string(),
                accrualId: canonical_batch.id.clone(),
                paymentDate: canonical_batch.paymentDate.clone(),
                sequence: effective_sequence,
            }
        };
        let impact = crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::
            assert_mutation_allowed(&tx, &mutation)?;
        PayrollRepository::save_in_transaction(&tx, &calculated)?;
        crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository::apply_impact(
            &tx, &impact,
        )?;
        tx.commit()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        Ok(calculated)
    }

    /// Persists a canonical calculated batch without creating a payment event.
    /// This is the explicit settlement path for negative adjustments: the
    /// overpayment remains in the recognized ledger and can be reviewed or
    /// later handled by a separate lawful recovery/offset workflow.
    pub fn save_retro_adjustment_batch(
        conn: &Connection,
        batch: &RetroAdjustmentBatch,
        allocations: &[RetroAllocation],
    ) -> Result<()> {
        if batch.status != crate::domain::models::CompensationRevisionStatus::CALCULATED {
            return Err(DomainError::ValidationError(
                "Yalnız CALCULATED retro batch saklanabilir.".into(),
            ));
        }
        let tx = conn
            .unchecked_transaction()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        let (canonical_batch, canonical_allocations) =
            Self::validate_retro_batch_input(&tx, batch, allocations)?;
        if canonical_batch.settlementStatus
            != crate::domain::models::RetroSettlementStatus::OVERPAYMENT
        {
            return Err(DomainError::ValidationError(
                "Yalnız açıkça OVERPAYMENT olarak çözülen retro batch payment event olmadan saklanabilir."
                    .into(),
            ));
        }
        let active_payment_event: Option<(String, String)> = tx
            .query_row(
                "SELECT id, status
                 FROM payroll_records
                 WHERE accrual_id = ?1
                   AND status IN ('DRAFT', 'CALCULATED', 'FINALIZED')
                 LIMIT 1",
                params![canonical_batch.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        if let Some((event_id, event_status)) = active_payment_event {
            return Err(if event_status == "FINALIZED" {
                DomainError::PayrollFinalized(format!(
                    "{} retro payment event'i FINALIZED olduğu için fazla tahakkuk batch'i değiştirilemez.",
                    event_id
                ))
            } else {
                DomainError::ValidationError(format!(
                    "{} retro payment event'i hâlâ {} durumunda; payment event silinmeden fazla tahakkuk batch'i saklanamaz.",
                    event_id, event_status
                ))
            });
        }
        crate::repositories::retro_repo::save_batch_in_transaction(
            &tx,
            &canonical_batch,
            &canonical_allocations,
        )?;
        tx.commit()
            .map_err(|error| DomainError::DatabaseError(error.to_string()))
    }

    /// Replays the persisted revision against the transaction snapshot before
    /// accepting a browser/native preview. The UI is a transport boundary,
    /// not an authority for money or source-month SGK fields.
    fn validate_retro_batch_input(
        conn: &Connection,
        batch: &RetroAdjustmentBatch,
        allocations: &[RetroAllocation],
    ) -> Result<(RetroAdjustmentBatch, Vec<RetroAllocation>)> {
        let mut dataset = Self::build_dataset_snapshot(conn)?;
        let revision = dataset
            .compensationRevisions
            .iter()
            .find(|revision| revision.id == batch.revisionId)
            .cloned()
            .ok_or_else(|| {
                DomainError::NotFound(format!(
                    "{} revision'ı persisted dataset'te bulunamadı.",
                    batch.revisionId
                ))
            })?;
        let overrides = dataset
            .compensationRevisionOverrides
            .iter()
            .filter(|override_item| override_item.revisionId == revision.id)
            .cloned()
            .collect::<Vec<_>>();

        // Recalculation of the same batch is idempotent. Its old row must not
        // become a second recognized retro allocation while validating it.
        dataset.retroBatches.retain(|stored| stored.id != batch.id);
        dataset
            .retroAllocations
            .retain(|allocation| allocation.batchId != batch.id);

        let calculated_at = batch
            .calculatedAt
            .clone()
            .or_else(|| batch.createdAt.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let canonical = RetroEntitlementEngine::calculate(&RetroCalculationRequest {
            batchId: batch.id.clone(),
            revision,
            overrides,
            personnelId: batch.personnelId.clone(),
            paymentDate: batch.paymentDate.clone(),
            calculatedAt: calculated_at,
            description: batch.description.clone(),
            dataset,
        })?;

        if batch.revisionId != canonical.batch.revisionId
            || batch.personnelId != canonical.batch.personnelId
            || batch.paymentDate != canonical.batch.paymentDate
            || batch.totalGrossDelta != canonical.batch.totalGrossDelta
        {
            return Err(DomainError::ValidationError(
                "Retro preview güncel persisted revision ile eşleşmiyor; ödeme reddedildi."
                    .into(),
            ));
        }

        let mut supplied = allocations.to_vec();
        let mut expected = canonical.allocations;
        let allocation_order = |allocation: &RetroAllocation| {
            (
                allocation.sourcePeriodId.clone(),
                allocation.earningCode,
                allocation.id.clone(),
            )
        };
        supplied.sort_by_key(allocation_order);
        expected.sort_by_key(allocation_order);
        if supplied != expected {
            return Err(DomainError::ValidationError(
                "Retro allocation preview güncel replay sonucu ile eşleşmiyor; ödeme reddedildi."
                    .into(),
            ));
        }

        let mut persisted_batch = canonical.batch;
        persisted_batch.description = batch.description.clone();
        persisted_batch.createdAt = batch.createdAt.clone();
        persisted_batch.calculatedAt = batch.calculatedAt.clone();
        persisted_batch.finalizedAt = None;
        Ok((persisted_batch, supplied))
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
                AccrualType::RETRO_ADJUSTMENT => Some(saved.gelirToplam),
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
        if finalized.accrualType == AccrualType::RETRO_ADJUSTMENT {
            tx.execute(
                "UPDATE retro_adjustment_batches
                 SET status = 'FINALIZED', settlement_status = 'PAID', finalized_at = ?1
                 WHERE id = ?2 AND status IN ('CALCULATED', 'DRAFT')",
                params![finalized.sonGuncellemeTarihi, finalized.accrualId],
            )
            .map_err(|error| DomainError::DatabaseError(error.to_string()))?;
        }
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

    pub fn build_dataset_snapshot(conn: &Connection) -> Result<payroll_core::PayrollDatasetSnapshot> {
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
            compensationRevisions: crate::repositories::retro_repo::get_revisions(conn)?,
            compensationRevisionOverrides: crate::repositories::retro_repo::get_overrides(conn)?,
            retroBatches: crate::repositories::retro_repo::get_batches(conn)?,
            retroAllocations: crate::repositories::retro_repo::get_allocations(conn)?,
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
