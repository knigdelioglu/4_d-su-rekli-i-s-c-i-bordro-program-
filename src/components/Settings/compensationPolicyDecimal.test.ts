import { describe, expect, test } from 'bun:test';
import type { CompensationPolicyPageProps, CompensationPolicyStateDraft, CompensationPolicyVersionWire } from '../../services/compensationPolicy/contract';
import {
  applyCompensationPolicyPercentageIncrease,
  compensationPolicyStateToDraft,
  copyCompensationPolicyStateDraft,
  createEmptyCompensationPolicyStateDraft,
  draftToCompensationPolicyOperation,
  draftToCompensationPolicyState,
  findScopedSameDateActiveLeaf,
  getCompensationPolicyBenefitRuleErrors,
  parseCompensationPolicyDecimal,
  validateCompensationPolicyDraft,
} from './compensationPolicyDecimal';

type PageDraft = CompensationPolicyPageProps['draft'];

const completeStateDraft = (): PageDraft => ({
  ...createEmptyCompensationPolicyStateDraft(),
  gunlukTabanUcret: '9007199254740993,1234',
  gunlukYemek: '50,00',
  birlestirilmisSosyalYardim: '20',
  gunlukVasitaYol: '30',
  giyimYardimi: '12,34',
  hizmetZammiBirimi: '5',
  isPrimiYuzde: { kind: 'VALUE', raw: '0' },
  isPrimiGruplari: [
    { id: 'A', ad: 'Atesli', oran: '65,50', aktif: true },
    { id: 'B', ad: 'Teknik', oran: '0', aktif: false },
  ],
  geceCalismaPrimiYuzde: '20',
  geceCalismaTatiliPrimiYuzde: '30',
  ekOdeme: { kind: 'VALUE', raw: '0' },
  digerGelirVarsayilan: { kind: 'ABSENT' },
  operationKind: 'NEW_EFFECTIVE_DATE',
  effectiveFrom: '2026-09-15',
  boundaryScope: 'FULL_EARNING',
  roadRoundingMode: 'PER_COMPENSATION_SLICE',
  reason: 'Yeni dönem ücret değişikliği',
  supersedesVersionId: null,
});

describe('exact compensation policy decimal boundary', () => {
  test('requires an explicit eligibility set and defaults newly selected thirty-day rules to the monthly cap', () => {
    const draft = completeStateDraft();
    draft.socialBenefitAccrualPolicies.birlestirilmisSosyalYardim = {
      method: 'PROPORTIONAL',
      eligibleDayTypes: [],
      denominator: 'THIRTY_DAY_MONTH',
      allowMonthlyAmountExcess: false,
    };
    expect(getCompensationPolicyBenefitRuleErrors(draft.socialBenefitAccrualPolicies))
      .toEqual({ birlestirilmisSosyalYardim: 'Bu ödeme yöntemi için en az bir hak kazandıran gün türü seçilmelidir.' });
    expect(validateCompensationPolicyDraft(draft).draft).toBe('INVALID');

    draft.socialBenefitAccrualPolicies.birlestirilmisSosyalYardim.eligibleDayTypes = ['WORKED'];
    const result = draftToCompensationPolicyState(draft);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.state.socialBenefitAccrualPolicies?.birlestirilmisSosyalYardim?.allowMonthlyAmountExcess).toBe(false);
    }
  });

  test('canonicalizes Turkish decimal input exactly without a JavaScript number conversion', () => {
    expect(parseCompensationPolicyDecimal('9007199254740993,1234')).toEqual({
      ok: true,
      value: '9007199254740993.1234',
    });
    expect(parseCompensationPolicyDecimal('00012.3400')).toEqual({ ok: true, value: '12.34' });
    expect(parseCompensationPolicyDecimal('-0,00')).toEqual({ ok: true, value: '0' });
    expect(parseCompensationPolicyDecimal('1.234,56').ok).toBe(false);
    expect(parseCompensationPolicyDecimal('1e6').ok).toBe(false);
  });

  test('rejects Rust Decimal coefficient and scale overflow instead of rounding', () => {
    expect(parseCompensationPolicyDecimal('79228162514264337593543950335').ok).toBe(true);
    expect(parseCompensationPolicyDecimal('79228162514264337593543950336').ok).toBe(false);
    expect(parseCompensationPolicyDecimal('0.00000000000000000000000000001').ok).toBe(false);
    expect(parseCompensationPolicyDecimal('1.000000000000000000000000000000').ok).toBe(true);
  });

  test('converts a valid full draft with exact strings, null versus zero, groups, and all state fields', () => {
    const result = draftToCompensationPolicyState(completeStateDraft());
    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.state).toEqual({
      gunlukTabanUcret: '9007199254740993.1234',
      gunlukYemek: '50',
      birlestirilmisSosyalYardim: '20',
      gunlukVasitaYol: '30',
      giyimYardimi: '12.34',
      hizmetZammiBirimi: '5',
      isPrimiYuzde: '0',
      isPrimiGruplari: [
        { id: 'A', ad: 'Atesli', oran: '65.5', aktif: true },
        { id: 'B', ad: 'Teknik', oran: '0', aktif: false },
      ],
      geceCalismaPrimiYuzde: '20',
      geceCalismaTatiliPrimiYuzde: '30',
      ekOdeme: '0',
      digerGelirVarsayilan: null,
      socialBenefitAccrualPolicies: null,
    });
    const userDraft = compensationPolicyStateToDraft(result.state);
    expect(userDraft.gunlukTabanUcret).toBe('9007199254740993,1234');
    expect(userDraft.isPrimiGruplari[0]?.oran).toBe('65,5');
    const roundtrip = draftToCompensationPolicyState(userDraft);
    expect(roundtrip.ok).toBe(true);
    if (roundtrip.ok === true) expect(roundtrip.state).toEqual(result.state);
  });

  test('validates group identity and rate in the compensation policy draft', () => {
    const duplicateId = completeStateDraft();
    duplicateId.isPrimiGruplari = [
      { id: 'A', ad: 'Bir', oran: '0', aktif: true },
      { id: 'A', ad: 'İki', oran: '0', aktif: true },
    ];
    expect(draftToCompensationPolicyState(duplicateId).ok).toBe(false);

    const duplicateName = completeStateDraft();
    duplicateName.isPrimiGruplari = [
      { id: 'A', ad: 'Grup', oran: '0', aktif: true },
      { id: 'B', ad: 'Grup', oran: '0', aktif: true },
    ];
    expect(draftToCompensationPolicyState(duplicateName).ok).toBe(false);
  });

  test('allows duplicate inactive identities but validates every compensation group', () => {
    const inactiveDuplicates = completeStateDraft();
    inactiveDuplicates.isPrimiGruplari = [
      { id: 'A', ad: 'Pasif', oran: '0', aktif: false },
      { id: 'A', ad: 'Pasif', oran: '100', aktif: false },
      { id: 'B', ad: 'Aktif', oran: '0', aktif: true },
    ];
    expect(draftToCompensationPolicyState(inactiveDuplicates).ok).toBe(true);

    const invalidInactiveRate = completeStateDraft();
    invalidInactiveRate.isPrimiGruplari = [
      { id: 'A', ad: 'Pasif', oran: '100.01', aktif: false },
    ];
    expect(draftToCompensationPolicyState(invalidInactiveRate).ok).toBe(false);
  });

  test('rejects negative optional compensation amounts', () => {
    const negativeSupplemental = completeStateDraft();
    negativeSupplemental.ekOdeme = { kind: 'VALUE', raw: '-1' };
    expect(draftToCompensationPolicyState(negativeSupplemental).ok).toBe(false);

    const negativeOtherIncome = completeStateDraft();
    negativeOtherIncome.digerGelirVarsayilan = { kind: 'VALUE', raw: '-1' };
    expect(draftToCompensationPolicyState(negativeOtherIncome).ok).toBe(false);
  });

  test('keeps absent and explicit zero distinct during partial prefill and roundtrip', () => {
    const partial = compensationPolicyStateToDraft({
      gunlukTabanUcret: '1000.00',
      gunlukYemek: '0',
      ekOdeme: null,
      digerGelirVarsayilan: '0',
      isPrimiGruplari: [{ id: 'A', ad: 'Atesli', oran: '0', aktif: false }],
    });

    expect(partial.gunlukTabanUcret).toBe('1000,00');
    expect(partial.birlestirilmisSosyalYardim).toBe('');
    expect(partial.gunlukYemek).toBe('0');
    expect(partial.ekOdeme).toEqual({ kind: 'ABSENT' });
    expect(partial.digerGelirVarsayilan).toEqual({ kind: 'VALUE', raw: '0' });
    expect(partial.isPrimiGruplari).toEqual([{ id: 'A', ad: 'Atesli', oran: '0', aktif: false }]);
  });

  test('returns typed incomplete and policy errors and never creates a state for invalid full-state', () => {
    const missing = createEmptyCompensationPolicyStateDraft();
    const incomplete = validateCompensationPolicyDraft({
      ...missing,
      operationKind: 'INITIAL_BASELINE',
      effectiveFrom: '2026-09-15',
      boundaryScope: 'ROAD_ONLY',
      roadRoundingMode: 'PERIOD_TOTAL',
      reason: 'Başlangıç',
      supersedesVersionId: null,
    });
    expect(incomplete.draft).toBe('INVALID');
    expect(incomplete.fullState.status).toBe('INCOMPLETE');
    expect(draftToCompensationPolicyState(missing).ok).toBe(false);

    const invalid = completeStateDraft();
    invalid.gunlukTabanUcret = '0';
    invalid.gunlukYemek = '-1';
    invalid.geceCalismaPrimiYuzde = '100.01';
    invalid.isPrimiGruplari[1] = { id: 'A', ad: 'Atesli', oran: '101', aktif: true };
    const validation = validateCompensationPolicyDraft(invalid);
    expect(validation.fullState.status).toBe('INVALID');
    if (validation.fullState.status === 'INVALID') {
      const errorText = validation.fullState.errors.join(' ');
      expect(errorText.includes('sıfırdan büyük')).toBe(true);
      expect(errorText.includes('negatif olamaz')).toBe(true);
      expect(errorText.includes('%0 ile %100')).toBe(true);
      expect(errorText.includes('yinelenen')).toBe(true);
    }
    expect(draftToCompensationPolicyState(invalid).ok).toBe(false);
  });

  test('requires canonical valid dates and a same-date leaf for correction operations', () => {
    const invalidDate = completeStateDraft();
    invalidDate.effectiveFrom = '2026-02-29';
    expect(validateCompensationPolicyDraft(invalidDate).operationErrors.join(' ').includes('takvim günü')).toBe(true);

    const correction = completeStateDraft();
    correction.operationKind = 'SAME_DATE_CORRECTION';
    correction.supersedesVersionId = 'leaf-7';
    const result = draftToCompensationPolicyOperation(correction);
    expect(result.ok).toBe(true);
    if (result.ok === true) {
      expect(result.operation.kind).toBe('SAME_DATE_CORRECTION');
      if (result.operation.kind === 'SAME_DATE_CORRECTION') expect(result.operation.supersedesVersionId).toBe('leaf-7');
      expect(result.operation.boundaryScope).toBe('FULL_EARNING');
      if (result.operation.kind !== 'INITIAL_BASELINE') expect(result.operation.changeIntent).toBe('FULL_STATE_CHANGE');
      expect(result.operation.state.gunlukTabanUcret).toBe('9007199254740993.1234');
    }
    correction.supersedesVersionId = null;
    expect(draftToCompensationPolicyOperation(correction).ok).toBe(false);
  });

  test('preserves explicit ROAD_ONLY baseline intent and its full-state values', () => {
    const baseline = completeStateDraft();
    baseline.operationKind = 'INITIAL_BASELINE';
    baseline.boundaryScope = 'ROAD_ONLY';
    baseline.roadRoundingMode = 'PERIOD_TOTAL';
    const result = draftToCompensationPolicyOperation(baseline);
    expect(result.ok).toBe(true);
    if (result.ok === true) {
      expect(result.operation.kind).toBe('INITIAL_BASELINE');
      if (result.operation.kind === 'INITIAL_BASELINE') expect(result.operation.boundaryScope).toBe('ROAD_ONLY');
      expect(result.operation.roadRoundingMode).toBe('PERIOD_TOTAL');
      expect(result.operation.state.giyimYardimi).toBe('12.34');
      expect('changeIntent' in result.operation).toBe(false);
    }
  });

  test('maps a later ROAD_ONLY version to road-only amount intent with full state', () => {
    const changeDraft = completeStateDraft();
    changeDraft.operationKind = 'NEW_EFFECTIVE_DATE';
    changeDraft.effectiveFrom = '2026-10-01';
    changeDraft.boundaryScope = 'ROAD_ONLY';

    const result = draftToCompensationPolicyOperation(changeDraft);
    expect(result.ok).toBe(true);
    if (result.ok === true) {
      expect(result.operation.kind).toBe('NEW_EFFECTIVE_DATE');
      if (result.operation.kind !== 'INITIAL_BASELINE') {
        expect(result.operation.boundaryScope).toBe('ROAD_ONLY');
        expect(result.operation.changeIntent).toBe('ROAD_AMOUNT_ONLY');
      }
      expect(result.operation.state.gunlukVasitaYol).toBe('30');
      expect(result.operation.state.gunlukTabanUcret).toBe('9007199254740993.1234');
    }
  });

  test('applies percentage increase immutably to base and meal while leaving clothing unchanged', () => {
    const source = completeStateDraft();
    source.gunlukTabanUcret = '1000.00';
    source.gunlukYemek = '50.00';
    const before = copyCompensationPolicyStateDraft(source);
    const result = applyCompensationPolicyPercentageIncrease(source, '10,00');

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.draft.gunlukTabanUcret).toBe('1100');
      expect(result.draft.gunlukYemek).toBe('55');
      expect(result.draft.giyimYardimi).toBe('12,34');
      expect(result.draft.isPrimiGruplari).toEqual(source.isPrimiGruplari);
      expect(source).toEqual(before);
      expect(validateCompensationPolicyDraft({ ...source, ...result.draft }).fullState.status).toBe('VALID');
    }
  });

  test('applies percentages beyond JavaScript precision exactly and fails when output cannot fit Rust Decimal', () => {
    const source = completeStateDraft();
    source.gunlukTabanUcret = '9007199254740993.01';
    const result = applyCompensationPolicyPercentageIncrease(source, '10');
    expect(result.ok).toBe(true);
    if (result.ok === true) expect(result.draft.gunlukTabanUcret).toBe('9907919180215092,311');

    expect(applyCompensationPolicyPercentageIncrease(source, '100', ['gunlukTabanUcret']).ok).toBe(true);
    expect(applyCompensationPolicyPercentageIncrease(source, '100.01').ok).toBe(false);
    expect(applyCompensationPolicyPercentageIncrease({ ...source, gunlukTabanUcret: '0.0000000000000000000000000001' }, '10').ok).toBe(false);
  });

  test('selects only one scoped same-date correction leaf', () => {
    const version = (id: string, supersedesVersionId: string | null): CompensationPolicyVersionWire => ({
      id,
      effectiveFrom: '2026-09-15',
      state: {
        gunlukTabanUcret: '1', gunlukYemek: '0', birlestirilmisSosyalYardim: '0', gunlukVasitaYol: '0',
        giyimYardimi: '0', hizmetZammiBirimi: '0', isPrimiYuzde: null, isPrimiGruplari: [],
        geceCalismaPrimiYuzde: '0', geceCalismaTatiliPrimiYuzde: '0', ekOdeme: null, digerGelirVarsayilan: null,
      },
      boundaryScope: 'FULL_EARNING',
      roadRoundingMode: 'PERIOD_TOTAL',
      createdAt: '2026-09-01T00:00:00Z',
      source: 'TEST',
      reason: 'test',
      supersedesVersionId,
    });
    expect(findScopedSameDateActiveLeaf([version('root', null), version('leaf-1', 'root'), version('leaf-2', 'leaf-1')], '2026-09-15')).toBe('leaf-2');
    expect(findScopedSameDateActiveLeaf([version('root-a', null), version('root-b', null)], '2026-09-15')).toBe(null);
    expect(findScopedSameDateActiveLeaf([version('root', null)], '2026-09-16')).toBe(null);
  });
});
