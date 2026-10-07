import { describe, expect, test } from 'bun:test';
import {
  assertExactDecimalDto,
  isDecimalBoundaryKey,
  isExactDecimalString,
  parsePayrollStorage,
  parseWasmPayrollBoundaryResult,
  parseWasmPayrollResult,
  mergePayrollUiIntoBoundary,
  serializePayrollRequestForWasm,
  serializePayrollStorage,
  toPayrollBoundaryDto,
  toPayrollUiModel,
} from './decimalBoundary';
import { RUST_DECIMAL_KEYS } from './generated/payrollContract';

// Covers direct Decimal and Option<Decimal> fields. Nested DTOs such as
// Option<Vec<DevredenPekKaydi>> and Vec<TaxBracket> are covered below through
// their named child structs and representative recursive fixtures.
const exactFixtures = [
  '0.1',
  '0.15',
  '7.59',
  '21.75',
  '2443.28',
  '999999999.99',
  '0.123456789012345678901',
  '123456789012345678.91',
] as const;

describe('WASM Decimal boundary', () => {
  test('preserves canonical compensation provenance at the typed boundary', () => {
    const request = {
      personnelId: 'p1',
      periodId: '2026-05',
      calculatedAt: '2026-06-10T00:00:00Z',
      dataset: {
        payrolls: [{
          calculationMetadata: { provenance: 'COMPENSATION_POLICY_ENGINE' },
        }],
      },
    } as unknown as Parameters<typeof serializePayrollRequestForWasm>[0];
    const wasmRequest = JSON.parse(serializePayrollRequestForWasm(request));
    expect(wasmRequest.dataset.payrolls[0].calculationMetadata.provenance).toBe('COMPENSATION_POLICY_ENGINE');

    const parsed = parseWasmPayrollBoundaryResult<{ calculationMetadata: { provenance: string } }>(
      JSON.stringify({
        calculationMetadata: {
          provenance: 'COMPENSATION_POLICY_ENGINE',
        },
      })
    );
    expect(parsed.calculationMetadata.provenance).toBe('COMPENSATION_POLICY_ENGINE');
  });

  test('covers every direct Decimal field declared by the shared Rust models', () => {
    expect(RUST_DECIMAL_KEYS.length).toBeGreaterThan(0);
    const missing = RUST_DECIMAL_KEYS.filter((key) => !isDecimalBoundaryKey(key));
    expect(missing).toEqual([]);
  });

  test('covers nested Decimal DTOs without widening their child values to numbers', () => {
    const nestedDto = {
      annualPayrollParameters: [{ gelirVergisiDilimleri: [{ limit: '190000', oran: '0.15' }] }],
      devredenPekGelen: [{ tutar: '123456789012345678.91', kalanAySayisi: 2 }],
      statutorySnapshot: {
        segments: [{ gunlukAsgariUcret: '2443.28', sgkPrimGunSayisi: 30 }],
        pekUstSinir: '999999999.99',
      },
    };

    expect(() => assertExactDecimalDto(nestedDto)).not.toThrow();
    expect(JSON.parse(serializePayrollStorage(nestedDto))).toEqual(nestedDto);
  });

  test('keeps compensation policy state exact through the generated string boundary', () => {
    const uiPolicy = {
      compensationPolicyVersions: [{
        id: 'baseline',
        effectiveFrom: '2026-01-15',
        boundaryScope: 'FULL_EARNING',
        roadRoundingMode: 'PER_COMPENSATION_SLICE',
        state: {
          gunlukTabanUcret: 123.456789,
          gunlukYemek: 10.12345,
          isPrimiYuzde: null,
          isPrimiGruplari: [{ id: 'g1', ad: '1. Grup', oran: 9.125, aktif: true }],
          geceCalismaPrimiYuzde: 0,
          geceCalismaTatiliPrimiYuzde: 0,
          ekOdeme: null,
          digerGelirVarsayilan: 0,
        },
      }],
    };
    const boundary = toPayrollBoundaryDto(uiPolicy);
    expect(boundary.compensationPolicyVersions[0].state.gunlukTabanUcret).toBe('123.456789');
    expect(boundary.compensationPolicyVersions[0].state.isPrimiGruplari[0].oran).toBe('9.125');
    expect(() => assertExactDecimalDto(uiPolicy)).toThrow('JS number kabul edilmez');
    assertExactDecimalDto(boundary);
  });

  test('keeps payroll calculation snapshot inputs exact across the typed boundary', () => {
    const uiSnapshot = {
      calculationMetadata: {
        provenance: 'COMPENSATION_POLICY_ENGINE',
        snapshot: {
          snapshotSchemaVersion: 2,
          calculationVersion: 'compensation-engine-v1',
          compensation: {
            segments: [{
              versionId: 'policy-v1',
              startDate: '2026-05-15',
              endDate: '2026-06-14',
              boundaryScope: 'FULL_EARNING',
              roadRoundingMode: 'PER_COMPENSATION_SLICE',
              state: {
                gunlukTabanUcret: 2443.281234567,
                gunlukYemek: 0,
                birlestirilmisSosyalYardim: 0,
                gunlukVasitaYol: 0,
                giyimYardimi: 0,
                hizmetZammiBirimi: 0,
                isPrimiYuzde: null,
                isPrimiGruplari: [{ id: 'g1', ad: '1. Grup', oran: 10, aktif: true }],
                geceCalismaPrimiYuzde: 0,
                geceCalismaTatiliPrimiYuzde: 0,
                ekOdeme: null,
                digerGelirVarsayilan: null,
              },
            }],
          },
          usedInputs: {
            personnelGroup: 'g1',
            serviceYear: 4,
            manualIncome: { supplied: true, tediye: 0, tisIkramiyesi: null },
            accrualGrossAmount: 0.123456789012345678901,
            statutoryParameters: { gunlukAsgariUcret: 1000 },
          },
        },
      },
    };
    const boundary = toPayrollBoundaryDto(uiSnapshot);
    expect(boundary.calculationMetadata.snapshot.usedInputs.manualIncome.tediye).toBe('0');
    expect(boundary.calculationMetadata.snapshot.usedInputs.accrualGrossAmount).toBe(
      '0.12345678901234568'
    );
    expect(boundary.calculationMetadata.snapshot.compensation.segments[0].state.gunlukTabanUcret)
      .toBe('2443.281234567');
    expect(() => assertExactDecimalDto(boundary)).not.toThrow();

    const exactBoundaryInput = {
      accrualGrossAmount: '0.123456789012345678901',
    };
    expect(toPayrollBoundaryDto(exactBoundaryInput).accrualGrossAmount).toBe(
      '0.123456789012345678901'
    );
  });

  test('serializes a schema-derived representative DTO with string Decimals', () => {
    const representativeDto = Object.fromEntries(
      RUST_DECIMAL_KEYS.map((key) => [key, '1234.56'])
    );
    const serialized = JSON.parse(serializePayrollStorage(representativeDto)) as Record<
      string,
      unknown
    >;

    for (const key of RUST_DECIMAL_KEYS) {
      expect(typeof serialized[key]).toBe('string');
    }
  });

  test('preserves exact fixture strings through WASM result, storage reload, and next request', () => {
    for (const fixture of exactFixtures) {
      const wasmResult = parseWasmPayrollBoundaryResult<{ netOdeme: string; pekDetay: { finalPek: string } }>(
        JSON.stringify({ netOdeme: fixture, pekDetay: { finalPek: fixture } })
      );
      expect(wasmResult.netOdeme).toBe(fixture);
      expect(wasmResult.pekDetay.finalPek).toBe(fixture);

      const storageJson = serializePayrollStorage({
        backupVersion: 2,
        bordrolar: [{ netOdeme: wasmResult.netOdeme, pekDetay: wasmResult.pekDetay }],
      });
      const reloaded = parsePayrollStorage<{
        bordrolar: Array<{ netOdeme: string; pekDetay: { finalPek: string } }>;
      }>(storageJson);
      expect(reloaded.bordrolar[0].netOdeme).toBe(fixture);
      expect(reloaded.bordrolar[0].pekDetay.finalPek).toBe(fixture);

      const requestJson = serializePayrollRequestForWasm({
        personnelId: 'person-1',
        periodId: '2026-01',
        calculatedAt: '2026-09-03T00:00:00.000Z',
        manualIncome: { tediye: fixture, tisIkramiyesi: null },
        dataset: {
          personnel: [],
          periods: [],
          institutionSettings: {},
          attendances: [],
          payrolls: reloaded.bordrolar,
          taxOpenings: [],
          sickLeaveRecords: [],
          annualPayrollParameters: [],
        },
      } as never);
      expect(JSON.parse(requestJson).manualIncome.tediye).toBe(fixture);
    }
  });

  test('encodes normal UI inputs, but the production request serializer rejects numeric Decimal fields', () => {
    const encoded = toPayrollBoundaryDto({ amount: 0.15, taxMonth: 3 });
    expect(encoded).toEqual({ amount: '0.15', taxMonth: 3 });
    expect(toPayrollBoundaryDto({ amount: 1e-7 })).toEqual({ amount: '0.0000001' });
    expect(() =>
      serializePayrollRequestForWasm({
        manualIncome: { tediye: 0.1 },
      } as never)
    ).toThrow('JS number kabul edilmez');
  });

  test('accepts the intended Decimal text grammar and rejects binary/scientific output', () => {
    expect(exactFixtures.every((value) => isExactDecimalString(value))).toBe(true);
    expect(isExactDecimalString('0.12345678901234568')).toBe(true);
    expect(isExactDecimalString('1e-7')).toBe(false);
    expect(isExactDecimalString('NaN')).toBe(false);
  });

  test('UI adapter is the only place that turns exact Decimal text into numbers', () => {
    const uiModel = toPayrollUiModel({ netOdeme: '999999999.99', taxMonth: 3 }) as unknown as {
      netOdeme: number;
      taxMonth: number;
    };
    expect(uiModel.netOdeme).toBe(999999999.99);
    expect(uiModel.taxMonth).toBe(3);

    const boundary = parseWasmPayrollResult<{ netOdeme: string }>(
      JSON.stringify({ netOdeme: '0.123456789012345678901' })
    );
    expect(boundary.netOdeme).toBe('0.123456789012345678901');
  });

  test('UI projection round-trips unchanged long Decimals without rewriting their text', () => {
    const exact = {
      netOdeme: '0.123456789012345678901',
      pekDetay: { finalPek: '123456789012345678.91' },
    };
    const ui = toPayrollUiModel(exact);
    const roundTripped = mergePayrollUiIntoBoundary(exact, ui);

    expect(roundTripped).toEqual(exact);
  });
});
