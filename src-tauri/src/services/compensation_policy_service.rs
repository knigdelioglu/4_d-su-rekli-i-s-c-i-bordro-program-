#![allow(non_snake_case)]

use crate::domain::{DomainError, Result};
use crate::repositories::compensation_policy_repo::CompensationPolicyRepository;
use crate::repositories::payroll_invalidation_repo::PayrollInvalidationRepository;
use crate::repositories::payroll_repo::PayrollRepository;
use crate::repositories::period_repo::PeriodRepository;
use crate::repositories::retro_repo;
use crate::repositories::settings_repo::SettingsRepository;
use crate::services::payroll_service::PayrollService;
use chrono::{Duration, NaiveDate, Utc};
use payroll_core::{
    find_road_rounding_issues, order_compensation_policy_versions, resolve_compensation_segments,
    validate_compensation_policy_timeline, validate_no_new_road_rounding_conflicts, BordroDonemi,
    BordroKaydi, BordroStatus, CompensationBoundaryScope, CompensationPolicyGroup,
    CompensationPolicyState, CompensationPolicyVersion, IsPrimiGrupItem, MutationImpact,
    PayrollKey, PayrollMutation, ResolvedCompensationSegment, RetroMutationAction,
    RetroMutationEffect, RoadRoundingIssue, RoadRoundingMode,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyReadScope {
    pub institutionScope: String,
    pub periodId: String,
    pub periodStartInclusive: String,
    pub periodEndInclusive: String,
    pub projectionDate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompensationPolicyWriteOperation {
    InitialBaseline {
        effectiveFrom: String,
        state: CompensationPolicyState,
        roadRoundingMode: RoadRoundingMode,
        reason: String,
        boundaryScope: CompensationBoundaryScope,
    },
    NewEffectiveDate {
        effectiveFrom: String,
        state: CompensationPolicyState,
        roadRoundingMode: RoadRoundingMode,
        reason: String,
        boundaryScope: CompensationBoundaryScope,
        changeIntent: String,
    },
    SameDateCorrection {
        effectiveFrom: String,
        state: CompensationPolicyState,
        roadRoundingMode: RoadRoundingMode,
        reason: String,
        boundaryScope: CompensationBoundaryScope,
        changeIntent: String,
        supersedesVersionId: String,
    },
}

impl CompensationPolicyWriteOperation {
    fn effective_from(&self) -> &str {
        match self {
            Self::InitialBaseline { effectiveFrom, .. }
            | Self::NewEffectiveDate { effectiveFrom, .. }
            | Self::SameDateCorrection { effectiveFrom, .. } => effectiveFrom,
        }
    }

    fn state(&self) -> &CompensationPolicyState {
        match self {
            Self::InitialBaseline { state, .. }
            | Self::NewEffectiveDate { state, .. }
            | Self::SameDateCorrection { state, .. } => state,
        }
    }

    fn flags(&self) -> (CompensationBoundaryScope, RoadRoundingMode) {
        match self {
            Self::InitialBaseline {
                boundaryScope,
                roadRoundingMode,
                ..
            }
            | Self::NewEffectiveDate {
                boundaryScope,
                roadRoundingMode,
                ..
            }
            | Self::SameDateCorrection {
                boundaryScope,
                roadRoundingMode,
                ..
            } => (*boundaryScope, *roadRoundingMode),
        }
    }

    fn reason(&self) -> &str {
        match self {
            Self::InitialBaseline { reason, .. }
            | Self::NewEffectiveDate { reason, .. }
            | Self::SameDateCorrection { reason, .. } => reason,
        }
    }

    fn change_intent_matches(&self) -> bool {
        match self {
            Self::InitialBaseline { .. } => true,
            Self::NewEffectiveDate {
                boundaryScope,
                changeIntent,
                ..
            }
            | Self::SameDateCorrection {
                boundaryScope,
                changeIntent,
                ..
            } => matches!(
                (*boundaryScope, changeIntent.as_str()),
                (CompensationBoundaryScope::FullEarning, "FULL_STATE_CHANGE")
                    | (CompensationBoundaryScope::RoadOnly, "ROAD_AMOUNT_ONLY")
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyTargetStateToken {
    pub effectiveFrom: String,
    pub activeVersionIdAtDate: Option<String>,
    pub sameDateActiveLeafVersionId: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPayrollStateToken {
    pub payrollId: String,
    pub personnelId: String,
    pub periodId: String,
    pub accrualId: String,
    pub status: BordroStatus,
    pub updatedAt: String,
    pub calculationProvenance: String,
    pub snapshotSchemaVersion: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPreviewToken {
    pub operation: CompensationPolicyWriteOperation,
    pub targetState: CompensationPolicyTargetStateToken,
    pub policyTimelineVersionIds: Vec<String>,
    pub dependencyPayrollStates: Vec<CompensationPolicyPayrollStateToken>,
    pub retroDependencyStates: Vec<CompensationPolicyRetroDependencyStateToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyRetroDependencyStateToken {
    pub batchId: String,
    pub sourcePeriodIds: Vec<String>,
    pub linkedPaymentEventIds: Vec<String>,
    /// Exact serialized persisted graph. It contains the batch, allocations,
    /// revision/overrides, source payrolls/periods/settings, and linked
    /// payment events/periods; it is state, not a hash or counter.
    pub persistedState: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyLoadRequest {
    pub scope: CompensationPolicyReadScope,
    #[serde(default)]
    pub nonAuthoritativePrefill: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPreviewRequest {
    pub scope: CompensationPolicyReadScope,
    pub operation: CompensationPolicyWriteOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicySaveRequest {
    pub scope: CompensationPolicyReadScope,
    pub operation: CompensationPolicyWriteOperation,
    pub previewToken: CompensationPolicyPreviewToken,
    #[serde(default)]
    pub initialBaselineConfirmation: Option<InitialBaselineConfirmation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialBaselineConfirmation {
    pub confirmed: bool,
    pub effectiveFrom: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyLoadData {
    pub scope: CompensationPolicyReadScope,
    pub timeline: CompensationPolicyTimelineData,
    pub activePolicyProjection: ActivePolicyProjection,
    pub initialBaselineRequired: bool,
    pub nonAuthoritativePrefill: CompensationPolicyPrefill,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyTimelineData {
    pub versions: Vec<CompensationPolicyVersion>,
    pub resolvedSegments: Vec<ResolvedCompensationSegment>,
    pub resolution: String,
    pub roadRoundingIssues: Vec<RoadRoundingIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
// This tagged response is short-lived and its direct payload keeps the wire DTO simple.
#[allow(clippy::large_enum_variant)]
pub enum ActivePolicyProjection {
    Active {
        asOfDate: String,
        version: CompensationPolicyVersion,
    },
    Missing {
        asOfDate: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
// This tagged response is short-lived and its direct payload keeps the wire DTO simple.
#[allow(clippy::large_enum_variant)]
pub enum CompensationPolicyPrefill {
    Available {
        authority: String,
        source: String,
        sourcePeriodId: String,
        state: CompensationPolicyPrefillState,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPrefillState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gunlukTabanUcret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gunlukYemek: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birlestirilmisSosyalYardim: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gunlukVasitaYol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub giyimYardimi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hizmetZammiBirimi: Option<String>,
    pub isPrimiYuzde: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isPrimiGruplari: Option<Vec<CompensationPolicyGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geceCalismaPrimiYuzde: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geceCalismaTatiliPrimiYuzde: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ekOdeme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digerGelirVarsayilan: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyDirectEffect {
    pub effectiveFrom: String,
    pub policyStateWindow: PolicyStateWindow,
    pub earningRoundingWindow: EarningRoundingWindow,
    pub boundaryScope: CompensationBoundaryScope,
    pub roadRoundingMode: RoadRoundingMode,
    pub changedFields: Vec<String>,
    pub affectedPeriods: Vec<AffectedPeriod>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyStateWindow {
    pub throughInclusive: Option<String>,
    pub nextEffectiveDate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarningRoundingWindow {
    pub kind: String,
    pub throughInclusive: Option<String>,
    pub nextFullEarningDate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedPeriod {
    pub periodId: String,
    pub startInclusive: String,
    pub endInclusive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPayrollReference {
    pub payrollId: String,
    pub personnelId: String,
    pub periodId: String,
    pub accrualId: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyDependencyEffect {
    pub kind: String,
    pub causePeriodId: String,
    pub affectedPayroll: CompensationPolicyPayrollReference,
    pub fromStatus: BordroStatus,
    pub toStatus: Option<BordroStatus>,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyPreviewData {
    pub directCompensationEffects: Vec<CompensationPolicyDirectEffect>,
    pub dependencyClosureEffects: Vec<CompensationPolicyDependencyEffect>,
    pub retroEffects: Vec<RetroMutationEffect>,
    pub finalizedBlockers: Vec<CompensationPolicyPayrollReference>,
    pub previewToken: CompensationPolicyPreviewToken,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyInvalidatedPayroll {
    pub payroll: CompensationPolicyPayrollReference,
    pub fromStatus: String,
    pub toStatus: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyInvalidatedRetroBatch {
    pub batchId: String,
    pub fromStatus: String,
    pub toStatus: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicyInvalidatedRetroPaymentEvent {
    pub payroll: CompensationPolicyPayrollReference,
    pub fromStatus: String,
    pub toStatus: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompensationPolicySaveReceipt {
    pub version: CompensationPolicyVersion,
    pub invalidatedPayrolls: Vec<CompensationPolicyInvalidatedPayroll>,
    pub invalidatedRetroBatches: Vec<CompensationPolicyInvalidatedRetroBatch>,
    pub invalidatedRetroPaymentEvents: Vec<CompensationPolicyInvalidatedRetroPaymentEvent>,
    pub authoritativeReloadRequired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompensationPolicyApiError {
    Conflict {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        actualTargetState: Option<CompensationPolicyTargetStateToken>,
    },
    FinalizedBlocker {
        message: String,
        blockers: Vec<CompensationPolicyPayrollReference>,
    },
    RetroBlocker {
        message: String,
        blockers: Vec<RetroMutationEffect>,
    },
    RoadOnlyStateConflict {
        message: String,
        proposedEffectiveFrom: String,
        conflictingVersionId: Option<String>,
        reason: String,
    },
    QuarantinedPolicy {
        message: String,
        policyVersion: QuarantinedPolicyReference,
    },
    InvalidRequest {
        message: String,
    },
    InvalidPolicy {
        message: String,
    },
    UnsupportedHistoricalReplay {
        message: String,
    },
    CurrentEngineSnapshotRequired {
        message: String,
    },
    Unauthorized {
        message: String,
    },
    NotFound {
        message: String,
    },
    Persistence {
        message: String,
    },
    Unavailable {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinedPolicyReference {
    pub id: String,
    pub effectiveFrom: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CompensationPolicyOperationResult<T> {
    Success {
        ok: bool,
        value: T,
    },
    Failure {
        ok: bool,
        error: CompensationPolicyApiError,
    },
}

impl<T> CompensationPolicyOperationResult<T> {
    fn success(value: T) -> Self {
        Self::Success { ok: true, value }
    }

    fn failure(error: CompensationPolicyApiError) -> Self {
        Self::Failure { ok: false, error }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadRoundingRepairPreview {
    pub previews: Vec<CompensationPolicyPreviewData>,
}

pub struct CompensationPolicyService;

impl CompensationPolicyService {
    /// Saves the legacy work-premium-group setting through the dated policy
    /// timeline. Existing groups remain immutable; changes begin at the start
    /// of the active period, or correct the active leaf when one already starts
    /// on the selected effective date.
    pub fn save_work_premium_groups(conn: &Connection, groups: &[IsPrimiGrupItem]) -> Result<()> {
        Self::validate_work_premium_groups(groups)?;
        let groups_json = serde_json::to_string(groups)
            .map_err(|error| DomainError::InvalidData(error.to_string()))?;
        crate::repositories::transaction::with_transaction(conn, |tx| {
            Self::save_work_premium_groups_in_transaction(tx, groups, &groups_json)
        })
    }

    fn validate_work_premium_groups(groups: &[IsPrimiGrupItem]) -> Result<()> {
        if groups.is_empty() {
            return Err(DomainError::InvalidData(
                "İş primi grupları boş olamaz.".into(),
            ));
        }
        let mut identifiers = BTreeMap::<String, usize>::new();
        for (index, group) in groups.iter().enumerate() {
            if group.id.trim().is_empty()
                || group.ad.trim().is_empty()
                || group.id.trim() != group.id
                || group.ad.trim() != group.ad
                || group.oran < rust_decimal::Decimal::ZERO
                || group.oran > rust_decimal::Decimal::from(100)
            {
                return Err(DomainError::InvalidData(
                    "İş primi grubu id/ad alanları boşluksuz olmalı; oran %0–%100 arasında olmalıdır.".into(),
                ));
            }
            if !group.aktif {
                continue;
            }
            for identifier in [&group.id, &group.ad] {
                if identifiers
                    .insert(identifier.clone(), index)
                    .is_some_and(|owner| owner != index)
                {
                    return Err(DomainError::InvalidData(format!(
                        "Aktif iş primi gruplarında yinelenen veya belirsiz id/ad: {identifier}."
                    )));
                }
            }
        }
        Ok(())
    }

    fn save_work_premium_groups_in_transaction(
        tx: &Connection,
        groups: &[IsPrimiGrupItem],
        groups_json: &str,
    ) -> Result<()> {
        let versions = CompensationPolicyRepository::get_all_versions(tx)?;
        if versions.is_empty() {
            return SettingsRepository::set_app_setting_in_transaction(
                tx,
                "is_primi_gruplari",
                groups_json,
            );
        }

        let periods = PeriodRepository::get_all(tx)?;
        let active_period_id = SettingsRepository::get_app_setting(tx, "active_period_id")?;
        let period = active_period_id
            .as_deref()
            .and_then(|active_id| periods.iter().find(|period| period.id == active_id))
            .cloned()
            .or_else(|| {
                periods
                    .into_iter()
                    .max_by(|left, right| left.baslangicTarihi.cmp(&right.baslangicTarihi))
            });
        let Some(period) = period else {
            return SettingsRepository::set_app_setting_in_transaction(
                tx,
                "is_primi_gruplari",
                groups_json,
            );
        };

        let active = Self::active_versions(&versions);
        let latest_in_period = active
            .iter()
            .rfind(|version| {
                version.effectiveFrom >= period.baslangicTarihi
                    && version.effectiveFrom <= period.bitisTarihi
            })
            .cloned();
        let effective_from = latest_in_period
            .as_ref()
            .map(|version| version.effectiveFrom.clone())
            .unwrap_or_else(|| period.baslangicTarihi.clone());
        let source = active
            .iter()
            .rfind(|version| version.effectiveFrom <= effective_from)
            .cloned();
        let Some(source) = source else {
            return SettingsRepository::set_app_setting_in_transaction(
                tx,
                "is_primi_gruplari",
                groups_json,
            );
        };
        let policy_groups = groups
            .iter()
            .map(|group| CompensationPolicyGroup {
                id: group.id.clone(),
                ad: group.ad.clone(),
                oran: group.oran,
                aktif: group.aktif,
            })
            .collect::<Vec<_>>();
        if source.state.isPrimiGruplari == policy_groups {
            return SettingsRepository::set_app_setting_in_transaction(
                tx,
                "is_primi_gruplari",
                groups_json,
            );
        }

        let mut state = source.state.clone();
        state.isPrimiGruplari = policy_groups;
        let operation = if source.effectiveFrom == effective_from {
            CompensationPolicyWriteOperation::SameDateCorrection {
                effectiveFrom: effective_from.clone(),
                state,
                roadRoundingMode: source.roadRoundingMode,
                reason: "İş primi grupları güncellendi".into(),
                boundaryScope: CompensationBoundaryScope::FullEarning,
                changeIntent: "FULL_STATE_CHANGE".into(),
                supersedesVersionId: source.id,
            }
        } else {
            CompensationPolicyWriteOperation::NewEffectiveDate {
                effectiveFrom: effective_from.clone(),
                state,
                roadRoundingMode: source.roadRoundingMode,
                reason: "İş primi grupları güncellendi".into(),
                boundaryScope: CompensationBoundaryScope::FullEarning,
                changeIntent: "FULL_STATE_CHANGE".into(),
            }
        };
        let scope = CompensationPolicyReadScope {
            institutionScope: "CURRENT_INSTITUTION".into(),
            periodId: period.id,
            periodStartInclusive: period.baslangicTarihi,
            periodEndInclusive: period.bitisTarihi,
            projectionDate: effective_from,
        };
        let preview = Self::preview_inner(tx, &scope, &operation).map_err(api_error_to_domain)?;
        Self::save_inner(
            tx,
            &CompensationPolicySaveRequest {
                scope,
                operation,
                previewToken: preview.previewToken,
                initialBaselineConfirmation: None,
            },
        )?;
        Ok(())
    }

    pub fn load(
        conn: &Connection,
        request: &CompensationPolicyLoadRequest,
    ) -> CompensationPolicyOperationResult<CompensationPolicyLoadData> {
        match Self::load_inner(conn, request) {
            Ok(data) => CompensationPolicyOperationResult::success(data),
            Err(error) => CompensationPolicyOperationResult::failure(error),
        }
    }

    pub fn preview(
        conn: &Connection,
        request: &CompensationPolicyPreviewRequest,
    ) -> CompensationPolicyOperationResult<CompensationPolicyPreviewData> {
        match Self::preview_inner(conn, &request.scope, &request.operation) {
            Ok(data) => CompensationPolicyOperationResult::success(data),
            Err(error) => CompensationPolicyOperationResult::failure(error),
        }
    }

    pub fn save(
        conn: &Connection,
        request: &CompensationPolicySaveRequest,
    ) -> CompensationPolicyOperationResult<CompensationPolicySaveReceipt> {
        let result = crate::repositories::transaction::with_transaction(conn, |tx| {
            Self::save_inner(tx, request)
        });
        match result {
            Ok(receipt) => CompensationPolicyOperationResult::success(receipt),
            Err(error) => CompensationPolicyOperationResult::failure(map_save_error(error)),
        }
    }

    // Build an explicit same-date correction set. Monetary state, dates and old versions remain immutable.
    fn rounding_repair_plan(
        conn: &Connection,
    ) -> std::result::Result<
        (
            Vec<(
                CompensationPolicyReadScope,
                CompensationPolicyWriteOperation,
            )>,
            Vec<CompensationPolicyVersion>,
        ),
        CompensationPolicyApiError,
    > {
        let versions =
            CompensationPolicyRepository::get_all_versions(conn).map_err(map_domain_error)?;
        let mut periods = PeriodRepository::get_all(conn).map_err(map_domain_error)?;
        periods.sort_by(|a, b| a.baslangicTarihi.cmp(&b.baslangicTarihi));
        let mut working = versions.clone();
        // Propagate the established method through each service period, including newly exposed conflicts.
        for period in &periods {
            loop {
                let issues = match find_road_rounding_issues(
                    &period.baslangicTarihi,
                    &period.bitisTarihi,
                    &working,
                ) {
                    Ok(issues) => issues,
                    Err(DomainError::NotFound(_)) => break,
                    Err(error) => return Err(map_domain_error(error)),
                };
                let Some(issue) = issues.first() else {
                    break;
                };
                let version = working
                    .iter_mut()
                    .find(|version| version.id == issue.actual_version_id)
                    .ok_or_else(|| CompensationPolicyApiError::InvalidPolicy {
                        message: "Uyuşmayan sürüm bulunamadı.".into(),
                    })?;
                version.roadRoundingMode = issue.expected_mode;
            }
        }
        let mut operations = Vec::new();
        let mut candidate = versions.clone();
        for source in &versions {
            let repaired = working
                .iter()
                .find(|version| version.id == source.id)
                .unwrap();
            if source.roadRoundingMode == repaired.roadRoundingMode {
                continue;
            }
            let period = periods
                .iter()
                .find(|period| {
                    period.baslangicTarihi <= source.effectiveFrom
                        && period.bitisTarihi >= source.effectiveFrom
                })
                .ok_or_else(|| CompensationPolicyApiError::InvalidPolicy {
                    message: "Düzeltme için hizmet dönemi bulunamadı.".into(),
                })?;
            let operation = CompensationPolicyWriteOperation::SameDateCorrection {
                effectiveFrom: source.effectiveFrom.clone(),
                state: source.state.clone(),
                roadRoundingMode: repaired.roadRoundingMode,
                boundaryScope: source.boundaryScope,
                changeIntent: if source.boundaryScope == CompensationBoundaryScope::RoadOnly {
                    "ROAD_AMOUNT_ONLY"
                } else {
                    "FULL_STATE_CHANGE"
                }
                .into(),
                reason: "Bağlı hizmet dönemlerindeki yol yuvarlama uyumsuzluğunun düzeltilmesi"
                    .into(),
                supersedesVersionId: source.id.clone(),
            };
            let mut version = Self::preview_version(&operation);
            version.id = format!("rounding-repair-{}", source.id);
            candidate.push(version);
            operations.push((
                CompensationPolicyReadScope {
                    institutionScope: "CURRENT_INSTITUTION".into(),
                    periodId: period.id.clone(),
                    periodStartInclusive: period.baslangicTarihi.clone(),
                    periodEndInclusive: period.bitisTarihi.clone(),
                    projectionDate: source.effectiveFrom.clone(),
                },
                operation,
            ));
        }
        let candidate =
            validate_compensation_policy_timeline(&candidate).map_err(map_domain_error)?;
        validate_no_new_road_rounding_conflicts(&periods, &versions, &candidate)
            .map_err(map_domain_error)?;
        Ok((operations, candidate))
    }

    fn rounding_repair_preview_inner(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
    ) -> std::result::Result<RoadRoundingRepairPreview, CompensationPolicyApiError> {
        Self::validate_scope(conn, scope)?;
        let (operations, candidate) = Self::rounding_repair_plan(conn)?;
        let previews = operations
            .iter()
            .map(|(scope, operation)| {
                Self::preview_inner_with_rounding(conn, scope, operation, Some(&candidate))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(RoadRoundingRepairPreview { previews })
    }

    pub fn preview_rounding_repair(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
    ) -> CompensationPolicyOperationResult<RoadRoundingRepairPreview> {
        match Self::rounding_repair_preview_inner(conn, scope) {
            Ok(value) => CompensationPolicyOperationResult::success(value),
            Err(error) => CompensationPolicyOperationResult::failure(error),
        }
    }

    pub fn save_rounding_repair(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
        preview_tokens: &[CompensationPolicyPreviewToken],
    ) -> CompensationPolicyOperationResult<Vec<CompensationPolicySaveReceipt>> {
        let result = crate::repositories::transaction::with_transaction(conn, |tx| {
            let fresh =
                Self::rounding_repair_preview_inner(tx, scope).map_err(api_error_to_domain)?;
            if fresh.previews.len() != preview_tokens.len()
                || fresh
                    .previews
                    .iter()
                    .zip(preview_tokens)
                    .any(|(a, b)| a.previewToken != *b)
            {
                return Err(api_error_to_domain(CompensationPolicyApiError::Conflict {
                    message: "Yol yuvarlama onarım önizlemesi değişti. Yeniden önizleyin.".into(),
                    actualTargetState: None,
                }));
            }
            let (operations, candidate) =
                Self::rounding_repair_plan(tx).map_err(api_error_to_domain)?;
            let mut receipts = Vec::new();
            for (scope, operation) in operations {
                let current =
                    Self::preview_inner_with_rounding(tx, &scope, &operation, Some(&candidate))
                        .map_err(api_error_to_domain)?;
                receipts.push(Self::save_inner_with_rounding(
                    tx,
                    &CompensationPolicySaveRequest {
                        scope,
                        operation,
                        previewToken: current.previewToken,
                        initialBaselineConfirmation: None,
                    },
                    Some(&candidate),
                )?);
            }
            Ok(receipts)
        });
        match result {
            Ok(value) => CompensationPolicyOperationResult::success(value),
            Err(error) => CompensationPolicyOperationResult::failure(map_save_error(error)),
        }
    }

    fn load_inner(
        conn: &Connection,
        request: &CompensationPolicyLoadRequest,
    ) -> std::result::Result<CompensationPolicyLoadData, CompensationPolicyApiError> {
        let period = Self::validate_scope(conn, &request.scope)?;
        if let Some(quarantined) = Self::find_quarantined(conn)? {
            return Err(CompensationPolicyApiError::QuarantinedPolicy {
                message: "Kurum ücret politikası geçmişinde karantinadaki kayıtlar var.".into(),
                policyVersion: quarantined,
            });
        }
        let versions = CompensationPolicyRepository::get_versions_for_period(
            conn,
            &period.baslangicTarihi,
            &period.bitisTarihi,
        )
        .map_err(map_domain_error)?;
        let resolved =
            resolve_compensation_segments(&period.baslangicTarihi, &period.bitisTarihi, &versions);
        let (resolution, resolvedSegments, roadRoundingIssues) = match resolved {
            Ok(segments) => ("RESOLVED", segments, Vec::new()),
            Err(DomainError::NotFound(_)) => ("MISSING_START_POLICY", Vec::new(), Vec::new()),
            Err(error) => {
                let issues = find_road_rounding_issues(
                    &period.baslangicTarihi,
                    &period.bitisTarihi,
                    &versions,
                )
                .map_err(map_domain_error)?;
                if issues.is_empty() {
                    return Err(map_domain_error(error));
                }
                ("INVALID_ROAD_ROUNDING", Vec::new(), issues)
            }
        };
        let active = Self::active_versions(&versions);
        let projection = active
            .iter()
            .rfind(|version| version.effectiveFrom <= request.scope.projectionDate)
            .cloned();
        let initialBaselineRequired = CompensationPolicyRepository::get_all_versions(conn)
            .map_err(map_domain_error)?
            .is_empty();

        Ok(CompensationPolicyLoadData {
            scope: request.scope.clone(),
            timeline: CompensationPolicyTimelineData {
                versions,
                resolvedSegments,
                resolution: resolution.into(),
                roadRoundingIssues,
            },
            activePolicyProjection: projection.map_or_else(
                || ActivePolicyProjection::Missing {
                    asOfDate: request.scope.projectionDate.clone(),
                },
                |version| ActivePolicyProjection::Active {
                    asOfDate: request.scope.projectionDate.clone(),
                    version,
                },
            ),
            initialBaselineRequired,
            nonAuthoritativePrefill: Self::non_authoritative_prefill(conn, &period)?,
        })
    }

    fn non_authoritative_prefill(
        conn: &Connection,
        period: &BordroDonemi,
    ) -> std::result::Result<CompensationPolicyPrefill, CompensationPolicyApiError> {
        let Some(settings) =
            crate::repositories::settings_repo::SettingsRepository::get_raw_institution_settings(
                conn, &period.id,
            )
            .map_err(map_domain_error)?
        else {
            return Ok(CompensationPolicyPrefill::Unavailable {
                reason: "NO_LEGACY_SETTINGS".into(),
            });
        };
        Ok(Self::legacy_settings_to_policy_prefill_only(
            &settings, &period.id,
        ))
    }

    fn legacy_settings_to_policy_prefill_only(
        settings: &serde_json::Value,
        period_id: &str,
    ) -> CompensationPolicyPrefill {
        let required_decimal_fields = [
            "gunlukTabanUcret",
            "gunlukYemek",
            "birlestirilmisSosyalYardim",
            "gunlukVasitaYol",
            "giyimYardimi",
            "hizmetZammiBirimi",
            "geceCalismaPrimiYuzde",
            "geceCalismaTatiliPrimiYuzde",
        ];
        let nullable_decimal_fields = ["isPrimiYuzde", "ekOdeme", "digerGelirVarsayilan"];
        let incomplete = || CompensationPolicyPrefill::Unavailable {
            reason: "INCOMPLETE_SETTINGS".into(),
        };
        if required_decimal_fields.iter().any(|field| {
            !settings
                .get(*field)
                .is_some_and(serde_json::Value::is_string)
        }) {
            return incomplete();
        }
        if nullable_decimal_fields.iter().any(|field| {
            !settings
                .get(*field)
                .is_some_and(|value| value.is_null() || value.is_string())
        }) {
            return incomplete();
        }
        let Some(group_values) = settings
            .get("isPrimiGruplari")
            .and_then(serde_json::Value::as_array)
        else {
            return incomplete();
        };
        if group_values.is_empty()
            || group_values.iter().any(|value| {
                let Some(group) = value.as_object() else {
                    return true;
                };
                !group.get("id").is_some_and(serde_json::Value::is_string)
                    || !group.get("ad").is_some_and(serde_json::Value::is_string)
                    || !group.get("oran").is_some_and(serde_json::Value::is_string)
                    || !group
                        .get("aktif")
                        .is_some_and(serde_json::Value::is_boolean)
            })
        {
            return incomplete();
        }
        let groups = match serde_json::from_value::<Vec<IsPrimiGrupItem>>(serde_json::Value::Array(
            group_values.clone(),
        )) {
            Ok(groups) => Some(
                groups
                    .into_iter()
                    .map(|group| CompensationPolicyGroup {
                        id: group.id,
                        ad: group.ad,
                        oran: group.oran,
                        aktif: group.aktif,
                    })
                    .collect(),
            ),
            Err(_) => return incomplete(),
        };
        let decimal = |field: &str| {
            settings
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let state = CompensationPolicyPrefillState {
            gunlukTabanUcret: decimal("gunlukTabanUcret"),
            gunlukYemek: decimal("gunlukYemek"),
            birlestirilmisSosyalYardim: decimal("birlestirilmisSosyalYardim"),
            gunlukVasitaYol: decimal("gunlukVasitaYol"),
            giyimYardimi: decimal("giyimYardimi"),
            hizmetZammiBirimi: decimal("hizmetZammiBirimi"),
            isPrimiYuzde: decimal("isPrimiYuzde"),
            isPrimiGruplari: groups,
            geceCalismaPrimiYuzde: decimal("geceCalismaPrimiYuzde"),
            geceCalismaTatiliPrimiYuzde: decimal("geceCalismaTatiliPrimiYuzde"),
            ekOdeme: decimal("ekOdeme"),
            digerGelirVarsayilan: decimal("digerGelirVarsayilan"),
        };
        CompensationPolicyPrefill::Available {
            authority: "PREFILL_ONLY".into(),
            source: "LEGACY_INSTITUTION_SETTINGS".into(),
            sourcePeriodId: period_id.to_owned(),
            state,
        }
    }

    fn preview_inner(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
        operation: &CompensationPolicyWriteOperation,
    ) -> std::result::Result<CompensationPolicyPreviewData, CompensationPolicyApiError> {
        Self::preview_inner_with_rounding(conn, scope, operation, None)
    }

    fn preview_inner_with_rounding(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
        operation: &CompensationPolicyWriteOperation,
        final_rounding: Option<&[CompensationPolicyVersion]>,
    ) -> std::result::Result<CompensationPolicyPreviewData, CompensationPolicyApiError> {
        let period = Self::validate_scope(conn, scope)?;
        if let Some(quarantined) = Self::find_quarantined(conn)? {
            return Err(CompensationPolicyApiError::QuarantinedPolicy {
                message: "Kurum ücret politikası geçmişinde karantinadaki kayıtlar var.".into(),
                policyVersion: quarantined,
            });
        }
        let versions =
            CompensationPolicyRepository::get_all_versions(conn).map_err(map_domain_error)?;
        let all_periods = PeriodRepository::get_all(conn).map_err(map_domain_error)?;
        Self::validate_operation(
            scope,
            &period,
            operation,
            &versions,
            &all_periods,
            final_rounding,
        )?;
        let targetState = Self::target_state(&versions, operation.effective_from());
        let directCompensationEffects = Self::direct_effects(&all_periods, &versions, operation)?;
        let direct_period_ids = directCompensationEffects
            .iter()
            .flat_map(|effect| {
                effect
                    .affectedPeriods
                    .iter()
                    .map(|period| period.periodId.clone())
            })
            .collect::<BTreeSet<_>>();
        let effect = directCompensationEffects.first().ok_or_else(|| {
            CompensationPolicyApiError::InvalidPolicy {
                message: "Politika doğrudan etki aralığı üretmedi.".into(),
            }
        })?;
        let (impact, causes) = Self::calculate_impact(
            conn,
            &effect.effectiveFrom,
            effect.policyStateWindow.throughInclusive.as_deref(),
        )?;
        let retro_blockers = impact
            .retroEffects
            .iter()
            .filter(|effect| effect.action == RetroMutationAction::ImmutableBlocker)
            .cloned()
            .collect::<Vec<_>>();
        if !retro_blockers.is_empty() {
            return Err(CompensationPolicyApiError::RetroBlocker {
                message:
                    "Değişiklik kesinleşmiş veya settlement uygulanmış retro tarihçesini etkiliyor."
                        .into(),
                blockers: retro_blockers,
            });
        }
        let retroDependencyStates = Self::retro_dependency_state_tokens(conn, &impact)?;
        let payrolls = PayrollRepository::get_all(conn).map_err(map_domain_error)?;
        let payroll_by_key = payrolls
            .iter()
            .map(|payroll| (key_for(payroll), payroll))
            .collect::<BTreeMap<_, _>>();
        let blockers = impact
            .blockedByFinalized
            .iter()
            .filter_map(|key| {
                payroll_by_key
                    .get(key)
                    .map(|payroll| payroll_reference(payroll))
            })
            .collect::<Vec<_>>();
        if !blockers.is_empty() {
            return Err(CompensationPolicyApiError::FinalizedBlocker {
                message: "Değişiklik kesinleştirilmiş bordro tarihçesini etkiliyor.".into(),
                blockers,
            });
        }
        let affected_set = impact.affectedPayrolls.iter().collect::<BTreeSet<_>>();
        let dependencyPayrollStates = affected_set
            .iter()
            .filter_map(|key| payroll_by_key.get(*key))
            .map(|payroll| state_token(payroll))
            .collect::<Vec<_>>();
        let dependencyClosureEffects = Self::dependency_effects(
            &impact.affectedPayrolls,
            &causes,
            &payroll_by_key,
            &direct_period_ids,
            &PeriodRepository::get_all(conn).map_err(map_domain_error)?,
        );
        let timeline = order_compensation_policy_versions(&versions).map_err(map_domain_error)?;
        let policyTimelineVersionIds = timeline.iter().map(|version| version.id.clone()).collect();

        Ok(CompensationPolicyPreviewData {
            directCompensationEffects,
            dependencyClosureEffects,
            retroEffects: impact.retroEffects.clone(),
            finalizedBlockers: Vec::new(),
            previewToken: CompensationPolicyPreviewToken {
                operation: operation.clone(),
                targetState,
                policyTimelineVersionIds,
                dependencyPayrollStates,
                retroDependencyStates,
            },
        })
    }

    fn save_inner(
        tx: &Connection,
        request: &CompensationPolicySaveRequest,
    ) -> Result<CompensationPolicySaveReceipt> {
        Self::save_inner_with_rounding(tx, request, None)
    }

    fn save_inner_with_rounding(
        tx: &Connection,
        request: &CompensationPolicySaveRequest,
        final_rounding: Option<&[CompensationPolicyVersion]>,
    ) -> Result<CompensationPolicySaveReceipt> {
        let is_initial = matches!(
            request.operation,
            CompensationPolicyWriteOperation::InitialBaseline { .. }
        );
        if is_initial {
            let confirmed =
                request
                    .initialBaselineConfirmation
                    .as_ref()
                    .is_some_and(|confirmation| {
                        confirmation.confirmed
                            && confirmation.effectiveFrom == request.operation.effective_from()
                    });
            if !confirmed {
                return Err(DomainError::ValidationError(
                    "Başlangıç baseline tarihi kullanıcı tarafından açıkça onaylanmalıdır.".into(),
                ));
            }
        } else if request.initialBaselineConfirmation.is_some() {
            return Err(DomainError::ValidationError(
                "Başlangıç baseline onayı yalnız ilk policy sürümünde kullanılabilir.".into(),
            ));
        }

        let preview = Self::preview_inner_with_rounding(
            tx,
            &request.scope,
            &request.operation,
            final_rounding,
        )
        .map_err(api_error_to_domain)?;
        if preview.previewToken != request.previewToken {
            return Err(api_error_to_domain(CompensationPolicyApiError::Conflict {
                message: "Ücret politikası önizlemesi değişti; güncel verilerle yeniden önizleyin."
                    .into(),
                actualTargetState: Some(preview.previewToken.targetState),
            }));
        }
        let period = Self::validate_scope(tx, &request.scope).map_err(api_error_to_domain)?;
        let versions = CompensationPolicyRepository::get_all_versions(tx)?;
        let all_periods = PeriodRepository::get_all(tx)?;
        Self::validate_operation(
            &request.scope,
            &period,
            &request.operation,
            &versions,
            &all_periods,
            final_rounding,
        )
        .map_err(api_error_to_domain)?;
        let effect = preview.directCompensationEffects.first().ok_or_else(|| {
            DomainError::InvalidData("Politika doğrudan etki aralığı üretmedi.".into())
        })?;
        let (impact, _) = Self::calculate_impact(
            tx,
            &effect.effectiveFrom,
            effect.policyStateWindow.throughInclusive.as_deref(),
        )
        .map_err(api_error_to_domain)?;
        if !impact.blockedByFinalized.is_empty()
            || impact
                .retroEffects
                .iter()
                .any(|effect| effect.action == RetroMutationAction::ImmutableBlocker)
        {
            return Err(DomainError::ValidationError(
                "Önizleme kapsamındaki dependency veya retro kayıtları değişti; işlem durduruldu."
                    .into(),
            ));
        }

        let (boundaryScope, roadRoundingMode) = request.operation.flags();
        let version = CompensationPolicyVersion {
            id: Uuid::new_v4().to_string(),
            effectiveFrom: request.operation.effective_from().to_owned(),
            state: request.operation.state().clone(),
            boundaryScope,
            roadRoundingMode,
            createdAt: Utc::now().to_rfc3339(),
            source: "SETTINGS_COMPENSATION_POLICY".into(),
            reason: request.operation.reason().trim().to_owned(),
            supersedesVersionId: match &request.operation {
                CompensationPolicyWriteOperation::SameDateCorrection {
                    supersedesVersionId,
                    ..
                } => Some(supersedesVersionId.clone()),
                _ => None,
            },
        };
        let mut candidate = versions.clone();
        candidate.push(version.clone());
        validate_compensation_policy_timeline(&candidate)?;
        CompensationPolicyRepository::insert_version_in_transaction(tx, &version)?;
        if let Some(latest) = CompensationPolicyRepository::get_all_versions(tx)?.last() {
            let groups = latest
                .state
                .isPrimiGruplari
                .iter()
                .map(|group| IsPrimiGrupItem {
                    id: group.id.clone(),
                    ad: group.ad.clone(),
                    oran: group.oran,
                    aktif: group.aktif,
                })
                .collect::<Vec<_>>();
            let groups_json = serde_json::to_string(&groups)
                .map_err(|error| DomainError::InvalidData(error.to_string()))?;
            SettingsRepository::set_app_setting_in_transaction(
                tx,
                "is_primi_gruplari",
                &groups_json,
            )?;
        }

        let mut mutable_impact = impact.clone();
        let payroll_by_key = PayrollRepository::get_all(tx)?
            .into_iter()
            .map(|payroll| (key_for(&payroll), (payroll.id, payroll.status)))
            .collect::<BTreeMap<_, _>>();
        mutable_impact.affectedPayrolls.retain(|key| {
            payroll_by_key
                .get(key)
                .is_some_and(|(_, status)| *status == BordroStatus::CALCULATED)
        });
        let invalidatedPayrolls = mutable_impact
            .affectedPayrolls
            .iter()
            .filter_map(|key| {
                payroll_by_key
                    .get(key)
                    .map(|(payrollId, _)| CompensationPolicyInvalidatedPayroll {
                        payroll: CompensationPolicyPayrollReference {
                            payrollId: payrollId.clone(),
                            personnelId: key.personnelId.clone(),
                            periodId: key.periodId.clone(),
                            accrualId: key.accrualId.clone(),
                        },
                        fromStatus: "CALCULATED".into(),
                        toStatus: "STALE".into(),
                    })
            })
            .collect::<Vec<_>>();
        PayrollInvalidationRepository::apply_impact(tx, &mutable_impact)?;

        let after_payrolls = PayrollRepository::get_all(tx)?;
        let after_batches = retro_repo::get_batches(tx)?;
        let invalidatedRetroBatches = impact
            .retroEffects
            .iter()
            .filter(|effect| effect.action == RetroMutationAction::StaleEligible)
            .filter_map(|effect| {
                let batch = after_batches
                    .iter()
                    .find(|batch| batch.id == effect.batchId)?;
                (batch.status == payroll_core::CompensationRevisionStatus::STALE).then_some(
                    CompensationPolicyInvalidatedRetroBatch {
                        batchId: effect.batchId.clone(),
                        fromStatus: "CALCULATED".into(),
                        toStatus: "STALE".into(),
                    },
                )
            })
            .collect::<Vec<_>>();
        if invalidatedRetroBatches.len()
            != impact
                .retroEffects
                .iter()
                .filter(|effect| effect.action == RetroMutationAction::StaleEligible)
                .count()
        {
            return Err(DomainError::InvalidData(
                "Retro batch invalidation receipt did not match the applied mutation; transaction rolled back.".into(),
            ));
        }
        let linked_event_ids = impact
            .retroEffects
            .iter()
            .filter(|effect| effect.action == RetroMutationAction::StaleEligible)
            .flat_map(|effect| effect.linkedPaymentEventIds.iter().cloned())
            .collect::<BTreeSet<_>>();
        let invalidatedRetroPaymentEvents = after_payrolls
            .iter()
            .filter(|payroll| {
                linked_event_ids.contains(&payroll.id) && payroll.status == BordroStatus::STALE
            })
            .map(|payroll| CompensationPolicyInvalidatedRetroPaymentEvent {
                payroll: payroll_reference(payroll),
                fromStatus: "CALCULATED".into(),
                toStatus: "STALE".into(),
            })
            .collect::<Vec<_>>();
        let expected_event_count = impact
            .retroEffects
            .iter()
            .filter(|effect| effect.action == RetroMutationAction::StaleEligible)
            .map(|effect| effect.linkedPaymentEventIds.len())
            .sum::<usize>();
        if invalidatedRetroPaymentEvents.len() != expected_event_count {
            return Err(DomainError::InvalidData(
                "Retro payment event invalidation receipt did not match the applied mutation; transaction rolled back.".into(),
            ));
        }

        Ok(CompensationPolicySaveReceipt {
            version,
            invalidatedPayrolls,
            invalidatedRetroBatches,
            invalidatedRetroPaymentEvents,
            authoritativeReloadRequired: true,
        })
    }

    fn validate_scope(
        conn: &Connection,
        scope: &CompensationPolicyReadScope,
    ) -> std::result::Result<BordroDonemi, CompensationPolicyApiError> {
        if scope.institutionScope != "CURRENT_INSTITUTION" {
            return Err(CompensationPolicyApiError::InvalidRequest {
                message: "Yalnız CURRENT_INSTITUTION kapsamı desteklenir.".into(),
            });
        }
        let period = PeriodRepository::get_by_id(conn, &scope.periodId)
            .map_err(map_domain_error)?
            .ok_or_else(|| CompensationPolicyApiError::NotFound {
                message: format!("Bordro dönemi bulunamadı: {}", scope.periodId),
            })?;
        if period.baslangicTarihi != scope.periodStartInclusive
            || period.bitisTarihi != scope.periodEndInclusive
        {
            return Err(CompensationPolicyApiError::Conflict {
                message: "Dönem tarihleri sunucudaki yetkili dönemle eşleşmiyor.".into(),
                actualTargetState: None,
            });
        }
        let start = parse_date(&period.baslangicTarihi)?;
        let end = parse_date(&period.bitisTarihi)?;
        let projection = parse_date(&scope.projectionDate)?;
        if projection < start || projection > end {
            return Err(CompensationPolicyApiError::InvalidRequest {
                message: "Projection tarihi seçilen dönem aralığında olmalıdır.".into(),
            });
        }
        Ok(period)
    }

    fn validate_operation(
        scope: &CompensationPolicyReadScope,
        period: &BordroDonemi,
        operation: &CompensationPolicyWriteOperation,
        versions: &[CompensationPolicyVersion],
        all_periods: &[BordroDonemi],
        final_rounding: Option<&[CompensationPolicyVersion]>,
    ) -> std::result::Result<(), CompensationPolicyApiError> {
        if !operation.change_intent_matches() {
            return Err(CompensationPolicyApiError::InvalidRequest {
                message: "boundaryScope ile changeIntent eşleşmiyor.".into(),
            });
        }
        if operation.reason().trim().is_empty() {
            return Err(CompensationPolicyApiError::InvalidRequest {
                message: "Değişiklik gerekçesi zorunludur.".into(),
            });
        }
        let date = parse_date(operation.effective_from())?;
        let period_start = parse_date(&period.baslangicTarihi)?;
        let period_end = parse_date(&period.bitisTarihi)?;
        if date < period_start || date > period_end {
            return Err(CompensationPolicyApiError::InvalidRequest {
                message: "effectiveFrom seçilen yetkili dönemin içinde olmalıdır.".into(),
            });
        }
        match operation {
            CompensationPolicyWriteOperation::InitialBaseline { .. } if !versions.is_empty() => {
                return Err(CompensationPolicyApiError::Conflict {
                    message: "Ücret politikası başlangıç baseline'ı yalnız boş zaman çizelgesine eklenebilir.".into(),
                    actualTargetState: Some(Self::target_state(versions, operation.effective_from())),
                });
            }
            CompensationPolicyWriteOperation::NewEffectiveDate { .. }
                if Self::active_versions(versions)
                    .iter()
                    .any(|version| version.effectiveFrom == operation.effective_from()) =>
            {
                return Err(CompensationPolicyApiError::Conflict {
                    message:
                        "Bu tarihte zaten etkin sürüm var; aynı tarih düzeltmesi seçilmelidir."
                            .into(),
                    actualTargetState: Some(Self::target_state(
                        versions,
                        operation.effective_from(),
                    )),
                });
            }
            CompensationPolicyWriteOperation::SameDateCorrection {
                supersedesVersionId,
                ..
            } => {
                let same_date_leaf = Self::active_versions(versions)
                    .into_iter()
                    .find(|version| version.effectiveFrom == operation.effective_from());
                let Some(leaf) = same_date_leaf else {
                    return Err(CompensationPolicyApiError::Conflict {
                        message: "Aynı effectiveFrom tarihinde düzeltilecek etkin sürüm yok."
                            .into(),
                        actualTargetState: Some(Self::target_state(
                            versions,
                            operation.effective_from(),
                        )),
                    });
                };
                if leaf.id != *supersedesVersionId {
                    return Err(CompensationPolicyApiError::Conflict {
                        message:
                            "Düzeltme token'ı aynı tarihin güncel etkin yaprağını hedeflemiyor."
                                .into(),
                        actualTargetState: Some(Self::target_state(
                            versions,
                            operation.effective_from(),
                        )),
                    });
                }
            }
            _ => {}
        }
        let (boundary_scope, _) = operation.flags();
        if boundary_scope == CompensationBoundaryScope::RoadOnly {
            let prior = Self::active_versions(versions)
                .into_iter()
                .rfind(|version| version.effectiveFrom.as_str() < operation.effective_from());
            if prior
                .as_ref()
                .is_some_and(|version| !same_policy_except_road(&version.state, operation.state()))
            {
                return Err(CompensationPolicyApiError::RoadOnlyStateConflict {
                    message: "ROAD_ONLY yalnız gunlukVasitaYol alanını değiştirebilir.".into(),
                    proposedEffectiveFrom: operation.effective_from().into(),
                    conflictingVersionId: prior.map(|version| version.id.clone()),
                    reason: "NON_ROAD_FIELDS_DIFFER".into(),
                });
            }
        }
        let mut candidate_input = versions.to_vec();
        candidate_input.push(Self::preview_version(operation));
        let candidate = match validate_compensation_policy_timeline(&candidate_input) {
            Ok(ordered) => ordered,
            Err(error) => {
                if let CompensationPolicyWriteOperation::SameDateCorrection { .. } = operation {
                    if let Some(descendant) =
                        Self::active_versions(versions).into_iter().find(|version| {
                            version.effectiveFrom.as_str() > operation.effective_from()
                                && version.boundaryScope == CompensationBoundaryScope::RoadOnly
                        })
                    {
                        return Err(CompensationPolicyApiError::RoadOnlyStateConflict {
                            message: error.to_string(),
                            proposedEffectiveFrom: operation.effective_from().into(),
                            conflictingVersionId: Some(descendant.id.clone()),
                            reason: "DESCENDANT_ROAD_ONLY_INVALIDATED".into(),
                        });
                    }
                }
                return Err(CompensationPolicyApiError::InvalidPolicy {
                    message: error.to_string(),
                });
            }
        };
        validate_no_new_road_rounding_conflicts(
            all_periods,
            versions,
            final_rounding.unwrap_or(&candidate),
        )
        .map_err(|error| CompensationPolicyApiError::InvalidPolicy {
            message: error.to_string(),
        })?;
        resolve_compensation_segments(
            &period.baslangicTarihi,
            &period.bitisTarihi,
            final_rounding.unwrap_or(&candidate),
        )
        .map_err(|error| CompensationPolicyApiError::InvalidPolicy {
            message: error.to_string(),
        })?;
        let _ = scope;
        Ok(())
    }

    fn direct_effects(
        all_periods: &[BordroDonemi],
        versions: &[CompensationPolicyVersion],
        operation: &CompensationPolicyWriteOperation,
    ) -> std::result::Result<Vec<CompensationPolicyDirectEffect>, CompensationPolicyApiError> {
        let date = operation.effective_from();
        let active = Self::active_versions(versions);
        let next = active
            .iter()
            .find(|version| version.effectiveFrom.as_str() > date);
        let through = next
            .map(|version| parse_date(&version.effectiveFrom))
            .transpose()?
            .map(|next| next - Duration::days(1));
        let (boundaryScope, roadRoundingMode) = operation.flags();
        let previous_state = match operation {
            CompensationPolicyWriteOperation::SameDateCorrection { .. } => active
                .iter()
                .find(|version| version.effectiveFrom == date)
                .map(|version| &version.state),
            _ => active
                .iter()
                .rfind(|version| version.effectiveFrom.as_str() < date)
                .map(|version| &version.state),
        };
        let changedFields = if boundaryScope == CompensationBoundaryScope::RoadOnly
            && previous_state
                .is_some_and(|previous| same_policy_except_road(previous, operation.state()))
        {
            vec!["gunlukVasitaYol".into()]
        } else {
            changed_fields(previous_state, operation.state())
        };
        let next_full = active.iter().find(|version| {
            version.effectiveFrom.as_str() > date
                && version.boundaryScope == CompensationBoundaryScope::FullEarning
        });
        let (window_kind, rounding_through) = match operation {
            CompensationPolicyWriteOperation::InitialBaseline { .. } => {
                let end = next_full
                    .map(|version| parse_date(&version.effectiveFrom))
                    .transpose()?
                    .map(|next| next - Duration::days(1));
                ("PERIOD_BASELINE", end)
            }
            _ if boundaryScope == CompensationBoundaryScope::FullEarning => {
                let end = next_full
                    .map(|version| parse_date(&version.effectiveFrom))
                    .transpose()?
                    .map(|next| next - Duration::days(1));
                ("OPENS_FULL_EARNING_SLICE", end)
            }
            _ => {
                let end = next_full
                    .map(|version| parse_date(&version.effectiveFrom))
                    .transpose()?
                    .map(|next| next - Duration::days(1));
                ("ROAD_ONLY_WITHIN_ACTIVE_SLICE", end)
            }
        };
        let effect_start = parse_date(date)?;
        let effect_end = through.unwrap_or(NaiveDate::MAX);
        let affectedPeriods = all_periods
            .iter()
            .filter(|candidate| {
                parse_date(&candidate.bitisTarihi).is_ok_and(|end| end >= effect_start)
                    && parse_date(&candidate.baslangicTarihi).is_ok_and(|start| start <= effect_end)
            })
            .map(|candidate| AffectedPeriod {
                periodId: candidate.id.clone(),
                startInclusive: candidate.baslangicTarihi.clone(),
                endInclusive: candidate.bitisTarihi.clone(),
            })
            .collect();
        Ok(vec![CompensationPolicyDirectEffect {
            effectiveFrom: date.into(),
            policyStateWindow: PolicyStateWindow {
                throughInclusive: through.map(|value| value.format("%Y-%m-%d").to_string()),
                nextEffectiveDate: next.map(|version| version.effectiveFrom.clone()),
            },
            earningRoundingWindow: EarningRoundingWindow {
                kind: window_kind.into(),
                throughInclusive: rounding_through
                    .map(|value| value.format("%Y-%m-%d").to_string()),
                nextFullEarningDate: next_full.map(|version| version.effectiveFrom.clone()),
            },
            boundaryScope,
            roadRoundingMode,
            changedFields,
            affectedPeriods,
        }])
    }

    fn calculate_impact(
        conn: &Connection,
        effective_from: &str,
        through_inclusive: Option<&str>,
    ) -> std::result::Result<
        (
            payroll_core::MutationImpact,
            BTreeMap<PayrollKey, BTreeSet<String>>,
        ),
        CompensationPolicyApiError,
    > {
        let dataset = PayrollService::build_dataset_snapshot(conn).map_err(map_domain_error)?;
        let mut causes = BTreeMap::<PayrollKey, BTreeSet<String>>::new();
        let impact = payroll_core::evaluate_payroll_invalidation(
            &dataset,
            &PayrollMutation::CompensationPolicy {
                effectiveFrom: effective_from.into(),
                throughInclusive: through_inclusive.map(str::to_owned),
            },
        )
        .map_err(map_domain_error)?;
        for cause in &impact.payrollCauses {
            causes
                .entry(cause.payroll.clone())
                .or_default()
                .insert(cause.causePeriodId.clone());
        }
        Ok((impact, causes))
    }

    fn retro_dependency_state_tokens(
        conn: &Connection,
        impact: &MutationImpact,
    ) -> std::result::Result<
        Vec<CompensationPolicyRetroDependencyStateToken>,
        CompensationPolicyApiError,
    > {
        let dataset = PayrollService::build_dataset_snapshot(conn).map_err(map_domain_error)?;
        let mut effects = impact.retroEffects.clone();
        effects.sort_by(|left, right| left.batchId.cmp(&right.batchId));
        let mut result = Vec::with_capacity(effects.len());
        for effect in effects {
            let unsupported = |message: String| {
                CompensationPolicyApiError::UnsupportedHistoricalReplay { message }
            };
            let batch = dataset
                .retroBatches
                .iter()
                .find(|batch| batch.id == effect.batchId)
                .ok_or_else(|| {
                    unsupported(format!("{} retro batch bulunamadı.", effect.batchId))
                })?;
            if batch.status != payroll_core::CompensationRevisionStatus::CALCULATED {
                return Err(unsupported(format!(
                    "{} retro batch CALCULATED değil; persisted replay token üretilemedi.",
                    batch.id
                )));
            }
            let mut allocations = dataset
                .retroAllocations
                .iter()
                .filter(|allocation| allocation.batchId == batch.id)
                .cloned()
                .collect::<Vec<_>>();
            allocations.sort_by(|left, right| left.id.cmp(&right.id));
            if allocations.is_empty() {
                return Err(unsupported(format!(
                    "{} retro batch allocations bulunamadı.",
                    batch.id
                )));
            }
            let sourcePeriodIds = allocations
                .iter()
                .map(|allocation| allocation.sourcePeriodId.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if sourcePeriodIds != effect.sourcePeriodIds {
                return Err(unsupported(format!(
                    "{} retro batch source period tokenı kapanışla eşleşmiyor.",
                    batch.id
                )));
            }
            let revision = dataset
                .compensationRevisions
                .iter()
                .find(|revision| revision.id == batch.revisionId)
                .ok_or_else(|| {
                    unsupported(format!(
                        "{} retro batch revision kaydı bulunamadı.",
                        batch.id
                    ))
                })?;
            let mut overrides = dataset
                .compensationRevisionOverrides
                .iter()
                .filter(|item| item.revisionId == revision.id)
                .cloned()
                .collect::<Vec<_>>();
            overrides.sort_by(|left, right| left.id.cmp(&right.id));

            let mut sourcePeriods = sourcePeriodIds
                .iter()
                .map(|period_id| {
                    dataset
                        .periods
                        .iter()
                        .find(|period| period.id == *period_id)
                        .cloned()
                        .ok_or_else(|| {
                            unsupported(format!(
                                "{} kaynak dönemi bulunamadı: {}.",
                                batch.id, period_id
                            ))
                        })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            sourcePeriods.sort_by(|left, right| left.id.cmp(&right.id));
            let source_ids = sourcePeriodIds.iter().collect::<BTreeSet<_>>();
            let mut sourcePayrolls = dataset
                .payrolls
                .iter()
                .filter(|payroll| {
                    payroll.personelId == batch.personnelId && source_ids.contains(&payroll.donemId)
                })
                .cloned()
                .collect::<Vec<_>>();
            sourcePayrolls.sort_by(|left, right| {
                left.donemId
                    .cmp(&right.donemId)
                    .then_with(|| left.accrualId.cmp(&right.accrualId))
                    .then_with(|| left.id.cmp(&right.id))
            });

            let mut legacySourcePeriodIds = BTreeSet::new();
            for source_period in &sourcePeriods {
                let normals = sourcePayrolls
                    .iter()
                    .filter(|payroll| {
                        payroll.donemId == source_period.id
                            && payroll.accrualType == payroll_core::AccrualType::NORMAL
                            && matches!(
                                payroll.status,
                                BordroStatus::CALCULATED | BordroStatus::FINALIZED
                            )
                    })
                    .collect::<Vec<_>>();
                if normals.len() != 1 {
                    return Err(unsupported(format!(
                        "{} kaynak dönemi {} için tek authoritative NORMAL kaydı bulunamadı.",
                        batch.id, source_period.id
                    )));
                }
                let normal = normals[0];
                match normal.calculationMetadata.as_ref() {
                    None => {
                        legacySourcePeriodIds.insert(source_period.id.clone());
                    }
                    Some(metadata) => {
                        if metadata.snapshot.is_none()
                            || payroll_core::validate_payroll_calculation_metadata_with_context(
                                normal,
                                source_period,
                                &dataset.payrolls,
                                &dataset.periods,
                            )
                                .map_err(|error| unsupported(format!(
                                    "{} NORMAL snapshot doğrulanamadı: {error}", normal.accrualId
                                )))?
                                != payroll_core::PayrollCalculationSnapshotDisposition::CurrentSnapshotValidated
                        {
                            return Err(unsupported(format!(
                                "{} NORMAL kaydında geçerli schema-2 snapshot bulunamadı.",
                                normal.accrualId
                            )));
                        }
                    }
                }
            }

            if !legacySourcePeriodIds.is_empty() {
                return Err(unsupported(format!(
                    "{} snapshotless NORMAL source cannot be replayed from mutable settings or attendance.",
                    batch.id
                )));
            }

            let sourceSettings = BTreeMap::<String, serde_json::Value>::new();
            let legacyHistoryPeriods = Vec::<BordroDonemi>::new();
            let sourceAttendances = Vec::<serde_json::Value>::new();
            let sourceSickLeaveRecords = Vec::<serde_json::Value>::new();
            let sourcePersonnel = serde_json::Value::Null;

            let mut linkedPaymentEvents = dataset
                .payrolls
                .iter()
                .filter(|payroll| effect.linkedPaymentEventIds.contains(&payroll.id))
                .cloned()
                .collect::<Vec<_>>();
            linkedPaymentEvents.sort_by(|left, right| left.id.cmp(&right.id));
            if linkedPaymentEvents.len() != effect.linkedPaymentEventIds.len() {
                return Err(unsupported(format!(
                    "{} linked payment event token is incomplete.",
                    batch.id
                )));
            }
            let payment_period_ids = linkedPaymentEvents
                .iter()
                .map(|event| event.donemId.clone())
                .collect::<BTreeSet<_>>();
            let mut paymentPeriods = payment_period_ids
                .iter()
                .map(|period_id| {
                    dataset
                        .periods
                        .iter()
                        .find(|period| period.id == *period_id)
                        .cloned()
                        .ok_or_else(|| {
                            unsupported(format!(
                                "{} payment period is missing: {}.",
                                batch.id, period_id
                            ))
                        })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            paymentPeriods.sort_by(|left, right| left.id.cmp(&right.id));

            let persistedState = serde_json::json!({
                "batch": batch,
                "allocations": allocations,
                "revision": revision,
                "overrides": overrides,
                "sourcePayrolls": sourcePayrolls,
                "sourcePeriods": sourcePeriods,
                "paymentEvents": linkedPaymentEvents,
                "paymentPeriods": paymentPeriods,
                "sourceSettings": sourceSettings,
                "sourcePersonnel": sourcePersonnel,
                "sourceAttendances": sourceAttendances,
                "sourceSickLeaveRecords": sourceSickLeaveRecords,
                "legacyHistoryPeriods": legacyHistoryPeriods,
                "sourceZamAylari": [],
            });
            result.push(CompensationPolicyRetroDependencyStateToken {
                batchId: batch.id.clone(),
                sourcePeriodIds: effect.sourcePeriodIds,
                linkedPaymentEventIds: effect.linkedPaymentEventIds,
                persistedState,
            });
        }
        Ok(result)
    }

    fn dependency_effects(
        affected: &[PayrollKey],
        causes: &BTreeMap<PayrollKey, BTreeSet<String>>,
        payrolls: &BTreeMap<PayrollKey, &BordroKaydi>,
        direct_periods: &BTreeSet<String>,
        periods: &[BordroDonemi],
    ) -> Vec<CompensationPolicyDependencyEffect> {
        let periods_by_id = periods
            .iter()
            .map(|period| (period.id.as_str(), period))
            .collect::<BTreeMap<_, _>>();
        let mut effects = Vec::new();
        for key in affected {
            let Some(payroll) = payrolls.get(key) else {
                continue;
            };
            let candidate_period = periods_by_id.get(payroll.donemId.as_str()).copied();
            let action = match payroll.status {
                BordroStatus::CALCULATED => ("CALCULATED_TO_STALE", Some(BordroStatus::STALE)),
                BordroStatus::STALE => ("ALREADY_STALE", None),
                _ => ("NO_MUTATION", None),
            };
            for cause_id in causes.get(key).into_iter().flatten() {
                let cause_period = periods_by_id.get(cause_id.as_str()).copied();
                let kind =
                    if payroll.donemId == *cause_id || direct_periods.contains(&payroll.donemId) {
                        "PAYMENT"
                    } else if payroll.devredenPekGelen.as_ref().is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item.kaynakDonemId.as_deref() == Some(cause_id.as_str()))
                    }) {
                        "CARRY"
                    } else if candidate_period
                        .zip(cause_period)
                        .is_some_and(|(candidate, cause)| candidate.taxYear == cause.taxYear)
                    {
                        "GV"
                    } else {
                        "PEK"
                    };
                effects.push(CompensationPolicyDependencyEffect {
                    kind: kind.into(),
                    causePeriodId: cause_id.clone(),
                    affectedPayroll: payroll_reference(payroll),
                    fromStatus: payroll.status,
                    toStatus: action.1,
                    action: action.0.into(),
                });
            }
        }
        effects
    }

    fn target_state(
        versions: &[CompensationPolicyVersion],
        effective_from: &str,
    ) -> CompensationPolicyTargetStateToken {
        let active = Self::active_versions(versions);
        let activeVersionIdAtDate = active
            .iter()
            .rfind(|version| version.effectiveFrom.as_str() <= effective_from)
            .map(|version| version.id.clone());
        let sameDateActiveLeafVersionId = active
            .iter()
            .find(|version| version.effectiveFrom == effective_from)
            .map(|version| version.id.clone());
        CompensationPolicyTargetStateToken {
            effectiveFrom: effective_from.into(),
            activeVersionIdAtDate,
            sameDateActiveLeafVersionId,
        }
    }

    fn active_versions(versions: &[CompensationPolicyVersion]) -> Vec<CompensationPolicyVersion> {
        let superseded = versions
            .iter()
            .filter_map(|version| version.supersedesVersionId.as_deref())
            .collect::<BTreeSet<_>>();
        order_compensation_policy_versions(versions)
            .unwrap_or_else(|_| versions.to_vec())
            .into_iter()
            .filter(|version| !superseded.contains(version.id.as_str()))
            .collect()
    }

    fn preview_version(operation: &CompensationPolicyWriteOperation) -> CompensationPolicyVersion {
        let (boundaryScope, roadRoundingMode) = operation.flags();
        CompensationPolicyVersion {
            id: "__compensation-policy-preview__".into(),
            effectiveFrom: operation.effective_from().into(),
            state: operation.state().clone(),
            boundaryScope,
            roadRoundingMode,
            createdAt: "2000-01-01T00:00:00Z".into(),
            source: "COMPENSATION_POLICY_PREVIEW".into(),
            reason: operation.reason().trim().into(),
            supersedesVersionId: match operation {
                CompensationPolicyWriteOperation::SameDateCorrection {
                    supersedesVersionId,
                    ..
                } => Some(supersedesVersionId.clone()),
                _ => None,
            },
        }
    }

    fn find_quarantined(
        conn: &Connection,
    ) -> std::result::Result<Option<QuarantinedPolicyReference>, CompensationPolicyApiError> {
        let row = conn
            .query_row(
                "SELECT id, effective_from, boundary_scope, road_rounding_mode, state_json
                 FROM compensation_policy_versions
                 WHERE boundary_scope IS NULL OR boundary_scope NOT IN ('FULL_EARNING','ROAD_ONLY')
                    OR road_rounding_mode IS NULL OR road_rounding_mode NOT IN ('PER_COMPENSATION_SLICE','PERIOD_TOTAL')
                 ORDER BY effective_from, id LIMIT 1",
                [],
                |row| Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                )),
            );
        match row {
            Ok((id, effectiveFrom, boundary, road, state_json)) => {
                let reason = if boundary
                    .as_deref()
                    .is_none_or(|value| !matches!(value, "FULL_EARNING" | "ROAD_ONLY"))
                {
                    "MISSING_BOUNDARY_SCOPE"
                } else if road
                    .as_deref()
                    .is_none_or(|value| !matches!(value, "PER_COMPENSATION_SLICE" | "PERIOD_TOTAL"))
                {
                    "MISSING_ROAD_ROUNDING_MODE"
                } else if serde_json::from_str::<CompensationPolicyState>(&state_json).is_err() {
                    "INVALID_POLICY_STATE"
                } else {
                    "UNKNOWN_BOUNDARY_METADATA"
                };
                Ok(Some(QuarantinedPolicyReference {
                    id,
                    effectiveFrom,
                    reason: reason.into(),
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(CompensationPolicyApiError::Persistence {
                message: error.to_string(),
            }),
        }
    }
}

fn parse_date(value: &str) -> std::result::Result<NaiveDate, CompensationPolicyApiError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        CompensationPolicyApiError::InvalidRequest {
            message: format!("Kanonik tarih geçersiz: {value} ({error})."),
        }
    })
}

fn same_policy_except_road(
    left: &CompensationPolicyState,
    right: &CompensationPolicyState,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.gunlukVasitaYol = rust_decimal::Decimal::ZERO;
    right.gunlukVasitaYol = rust_decimal::Decimal::ZERO;
    left == right
}

fn changed_fields(
    previous: Option<&CompensationPolicyState>,
    next: &CompensationPolicyState,
) -> Vec<String> {
    let next = serde_json::to_value(next).unwrap_or_default();
    let previous = previous.and_then(|state| serde_json::to_value(state).ok());
    let Some(next_object) = next.as_object() else {
        return Vec::new();
    };
    let previous_object = previous.as_ref().and_then(serde_json::Value::as_object);
    let keys = next_object
        .keys()
        .chain(previous_object.into_iter().flat_map(|object| object.keys()))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    keys.into_iter()
        .filter(|key| next_object.get(key) != previous_object.and_then(|object| object.get(key)))
        .collect()
}

fn key_for(payroll: &BordroKaydi) -> PayrollKey {
    PayrollKey {
        personnelId: payroll.personelId.clone(),
        periodId: payroll.donemId.clone(),
        accrualId: payroll.accrualId.clone(),
    }
}

fn payroll_reference(payroll: &BordroKaydi) -> CompensationPolicyPayrollReference {
    CompensationPolicyPayrollReference {
        payrollId: payroll.id.clone(),
        personnelId: payroll.personelId.clone(),
        periodId: payroll.donemId.clone(),
        accrualId: payroll.accrualId.clone(),
    }
}

fn state_token(payroll: &BordroKaydi) -> CompensationPolicyPayrollStateToken {
    let metadata = payroll.calculationMetadata.as_ref();
    CompensationPolicyPayrollStateToken {
        payrollId: payroll.id.clone(),
        personnelId: payroll.personelId.clone(),
        periodId: payroll.donemId.clone(),
        accrualId: payroll.accrualId.clone(),
        status: payroll.status,
        updatedAt: payroll.sonGuncellemeTarihi.clone(),
        calculationProvenance: if metadata.is_some() {
            "COMPENSATION_POLICY_ENGINE"
        } else {
            "LEGACY"
        }
        .into(),
        snapshotSchemaVersion: metadata.and_then(|metadata| {
            metadata
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.snapshotSchemaVersion)
        }),
    }
}

fn map_domain_error(error: DomainError) -> CompensationPolicyApiError {
    match error {
        DomainError::NotFound(message) => CompensationPolicyApiError::NotFound { message },
        DomainError::ValidationError(message) => {
            CompensationPolicyApiError::InvalidRequest { message }
        }
        DomainError::PayrollFinalized(message) => CompensationPolicyApiError::FinalizedBlocker {
            message,
            blockers: Vec::new(),
        },
        DomainError::DatabaseError(message) => CompensationPolicyApiError::Persistence { message },
        DomainError::InvalidData(message)
            if message.starts_with("UNSUPPORTED_HISTORICAL_REPLAY:") =>
        {
            CompensationPolicyApiError::UnsupportedHistoricalReplay { message }
        }
        other => CompensationPolicyApiError::InvalidPolicy {
            message: other.to_string(),
        },
    }
}

fn api_error_to_domain(error: CompensationPolicyApiError) -> DomainError {
    let encoded = serde_json::to_string(&error).unwrap_or_else(|_| "{}".into());
    DomainError::ValidationError(format!("COMP_POLICY_API:{encoded}"))
}

fn map_save_error(error: DomainError) -> CompensationPolicyApiError {
    if let DomainError::ValidationError(message) = &error {
        if let Some(encoded) = message.strip_prefix("COMP_POLICY_API:") {
            if let Ok(api_error) = serde_json::from_str(encoded) {
                return api_error;
            }
        }
    }
    map_domain_error(error)
}

#[cfg(test)]
mod compensation_prefill_tests {
    use super::{CompensationPolicyPrefill, CompensationPolicyService};
    use serde_json::json;

    fn archived_settings() -> serde_json::Value {
        json!({
            "gunlukTabanUcret": "2616.26",
            "gunlukYemek": "300.75",
            "birlestirilmisSosyalYardim": "5089.7",
            "gunlukVasitaYol": "128.93",
            "giyimYardimi": "269.7",
            "hizmetZammiBirimi": "24.67",
            "isPrimiYuzde": "0",
            "isPrimiGruplari": [
                { "id": "1. Grup", "ad": "1. Grup", "oran": "9", "aktif": true },
                { "id": "2. Grup", "ad": "2. Grup", "oran": "8", "aktif": true },
                { "id": "3. Grup", "ad": "3. Grup", "oran": "7", "aktif": true },
                { "id": "grup_1791176163314", "ad": "UX Deneme Grubu", "oran": "6", "aktif": true }
            ],
            "geceCalismaPrimiYuzde": "0",
            "geceCalismaTatiliPrimiYuzde": "0",
            "ekOdeme": "0",
            "digerGelirVarsayilan": "0"
        })
    }

    #[test]
    fn keeps_archived_decimal_text_nulls_and_all_premium_groups() {
        let prefill = CompensationPolicyService::legacy_settings_to_policy_prefill_only(
            &archived_settings(),
            "2026-09",
        );
        let CompensationPolicyPrefill::Available {
            state,
            sourcePeriodId,
            ..
        } = prefill
        else {
            panic!("complete archive settings should be available as a draft");
        };
        assert_eq!(sourcePeriodId, "2026-09");
        assert_eq!(state.gunlukTabanUcret.as_deref(), Some("2616.26"));
        assert_eq!(state.birlestirilmisSosyalYardim.as_deref(), Some("5089.7"));
        assert_eq!(state.isPrimiYuzde.as_deref(), Some("0"));
        assert_eq!(state.ekOdeme.as_deref(), Some("0"));
        assert_eq!(state.digerGelirVarsayilan.as_deref(), Some("0"));
        let groups = state.isPrimiGruplari.expect("groups should be carried");
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[3].id, "grup_1791176163314");
        assert_eq!(groups[3].ad, "UX Deneme Grubu");
        assert_eq!(groups[3].oran.to_string(), "6");
        assert!(groups.iter().all(|group| group.aktif));
    }

    #[test]
    fn refuses_partial_or_numeric_archive_values_instead_of_inventing_defaults() {
        let mut partial = archived_settings();
        partial.as_object_mut().unwrap().remove("ekOdeme");
        assert!(matches!(
            CompensationPolicyService::legacy_settings_to_policy_prefill_only(&partial, "2026-09"),
            CompensationPolicyPrefill::Unavailable { reason } if reason == "INCOMPLETE_SETTINGS"
        ));

        let mut numeric = archived_settings();
        numeric["gunlukTabanUcret"] = json!(2616.26);
        assert!(matches!(
            CompensationPolicyService::legacy_settings_to_policy_prefill_only(&numeric, "2026-09"),
            CompensationPolicyPrefill::Unavailable { reason } if reason == "INCOMPLETE_SETTINGS"
        ));
    }
}
