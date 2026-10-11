import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { deletePeriodFromPayload } from './periodDeletion';
import { parseImportedBackup, parseCurrentBrowserSnapshot } from './payrollPayload';
import { serializePayrollStorage } from '../payrollEngine/decimalBoundary';
import type { MutationImpact } from '../payrollEngine';

const fixture = () => parseImportedBackup(readFileSync('src-tauri/tests/fixtures/session3-atomic-reference.json', 'utf8'));
const emptyImpact: MutationImpact = {
  affectedPayrolls: [], blockedByFinalized: [], affectedRetroBatches: [], blockedByFinalizedRetroBatches: [],
};

test('active period deletion leaves a valid persisted snapshot and preserves personnel', () => {
  const original = fixture();
  original.donemler.reverse();
  const result = deletePeriodFromPayload(original, original.aktifDonemId, emptyImpact);
  const restored = parseCurrentBrowserSnapshot(serializePayrollStorage(result));
  expect(restored.donemler.length).toBe(original.donemler.length - 1);
  expect(restored.bordrolar.length).toBe(0);
  expect(restored.personeller).toEqual(original.personeller);
  expect(restored.annualPayrollParameters).toEqual(original.annualPayrollParameters);
  expect(restored.aktifDonemId).toBe('2025-12');
  expect(original.donemler.length).toBe(9);
});

test('deleting one tax opening period clears only its independent opening component', () => {
  const original = fixture();
  original.taxOpenings = [{
    id: 'opening-1',
    personnelId: original.personeller[0].id,
    year: 2026,
    gvCumulativeOpening: '120000',
    effectiveFromPeriodId: '2026-01',
    asgariGvCumulativeOpening: '60000',
    asgariGvEffectiveFromPeriodId: '2026-03',
  }];

  const result = deletePeriodFromPayload(original, '2026-01', emptyImpact);
  expect(result.taxOpenings.length).toBe(1);
  expect(result.taxOpenings[0].gvCumulativeOpening).toBe(null);
  expect(result.taxOpenings[0].effectiveFromPeriodId).toBe(null);
  expect(result.taxOpenings[0].asgariGvCumulativeOpening).toBe('60000');
  expect(result.taxOpenings[0].asgariGvEffectiveFromPeriodId).toBe('2026-03');
  parseCurrentBrowserSnapshot(serializePayrollStorage(result));
});

test('deleting a period is rejected when downstream finalized records would be affected', () => {
  const original = fixture();
  original.bordrolar[0].status = 'FINALIZED';
  const payroll = original.bordrolar[0];
  const impact: MutationImpact = {
    ...emptyImpact,
    blockedByFinalized: [{ personnelId: payroll.personelId, periodId: payroll.donemId, accrualId: payroll.accrualId }],
  };
  expect(() => deletePeriodFromPayload(original, '2025-12', impact)).toThrow(
    'Dönem silme işlemi sonraki kesinleştirilmiş bordro veya retro kayıtlarını etkilediği için izin verilmez.'
  );
  expect(original.bordrolar[0].status).toBe('FINALIZED');
});

test('deleting a period containing a finalized payroll is rejected', () => {
  const original = fixture();
  original.bordrolar[0].status = 'FINALIZED';
  expect(() => deletePeriodFromPayload(original, original.bordrolar[0].donemId, emptyImpact)).toThrow(
    'Kesinleştirilmiş (FINALIZED) bordro içeren dönem silinemez.'
  );
  expect(original.bordrolar[0].status).toBe('FINALIZED');
});

test('deleting a period linked to finalized retro history preserves its batch and allocations', () => {
  const original = fixture();
  const periodId = original.donemler[0].id;
  original.retroBatches = [{
    id: 'final-retro-batch',
    revisionId: 'revision-1',
    personnelId: original.personeller[0].id,
    paymentDate: '2026-01-30',
    status: 'FINALIZED',
    settlementStatus: 'SETTLED_BY_OFFSET',
    totalGrossDelta: '100',
    payableSettlementAmount: '100',
    offsetSettlementAmount: '0',
    recoveredAmount: '0',
    recoverableAmount: '0',
    outstandingReceivable: '0',
  } as typeof original.retroBatches[number]];
  original.retroAllocations = [{
    id: 'final-retro-allocation',
    batchId: 'final-retro-batch',
    personnelId: original.personeller[0].id,
    sourcePeriodId: periodId,
    earningCode: 'BASE_WAGE',
    originalRecognizedAmount: '0',
    previousAuthoritativeRetroAmount: '0',
    targetAmount: '100',
    deltaAmount: '100',
    sgkTreatment: 'WAGE_SOURCE_MONTH',
    incomeTaxTreatment: 'TAXABLE',
    stampTaxTreatment: 'TAXABLE',
  } as typeof original.retroAllocations[number]];
  const batchesBefore = structuredClone(original.retroBatches);
  const allocationsBefore = structuredClone(original.retroAllocations);

  expect(() => deletePeriodFromPayload(original, periodId, emptyImpact)).toThrow(
    'Kesinleştirilmiş (FINALIZED) geriye dönük fark içeren dönem silinemez.'
  );
  expect(original.retroBatches).toEqual(batchesBefore);
  expect(original.retroAllocations).toEqual(allocationsBefore);
  expect(original.donemler.some((period) => period.id === periodId)).toBe(true);
});

test('last period clears selection and missing periods fail visibly', () => {
  const original = fixture();
  let result = original;
  for (const period of original.donemler) result = deletePeriodFromPayload(result, period.id, emptyImpact);
  expect(result.aktifDonemId).toBe('');
  expect(result.donemler).toEqual([]);
  expect(result.puantajlar).toEqual([]);
  expect(result.kurumDegerleriMap).toEqual({});
  parseCurrentBrowserSnapshot(serializePayrollStorage(result));
  expect(() => deletePeriodFromPayload(result, 'missing', emptyImpact)).toThrow('Silinecek dönem bulunamadı.');
});
