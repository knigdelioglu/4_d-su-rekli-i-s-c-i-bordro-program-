import { expect, test } from 'bun:test';
import {
  assertPayrollCalculationSnapshotCurrent,
  assertPayrollCalculationSnapshotCurrentForEngine,
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

test('keeps the snapshot guard for browser calculations but relies on the native transaction for Tauri', () => {
  const calculationSnapshot = {} as PayrollDatasetSnapshot;
  const refreshedSnapshot = {} as PayrollDatasetSnapshot;

  expect(() =>
    assertPayrollCalculationSnapshotCurrentForEngine(
      'wasm',
      calculationSnapshot,
      refreshedSnapshot
    )
  ).toThrow('Eski hesaplama kaydedilmedi');
  expect(() =>
    assertPayrollCalculationSnapshotCurrentForEngine(
      'tauri',
      calculationSnapshot,
      refreshedSnapshot
    )
  ).not.toThrow();
});
