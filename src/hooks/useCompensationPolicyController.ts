import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { BordroDonemi } from '../types/payroll';
import { INITIAL_BASELINE_DECISIONS } from '../services/compensationPolicy/contract';
import type {
  CompensationPolicyApiError,
  CompensationPolicyAsyncState,
  CompensationPolicyDraftValidation,
  CompensationPolicyLoadData,
  CompensationPolicyPageProps,
  CompensationPolicyStateWire,
  CompensationPolicyOperationResult,
  CompensationPolicyPreviewData,
  CompensationPolicySaveReceipt,
  CompensationPolicySaveViewState,
  CompensationPolicyWriteOperation,
} from '../services/compensationPolicy/contract';
import { getCompensationPolicyApi } from '../services/compensationPolicy/api';
import {
  compensationPolicyStateToDraft,
  createEmptyCompensationPolicyStateDraft,
  validateCompensationPolicyDraft,
} from '../components/Settings/compensationPolicyDecimal';

type Draft = CompensationPolicyPageProps['draft'];

function scopeForPeriod(period: BordroDonemi | null | undefined): CompensationPolicyPageProps['scope'] | null {
  if (!period) return null;
  return {
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: period.id,
    periodStartInclusive: period.baslangicTarihi,
    periodEndInclusive: period.bitisTarihi,
    projectionDate: period.baslangicTarihi,
  };
}

function draftForScope(
  scope: CompensationPolicyPageProps['scope'],
  initialBaselineRequired: boolean
): Draft {
  return {
    ...createEmptyCompensationPolicyStateDraft(),
    operationKind: initialBaselineRequired ? 'INITIAL_BASELINE' : 'NEW_EFFECTIVE_DATE',
    effectiveFrom: scope.periodStartInclusive,
    boundaryScope: initialBaselineRequired ? INITIAL_BASELINE_DECISIONS.boundaryScope : 'FULL_EARNING',
    roadRoundingMode: initialBaselineRequired ? INITIAL_BASELINE_DECISIONS.roadRoundingMode : 'PER_COMPENSATION_SLICE',
    reason: '',
    supersedesVersionId: null,
  };
}

const requiredPrefillFields = [
  'gunlukTabanUcret',
  'gunlukYemek',
  'birlestirilmisSosyalYardim',
  'gunlukVasitaYol',
  'giyimYardimi',
  'hizmetZammiBirimi',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
] as const;
const nullablePrefillFields = ['isPrimiYuzde', 'ekOdeme', 'digerGelirVarsayilan'] as const;

function isCompletePrefill(
  state: Partial<CompensationPolicyStateWire>
): boolean {
  const source = state as Record<string, unknown>;
  if (!requiredPrefillFields.every((field) => typeof source[field] === 'string')) return false;
  if (!nullablePrefillFields.every((field) =>
    Object.prototype.hasOwnProperty.call(source, field) &&
    (source[field] === null || typeof source[field] === 'string')
  )) return false;
  const groups = source.isPrimiGruplari;
  return Array.isArray(groups) && groups.length > 0 && groups.every((group) =>
    Boolean(group) &&
    typeof group === 'object' &&
    typeof (group as Record<string, unknown>).id === 'string' &&
    typeof (group as Record<string, unknown>).ad === 'string' &&
    typeof (group as Record<string, unknown>).oran === 'string' &&
    typeof (group as Record<string, unknown>).aktif === 'boolean'
  );
}

/** Builds the first editable draft from the best verified source without writing policy authority. */
export function draftForCompensationPolicyLoad(
  scope: CompensationPolicyPageProps['scope'],
  loaded: CompensationPolicyLoadData
): Draft {
  const draft = draftForScope(scope, loaded.initialBaselineRequired);
  if (loaded.initialBaselineRequired && loaded.workPremiumGroups) {
    try {
      draft.isPrimiGruplari = compensationPolicyStateToDraft({ isPrimiGruplari: loaded.workPremiumGroups }).isPrimiGruplari;
    } catch {
      // Invalid groups remain empty and must be corrected in settings.
    }
  }
  const state = loaded.activePolicyProjection.kind === 'ACTIVE'
    ? loaded.activePolicyProjection.version.state
    : loaded.initialBaselineRequired && loaded.nonAuthoritativePrefill.kind === 'AVAILABLE'
      ? loaded.nonAuthoritativePrefill.state
      : null;
  if (!state) return draft;
  if (
    loaded.activePolicyProjection.kind !== 'ACTIVE' &&
    !isCompletePrefill(state)
  ) return draft;

  try {
    const activeVersion = loaded.activePolicyProjection.kind === 'ACTIVE'
      ? loaded.activePolicyProjection.version
      : null;
    return {
      ...draft,
      ...compensationPolicyStateToDraft(state),
      ...(loaded.initialBaselineRequired && loaded.workPremiumGroups && draft.isPrimiGruplari.length > 0
        ? { isPrimiGruplari: draft.isPrimiGruplari } : {}),
      ...(activeVersion ? {
        boundaryScope: activeVersion.boundaryScope,
        roadRoundingMode: activeVersion.roadRoundingMode,
      } : {}),
    };
  } catch {
    // An unreadable prefill remains a blank draft; never invent a financial value.
    return draft;
  }
}

function unavailable(message: string): CompensationPolicyApiError {
  return { code: 'UNAVAILABLE', message };
}

export function useCompensationPolicyController(
  period: BordroDonemi | null | undefined,
  reloadPayrollData: () => Promise<boolean>
): CompensationPolicyPageProps | null {
  const scope = useMemo(() => scopeForPeriod(period), [
    period?.id,
    period?.baslangicTarihi,
    period?.bitisTarihi,
  ]);
  const scopeKey = scope
    ? `${scope.periodId}\u0000${scope.periodStartInclusive}\u0000${scope.periodEndInclusive}`
    : '';
  const [loadState, setLoadState] = useState<CompensationPolicyAsyncState<CompensationPolicyLoadData>>(
    scope ? { status: 'loading' } : { status: 'idle' }
  );
  const [draft, setDraft] = useState<Draft>(() => scope ? draftForScope(scope, true) : draftForScope({
    institutionScope: 'CURRENT_INSTITUTION',
    periodId: '',
    periodStartInclusive: '',
    periodEndInclusive: '',
    projectionDate: '',
  }, true));
  const [draftValidation, setDraftValidation] = useState<CompensationPolicyDraftValidation>(
    () => validateCompensationPolicyDraft(draft)
  );
  const [previewState, setPreviewState] = useState<CompensationPolicyAsyncState<CompensationPolicyPreviewData>>({ status: 'idle' });
  const [saveState, setSaveState] = useState<CompensationPolicySaveViewState>({ status: 'idle' });
  const baselineConfirmationDate = useRef<string | null>(null);
  const requestGeneration = useRef(0);
  const loadedScopeKey = useRef(scopeKey);

  const load = useCallback(async (): Promise<CompensationPolicyLoadData | null> => {
    if (!scope) {
      setLoadState({ status: 'idle' });
      return null;
    }
    const generation = ++requestGeneration.current;
    setLoadState({ status: 'loading' });
    setPreviewState({ status: 'idle' });
    let result: CompensationPolicyOperationResult<CompensationPolicyLoadData>;
    try {
      result = await getCompensationPolicyApi().load({ scope });
    } catch (error) {
      if (generation !== requestGeneration.current) return null;
      setLoadState({ status: 'error', error: unavailable(error instanceof Error ? error.message : String(error)) });
      return null;
    }
    if (generation !== requestGeneration.current) return null;
    if (result.ok === false) {
      setLoadState({ status: 'error', error: result.error });
      return null;
    }
    setLoadState({ status: 'ready', value: result.value });
    return result.value;
  }, [scope]);

  useEffect(() => {
    baselineConfirmationDate.current = null;
    loadedScopeKey.current = scopeKey;
    setPreviewState({ status: 'idle' });
    setSaveState({ status: 'idle' });
    if (!scope) {
      setLoadState({ status: 'idle' });
      return;
    }
    setDraft(draftForScope(scope, true));
    setLoadState({ status: 'loading' });
    void load().then((loaded) => {
      if (!loaded || loadedScopeKey.current !== scopeKey) return;
      const nextDraft = draftForCompensationPolicyLoad(scope, loaded);
      setDraft(nextDraft);
      setDraftValidation(validateCompensationPolicyDraft(nextDraft));
    });
    return () => {
      requestGeneration.current += 1;
    };
  }, [load, scope, scopeKey]);

  const onDraftChange = useCallback((next: Draft) => {
    setDraft(next);
    setDraftValidation(validateCompensationPolicyDraft(next));
    setPreviewState({ status: 'idle' });
  }, []);

  const onValidateDraft = useCallback((next: Draft) => {
    setDraftValidation(validateCompensationPolicyDraft(next));
  }, []);

  const onPreview = useCallback(async (operation: CompensationPolicyWriteOperation) => {
    if (!scope) return;
    setPreviewState({ status: 'loading' });
    try {
      const result = await getCompensationPolicyApi().preview({ scope, operation });
      setPreviewState(result.ok === true ? { status: 'ready', value: result.value } : { status: 'error', error: result.error });
    } catch (error) {
      setPreviewState({ status: 'error', error: unavailable(error instanceof Error ? error.message : String(error)) });
    }
  }, [scope]);

  const onSave = useCallback(async (request: Parameters<CompensationPolicyPageProps['onSave']>[0]) => {
    if (!scope) return;
    if (
      request.operation.kind === 'INITIAL_BASELINE' &&
      baselineConfirmationDate.current !== request.operation.effectiveFrom
    ) {
      setSaveState({
        status: 'error',
        error: { code: 'INVALID_REQUEST', message: 'Başlangıç ücreti ekrandan ayrıca onaylanmalıdır.' },
      });
      return;
    }
    setSaveState({ status: 'saving' });
    let result: CompensationPolicyOperationResult<CompensationPolicySaveReceipt>;
    try {
      result = await getCompensationPolicyApi().save(request);
    } catch (error) {
      setSaveState({ status: 'error', error: unavailable(error instanceof Error ? error.message : String(error)) });
      return;
    }
    if (result.ok === false) {
      if (result.error.code === 'CONFLICT') {
        setSaveState({ status: 'conflict', error: result.error });
      } else if (result.error.code === 'FINALIZED_BLOCKER' || result.error.code === 'RETRO_BLOCKER') {
        setSaveState({ status: 'blocked', error: result.error });
      } else {
        setSaveState({ status: 'error', error: result.error });
      }
      return;
    }

    const receipt: CompensationPolicySaveReceipt = result.value;
    setSaveState({ status: 'reloading-authoritative', versionId: receipt.version.id });
    try {
      if (!(await reloadPayrollData())) {
        throw new Error('Uygulamanın yetkili bordro snapshotı kaydetme sonrasında yeniden yüklenemedi.');
      }
      const authoritative = await getCompensationPolicyApi().load({ scope });
      if (authoritative.ok === false) {
        setSaveState({ status: 'persisted-reload-failed', versionId: receipt.version.id, error: authoritative.error });
        return;
      }
      setLoadState({ status: 'ready', value: authoritative.value });
      setPreviewState({ status: 'idle' });
      baselineConfirmationDate.current = null;
      setSaveState({ status: 'succeeded', receipt, authoritativeLoad: authoritative.value });
    } catch (error) {
      setSaveState({
        status: 'persisted-reload-failed',
        versionId: receipt.version.id,
        error: unavailable(error instanceof Error ? error.message : String(error)),
      });
    }
  }, [reloadPayrollData, scope]);

  const onReloadAuthoritative = useCallback(async () => {
    if (!(await reloadPayrollData())) {
      const error = unavailable('Uygulamanın yetkili bordro snapshotı yeniden yüklenemedi.');
      setLoadState({ status: 'error', error });
      return;
    }
    await load();
  }, [load, reloadPayrollData]);

  const onConfirmInitialBaseline = useCallback((effectiveFrom: string) => {
    baselineConfirmationDate.current = effectiveFrom;
  }, []);
  const onCancelInitialBaseline = useCallback(() => {
    baselineConfirmationDate.current = null;
  }, []);

  if (!scope) return null;
  return {
    scope,
    loadState,
    draft,
    draftValidation,
    previewState,
    saveState,
    initialBaselineConfirmationRequired: loadState.status === 'ready' && loadState.value.initialBaselineRequired,
    onLoad: load,
    onDraftChange,
    onValidateDraft,
    onConfirmInitialBaseline,
    onCancelInitialBaseline,
    onPreview,
    onSave,
    onReloadAuthoritative,
  };
}
