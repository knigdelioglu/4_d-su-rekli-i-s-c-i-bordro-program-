import { describe, expect, test } from 'bun:test';
import wasmInit, {
  validate_annual_payroll_parameters_json,
} from '../../wasm/pkg/payroll_wasm.js';
import { validateAnnualPayrollParameters } from './payrollPayloadSchema';
import { annualPayrollParameterSemanticIssue } from './annualPayrollParametersValidation';

type AnnualParameters = {
  year: number;
  gelirVergisiDilimleri: Array<{ limit: string; oran: string }>;
  sigortaGvYillikBrutAsgariUcretTavani?: string | null;
  updatedAt?: string | null;
};

const valid2026: AnnualParameters = {
  year: 2026,
  gelirVergisiDilimleri: [
    { limit: '190000', oran: '0.15' },
    { limit: '400000', oran: '0.20' },
    { limit: '1500000', oran: '0.27' },
    { limit: '5300000', oran: '0.35' },
    { limit: '1000000000000000', oran: '0.40' },
  ],
  sigortaGvYillikBrutAsgariUcretTavani: '396360.00',
  updatedAt: null,
};

function clone(value: AnnualParameters): AnnualParameters {
  return JSON.parse(JSON.stringify(value)) as AnnualParameters;
}

function browserAccepts(value: AnnualParameters): boolean {
  try {
    validateAnnualPayrollParameters(value);
    return true;
  } catch {
    return false;
  }
}

describe('annual payroll parameter semantic parity', () => {
  test('browser persistence validator and Rust/WASM accept or reject the same matrix', async () => {
    const cases: Array<{ name: string; value: AnnualParameters; expected: boolean }> = [
      {
        name: 'empty bracket list',
        value: { ...clone(valid2026), gelirVergisiDilimleri: [] },
        expected: false,
      },
      { name: 'year zero', value: { ...clone(valid2026), year: 0 }, expected: false },
      { name: 'negative year', value: { ...clone(valid2026), year: -1 }, expected: false },
      {
        name: 'equal bracket limits',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[1].limit = value.gelirVergisiDilimleri[0].limit;
          return value;
        })(),
        expected: false,
      },
      {
        name: 'descending bracket limits',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[1].limit = '100';
          return value;
        })(),
        expected: false,
      },
      {
        name: 'nonpositive bracket limit',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[0].limit = '0';
          return value;
        })(),
        expected: false,
      },
      {
        name: 'negative rate',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[0].oran = '-0.01';
          return value;
        })(),
        expected: false,
      },
      {
        name: 'rate above one',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[0].oran = '1.01';
          return value;
        })(),
        expected: false,
      },
      {
        name: 'persisted upper limit overflow',
        value: (() => {
          const value = clone(valid2026);
          value.gelirVergisiDilimleri[0].limit = '1000000000000001';
          return value;
        })(),
        expected: false,
      },
      {
        name: 'insurance cap zero',
        value: { ...clone(valid2026), sigortaGvYillikBrutAsgariUcretTavani: '0' },
        expected: false,
      },
      {
        name: 'insurance cap negative',
        value: { ...clone(valid2026), sigortaGvYillikBrutAsgariUcretTavani: '-1' },
        expected: false,
      },
      { name: 'valid 2026 parameters', value: valid2026, expected: true },
    ];

    await wasmInit();
    for (const testCase of cases) {
      const browser = browserAccepts(testCase.value);
      const semanticIssue = annualPayrollParameterSemanticIssue(testCase.value);
      const wasmAccepted = (() => {
        try {
          validate_annual_payroll_parameters_json(JSON.stringify(testCase.value));
          return true;
        } catch {
          return false;
        }
      })();

      expect(browser).toBe(testCase.expected);
      expect(wasmAccepted).toBe(testCase.expected);
      expect(browser).toBe(wasmAccepted);
      expect(semanticIssue === null).toBe(testCase.expected);
    }
  });
});
