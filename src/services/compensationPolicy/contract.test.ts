import { describe, expect, test } from 'bun:test';
import { assertExactDecimalDto } from '../payrollEngine/decimalBoundary';
import type {
  CompensationPolicyApi,
  CompensationPolicyApiError,
  CompensationPolicyPageProps,
  CompensationPolicyPreviewData,
  CompensationPolicyPreviewToken,
  CompensationPolicyStateDraft,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
  CompensationPolicyWriteOperation,
  CompensationPolicySaveRequest,
} from './contract';
import { compensationPolicyStateEqualsExceptRoad } from './contract';

const exactState = {
  gunlukTabanUcret: '1000.000000000000000001',
  gunlukYemek: '0',
  birlestirilmisSosyalYardim: '125.5',
  gunlukVasitaYol: '20.005',
  giyimYardimi: '0',
  hizmetZammiBirimi: '0.000000000000000009',
  isPrimiYuzde: null,
  isPrimiGruplari: [
    { id: 'worker', ad: 'Worker', oran: '65.125', aktif: false },
  ],
  geceCalismaPrimiYuzde: '0',
  geceCalismaTatiliPrimiYuzde: '18.75',
  ekOdeme: '0',
  digerGelirVarsayilan: null,
} satisfies CompensationPolicyStateWire;

const baselineOperation = {
  kind: 'INITIAL_BASELINE',
  effectiveFrom: '2026-09-15',
  state: exactState,
  boundaryScope: 'ROAD_ONLY',
  roadRoundingMode: 'PERIOD_TOTAL',
  reason: 'İlk onaylı başlangıç',
} satisfies CompensationPolicyWriteOperation;

const newEffectiveDateOperation = {
  kind: 'NEW_EFFECTIVE_DATE',
  effectiveFrom: '2027-03-01',
  state: exactState,
  boundaryScope: 'ROAD_ONLY',
  changeIntent: 'ROAD_AMOUNT_ONLY',
  roadRoundingMode: 'PERIOD_TOTAL',
  reason: 'Yol tutarı değişikliği',
} satisfies CompensationPolicyWriteOperation;

test('ROAD_ONLY state comparison uses exact decimal value equality and preserves null versus zero', () => {
  const sameEconomicValues = {
    ...exactState,
    gunlukTabanUcret: '01000.000000000000000001',
    gunlukYemek: '0.000',
    birlestirilmisSosyalYardim: '0125.500',
    giyimYardimi: '0.00',
    hizmetZammiBirimi: '0.0000000000000000090',
    gunlukVasitaYol: '200.000',
    geceCalismaPrimiYuzde: '0.00',
    geceCalismaTatiliPrimiYuzde: '18.7500',
    ekOdeme: '0.000',
    isPrimiGruplari: [{ id: 'worker', ad: 'Worker', oran: '65.1250', aktif: false }],
  } satisfies CompensationPolicyStateWire;
  expect(compensationPolicyStateEqualsExceptRoad(exactState, sameEconomicValues)).toBe(true);

  const explicitZero = { ...sameEconomicValues, isPrimiYuzde: '0' } satisfies CompensationPolicyStateWire;
  expect(compensationPolicyStateEqualsExceptRoad(exactState, explicitZero)).toBe(false);

  const changedGroup = {
    ...sameEconomicValues,
    isPrimiGruplari: [{ id: 'worker', ad: 'Different', oran: '65.125', aktif: false }],
  } satisfies CompensationPolicyStateWire;
  expect(compensationPolicyStateEqualsExceptRoad(exactState, changedGroup)).toBe(false);
});

const correctionOperation = {
  kind: 'SAME_DATE_CORRECTION',
  effectiveFrom: '2026-09-15',
  state: exactState,
  boundaryScope: 'ROAD_ONLY',
  changeIntent: 'ROAD_AMOUNT_ONLY',
  roadRoundingMode: 'PERIOD_TOTAL',
  reason: 'Aynı gün düzeltme',
  supersedesVersionId: 'policy-v1',
} satisfies CompensationPolicyWriteOperation;

const policyVersion = {
  id: 'policy-v1',
  effectiveFrom: '2026-09-15',
  state: exactState,
  boundaryScope: 'ROAD_ONLY',
  roadRoundingMode: 'PERIOD_TOTAL',
  createdAt: '2026-10-06T10:00:00.000Z',
  source: 'SETTINGS_POLICY_UI',
  reason: 'İlk onaylı başlangıç',
  supersedesVersionId: null,
} satisfies CompensationPolicyVersionWire;

// The generated Decimal boundary rejects numbers at compile time.
const invalidNumericWireState = {
  ...exactState,
  // @ts-expect-error monetary values in wire DTOs must be exact decimal strings
  gunlukYemek: 0,
} satisfies CompensationPolicyStateWire;
void invalidNumericWireState;

// ROAD_ONLY needs its explicit intention for changes, while a clipped baseline
// has no preceding state to compare and may itself be ROAD_ONLY.
// @ts-expect-error ROAD_ONLY changes require the matching explicit intention
const invalidRoadOnlyOperation: CompensationPolicyWriteOperation = {
  ...correctionOperation,
  changeIntent: 'FULL_STATE_CHANGE',
};
void invalidRoadOnlyOperation;

const draft: CompensationPolicyStateDraft = {
  gunlukTabanUcret: '1000.000000000000000001',
  gunlukYemek: '0',
  birlestirilmisSosyalYardim: '125.5',
  gunlukVasitaYol: '20.005',
  giyimYardimi: '0',
  hizmetZammiBirimi: '0',
  isPrimiYuzde: { kind: 'ABSENT' },
  isPrimiGruplari: [{ id: 'worker', ad: 'Worker', oran: '65.125', aktif: false }],
  geceCalismaPrimiYuzde: '0',
  geceCalismaTatiliPrimiYuzde: '18.75',
  ekOdeme: { kind: 'VALUE', raw: '0' },
  digerGelirVarsayilan: { kind: 'ABSENT' },
};

const previewData: CompensationPolicyPreviewData = {
  directCompensationEffects: [
    {
      effectiveFrom: correctionOperation.effectiveFrom,
      policyStateWindow: {
        throughInclusive: '2026-09-30',
        nextEffectiveDate: '2026-10-01',
      },
      earningRoundingWindow: {
        kind: 'ROAD_ONLY_WITHIN_ACTIVE_SLICE',
        throughInclusive: '2027-02-28',
        nextFullEarningDate: '2027-03-01',
      },
      boundaryScope: correctionOperation.boundaryScope,
      roadRoundingMode: correctionOperation.roadRoundingMode,
      changedFields: ['gunlukVasitaYol'],
      affectedPeriods: [],
    },
  ],
  dependencyClosureEffects: [],
  retroEffects: [],
  finalizedBlockers: [],
  previewToken: {
    operation: correctionOperation,
    targetState: {
      effectiveFrom: correctionOperation.effectiveFrom,
      activeVersionIdAtDate: 'policy-v1',
      sameDateActiveLeafVersionId: 'policy-v1',
    },
    policyTimelineVersionIds: ['policy-v1'],
    dependencyPayrollStates: [],
    retroDependencyStates: [],
  },
};

const baselinePreviewToken: CompensationPolicyPreviewToken = {
  ...previewData.previewToken,
  operation: baselineOperation,
  targetState: {
    effectiveFrom: baselineOperation.effectiveFrom,
    activeVersionIdAtDate: null,
    sameDateActiveLeafVersionId: null,
  },
  policyTimelineVersionIds: [],
};

const pageProps: CompensationPolicyPageProps = {
  scope: {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: '2026-09',
    periodStartInclusive: '2026-09-15',
    periodEndInclusive: '2026-10-14',
    projectionDate: '2026-09-15',
  },
  loadState: { status: 'ready', value: {
    scope: {
      institutionScope: 'CURRENT_INSTITUTION',
      periodId: '2026-09',
      periodStartInclusive: '2026-09-15',
      periodEndInclusive: '2026-10-14',
      projectionDate: '2026-09-15',
    },
    timeline: { versions: [], resolvedSegments: [], resolution: 'MISSING_START_POLICY' },
    activePolicyProjection: { kind: 'MISSING', asOfDate: '2026-09-15' },
    initialBaselineRequired: true,
    nonAuthoritativePrefill: {
      kind: 'AVAILABLE',
      authority: 'PREFILL_ONLY',
      source: 'LEGACY_INSTITUTION_SETTINGS',
      sourcePeriodId: '2026-09',
      state: { gunlukYemek: '0', ekOdeme: null },
    },
  } },
  draft: {
    ...draft,
    operationKind: 'INITIAL_BASELINE',
    effectiveFrom: '2026-09-15',
    boundaryScope: 'ROAD_ONLY',
    roadRoundingMode: 'PERIOD_TOTAL',
    reason: 'İlk onaylı başlangıç',
    supersedesVersionId: null,
  },
  draftValidation: {
    draft: 'VALID',
    fullState: { status: 'VALID', state: exactState },
    operationErrors: [],
  },
  previewState: { status: 'ready', value: previewData },
  saveState: { status: 'idle' },
  initialBaselineConfirmationRequired: true,
  onLoad: () => undefined,
  onDraftChange: () => undefined,
  onValidateDraft: () => undefined,
  onConfirmInitialBaseline: () => undefined,
  onCancelInitialBaseline: () => undefined,
  onPreview: () => undefined,
  onSave: () => undefined,
  onReloadAuthoritative: () => undefined,
};

const quarantinedResult: CompensationPolicyApiError = {
  code: 'QUARANTINED_POLICY',
  message: 'Boundary metadata eksik.',
  policyVersion: {
    id: 'legacy-v0',
    effectiveFrom: '2026-08-15',
    reason: 'MISSING_BOUNDARY_SCOPE',
  },
};

const roadOnlyStateConflict: CompensationPolicyApiError = {
  code: 'ROAD_ONLY_STATE_CONFLICT',
  message: 'Önerilen düzeltme sonraki ROAD_ONLY sürümünü geçersiz kılıyor.',
  proposedEffectiveFrom: '2026-09-15',
  conflictingVersionId: 'policy-v2',
  reason: 'DESCENDANT_ROAD_ONLY_INVALIDATED',
};

const correctionSave: CompensationPolicySaveRequest = {
  scope: pageProps.scope,
  operation: correctionOperation,
  previewToken: previewData.previewToken,
};

// @ts-expect-error explicit user confirmation is required only for a baseline
const invalidCorrectionSave: CompensationPolicySaveRequest = {
  ...correctionSave,
  initialBaselineConfirmation: { confirmed: true, effectiveFrom: '2026-09-15' },
};
void invalidCorrectionSave;

const apiContract: CompensationPolicyApi = {
  async load() {
    return { ok: false, error: quarantinedResult };
  },
  async preview() {
    return { ok: true, value: previewData };
  },
  async save() {
    return {
      ok: true,
      value: {
        version: policyVersion,
        invalidatedPayrolls: [],
        invalidatedRetroBatches: [],
        invalidatedRetroPaymentEvents: [],
        authoritativeReloadRequired: true,
      },
    };
  },
};

describe('compensation policy frozen contract', () => {
  test('full-state wire DTO preserves exact decimal text, null, zero, flags and group state', () => {
    assertExactDecimalDto(policyVersion);
    const roundTripped = JSON.parse(JSON.stringify(policyVersion)) as CompensationPolicyVersionWire;
    expect(roundTripped.state.gunlukTabanUcret).toBe('1000.000000000000000001');
    expect(roundTripped.state.ekOdeme).toBe('0');
    expect(roundTripped.state.digerGelirVarsayilan).toBe(null);
    expect(roundTripped.state.isPrimiYuzde).toBe(null);
    expect(roundTripped.state.isPrimiGruplari[0].aktif).toBe(false);
    expect(roundTripped.boundaryScope).toBe('ROAD_ONLY');
    expect(roundTripped.roadRoundingMode).toBe('PERIOD_TOTAL');
  });

  test('form draft keeps absent and explicit zero distinct', () => {
    expect(draft.ekOdeme).toEqual({ kind: 'VALUE', raw: '0' });
    expect(draft.digerGelirVarsayilan).toEqual({ kind: 'ABSENT' });
  });

  test('same-date correction explicitly targets its active leaf', () => {
    const token: CompensationPolicyPreviewToken = previewData.previewToken;
    expect(correctionSave.operation.kind).toBe('SAME_DATE_CORRECTION');
    if (correctionSave.operation.kind === 'SAME_DATE_CORRECTION') {
      expect(correctionSave.operation.supersedesVersionId).toBe('policy-v1');
    }
    expect(token.targetState.sameDateActiveLeafVersionId).toBe('policy-v1');
  });

  test('new effective dates require explicit boundary intention', () => {
    expect(newEffectiveDateOperation.kind).toBe('NEW_EFFECTIVE_DATE');
    if (newEffectiveDateOperation.kind === 'NEW_EFFECTIVE_DATE') {
      expect(newEffectiveDateOperation.changeIntent).toBe('ROAD_AMOUNT_ONLY');
      expect('supersedesVersionId' in newEffectiveDateOperation).toBe(false);
    }
  });

  test('ROAD_ONLY state authority and earning-rounding slice have separate end dates', () => {
    const roadOnlyEffect = previewData.directCompensationEffects[0];
    expect(roadOnlyEffect.policyStateWindow.nextEffectiveDate).toBe('2026-10-01');
    expect(roadOnlyEffect.earningRoundingWindow.kind).toBe('ROAD_ONLY_WITHIN_ACTIVE_SLICE');
    expect(roadOnlyEffect.earningRoundingWindow.nextFullEarningDate).toBe('2027-03-01');
    const clippedRoadOnlyBaseline = {
      ...previewData.directCompensationEffects[0],
      earningRoundingWindow: {
        kind: 'PERIOD_BASELINE',
        throughInclusive: '2026-10-14',
        nextFullEarningDate: null,
      },
    } satisfies CompensationPolicyPreviewData['directCompensationEffects'][number];
    expect(clippedRoadOnlyBaseline.boundaryScope).toBe('ROAD_ONLY');
    expect(clippedRoadOnlyBaseline.earningRoundingWindow.kind).toBe('PERIOD_BASELINE');
  });

  test('a correction that breaks a ROAD_ONLY descendant has a typed fail-closed result', () => {
    expect(roadOnlyStateConflict.code).toBe('ROAD_ONLY_STATE_CONFLICT');
    expect(roadOnlyStateConflict.reason).toBe('DESCENDANT_ROAD_ONLY_INVALIDATED');
    expect(roadOnlyStateConflict.conflictingVersionId).toBe('policy-v2');
  });

  test('operation results distinguish success from quarantined history and props expose lifecycle callbacks', async () => {
    const [loaded, previewed, saved] = await Promise.all([
      apiContract.load({ scope: pageProps.scope }),
      apiContract.preview({ scope: pageProps.scope, operation: baselineOperation }),
      apiContract.save({
        scope: pageProps.scope,
        operation: baselineOperation,
        previewToken: baselinePreviewToken,
        initialBaselineConfirmation: { confirmed: true, effectiveFrom: '2026-09-15' },
      }),
    ]);
    expect(loaded.ok).toBe(false);
    if (loaded.ok === false) expect(loaded.error.code).toBe('QUARANTINED_POLICY');
    expect(previewed.ok).toBe(true);
    expect(saved.ok).toBe(true);
    expect(pageProps.initialBaselineConfirmationRequired).toBe(true);
    expect(typeof pageProps.onCancelInitialBaseline).toBe('function');
    expect(typeof pageProps.onReloadAuthoritative).toBe('function');
  });
});
