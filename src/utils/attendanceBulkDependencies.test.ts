import { expect, test } from 'bun:test';
import { previewAttendanceBulkDependencies } from './attendanceBulkDependencies';
import type { PayrollDatasetSnapshot, PayrollEngine } from '../services/payrollEngine';
test('attendance preview separates eligible people from finalized payroll and retro dependencies', async () => {
  const engine = { evaluateMutationPolicy: async (mutation) => ({
    affectedPayrolls: [], affectedRetroBatches: ['recalc'],
    blockedByFinalized: mutation.personnelId === 'a' ? [{ periodId: 'later' }] : [],
    blockedByFinalizedRetroBatches: mutation.personnelId === 'b' ? ['locked-retro'] : [],
  }) } as unknown as PayrollEngine;
  const dataset = { periods: [{ id: 'later', donemAdi: 'Ocak 2028' }], retroBatches: [] } as unknown as PayrollDatasetSnapshot;
  const rows = await previewAttendanceBulkDependencies(engine, dataset, 'period', ['a', 'b', 'c']);
  expect(rows.map((row) => row.blocked)).toEqual([true, true, false]);
  expect(rows[0].message).toContain('Ocak 2028');
  expect(rows[1].message).toContain('locked-retro');
  expect(rows[2].message).toContain('1 retro');
});
