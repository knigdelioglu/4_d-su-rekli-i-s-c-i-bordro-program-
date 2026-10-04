import { expect, test } from 'bun:test';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from './payrollDefaults';
import { validateInstitutionSettings } from './payrollPayloadSchema';
import { transportRateForNewPeriod, validateTransportRatesForPeriod } from './transportRatesValidation';
import { toPayrollBoundaryDto, serializePayrollStorage, parsePayrollStorage } from '../payrollEngine/decimalBoundary';

const period = { baslangicTarihi: '2026-08-15', bitisTarihi: '2026-09-14' };
const settings = { ...DEFAULT_PRODUCTION_KURUM_DEGERLERI, donemId: '2026-08', gunlukVasitaYol: 100,
  transportRateChanges: [{ effectiveFrom: '2026-08-20', gunlukVasitaYol: 120 }, { effectiveFrom: '2026-09-05', gunlukVasitaYol: 140 }] };

test('dated transport preserves exact amounts through storage and accepts legacy missing/null', () => {
  const dto = toPayrollBoundaryDto(settings);
  const restored = parsePayrollStorage<typeof dto>(serializePayrollStorage(dto));
  expect(restored.transportRateChanges[0].gunlukVasitaYol).toBe('120');
  expect(restored).toEqual(dto);
  validateInstitutionSettings(restored);
  for (const changes of [undefined, null, []]) {
    validateInstitutionSettings({ ...restored, transportRateChanges: changes });
  }
});

test('transport validation rejects dates outside period, duplicates, wrong order and negative amounts', () => {
  validateTransportRatesForPeriod(period, settings);
  for (const effectiveFrom of ['', '2026-02-30', '2026-08-14', '2026-09-15']) {
    expect(() => validateTransportRatesForPeriod(period, { transportRateChanges: [{ effectiveFrom, gunlukVasitaYol: 120 }] })).toThrow();
  }
  expect(() => validateTransportRatesForPeriod(period, { transportRateChanges: [...settings.transportRateChanges].reverse() })).toThrow();
  expect(() => validateTransportRatesForPeriod(period, { transportRateChanges: [settings.transportRateChanges[0], settings.transportRateChanges[0]] })).toThrow();
  expect(() => validateTransportRatesForPeriod(period, { transportRateChanges: [{ effectiveFrom: '2026-08-20', gunlukVasitaYol: -1 }] })).toThrow();
  expect(() => validateInstitutionSettings({ ...toPayrollBoundaryDto(settings), transportRateChanges: [{ effectiveFrom: '2026-02-30', gunlukVasitaYol: '100' }] })).toThrow();
});

test('new period inherits last transport rate including zero, without copying dated rows', () => {
  expect(transportRateForNewPeriod(settings, '2026-09-15')).toBe(140);
  expect(transportRateForNewPeriod(settings, '2026-08-15')).toBe(100);
  expect(transportRateForNewPeriod({ ...settings, transportRateChanges: [] }, '2026-09-15')).toBe(100);
  expect(transportRateForNewPeriod({ ...settings, transportRateChanges: [{ effectiveFrom: '2026-08-20', gunlukVasitaYol: 0 }] }, '2026-09-15')).toBe(0);
});
