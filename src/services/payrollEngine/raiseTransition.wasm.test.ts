import { describe, expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import init, { calculate_payroll_json, evaluate_mutation_policy_json } from '../../wasm/pkg/payroll_wasm.js';
import { parseWasmPayrollBoundaryResult } from './decimalBoundary';
import type { PayrollBoundaryPayroll, PayrollCalculationRequest } from './types';

async function request(): Promise<PayrollCalculationRequest> {
  await init();
  const fixture = JSON.parse(await readFile(new URL(
    '../../../crates/payroll-core/tests/golden/2026/G020-social-aid.json', import.meta.url
  ), 'utf8')) as { request: PayrollCalculationRequest };
  const value = fixture.request;
  const current = value.dataset.periods[0];
  const settings = value.dataset.institutionSettings[current.id];
  Object.assign(settings, {
    gunlukTabanUcret: '110', gunlukYemek: '10',
    birlestirilmisSosyalYardim: '1000', giyimYardimi: '100',
  });
  const previous = {
    ...current, id: '2025-12', yil: 2025, ay: 12,
    baslangicTarihi: '2025-12-15', bitisTarihi: '2026-01-14', taxYear: 2025, taxMonth: 12,
  };
  value.dataset.periods.push(previous);
  value.dataset.institutionSettings[previous.id] = {
    ...structuredClone(settings), donemId: previous.id, gunlukTabanUcret: '100',
  };
  value.dataset.zamAylari = [2];
  return value;
}

function calculate(value: PayrollCalculationRequest): PayrollBoundaryPayroll {
  return parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(calculate_payroll_json(JSON.stringify(value)));
}

describe('production WASM raise transition', () => {
  test('defaults meal policy on and switches GV/DV without changing the split meal or SGK', async () => {
    const value = await request();
    const settings = value.dataset.institutionSettings[value.periodId];
    settings.gunlukTabanUcret = '2616.26';
    settings.gunlukYemek = '300.75';
    settings.gunlukYemekIstisnasiGV = '300';
    settings.gunlukYemekIstisnasiSGK = '300';
    const previous = value.dataset.institutionSettings['2025-12'];
    previous.gunlukTabanUcret = '2443.28';
    previous.gunlukYemek = '300.75';
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

    // A fixed deduction of the same amount must produce identical taxes/net.
    person.kesintiler.sabitSendikaAidati = '71.50';
    const fixed = calculate(value);
    expect(member.gvDetay).toEqual(fixed.gvDetay);
    expect(member.netOdeme).toBe(fixed.netOdeme);
    expect(member.kesintiToplam).toBe(fixed.kesintiToplam);
    person.kesintiler.sendikaUyesi = false;
    expect(calculate(value).kesintiler.sendikaAidati).toBe('0');
  });

  test('splits the four incomes and preserves full raised rates through the next JSON request', async () => {
    const value = await request();
    const first = calculate(value);
    expect(first.gelirler.tabanBrutAylik).toBe('3240');
    expect(first.gelirler.yemek).toBe('324');
    expect(first.gelirler.birlestirilmisSosyalYardim).toBe('1045.16');
    expect(first.gelirler.giyimYardimi).toBe('104.52');
    value.dataset.payrolls.push(first);
    const next = {
      ...value.dataset.periods[0], id: '2026-02', ay: 2,
      baslangicTarihi: '2026-02-15', bitisTarihi: '2026-03-14', taxMonth: 2,
    };
    value.dataset.institutionSettings[next.id] = {
      ...structuredClone(value.dataset.institutionSettings[value.periodId]), donemId: next.id,
    };
    value.dataset.periods.push(next);
    value.dataset.attendances.push({
      id: `${value.personnelId}_${next.id}`, personelId: value.personnelId, donemId: next.id,
      gunler: Object.fromEntries([
        ...Array.from({ length: 14 }, (_, index) => [`2026-02-${index + 15}`, 'Ç']),
        ...Array.from({ length: 14 }, (_, index) => [`2026-03-${String(index + 1).padStart(2, '0')}`, 'Ç']),
      ]),
    });
    value.periodId = next.id;
    value.accrual = {
      accrualId: `${value.personnelId}_${next.id}`, accrualType: 'NORMAL',
      paymentDate: '2026-02-28', sequence: 0, grossAmount: null, description: null,
    };
    const second = calculate(value);
    expect(second.gelirler.tabanBrutAylik).toBe('3080');
    expect(second.gelirler.yemek).toBe('308');
    expect(second.gelirler.birlestirilmisSosyalYardim).toBe('1100');
    expect(second.gelirler.giyimYardimi).toBe('110');
    expect(value.dataset.institutionSettings[next.id].gunlukYemek).toBe('10');
  });

  test('rejects a transition with missing previous settings at the WASM boundary', async () => {
    const value = await request();
    delete value.dataset.institutionSettings['2025-12'];
    expect(() => calculate(value)).toThrow();
  });

  test('protects finalized payrolls across tax years when their historical rate settings change', async () => {
    const value = await request();
    const payroll = calculate(value);
    payroll.status = 'FINALIZED';
    value.dataset.payrolls.push(payroll);
    const impact = JSON.parse(evaluate_mutation_policy_json(JSON.stringify({
      dataset: value.dataset, mutation: { kind: 'PERIOD', periodId: '2025-12' },
    }))) as { blockedByFinalized: Array<{ periodId: string }> };
    expect(impact.blockedByFinalized.map((key) => key.periodId)).toEqual([value.periodId]);
  });
});
