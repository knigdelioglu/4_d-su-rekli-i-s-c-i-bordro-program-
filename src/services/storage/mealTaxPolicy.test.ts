import { expect, test } from 'bun:test';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from './payrollDefaults';
import { validateInstitutionSettings } from './payrollPayloadSchema';
import { toPayrollBoundaryDto, serializePayrollStorage, parsePayrollStorage } from '../payrollEngine/decimalBoundary';

test('meal policy defaults checked and preserves an explicit false in storage JSON', () => {
  expect(DEFAULT_PRODUCTION_KURUM_DEGERLERI.yemekTamGvIstisnaDamgaTabi).toBe(true);
  const settings = toPayrollBoundaryDto({ donemId: '2026-08', ...DEFAULT_PRODUCTION_KURUM_DEGERLERI });
  for (const selected of [true, false]) {
    const value = { ...settings, yemekTamGvIstisnaDamgaTabi: selected };
    const restored = parsePayrollStorage<typeof value>(serializePayrollStorage(value));
    expect(() => validateInstitutionSettings(restored)).not.toThrow();
    expect(restored.yemekTamGvIstisnaDamgaTabi).toBe(selected);
  }
  const legacy = { ...settings };
  delete legacy.yemekTamGvIstisnaDamgaTabi;
  expect(() => validateInstitutionSettings(legacy)).not.toThrow();
  for (const invalid of ['true', 1, null]) {
    expect(() => validateInstitutionSettings({ ...settings, yemekTamGvIstisnaDamgaTabi: invalid })).toThrow();
  }
});

test('old backup preserves the recorded daily-limited policy without changing its financial history', async () => {
  const { readFile } = await import('node:fs/promises');
  const { parseImportedBackup } = await import('./payrollPayload');
  const source = JSON.parse(await readFile(new URL('../../../tests/fixtures/session3-reference.json', import.meta.url), 'utf8'));
  const restored = parseImportedBackup(JSON.stringify(source));
  const historical = new Set([
    ...source.bordrolar.map((payroll: { donemId: string }) => payroll.donemId),
    ...source.retroAllocations.map((allocation: { sourcePeriodId: string }) => allocation.sourcePeriodId),
  ]);
  for (const periodId of historical) {
    expect(restored.kurumDegerleriMap[periodId as string].yemekTamGvIstisnaDamgaTabi).toBe(false);
  }
  expect(restored.bordrolar).toEqual(source.bordrolar);
  expect(restored.retroAllocations).toEqual(source.retroAllocations);
  expect(parseImportedBackup(serializePayrollStorage(restored))).toEqual(restored);
});
