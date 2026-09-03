import { describe, expect, test } from 'bun:test';
import {
  parsePayrollBoundaryJson,
  parsePayrollStorage,
  parseWasmPayrollResult,
  serializePayrollRequestForWasm,
  serializePayrollStorage,
} from './decimalBoundary';

describe('WASM Decimal boundary', () => {
  test('serializes monetary fields as exact decimal strings and leaves chronology integers intact', () => {
    const json = serializePayrollRequestForWasm({
      personnelId: 'person-1',
      periodId: '2026-01',
      calculatedAt: '2026-09-03T00:00:00.000Z',
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

  test('round-trips representative monetary values without a binary number at the boundary', () => {
    const request = {
      calculatedAt: '2026-09-03T00:00:00.000Z',
      manualIncome: { tediye: 0.1, tisIkramiyesi: 0.15 },
      dataset: {
        institutionSettings: {
          '2026-01': {
            gunlukTabanUcret: 75000.1,
            damgaVergisiOraniBinde: 7.59,
          },
        },
        payrolls: [
          {
            gelirToplam: 999999999.99,
            kesintiToplam: 7.59,
            netOdeme: 999999992.4,
          },
        ],
      },
    };

    const encoded = JSON.parse(serializePayrollRequestForWasm(request as never)) as any;
    expect(encoded.manualIncome.tediye).toBe('0.1');
    expect(encoded.manualIncome.tisIkramiyesi).toBe('0.15');
    expect(encoded.dataset.institutionSettings['2026-01'].gunlukTabanUcret).toBe('75000.1');
    expect(encoded.dataset.payrolls[0].gelirToplam).toBe('999999999.99');
    expect(encoded.dataset.payrolls[0].kesintiToplam).toBe('7.59');

    const boundary = parsePayrollBoundaryJson(
      JSON.stringify({
        netOdeme: '999999999.99',
        devredenPekGelen: [{ tutar: '7.59' }],
        incomeItem: { amount: '0.15' },
        taxMonth: 3,
      })
    ) as any;
    expect(boundary.netOdeme).toBe('999999999.99');
    expect(boundary.taxMonth).toBe(3);
    expect((boundary.devredenPekGelen as any)[0].tutar).toBe('7.59');
    expect((boundary.incomeItem as any).amount).toBe('0.15');
    expect(JSON.parse(serializePayrollStorage({ amount: 0.15 })).amount).toBe('0.15');
  });

  test('UI compatibility decoding does not change the Decimal-safe storage contract', () => {
    const uiModel = parsePayrollStorage<{ netOdeme: number; taxMonth: number }>(
      '{"netOdeme":"999999999.99","taxMonth":3}'
    );
    expect(uiModel.netOdeme).toBe(999999999.99);
    expect(JSON.parse(serializePayrollStorage(uiModel))).toEqual({
      netOdeme: '999999999.99',
      taxMonth: 3,
    });

    const payroll = parseWasmPayrollResult(
      JSON.stringify({
        id: 'p-1_2026-01',
        personelId: 'p-1',
        donemId: '2026-01',
        gelirler: { tediye: '0.15' },
        gelirToplam: '0.15',
        kesintiler: {},
        kesintiToplam: '0.00',
        netOdeme: '0.15',
        status: 'CALCULATED',
      })
    );
    const persisted = JSON.parse(serializePayrollStorage(payroll)) as any;
    expect(persisted.gelirler.tediye).toBe('0.15');
    expect(persisted.netOdeme).toBe('0.15');
  });
});
