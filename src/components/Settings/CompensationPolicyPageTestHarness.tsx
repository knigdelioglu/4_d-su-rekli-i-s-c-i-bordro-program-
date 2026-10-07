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
import { INITIAL_BASELINE_DECISIONS } from '../../services/compensationPolicy/contract';
import { AppViewportFrame } from '../AppViewportFrame';
import { CompensationPolicyPage } from './CompensationPolicyPage';
import { TopBar } from '../TopBar';
import { Sidebar } from '../Sidebar';
import { compensationPolicyStateToDraft, createEmptyCompensationPolicyStateDraft, validateCompensationPolicyDraft } from './compensationPolicyDecimal';
import { createBordroDonemi } from '../../utils/payrollPresentation';

const scope: CompensationPolicyPageProps['scope'] = {
  institutionScope: 'CURRENT_INSTITUTION',
  periodId: '2026-09',
  periodStartInclusive: '2026-09-15',
  periodEndInclusive: '2026-10-14',
  projectionDate: '2026-09-30',
};

const precisionState: CompensationPolicyStateWire = {
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

function initialLoad(state: CompensationPolicyStateWire = eylulState): CompensationPolicyLoadData {
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
      state,
    },
  };
}

const eylulState: CompensationPolicyStateWire = {
  gunlukTabanUcret: '2616.26',
  gunlukYemek: '300.75',
  birlestirilmisSosyalYardim: '5089.7',
  gunlukVasitaYol: '128.93',
  giyimYardimi: '269.7',
  hizmetZammiBirimi: '24.67',
  isPrimiYuzde: '0',
  isPrimiGruplari: [
    { id: '1. Grup', ad: '1. Grup', oran: '9', aktif: true },
    { id: '2. Grup', ad: '2. Grup', oran: '8', aktif: true },
    { id: '3. Grup', ad: '3. Grup', oran: '7', aktif: true },
    { id: 'grup_1791176163314', ad: 'UX Deneme Grubu', oran: '6', aktif: true },
  ],
  geceCalismaPrimiYuzde: '0',
  geceCalismaTatiliPrimiYuzde: '0',
  ekOdeme: '0',
  digerGelirVarsayilan: '0',
};

function existingLoad(): CompensationPolicyLoadData {
  const root = makeVersion('root-1', precisionState, null);
  const leaf = makeVersion('leaf-2', precisionState, 'root-1', '2026-09-15', 'Aynı tarihli son düzeltme');
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

function draftForBaseline(data: CompensationPolicyLoadData = initialLoad()): CompensationPolicyPageProps['draft'] {
  const state = data.activePolicyProjection.kind === 'ACTIVE'
    ? data.activePolicyProjection.version.state
    : data.nonAuthoritativePrefill.kind === 'AVAILABLE'
      ? data.nonAuthoritativePrefill.state
      : null;
  return {
    ...createEmptyCompensationPolicyStateDraft(),
    ...(state ? compensationPolicyStateToDraft(state) : {}),
    operationKind: 'INITIAL_BASELINE',
    effectiveFrom: '2026-09-15',
    ...INITIAL_BASELINE_DECISIONS,
    reason: '',
    supersedesVersionId: null,
  };
}

function roadOnlyPolicyLoad(): CompensationPolicyLoadData {
  const version = makeVersion('saved-road-only', precisionState, null, '2026-09-15', 'Kayıtlı yol politikası', 'ROAD_ONLY', 'PER_COMPENSATION_SLICE');
  return {
    scope,
    timeline: {
      versions: [version],
      resolvedSegments: [{
        versionId: version.id,
        startDate: scope.periodStartInclusive,
        endDate: scope.periodEndInclusive,
        state: version.state,
        boundaryScope: version.boundaryScope,
        roadRoundingMode: version.roadRoundingMode,
      }],
      resolution: 'RESOLVED',
    },
    activePolicyProjection: { kind: 'ACTIVE', asOfDate: scope.projectionDate, version },
    initialBaselineRequired: false,
    nonAuthoritativePrefill: { kind: 'UNAVAILABLE', reason: 'NO_LEGACY_SETTINGS' },
  };
}

function correctionPreferenceLoad(): CompensationPolicyLoadData {
  const root = makeVersion('preference-root', precisionState, null, '2026-09-15', 'Başlangıç kaydı');
  const targetState = { ...eylulState, gunlukTabanUcret: '3141.59', gunlukVasitaYol: '175.25' };
  const target = makeVersion('preference-target', targetState, null, '2026-10-01', 'Yol politikası düzeltmesi', 'ROAD_ONLY', 'PERIOD_TOTAL');
  return {
    scope,
    timeline: {
      versions: [root, target],
      resolvedSegments: [
        {
          versionId: root.id,
          startDate: '2026-09-15',
          endDate: '2026-09-30',
          state: root.state,
          boundaryScope: root.boundaryScope,
          roadRoundingMode: root.roadRoundingMode,
        },
        {
          versionId: target.id,
          startDate: '2026-10-01',
          endDate: scope.periodEndInclusive,
          state: target.state,
          boundaryScope: target.boundaryScope,
          roadRoundingMode: target.roadRoundingMode,
        },
      ],
      resolution: 'RESOLVED',
    },
    activePolicyProjection: { kind: 'ACTIVE', asOfDate: '2026-09-30', version: root },
    initialBaselineRequired: false,
    nonAuthoritativePrefill: { kind: 'UNAVAILABLE', reason: 'NO_LEGACY_SETTINGS' },
  };
}

function draftForExisting(data: CompensationPolicyLoadData = existingLoad()): CompensationPolicyPageProps['draft'] {
  const activeVersion = data.activePolicyProjection.kind === 'ACTIVE' ? data.activePolicyProjection.version : null;
  return {
    ...compensationPolicyStateToDraft(activeVersion?.state ?? precisionState),
    operationKind: 'NEW_EFFECTIVE_DATE',
    effectiveFrom: '2026-10-01',
    boundaryScope: activeVersion?.boundaryScope ?? 'FULL_EARNING',
    roadRoundingMode: activeVersion?.roadRoundingMode ?? 'PER_COMPENSATION_SLICE',
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
      usePersistedRoadOnlyPolicy(): void;
      useCorrectionPreferenceTimeline(): void;
      useEylulPrefill(): void;
      usePrecisionPrefill(): void;
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
      const nextDraft = draftForExisting(data);
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(false);
      setPreviewState({ status: 'idle' });
      setSaveState({ status: 'idle' });
    },
    usePersistedRoadOnlyPolicy() {
      const data = roadOnlyPolicyLoad();
      const nextDraft = draftForExisting(data);
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(false);
      setPreviewState({ status: 'idle' });
      setSaveState({ status: 'idle' });
    },
    useCorrectionPreferenceTimeline() {
      const data = correctionPreferenceLoad();
      const nextDraft = draftForExisting(data);
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(false);
      setPreviewState({ status: 'idle' });
      setSaveState({ status: 'idle' });
    },
    useEylulPrefill() {
      const data: CompensationPolicyLoadData = {
        ...initialLoad(),
        nonAuthoritativePrefill: {
          kind: 'AVAILABLE',
          authority: 'PREFILL_ONLY',
          source: 'LEGACY_INSTITUTION_SETTINGS',
          sourcePeriodId: '2026-09',
          state: eylulState,
        },
      };
      const nextDraft = draftForBaseline(data);
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(true);
      setPreviewState({ status: 'idle' });
      setSaveState({ status: 'idle' });
    },
    usePrecisionPrefill() {
      const data = initialLoad(precisionState);
      const nextDraft = draftForBaseline(data);
      setLoadState({ status: 'ready', value: data });
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
      setInitialBaselineConfirmationRequired(true);
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
        const data = initialLoad();
        const nextDraft = draftForBaseline(data);
        setLoadState({ status: 'ready', value: data });
        setDraft(nextDraft);
        setDraftValidation(validateCompensationPolicyDraft(nextDraft));
        setInitialBaselineConfirmationRequired(true);
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

const testPeriod = createBordroDonemi(2026, 9, 2026, 10);
const AppTestFrame: React.FC<React.PropsWithChildren> = ({ children }) => (
  <AppViewportFrame
    viewportBounded
    header={(
      <TopBar
        donemler={[testPeriod]}
        aktifDonemId={testPeriod.id}
        onSelectDonem={() => undefined}
        onExportBackup={() => undefined}
        onImportFileSelected={() => undefined}
        onResetSampleData={() => undefined}
        onOpenNoticeSummary={() => undefined}
        isSidebarOpen={false}
        onToggleSidebar={() => undefined}
      />
    )}
    navigation={(
      <Sidebar
        activeTab="parametrelar"
        activeKesintiType="sendika"
        activeParametreSection="gelir"
        activePayrollView="normal"
        onTabChange={() => undefined}
        onKesintiTypeChange={() => undefined}
        onParametreSectionChange={() => undefined}
        onPayrollViewChange={() => undefined}
        isOpen={false}
        onClose={() => undefined}
      />
    )}
  >
    {children}
  </AppViewportFrame>
);

createRoot(rootElement).render(<AppTestFrame><Harness /></AppTestFrame>);
