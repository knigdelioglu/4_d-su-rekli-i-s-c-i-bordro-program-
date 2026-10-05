import { expect, test } from 'bun:test';
import { createBordroDonemi } from '../../utils/payrollPresentation';
import { findPeriodSettingsInheritanceSource } from './periodInheritance';

test('period settings inherit from the closest configured period before the new date', () => {
  const march = createBordroDonemi(2026, 3);
  const december = createBordroDonemi(2026, 12);
  const january = createBordroDonemi(2027, 1);
  const source = findPeriodSettingsInheritanceSource(
    [march, january, december],
    { [march.id]: {}, [december.id]: {}, [january.id]: {} },
    january.baslangicTarihi
  );

  expect(source?.id).toBe(december.id);
});

test('periods after a backdated new period are never inherited', () => {
  const january = createBordroDonemi(2026, 1);
  const march = createBordroDonemi(2026, 3);
  const source = findPeriodSettingsInheritanceSource(
    [march],
    { [march.id]: {} },
    january.baslangicTarihi
  );

  expect(source === undefined).toBe(true);
});
