import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import init, {
  calculate_payroll_json,
  calculate_retro_preview_json,
  evaluate_mutation_policy_json,
  validate_payroll_json,
} from '../../wasm/pkg/payroll_wasm.js';
import { parseWasmPayrollBoundaryResult } from './decimalBoundary';
import type { PayrollBoundaryPayroll, PayrollCalculationRequest, RetroCalculationRequest, RetroCalculationResult } from './types';

type PolicyState = PayrollCalculationRequest['dataset']['compensationPolicyVersions'][number]['state'];
type PolicyVersion = PayrollCalculationRequest['dataset']['compensationPolicyVersions'][number];

const LEGACY_COMPENSATION_FIELDS = [
  'gunlukTabanUcret',
  'gunlukYemek',
  'birlestirilmisSosyalYardim',
  'gunlukVasitaYol',
  'giyimYardimi',
  'hizmetZammiBirimi',
  'isPrimiYuzde',
  'isPrimiGruplari',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
  'ekOdeme',
  'digerGelirVarsayilan',
] as const;

function policyState(
  gunlukTabanUcret: string,
  gunlukYemek: string,
  birlestirilmisSosyalYardim: string,
  giyimYardimi: string,
): PolicyState {
  return {
    gunlukTabanUcret,
    gunlukYemek,
    birlestirilmisSosyalYardim,
    gunlukVasitaYol: '80',
    giyimYardimi,
    hizmetZammiBirimi: '0',
    isPrimiYuzde: '0',
    isPrimiGruplari: [{ id: '1. Grup', ad: '1. Grup', oran: '0', aktif: true }],
    geceCalismaPrimiYuzde: '0',
    geceCalismaTatiliPrimiYuzde: '0',
    ekOdeme: '0',
    digerGelirVarsayilan: '0',
  };
}

function policyVersion(
  id: string,
  effectiveFrom: string,
  state: PolicyState,
): PolicyVersion {
  return {
    id,
    effectiveFrom,
    state,
    boundaryScope: 'FULL_EARNING',
    roadRoundingMode: 'PER_COMPENSATION_SLICE',
    createdAt: `${effectiveFrom}T00:00:00Z`,
    source: 'ADMINISTRATIVE_DECISION',
    reason: 'Explicit WASM regression fixture',
    supersedesVersionId: null,
  };
}

function removeLegacyCompensationFields(settings: Record<string, unknown>): void {
  for (const field of LEGACY_COMPENSATION_FIELDS) delete settings[field];
}

async function request(): Promise<PayrollCalculationRequest> {
  await init();
  const fixture = JSON.parse(await readFile(new URL(
    '../../../crates/payroll-core/tests/golden/2026/G020-social-aid.json', import.meta.url
  ), 'utf8')) as { request: PayrollCalculationRequest };
  const value = fixture.request;
  const current = value.dataset.periods[0];
  const settings = value.dataset.institutionSettings[current.id] as unknown as Record<string, unknown>;
  removeLegacyCompensationFields(settings);

  const previous = {
    ...current,
    id: '2025-12',
    yil: 2025,
    ay: 12,
    baslangicTarihi: '2025-12-15',
    bitisTarihi: '2026-01-14',
    taxYear: 2025,
    taxMonth: 12,
  };
  value.dataset.periods.push(previous);
  value.dataset.institutionSettings[previous.id] = {
    ...structuredClone(value.dataset.institutionSettings[current.id]),
    donemId: previous.id,
  };
  value.dataset.compensationPolicyVersions = [
    policyVersion('policy-root', '2025-12-15', policyState('100', '10', '1000', '100')),
    policyVersion('policy-raise', '2026-02-01', policyState('110', '11', '1100', '110')),
  ];
  return value;
}

function calculate(value: PayrollCalculationRequest): PayrollBoundaryPayroll {
  return parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(calculate_payroll_json(JSON.stringify(value)));
}

describe('production WASM compensation policy transition', () => {
  test('transports a partial retro meal exemption and signed premium refund through the real WASM boundary', async () => {
    for (const refund of [false, true]) {
      const value = await request();
      const source = value.dataset.periods[0];
      for (const version of value.dataset.compensationPolicyVersions) {
        version.state = policyState('2000', refund ? '180' : '260', '0', '0');
        version.state.gunlukVasitaYol = '0';
      }
      const settings = value.dataset.institutionSettings[value.periodId];
      settings.yemekTamGvIstisnaDamgaTabi = false;
      settings.gunlukYemekIstisnasiGV = '300';
      settings.gunlukYemekIstisnasiSGK = '300';
      value.dataset.personnel[0].kesintiler = null;
      value.dataset.personnel[0].hizmetYili = 0;
      value.dataset.payrolls.push(calculate(value));
      const payment = { ...source, id: 'retro-payment', ay: 2,
        baslangicTarihi: '2026-02-15', bitisTarihi: '2026-03-14', taxMonth: 2 };
      value.dataset.periods.push(payment);
      value.dataset.institutionSettings[payment.id] = { ...settings, donemId: payment.id };
      const days: PayrollCalculationRequest['dataset']['attendances'][number]['gunler'] = {};
      for (let date = new Date('2026-02-15T00:00:00Z'); date <= new Date('2026-03-14T00:00:00Z'); date.setUTCDate(date.getUTCDate() + 1)) {
        days[date.toISOString().slice(0, 10)] = 'Ç';
      }
      value.dataset.attendances.push({ id: 'retro-payment-attendance', personelId: value.personnelId,
        donemId: payment.id, gunler: days });
      value.periodId = payment.id;
      value.accrual = { accrualId: 'payment-normal', accrualType: 'NORMAL', paymentDate: '2026-02-28',
        sequence: 0, grossAmount: null, description: null };
      value.dataset.payrolls.push(calculate(value));
      const retroRequest: RetroCalculationRequest = {
        batchId: 'meal-retro', personnelId: value.personnelId, paymentDate: '2026-02-28',
        calculatedAt: value.calculatedAt, description: null, dataset: value.dataset,
        revision: { id: 'meal-revision', reason: 'COLLECTIVE_AGREEMENT', title: 'Meal correction',
          effectiveFrom: source.baslangicTarihi, effectiveTo: source.bitisTarihi,
          status: 'DRAFT', scope: 'ALL_PERSONNEL', personnelIds: [] },
        overrides: [{ id: 'meal-override', revisionId: 'meal-revision', parameter: 'GUNLUK_YEMEK',
          value: refund ? '280' : '350', personnelId: null }],
      };
      if (refund) retroRequest.overrides.push({ id: 'wage-override', revisionId: 'meal-revision',
        parameter: 'GUNLUK_TABAN_UCRET', value: '1950', personnelId: null });
      const retro = JSON.parse(calculate_retro_preview_json(JSON.stringify(retroRequest))) as RetroCalculationResult;
      value.dataset.retroBatches.push(retro.batch);
      value.dataset.retroAllocations.push(...retro.allocations);
      value.accrual = { accrualId: retro.batch.id, accrualType: 'RETRO_ADJUSTMENT',
        paymentDate: retro.batch.paymentDate, sequence: 1, grossAmount: null, description: null };
      const payroll = calculate(value);
      expect(Number(payroll.gelirToplam)).toBe(refund ? 1550 : 2790);
      expect(Number(payroll.gvDetay!.cariGvMatrahi)).toBe(refund ? 232.5 : 1317.5);
      expect(Number(payroll.kesintiler.isciSgkPrimi)).toBe(refund ? -217 : 217);
      expect(Number(payroll.damgaDetay!.brutDamgaVergisi)).toBe(refund ? 0 : 11.76);
      if (refund) expect(Number(payroll.kesintiToplam)).toBeLessThan(0);
    }
  });

  test('defaults meal policy on and switches GV/DV without changing the split meal or SGK', async () => {
    const value = await request();
    value.dataset.compensationPolicyVersions[0].state = policyState('2443.28', '300.75', '1000', '100');
    value.dataset.compensationPolicyVersions[1].state = policyState('2616.26', '322.0431', '1070.8', '107.08');
    const settings = value.dataset.institutionSettings[value.periodId];
    settings.gunlukYemekIstisnasiGV = '300';
    settings.gunlukYemekIstisnasiSGK = '300';
    const days = value.dataset.attendances[0].gunler;
    Object.keys(days).forEach((date) => { days[date] = 'T'; });
    for (let day = 15; day <= 25; day++) days[`2026-01-${day}`] = 'Ç';
    days['2026-02-01'] = 'Ç';
    days['2026-02-02'] = 'Ç';
    delete settings.yemekTamGvIstisnaDamgaTabi;
    const on = calculate(value);
    expect(on.gelirler.yemek).toBe('3952.34');
    expect(Number(on.damgaDetay!.brutDamgaVergisi)).toBe(Number((Number(on.gelirToplam) * 0.00759).toFixed(2)));
    settings.yemekTamGvIstisnaDamgaTabi = true;
    expect(calculate(value)).toEqual(on);
    settings.yemekTamGvIstisnaDamgaTabi = false;
    const restored = JSON.parse(JSON.stringify(value)) as PayrollCalculationRequest;
    const off = calculate(restored);
    expect(off.gelirler).toEqual(on.gelirler);
    expect(off.pekDetay).toEqual(on.pekDetay);
    expect(Number((Number(off.gvDetay!.cariGvMatrahi) - Number(on.gvDetay!.cariGvMatrahi)).toFixed(2))).toBe(52.34);
    expect(Number(off.damgaDetay!.brutDamgaVergisi)).toBe(Number(((Number(off.gelirToplam) - 3900) * 0.00759).toFixed(2)));
  });

  test('uses the raised daily wage for union dues and includes them in the tax base and net', async () => {
    const value = await request();
    const person = value.dataset.personnel[0];
    person.kesintiler = { sendikaUyesi: true, besUyesi: false };
    const settings = value.dataset.institutionSettings[value.periodId];
    settings.sendikaAidatiYuzde = '65';
    settings.sabitSendikaAidati = '0';
    const member = calculate(value);
    expect(member.kesintiler.sendikaAidati).toBe('71.50');
    expect(member.gelirler.tabanBrutAylik).toBe('3240');

    person.kesintiler.sabitSendikaAidati = '71.50';
    const fixed = calculate(value);
    expect(member.gvDetay).toEqual(fixed.gvDetay);
    expect(member.netOdeme).toBe(fixed.netOdeme);
    expect(member.kesintiToplam).toBe(fixed.kesintiToplam);
    person.kesintiler.sendikaUyesi = false;
    expect(calculate(value).kesintiler.sendikaAidati).toBe('0');
  });

  test('splits the four incomes and carries policy rates into the next JSON request', async () => {
    const value = await request();
    const first = calculate(value);
    expect(first.gelirler.tabanBrutAylik).toBe('3240');
    expect(first.gelirler.yemek).toBe('324');
    expect(first.gelirler.birlestirilmisSosyalYardim).toBe('1045.16');
    expect(first.gelirler.giyimYardimi).toBe('104.52');
    value.dataset.payrolls.push(first);
    const next = {
      ...value.dataset.periods[0],
      id: '2026-02',
      ay: 2,
      baslangicTarihi: '2026-02-15',
      bitisTarihi: '2026-03-14',
      taxMonth: 2,
    };
    value.dataset.institutionSettings[next.id] = {
      ...structuredClone(value.dataset.institutionSettings[value.periodId]),
      donemId: next.id,
    };
    value.dataset.periods.push(next);
    value.dataset.attendances.push({
      id: `${value.personnelId}_${next.id}`,
      personelId: value.personnelId,
      donemId: next.id,
      gunler: Object.fromEntries([
        ...Array.from({ length: 14 }, (_, index) => [`2026-02-${index + 15}`, 'Ç']),
        ...Array.from({ length: 14 }, (_, index) => [`2026-03-${String(index + 1).padStart(2, '0')}`, 'Ç']),
      ]),
    });
    value.periodId = next.id;
    value.accrual = {
      accrualId: `${value.personnelId}_${next.id}`,
      accrualType: 'NORMAL',
      paymentDate: '2026-02-28',
      sequence: 0,
      grossAmount: null,
      description: null,
    };
    const second = calculate(value);
    expect(second.gelirler.tabanBrutAylik).toBe('3080');
    expect(second.gelirler.yemek).toBe('308');
    expect(second.gelirler.birlestirilmisSosyalYardim).toBe('1100');
    expect(second.gelirler.giyimYardimi).toBe('110');
    expect('gunlukYemek' in value.dataset.institutionSettings[next.id]).toBe(false);
    expect(value.dataset.compensationPolicyVersions[1].state.gunlukYemek).toBe('11');
  });

  test('resolves the raise from policy history without prior period compensation settings', async () => {
    const value = await request();
    delete value.dataset.institutionSettings['2025-12'];
    const payroll = calculate(value);
    expect(payroll.gelirler.tabanBrutAylik).toBe('3240');
    expect(payroll.gelirler.yemek).toBe('324');
  });

  test('blocks finalized payrolls when their historical compensation policy changes', async () => {
    const value = await request();
    const payroll = calculate(value);
    payroll.status = 'FINALIZED';
    value.dataset.payrolls.push(payroll);
    const impact = JSON.parse(evaluate_mutation_policy_json(JSON.stringify({
      dataset: value.dataset,
      mutation: {
        kind: 'COMPENSATION_POLICY',
        effectiveFrom: '2025-12-15',
        throughInclusive: null,
      },
    }))) as { blockedByFinalized: Array<{ periodId: string }> };
    expect(impact.blockedByFinalized.map((key) => key.periodId)).toEqual([value.periodId]);
  });

  test('does not infer a NORMAL baseline from archived rates or a legacy raise calendar', async () => {
    const value = await request();
    value.dataset.compensationPolicyVersions = [];
    const settings = value.dataset.institutionSettings[value.periodId] as unknown as Record<string, unknown>;
    Object.assign(settings, {
      gunlukTabanUcret: '110',
      gunlukYemek: '10',
      birlestirilmisSosyalYardim: '1000',
      giyimYardimi: '100',
      gunlukVasitaYol: '80',
    });
    Object.assign(value.dataset, { zamAylari: [2] });
    const requestJson = JSON.stringify(value);
    expect(() => validate_payroll_json(requestJson)).toThrow();
    expect(() => calculate_payroll_json(requestJson)).toThrow();
  });
});
