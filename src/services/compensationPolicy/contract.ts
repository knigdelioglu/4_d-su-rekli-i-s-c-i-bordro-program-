import type {
  BordroDonemi,
  BordroKaydi,
  BordroStatus,
  PayrollPeriodInstitutionSettings,
  Personel,
  PersonelPuantaj,
  CompensationRevision,
  CompensationRevisionOverride,
  CompensationBoundaryScope,
  CompensationPolicyState,
  BenefitAccrualPolicy,
  BenefitDayType,
  BenefitPaymentMethod,
  BenefitProrationDenominator,
  SocialBenefitAccrualPolicies,
  CompensationPolicyVersion,
  RetroAdjustmentBatch,
  RetroAllocation,
  ResolvedCompensationSegment,
  RoadRoundingMode,
  SickLeaveRecord,
} from '../../types/payroll';
import type { Exactify } from '../payrollEngine/decimalBoundary';

/** Exact JSON DTOs; money never crosses this contract as a JavaScript number. */
export type CompensationPolicyStateWire = Exactify<CompensationPolicyState>;
export type CompensationPolicyVersionWire = Exactify<CompensationPolicyVersion>;
export type ResolvedCompensationSegmentWire = Exactify<ResolvedCompensationSegment>;
export type CompensationRevisionWire = Exactify<CompensationRevision>;
export type CompensationRevisionOverrideWire = Exactify<CompensationRevisionOverride>;
export type RetroAdjustmentBatchWire = Exactify<RetroAdjustmentBatch>;
export type RetroAllocationWire = Exactify<RetroAllocation>;
export type PayrollRecordWire = Exactify<BordroKaydi>;
export type PayrollPeriodWire = Exactify<BordroDonemi>;

export type {
  CompensationBoundaryScope,
  RoadRoundingMode,
  SocialBenefitAccrualPolicies,
};

export type CompensationPolicyWriteOperationKind =
  | 'INITIAL_BASELINE'
  | 'NEW_EFFECTIVE_DATE'
  | 'SAME_DATE_CORRECTION';

export interface CompensationPolicyReadScope {
  /** This release has one institution-wide timeline and no person-level scope. */
  institutionScope: 'CURRENT_INSTITUTION';
  periodId: string;
  periodStartInclusive: string;
  periodEndInclusive: string;
  projectionDate: string;
}

export type CompensationPolicyNullableDecimalDraft =
  | { kind: 'ABSENT' }
  | { kind: 'VALUE'; raw: string };

/**
 * Form-only representation. Strings may be incomplete while the user types;
 * VALUE "0" and ABSENT remain distinct. This type must be validated and
 * converted before it can become a wire DTO.
 */
export interface CompensationPolicyStateDraft {
  gunlukTabanUcret: string;
  gunlukYemek: string;
  birlestirilmisSosyalYardim: string;
  gunlukVasitaYol: string;
  giyimYardimi: string;
  hizmetZammiBirimi: string;
  isPrimiYuzde: CompensationPolicyNullableDecimalDraft;
  isPrimiGruplari: Array<{
    id: string;
    ad: string;
    oran: string;
    aktif: boolean;
  }>;
  geceCalismaPrimiYuzde: string;
  geceCalismaTatiliPrimiYuzde: string;
  ekOdeme: CompensationPolicyNullableDecimalDraft;
  digerGelirVarsayilan: CompensationPolicyNullableDecimalDraft;
  socialBenefitAccrualPolicies: SocialBenefitAccrualPolicies;
}

export type {
  BenefitAccrualPolicy,
  BenefitDayType,
  BenefitPaymentMethod,
  BenefitProrationDenominator,
};

export type CompensationPolicyBoundaryIntent =
  | {
      boundaryScope: 'FULL_EARNING';
      changeIntent: 'FULL_STATE_CHANGE';
    }
  | {
      boundaryScope: 'ROAD_ONLY';
      changeIntent: 'ROAD_AMOUNT_ONLY';
    };

interface CompensationPolicyOperationFields {
  effectiveFrom: string;
  state: CompensationPolicyStateWire;
  roadRoundingMode: RoadRoundingMode;
  reason: string;
}

/** Initial clipped baselines may legitimately carry ROAD_ONLY. */
export interface InitialCompensationPolicyBaselineOperation
  extends CompensationPolicyOperationFields {
  kind: 'INITIAL_BASELINE';
  boundaryScope: CompensationBoundaryScope;
}

/** Approved choices for the first baseline when no policy version exists. */
export const INITIAL_BASELINE_DECISIONS = {
  boundaryScope: 'FULL_EARNING',
  roadRoundingMode: 'PERIOD_TOTAL',
} as const satisfies Pick<InitialCompensationPolicyBaselineOperation, 'boundaryScope' | 'roadRoundingMode'>;

export type NewCompensationPolicyEffectiveDateOperation =
  CompensationPolicyOperationFields &
    CompensationPolicyBoundaryIntent & {
      kind: 'NEW_EFFECTIVE_DATE';
    };

export type SameDateCompensationPolicyCorrectionOperation =
  CompensationPolicyOperationFields &
    CompensationPolicyBoundaryIntent & {
      kind: 'SAME_DATE_CORRECTION';
      /** Must be the currently active leaf at effectiveFrom. */
      supersedesVersionId: string;
    };

export type CompensationPolicyWriteOperation =
  | InitialCompensationPolicyBaselineOperation
  | NewCompensationPolicyEffectiveDateOperation
  | SameDateCompensationPolicyCorrectionOperation;

const policyDecimalFields = new Set<keyof CompensationPolicyStateWire>([
  'gunlukTabanUcret',
  'gunlukYemek',
  'birlestirilmisSosyalYardim',
  'giyimYardimi',
  'hizmetZammiBirimi',
  'isPrimiYuzde',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
  'ekOdeme',
  'digerGelirVarsayilan',
]);

function decimalValueKey(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const match = /^([+-]?)(\d+)(?:\.(\d+))?$/u.exec(value);
  if (!match) return null;
  const whole = match[2].replace(/^0+(?=\d)/u, '') || '0';
  const fraction = (match[3] ?? '').replace(/0+$/u, '');
  const zero = whole === '0' && fraction.length === 0;
  return `${match[1] === '-' && !zero ? '-' : ''}${whole}${fraction ? `.${fraction}` : ''}`;
}

function sameDecimalValue(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  const leftKey = decimalValueKey(left);
  const rightKey = decimalValueKey(right);
  return leftKey !== null && rightKey !== null && leftKey === rightKey;
}

/** Rust Decimal value equality for ROAD_ONLY's non-road economic state. */
export function compensationPolicyStateEqualsExceptRoad(
  left: CompensationPolicyStateWire,
  right: CompensationPolicyStateWire
): boolean {
  const leftRecord = left as unknown as Record<string, unknown>;
  const rightRecord = right as unknown as Record<string, unknown>;
  const keys = new Set([...Object.keys(leftRecord), ...Object.keys(rightRecord)]);
  keys.delete('gunlukVasitaYol');
  for (const key of keys) {
    const leftValue = leftRecord[key];
    const rightValue = rightRecord[key];
    if (key === 'isPrimiGruplari') {
      if (!Array.isArray(leftValue) || !Array.isArray(rightValue) || leftValue.length !== rightValue.length) {
        return false;
      }
      for (let index = 0; index < leftValue.length; index += 1) {
        const leftGroup = leftValue[index] as Record<string, unknown>;
        const rightGroup = rightValue[index] as Record<string, unknown>;
        if (
          leftGroup.id !== rightGroup.id ||
          leftGroup.ad !== rightGroup.ad ||
          leftGroup.aktif !== rightGroup.aktif ||
          !sameDecimalValue(leftGroup.oran, rightGroup.oran)
        ) return false;
      }
      continue;
    }
    if (key === 'socialBenefitAccrualPolicies') {
      const leftPolicies = (leftValue ?? null) as SocialBenefitAccrualPolicies | null;
      const rightPolicies = (rightValue ?? null) as SocialBenefitAccrualPolicies | null;
      if (leftPolicies === null || rightPolicies === null) {
        if (leftPolicies !== rightPolicies) return false;
        continue;
      }
      for (const benefit of [
        'birlestirilmisSosyalYardim',
        'giyimYardimi',
        'hizmetZammi',
        'ekOdeme',
        'digerGelir',
      ] as const) {
        const leftPolicy = leftPolicies[benefit];
        const rightPolicy = rightPolicies[benefit];
        if (leftPolicy === null || rightPolicy === null) {
          if (leftPolicy !== rightPolicy) return false;
          continue;
        }
        if (
          leftPolicy.method !== rightPolicy.method ||
          leftPolicy.denominator !== rightPolicy.denominator ||
          leftPolicy.allowMonthlyAmountExcess !== rightPolicy.allowMonthlyAmountExcess ||
          leftPolicy.eligibleDayTypes.length !== rightPolicy.eligibleDayTypes.length ||
          leftPolicy.eligibleDayTypes.some((dayType) => !rightPolicy.eligibleDayTypes.includes(dayType))
        ) return false;
      }
      continue;
    }
    if (policyDecimalFields.has(key as keyof CompensationPolicyStateWire)) {
      if (!sameDecimalValue(leftValue, rightValue)) return false;
    } else if (leftValue !== rightValue) {
      return false;
    }
  }
  return true;
}

/** Compare UI drafts with Rust's serialized Decimal/Option representation without Number conversion. */
export function compensationPolicyOperationFingerprint(operation: CompensationPolicyWriteOperation): string {
  const state = { ...operation.state } as Record<string, unknown>;
  for (const field of [...policyDecimalFields, 'gunlukVasitaYol']) {
    if (typeof state[field] === 'string') state[field] = decimalValueKey(state[field]) ?? state[field];
  }
  state.isPrimiGruplari = operation.state.isPrimiGruplari.map((group) => ({
    ...group, oran: decimalValueKey(group.oran) ?? group.oran,
  }));
  state.socialBenefitAccrualPolicies = operation.state.socialBenefitAccrualPolicies == null ? null
    : Object.fromEntries(['birlestirilmisSosyalYardim', 'giyimYardimi', 'hizmetZammi', 'ekOdeme', 'digerGelir'].map((key) => {
      const rule = operation.state.socialBenefitAccrualPolicies![key];
      return [key, rule == null ? null : { ...rule,
        eligibleDayTypes: rule.eligibleDayTypes ?? [],
        allowMonthlyAmountExcess: rule.allowMonthlyAmountExcess ?? null,
      }];
    }));
  const stable = (value: unknown): string => {
    if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
    if (value && typeof value === 'object') return `{${Object.entries(value).filter(([, v]) => v !== undefined)
      .sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`).join(',')}}`;
    return JSON.stringify(value);
  };
  return stable({ ...operation, state });
}

export interface CompensationPolicyLoadRequest {
  scope: CompensationPolicyReadScope;
  /** Partial exact state from current period settings; never an authority. */
  nonAuthoritativePrefill?: {
    source: 'LEGACY_INSTITUTION_SETTINGS';
    sourcePeriodId: string;
    state: Partial<CompensationPolicyStateWire>;
  };
}

export interface CompensationPolicyLoadData {
  scope: CompensationPolicyReadScope;
  timeline: {
    /** Scoped read: baseline at/before period start plus effective dates in range. */
    versions: CompensationPolicyVersionWire[];
    resolvedSegments: ResolvedCompensationSegmentWire[];
    resolution: 'RESOLVED' | 'MISSING_START_POLICY' | 'INVALID_ROAD_ROUNDING';
    /** Active mode changes inside this exact inclusive payroll service period. */
    roadRoundingIssues?: Array<{
      effectiveFrom: string;
      expectedMode: RoadRoundingMode;
      actualMode: RoadRoundingMode;
      expectedVersionId: string;
      actualVersionId: string;
      periodStartInclusive: string;
      periodEndInclusive: string;
    }>;
  };
  activePolicyProjection:
    | {
        kind: 'ACTIVE';
        asOfDate: string;
        version: CompensationPolicyVersionWire;
      }
    | { kind: 'MISSING'; asOfDate: string };
  /** Empty timelines need a user-confirmed baseline; the loader never creates one. */
  initialBaselineRequired: boolean;
  nonAuthoritativePrefill:
    | {
        kind: 'AVAILABLE';
        authority: 'PREFILL_ONLY';
        source: 'LEGACY_INSTITUTION_SETTINGS';
        sourcePeriodId: string;
        state: Partial<CompensationPolicyStateWire>;
      }
    | {
        kind: 'UNAVAILABLE';
        reason: 'NOT_PROVIDED' | 'NO_LEGACY_SETTINGS' | 'INCOMPLETE_SETTINGS';
      };
}

export interface CompensationPolicyTargetStateToken {
  effectiveFrom: string;
  /** Active policy effective at the proposed date, if any. */
  activeVersionIdAtDate: string | null;
  /** Active leaf whose effectiveFrom exactly equals the proposed date, if any. */
  sameDateActiveLeafVersionId: string | null;
}

export interface CompensationPolicyPayrollStateToken {
  payrollId: string;
  personnelId: string;
  periodId: string;
  accrualId: string;
  status: BordroStatus;
  updatedAt: string;
  calculationProvenance: 'LEGACY' | 'COMPENSATION_POLICY_ENGINE';
  snapshotSchemaVersion: number | null;
}

/**
 * Optimistic concurrency basis returned by preview and required by save.
 * IDs/status/provenance are existing persisted identities, not a new revision
 * counter. Save re-reads these values and recomputes impact in its transaction.
 */
export interface CompensationPolicyPreviewToken {
  operation: CompensationPolicyWriteOperation;
  targetState: CompensationPolicyTargetStateToken;
  /** Full institution timeline IDs, in canonical order, including superseded leaves. */
  policyTimelineVersionIds: string[];
  /** All payroll rows in the transitive dependency closure, in canonical order. */
  dependencyPayrollStates: CompensationPolicyPayrollStateToken[];
  /** Exact persisted batch/allocation/revision/source/payment state for each affected retro ledger. */
  retroDependencyStates: CompensationPolicyRetroDependencyStateToken[];
}

export interface CompensationPolicyRetroDependencyStateToken {
  batchId: string;
  sourcePeriodIds: string[];
  linkedPaymentEventIds: string[];
  persistedState: {
    batch: RetroAdjustmentBatchWire;
    allocations: RetroAllocationWire[];
    revision: CompensationRevisionWire;
    overrides: CompensationRevisionOverrideWire[];
    sourcePayrolls: PayrollRecordWire[];
    sourcePeriods: PayrollPeriodWire[];
    paymentEvents: PayrollRecordWire[];
    paymentPeriods: PayrollPeriodWire[];
    sourceSettings: Record<string, Exactify<PayrollPeriodInstitutionSettings>>;
    sourcePersonnel: Exactify<Personel> | null;
    sourceAttendances: Exactify<PersonelPuantaj>[];
    sourceSickLeaveRecords: Exactify<SickLeaveRecord>[];
    legacyHistoryPeriods: PayrollPeriodWire[];
    sourceZamAylari: number[];
  };
}

export type CompensationPolicyField = keyof CompensationPolicyState;

export interface CompensationPolicyDirectEffect {
  effectiveFrom: string;
  /** State values are authoritative through the day before the next active version of any scope. */
  policyStateWindow: {
    throughInclusive: string | null;
    nextEffectiveDate: string | null;
  };
  /** FULL_EARNING opens a slice; ROAD_ONLY does not cut it, and period start is the natural baseline. */
  earningRoundingWindow: {
    kind:
      | 'PERIOD_BASELINE'
      | 'OPENS_FULL_EARNING_SLICE'
      | 'ROAD_ONLY_WITHIN_ACTIVE_SLICE';
    throughInclusive: string | null;
    nextFullEarningDate: string | null;
  };
  boundaryScope: CompensationBoundaryScope;
  roadRoundingMode: RoadRoundingMode;
  changedFields: CompensationPolicyField[];
  affectedPeriods: Array<{
    periodId: string;
    startInclusive: string;
    endInclusive: string;
  }>;
}

export type CompensationPolicyDependencyKind = 'GV' | 'PEK' | 'PAYMENT' | 'CARRY';

export interface CompensationPolicyPayrollReference {
  payrollId: string;
  personnelId: string;
  periodId: string;
  accrualId: string;
}

export interface CompensationPolicyDependencyEffect {
  kind: CompensationPolicyDependencyKind;
  causePeriodId: string;
  affectedPayroll: CompensationPolicyPayrollReference;
  fromStatus: BordroStatus;
  /** Null when this preview is blocked and the row will not be mutated. */
  toStatus: BordroStatus | null;
  action: 'CALCULATED_TO_STALE' | 'ALREADY_STALE' | 'FINALIZED_BLOCKER' | 'NO_MUTATION';
}

export interface CompensationPolicyPreviewData {
  directCompensationEffects: CompensationPolicyDirectEffect[];
  dependencyClosureEffects: CompensationPolicyDependencyEffect[];
  retroEffects: CompensationPolicyRetroEffect[];
  /** Kept separate from dependency effects for a clear blocking decision. */
  finalizedBlockers: CompensationPolicyPayrollReference[];
  previewToken: CompensationPolicyPreviewToken;
}

export type CompensationPolicyRetroBlockerReason =
  | 'FINALIZED_BATCH'
  | 'SETTLED_BATCH'
  | 'FINALIZED_SOURCE'
  | 'FINALIZED_PAYMENT_EVENT';

export interface CompensationPolicyRetroEffect {
  batchId: string;
  sourcePeriodIds: string[];
  linkedPaymentEventIds: string[];
  action: 'STALE_ELIGIBLE' | 'IMMUTABLE_BLOCKER';
  blockers: CompensationPolicyRetroBlockerReason[];
}

export interface CompensationPolicyPreviewRequest {
  scope: CompensationPolicyReadScope;
  operation: CompensationPolicyWriteOperation;
}

export type CompensationPolicyApiError =
  | {
      code: 'CONFLICT';
      message: string;
      actualTargetState?: CompensationPolicyTargetStateToken;
    }
  | {
      code: 'QUARANTINED_POLICY';
      message: string;
      policyVersion: {
        id: string;
        effectiveFrom: string;
        reason:
          | 'MISSING_BOUNDARY_SCOPE'
          | 'MISSING_ROAD_ROUNDING_MODE'
          | 'UNKNOWN_BOUNDARY_METADATA'
          | 'INVALID_POLICY_STATE'
          | 'INVALID_POLICY_GRAPH';
      };
    }
  | {
      code: 'ROAD_ONLY_STATE_CONFLICT';
      message: string;
      proposedEffectiveFrom: string;
      conflictingVersionId: string | null;
      reason: 'NON_ROAD_FIELDS_DIFFER' | 'DESCENDANT_ROAD_ONLY_INVALIDATED';
    }
  | {
      code: 'FINALIZED_BLOCKER';
      message: string;
      blockers: CompensationPolicyPayrollReference[];
    }
  | {
      code: 'RETRO_BLOCKER';
      message: string;
      blockers: CompensationPolicyRetroEffect[];
    }
  | {
      code:
        | 'INVALID_REQUEST'
        | 'INVALID_POLICY'
        | 'UNSUPPORTED_HISTORICAL_REPLAY'
        | 'CURRENT_ENGINE_SNAPSHOT_REQUIRED'
        | 'UNAUTHORIZED'
        | 'NOT_FOUND'
        | 'PERSISTENCE'
        | 'UNAVAILABLE';
      message: string;
    };

export type CompensationPolicyOperationResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: CompensationPolicyApiError };

export interface CompensationPolicyInvalidatedPayroll {
  payroll: CompensationPolicyPayrollReference;
  fromStatus: 'CALCULATED';
  toStatus: 'STALE';
}

export interface CompensationPolicyInvalidatedRetroBatch {
  batchId: string;
  fromStatus: 'DRAFT' | 'CALCULATED';
  toStatus: 'STALE';
}

export interface CompensationPolicyInvalidatedRetroPaymentEvent {
  payroll: CompensationPolicyPayrollReference;
  fromStatus: 'DRAFT' | 'CALCULATED';
  toStatus: 'STALE';
}

export interface CompensationPolicySaveReceipt {
  /** Backend-owned provenance: ID/time/source and supersession link are assigned there. */
  version: CompensationPolicyVersionWire;
  invalidatedPayrolls: CompensationPolicyInvalidatedPayroll[];
  invalidatedRetroBatches: CompensationPolicyInvalidatedRetroBatch[];
  invalidatedRetroPaymentEvents: CompensationPolicyInvalidatedRetroPaymentEvent[];
  authoritativeReloadRequired: true;
}

interface CompensationPolicySaveRequestBase {
  scope: CompensationPolicyReadScope;
  previewToken: CompensationPolicyPreviewToken;
}

export type CompensationPolicySaveRequest =
  | (CompensationPolicySaveRequestBase & {
      operation: InitialCompensationPolicyBaselineOperation;
      initialBaselineConfirmation: {
        confirmed: true;
        effectiveFrom: string;
      };
    })
  | (CompensationPolicySaveRequestBase & {
      operation:
        | NewCompensationPolicyEffectiveDateOperation
        | SameDateCompensationPolicyCorrectionOperation;
      initialBaselineConfirmation?: never;
    });

export interface RoadRoundingRepairPreview {
  previews: CompensationPolicyPreviewData[];
}

export interface CompensationPolicyApi {
  load(
    request: CompensationPolicyLoadRequest
  ): Promise<CompensationPolicyOperationResult<CompensationPolicyLoadData>>;
  preview(
    request: CompensationPolicyPreviewRequest
  ): Promise<CompensationPolicyOperationResult<CompensationPolicyPreviewData>>;
  save(
    request: CompensationPolicySaveRequest
  ): Promise<CompensationPolicyOperationResult<CompensationPolicySaveReceipt>>;
  saveWorkPremiumGroups(
    groups: CompensationPolicyStateWire['isPrimiGruplari']
  ): Promise<CompensationPolicyOperationResult<CompensationPolicySaveReceipt | null>>;
}

export type CompensationPolicyAsyncState<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ready'; value: T }
  | { status: 'error'; error: CompensationPolicyApiError };

export interface CompensationPolicyDraftValidation {
  draft: 'VALID' | 'INVALID';
  fullState:
    | { status: 'INCOMPLETE'; fieldErrors: Partial<Record<keyof CompensationPolicyStateDraft, string>> }
    | { status: 'INVALID'; errors: string[] }
    | { status: 'VALID'; state: CompensationPolicyStateWire };
  operationErrors: string[];
}

export type CompensationPolicySaveViewState =
  | { status: 'idle' }
  | { status: 'saving' }
  | { status: 'reloading-authoritative'; versionId: string }
  | { status: 'conflict'; error: Extract<CompensationPolicyApiError, { code: 'CONFLICT' }> }
  | { status: 'blocked'; error: Extract<CompensationPolicyApiError, { code: 'FINALIZED_BLOCKER' }> }
  | { status: 'error'; error: CompensationPolicyApiError }
  | {
      status: 'persisted-reload-failed';
      versionId: string;
      error: CompensationPolicyApiError;
    }
  | {
      /** Success includes the authoritative reload result; draft-only data cannot satisfy this. */
      status: 'succeeded';
      receipt: CompensationPolicySaveReceipt;
      authoritativeLoad: CompensationPolicyLoadData;
    };

/** Frozen UI contract. App routing and this component implementation are owned by separate tasks. */
export interface CompensationPolicyPageProps {
  scope: CompensationPolicyReadScope;
  loadState: CompensationPolicyAsyncState<CompensationPolicyLoadData>;
  draft: CompensationPolicyStateDraft & {
    operationKind: CompensationPolicyWriteOperationKind;
    effectiveFrom: string;
    boundaryScope: CompensationBoundaryScope;
    roadRoundingMode: RoadRoundingMode;
    reason: string;
    supersedesVersionId: string | null;
  };
  draftValidation: CompensationPolicyDraftValidation;
  previewState: CompensationPolicyAsyncState<CompensationPolicyPreviewData>;
  saveState: CompensationPolicySaveViewState;
  initialBaselineConfirmationRequired: boolean;
  onLoad(): void | Promise<void>;
  onDraftChange(draft: CompensationPolicyPageProps['draft']): void;
  onValidateDraft(draft: CompensationPolicyPageProps['draft']): void;
  onConfirmInitialBaseline(effectiveFrom: string): void;
  onCancelInitialBaseline(): void;
  onPreview(operation: CompensationPolicyWriteOperation): void | Promise<void>;
  onSave(request: CompensationPolicySaveRequest): void | Promise<void>;
  onReloadAuthoritative(): void | Promise<void>;
}
