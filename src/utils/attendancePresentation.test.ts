import { describe, expect, test } from 'bun:test';
import { calculatePuantajOzeti, generateDefaultPuantajGunler, getPeriodDaysList } from './payrollPresentation';

describe('4/D varsayılan puantaj sunumu', () => {
  test('Cumartesi ve Pazar T, hafta içi Ç olarak önerilir', () => {
    const gunler = generateDefaultPuantajGunler('2026-09-01', '2026-09-07');
    const days = getPeriodDaysList('2026-09-01', '2026-09-07');
    const summary = calculatePuantajOzeti(gunler);

    expect(summary.Ç).toBe(5);
    expect(summary.T).toBe(2);
    expect(days.filter((day) => day.isWeekend).map((day) => gunler[day.dateStr])).toEqual(['T', 'T']);
  });
});
