import { describe, expect, test } from 'bun:test';
import type {
  CompensationPolicyLoadData,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
} from '../services/compensationPolicy/contract';
import { draftForCompensationPolicyLoad } from './useCompensationPolicyController';

const scope: CompensationPolicyLoadData['scope'] = {
  institutionScope: 'CURRENT_INSTITUTION',
  periodId: '2026-09',
  periodStartInclusive: '2026-09-15',
  periodEndInclusive: '2026-10-14',
  projectionDate: '2026-09-15',
};

const state: CompensationPolicyStateWire = {
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
  digerGelirVarsayilan: null,
};

function loadWithPrefill(prefill: Partial<CompensationPolicyStateWire>): CompensationPolicyLoadData {
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
      state: prefill,
    },
  };
}

describe('draftForCompensationPolicyLoad', () => {
  test('loads the exact complete archive state as an editable, unapproved initial draft', () => {
    const draft = draftForCompensationPolicyLoad(scope, loadWithPrefill(state));

    expect(draft.operationKind).toBe('INITIAL_BASELINE');
    expect(draft.effectiveFrom).toBe(scope.periodStartInclusive);
    expect(draft.reason).toBe('');
    expect(draft.boundaryScope).toBe('FULL_EARNING');
    expect(draft.roadRoundingMode).toBe('PERIOD_TOTAL');
    expect(draft.gunlukTabanUcret).toBe('2616,26');
    expect(draft.gunlukYemek).toBe('300,75');
    expect(draft.birlestirilmisSosyalYardim).toBe('5089,7');
    expect(draft.gunlukVasitaYol).toBe('128,93');
    expect(draft.giyimYardimi).toBe('269,7');
    expect(draft.hizmetZammiBirimi).toBe('24,67');
    expect(draft.isPrimiYuzde).toEqual({ kind: 'VALUE', raw: '0' });
    expect(draft.ekOdeme).toEqual({ kind: 'VALUE', raw: '0' });
    expect(draft.digerGelirVarsayilan).toEqual({ kind: 'ABSENT' });
    expect(draft.isPrimiGruplari).toEqual(state.isPrimiGruplari);
  });

  test('keeps the draft blank when a required archive value or explicit nullable value is missing', () => {
    const { gunlukYemek: _meal, ...missingRequired } = state;
    const draft = draftForCompensationPolicyLoad(scope, loadWithPrefill(missingRequired));

    expect(draft.gunlukTabanUcret).toBe('');
    expect(draft.gunlukYemek).toBe('');
    expect(draft.isPrimiGruplari).toEqual([]);
    expect(draft.ekOdeme).toEqual({ kind: 'ABSENT' });
    expect(draft.boundaryScope).toBe('FULL_EARNING');
    expect(draft.roadRoundingMode).toBe('PERIOD_TOTAL');
  });

  test('does not coerce numeric amounts or groups with missing active state', () => {
    const numericAmount = { ...state, gunlukTabanUcret: 2616.26 as unknown as string };
    const amountDraft = draftForCompensationPolicyLoad(scope, loadWithPrefill(numericAmount));
    const groups = state.isPrimiGruplari.map(({ aktif: _active, ...group }) => group);
    const groupDraft = draftForCompensationPolicyLoad(scope, loadWithPrefill({ ...state, isPrimiGruplari: groups as CompensationPolicyStateWire['isPrimiGruplari'] }));

    expect(amountDraft.gunlukTabanUcret).toBe('');
    expect(groupDraft.gunlukTabanUcret).toBe('');
  });

  test('uses the active policy projection before archive settings when a saved policy exists', () => {
    const savedState = { ...state, gunlukTabanUcret: '3000.00' };
    const version: CompensationPolicyVersionWire = {
      id: 'version-1',
      effectiveFrom: scope.periodStartInclusive,
      state: savedState,
      boundaryScope: 'ROAD_ONLY',
      roadRoundingMode: 'PER_COMPENSATION_SLICE',
      createdAt: '2026-09-15T00:00:00.000Z',
      source: 'TEST',
      reason: 'Saved policy',
      supersedesVersionId: null,
    };
    const loaded = loadWithPrefill(state);
    loaded.initialBaselineRequired = false;
    loaded.activePolicyProjection = { kind: 'ACTIVE', asOfDate: scope.projectionDate, version };

    const draft = draftForCompensationPolicyLoad(scope, loaded);

    expect(draft.operationKind).toBe('NEW_EFFECTIVE_DATE');
    expect(draft.gunlukTabanUcret).toBe('3000,00');
    expect(draft.boundaryScope).toBe('ROAD_ONLY');
    expect(draft.roadRoundingMode).toBe('PER_COMPENSATION_SLICE');
  });
});
