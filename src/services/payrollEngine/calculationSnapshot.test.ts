import { expect, test } from 'bun:test';
import {
  assertPayrollCalculationSnapshotCurrent,
} from './calculationSnapshot';
import type { PayrollDatasetSnapshot } from './types';

test('rejects a payroll result when its source dataset changed during calculation', () => {
  const original = {} as PayrollDatasetSnapshot;
  const changed = {} as PayrollDatasetSnapshot;

  expect(() => assertPayrollCalculationSnapshotCurrent(original, original)).not.toThrow();
  expect(() => assertPayrollCalculationSnapshotCurrent(original, changed)).toThrow(
    'Eski hesaplama kaydedilmedi'
  );
});
