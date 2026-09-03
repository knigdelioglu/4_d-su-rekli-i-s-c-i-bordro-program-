import { describe, expect, test } from 'bun:test';
import { serializePayrollRequestForWasm } from './decimalBoundary';

describe('WASM Decimal boundary', () => {
  test('serializes monetary fields as exact decimal strings and leaves chronology integers intact', () => {
    const json = serializePayrollRequestForWasm({
      personnelId: 'person-1',
      periodId: '2026-01',
      manualIncome: { tediye: 75000.1, tisIkramiyesi: 0.15 },
      dataset: {
        personnel: [],
        periods: [{ taxYear: 2026, taxMonth: 2 }],
        institutionSettings: {
          '2026-01': { gunlukAsgariUcret: 1101, damgaVergisiOraniBinde: 7.59 },
        },
        attendances: [],
        payrolls: [],
        taxOpenings: [],
        sickLeaveRecords: [],
        annualPayrollParameters: [
          { year: 2026, gelirVergisiDilimleri: [{ limit: 190000, oran: 0.15 }] },
        ],
        zamAylari: [],
      },
    } as never);

    const value = JSON.parse(json) as Record<string, any>;
    expect(value.manualIncome.tediye).toBe('75000.1');
    expect(value.manualIncome.tisIkramiyesi).toBe('0.15');
    expect(value.dataset.institutionSettings['2026-01'].gunlukAsgariUcret).toBe('1101');
    expect(value.dataset.periods[0].taxYear).toBe(2026);
    expect(value.dataset.periods[0].taxMonth).toBe(2);
    expect(value.dataset.annualPayrollParameters[0].gelirVergisiDilimleri[0].limit).toBe(
      '190000'
    );
  });
});
