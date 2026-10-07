import React, { useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type {
  CompensationPolicyLoadData,
  CompensationPolicyPageProps,
  CompensationPolicyPreviewData,
  CompensationPolicySaveReceipt,
  CompensationPolicySaveViewState,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
  CompensationPolicyWriteOperation,
} from '../../services/compensationPolicy/contract';
import { CompensationPolicyPage } from './CompensationPolicyPage';
import { compensationPolicyStateToDraft, createEmptyCompensationPolicyStateDraft, validateCompensationPolicyDraft } from './compensationPolicyDecimal';

const scope: CompensationPolicyPageProps['scope'] = {
  institutionScope: 'CURRENT_INSTITUTION',
  periodId: '2026-09',
  periodStartInclusive: '2026-09-15',
  periodEndInclusive: '2026-10-14',
  projectionDate: '2026-09-30',
};

const initialState: CompensationPolicyStateWire = {
  gunlukTabanUcret: '9007199254740993.12',
  gunlukYemek: '50.00',
  birlestirilmisSosyalYardim: '20.00',
  gunlukVasitaYol: '30.00',
  giyimYardimi: '0',
  hizmetZammiBirimi: '5.00',
  isPrimiYuzde: null,
  isPrimiGruplari: [
    { id: 'A', ad: 'Atesli', oran: '65.50', aktif: true },
    { id: 'B', ad: 'Teknik', oran: '0', aktif: false },
  ],
  geceCalismaPrimiYuzde: '20.00',
  geceCalismaTatiliPrimiYuzde: '30.00',
  ekOdeme: '0',
  digerGelirVarsayilan: null,
};

const makeVersion = (
  id: string,
  state: CompensationPolicyStateWire,
  supersedesVersionId: string | null,
  effectiveFrom = '2026-09-15',
  reason = 'Test ücret politikası',
  boundaryScope: CompensationPolicyVersionWire['boundaryScope'] = 'FULL_EARNING',
  roadRoundingMode: CompensationPolicyVersionWire['roadRoundingMode'] = 'PER_COMPENSATION_SLICE'
): CompensationPolicyVersionWire => ({
  id,
  effectiveFrom,
  state,
  boundaryScope,
  roadRoundingMode,
  createdAt: '2026-09-01T00:00:00.000Z',
  source: 'TEST',
  reason,
  supersedesVersionId,
});

function initialLoad(): CompensationPolicyLoadData {
  return {
    scope,
    timeline: { versions: [], resolvedSegments: [], resolution: 'MISSING_START_POLICY' },
    activePolicyProjection: { kind: 'MISSING', asOfDate: scope.projectionDate },
    initialBaselineRequired: true,
    nonAuthoritativePrefill: {
      kind: 'AVAILABLE',
      authority: 'PREFILL_ONLY',
      source: 'LEGACY_INSTITUTION_SETTINGS',
      sourcePeriodId: '2026-09',
      state: initialState,
    },
  };
}

function existingLoad(): CompensationPolicyLoadData {
  const root = makeVersion('root-1', initialState, null);
  const leaf = makeVersion('leaf-2', initialState, 'root-1', '2026-09-15', 'Aynı tarihli son düzeltme');
  return {
    scope,
    timeline: {
      versions: [root, leaf],
      resolvedSegments: [{
        versionId: leaf.id,
        startDate: scope.periodStartInclusive,
        endDate: scope.periodEndInclusive,
        state: leaf.state,
        boundaryScope: leaf.boundaryScope,
        roadRoundingMode: leaf.roadRoundingMode,
      }],
      resolution: 'RESOLVED',
    },
    activePolicyProjection: { kind: 'ACTIVE', asOfDate: scope.projectionDate, version: leaf },
    initialBaselineRequired: false,
    nonAuthoritativePrefill: { kind: 'UNAVAILABLE', reason: 'NO_LEGACY_SETTINGS' },
  };
}

function draftForBaseline(): CompensationPolicyPageProps['draft'] {
  return {
    ...createEmptyCompensationPolicyStateDraft(),
    operationKind: 'INITIAL_BASELINE',
    effectiveFrom: '2026-09-15',
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    reason: 'Başlangıç ücret politikası',
    supersedesVersionId: null,
  };
}

function draftForExisting(): CompensationPolicyPageProps['draft'] {
  return {
    ...compensationPolicyStateToDraft(initialState),
    operationKind: 'NEW_EFFECTIVE_DATE',
    effectiveFrom: '2026-10-01',
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    reason: 'Yeni dönem ücret değişikliği',
    supersedesVersionId: null,
  };
}

function previewFor(operation: CompensationPolicyWriteOperation, data: CompensationPolicyLoadData, scenario: string): CompensationPolicyPreviewData {
  const finalizedBlockers = scenario === 'finalized'
    ? [{ payrollId: 'payroll-final-4', personnelId: 'person-4', periodId: '2026-09', accrualId: 'accrual-4' }]
    : [];
  const retroEffects = scenario === 'retro'
    ? [{
      batchId: 'retro-batch-1',
      sourcePeriodIds: ['2026-08'],
      linkedPaymentEventIds: ['retro-payment-1'],
      action: 'STALE_ELIGIBLE' as const,
      blockers: [],
    }]
    : [];
  const sameDateLeaf = operation.kind === 'SAME_DATE_CORRECTION' ? operation.supersedesVersionId : null;
  return {
    directCompensationEffects: [{
      effectiveFrom: operation.effectiveFrom,
      policyStateWindow: { throughInclusive: '2026-10-14', nextEffectiveDate: null },
      earningRoundingWindow: {
        kind: operation.boundaryScope === 'ROAD_ONLY' ? 'ROAD_ONLY_WITHIN_ACTIVE_SLICE' : 'OPENS_FULL_EARNING_SLICE',
        throughInclusive: '2026-10-14',
        nextFullEarningDate: null,
      },
      boundaryScope: operation.boundaryScope,
      roadRoundingMode: operation.roadRoundingMode,
      changedFields: ['gunlukTabanUcret', 'gunlukYemek'],
      affectedPeriods: [{ periodId: scope.periodId, startInclusive: scope.periodStartInclusive, endInclusive: scope.periodEndInclusive }],
    }],
    dependencyClosureEffects: [{
      kind: 'GV',
      causePeriodId: scope.periodId,
      affectedPayroll: { payrollId: 'payroll-calculated-2', personnelId: 'person-2', periodId: scope.periodId, accrualId: 'accrual-2' },
      fromStatus: 'CALCULATED',
      toStatus: 'STALE',
      action: 'CALCULATED_TO_STALE',
    }],
    retroEffects,
    finalizedBlockers,
    previewToken: {
      operation,
      targetState: { effectiveFrom: operation.effectiveFrom, activeVersionIdAtDate: sameDateLeaf ?? 'leaf-2', sameDateActiveLeafVersionId: sameDateLeaf },
      policyTimelineVersionIds: data.timeline.versions.map((version) => version.id),
      dependencyPayrollStates: [],
      retroDependencyStates: [],
    },
  };
}

function savedLoad(data: CompensationPolicyLoadData, operation: CompensationPolicyWriteOperation, receipt: CompensationPolicySaveReceipt): CompensationPolicyLoadData {
  const version = receipt.version;
  const versions = [...data.timeline.versions, version];
  return {
    ...data,
    timeline: {
      resolution: 'RESOLVED',
      versions,
      resolvedSegments: [{
        versionId: version.id,
        startDate: version.effectiveFrom,
        endDate: scope.periodEndInclusive,
        state: version.state,
        boundaryScope: version.boundaryScope,
        roadRoundingMode: version.roadRoundingMode,
      }],
    },
    activePolicyProjection: { kind: 'ACTIVE', asOfDate: scope.projectionDate, version },
    initialBaselineRequired: false,
    nonAuthoritativePrefill: { kind: 'UNAVAILABLE', reason: 'NO_LEGACY_SETTINGS' },
  };
}

declare global {
  interface Window {
    __compensationPolicyTest: {
      events: Array<{ type: string; value?: unknown }>;
      setScenario(scenario: string): void;
      useExistingTimeline(): void;
      setLoadIdle(): void;
    };
  }
}

const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Test harness root is missing.');

const Harness: React.FC = () => {
  const [scenario, setScenario] = useState('success');
  const [loadState, setLoadState] = useState<CompensationPolicyPageProps['loadState']>({ status: 'ready', value: initialLoad() });
  const [draft, setDraft] = useState(draftForBaseline);
  const [draftValidation, setDraftValidation] = useState(() => validateCompensationPolicyDraft(draftForBaseline()));
  const [previewState, setPreviewState] = useState<CompensationPolicyPageProps['previewState']>({ status: 'idle' });
  const [saveState, setSaveState] = useState<CompensationPolicySaveViewState>({ status: 'idle' });
  const [initialBaselineConfirmationRequired, setInitialBaselineConfirmationRequired] = useState(true);
  const eventsRef = useRef<Array<{ type: string; value?: unknown }>>([]);
  const lastSavedRef = useRef<{ receipt: CompensationPolicySaveReceipt; data: CompensationPolicyLoadData } | null>(null);
  const record = (type: string, value?: unknown) => eventsRef.current.push({ type, value: value === undefined ? undefined : JSON.parse(JSON.stringify(value)) as unknown });

  window.__compensationPolicyTest = {
    events: eventsRef.current,
    setScenario,
    useExistingTimeline() {
      const data = existingLoad();
      const nextDraft = draftForExisting();
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(false);
      setPreviewState({ status: 'idle' });
      setSaveState({ status: 'idle' });
    },
    setLoadIdle() {
      setLoadState({ status: 'idle' });
    },
  };

  const onLoad = () => {
    record('load');
    setLoadState({ status: 'loading' });
    window.setTimeout(() => {
      if (scenario === 'quarantine') {
        setLoadState({ status: 'error', error: {
          code: 'QUARANTINED_POLICY',
          message: 'Test quarantine.',
          policyVersion: { id: 'quarantined-9', effectiveFrom: '2026-09-15', reason: 'MISSING_ROAD_ROUNDING_MODE' },
        } });
      } else if (scenario === 'loaderror') {
        setLoadState({ status: 'error', error: { code: 'UNAVAILABLE', message: 'Test ortamı yükleme hatası.' } });
      } else {
        setLoadState({ status: 'ready', value: initialLoad() });
      }
    }, 20);
  };
  const onDraftChange = (next: CompensationPolicyPageProps['draft']) => {
    record('draft', next);
    setDraft(next);
    setDraftValidation(validateCompensationPolicyDraft(next));
  };
  const onValidateDraft = (next: CompensationPolicyPageProps['draft']) => {
    record('validate', next);
    setDraftValidation(validateCompensationPolicyDraft(next));
  };
  const onConfirmInitialBaseline = (effectiveFrom: string) => {
    record('confirm-baseline', effectiveFrom);
    setInitialBaselineConfirmationRequired(false);
  };
  const onCancelInitialBaseline = () => {
    record('cancel-baseline');
    setInitialBaselineConfirmationRequired(true);
  };
  const onPreview = (operation: CompensationPolicyWriteOperation) => {
    record('preview', operation);
    setPreviewState({ status: 'loading' });
    const data = loadState.status === 'ready' ? loadState.value : initialLoad();
    window.setTimeout(() => {
      if (scenario === 'retroblocker') {
        setPreviewState({ status: 'error', error: {
          code: 'RETRO_BLOCKER',
          message: 'Retro fixture blocker.',
          blockers: [{
            batchId: 'retro-final-batch',
            sourcePeriodIds: ['2026-08'],
            linkedPaymentEventIds: ['retro-final-payment'],
            action: 'IMMUTABLE_BLOCKER',
            blockers: ['FINALIZED_BATCH', 'SETTLED_BATCH'],
          }],
        } });
      } else {
        setPreviewState({ status: 'ready', value: previewFor(operation, data, scenario) });
      }
    }, 30);
  };
  const onSave = (request: Parameters<CompensationPolicyPageProps['onSave']>[0]) => {
    record('save', request);
    setSaveState({ status: 'saving' });
    const operation = request.operation;
    const version = makeVersion(
      'saved-version-3',
      operation.state,
      operation.kind === 'SAME_DATE_CORRECTION' ? operation.supersedesVersionId : null,
      operation.effectiveFrom,
      operation.reason,
      operation.boundaryScope,
      operation.roadRoundingMode
    );
    const receipt: CompensationPolicySaveReceipt = {
      version,
      invalidatedPayrolls: [],
      invalidatedRetroBatches: scenario === 'retro'
        ? [{ batchId: 'retro-batch-1', fromStatus: 'CALCULATED', toStatus: 'STALE' }]
        : [],
      invalidatedRetroPaymentEvents: scenario === 'retro'
        ? [{ payroll: { payrollId: 'retro-payment-1', personnelId: 'person-1', periodId: '2026-09', accrualId: 'retro-batch-1' }, fromStatus: 'CALCULATED', toStatus: 'STALE' }]
        : [],
      authoritativeReloadRequired: true,
    };
    const currentData = loadState.status === 'ready' ? loadState.value : initialLoad();
    const persistedData = savedLoad(currentData, operation, receipt);
    lastSavedRef.current = { receipt, data: persistedData };

    const finish = () => {
      if (scenario === 'conflict') {
        setSaveState({ status: 'conflict', error: {
          code: 'CONFLICT',
          message: 'Test timeline conflict.',
          actualTargetState: { effectiveFrom: operation.effectiveFrom, activeVersionIdAtDate: 'newer-version', sameDateActiveLeafVersionId: 'newer-leaf' },
        } });
      } else if (scenario === 'persistence') {
        setSaveState({ status: 'error', error: { code: 'PERSISTENCE', message: 'Test write failure.' } });
      } else if (scenario === 'reloadfailure') {
        setSaveState({ status: 'persisted-reload-failed', versionId: version.id, error: { code: 'UNAVAILABLE', message: 'Test authoritative reload failure.' } });
      } else {
        setSaveState({ status: 'reloading-authoritative', versionId: version.id });
        window.setTimeout(() => {
          setLoadState({ status: 'ready', value: persistedData });
          setSaveState({ status: 'succeeded', receipt, authoritativeLoad: persistedData });
        }, scenario === 'reloadwait' ? 1_400 : 35);
      }
    };
    window.setTimeout(finish, scenario === 'slow' ? 250 : 25);
  };
  const onReloadAuthoritative = () => {
    record('reload-authoritative');
    setLoadState({ status: 'loading' });
    window.setTimeout(() => {
      const saved = lastSavedRef.current;
      if (saved) {
        setLoadState({ status: 'ready', value: saved.data });
        setSaveState({ status: 'succeeded', receipt: saved.receipt, authoritativeLoad: saved.data });
      } else {
        setLoadState({ status: 'ready', value: existingLoad() });
        setSaveState({ status: 'idle' });
      }
    }, 35);
  };

  return <CompensationPolicyPage
    scope={scope}
    loadState={loadState}
    draft={draft}
    draftValidation={draftValidation}
    previewState={previewState}
    saveState={saveState}
    initialBaselineConfirmationRequired={initialBaselineConfirmationRequired}
    onLoad={onLoad}
    onDraftChange={onDraftChange}
    onValidateDraft={onValidateDraft}
    onConfirmInitialBaseline={onConfirmInitialBaseline}
    onCancelInitialBaseline={onCancelInitialBaseline}
    onPreview={onPreview}
    onSave={onSave}
    onReloadAuthoritative={onReloadAuthoritative}
  />;
};

createRoot(rootElement).render(<Harness />);
