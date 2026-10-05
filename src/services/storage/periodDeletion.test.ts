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

test('deleting an inactive prior period invalidates affected finalized history without changing selection', () => {
  const original = fixture();
  original.bordrolar[0].status = 'FINALIZED';
  const payroll = original.bordrolar[0];
  const impact: MutationImpact = {
    ...emptyImpact,
    blockedByFinalized: [{ personnelId: payroll.personelId, periodId: payroll.donemId, accrualId: payroll.accrualId }],
  };
  const result = deletePeriodFromPayload(original, '2025-12', impact);
  expect(result.aktifDonemId).toBe(original.aktifDonemId);
  expect(result.bordrolar[0].status).toBe('STALE');
  expect(result.bordrolar[1].status).toBe(original.bordrolar[1].status);
  parseCurrentBrowserSnapshot(serializePayrollStorage(result));
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
