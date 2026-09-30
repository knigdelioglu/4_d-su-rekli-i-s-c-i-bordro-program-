import { describe, expect, test } from 'bun:test';
import { getInitialDataset } from './sampleData';

describe('getInitialDataset', () => {
  test('starts the sample tax year with its preceding December reference period', () => {
    const dataset = getInitialDataset();
    const currentYear = new Date().getFullYear();
    const referenceId = `${currentYear - 1}-12`;
    const reference = dataset.donemler.find((period) => period.id === referenceId);

    expect(reference?.yil).toBe(currentYear - 1);
    expect(reference?.ay).toBe(12);
    expect(reference?.taxYear).toBe(currentYear);
    expect(reference?.taxMonth).toBe(1);
    expect(dataset.kurumDegerleriMap[referenceId]?.donemId).toBe(referenceId);
    expect(dataset.puantajlar.some((row) => row.donemId === referenceId)).toBe(true);

    for (let month = 1; month <= 8; month += 1) {
      const period = dataset.donemler.find(
        (item) => item.id === `${currentYear}-${String(month).padStart(2, '0')}`
      );
      expect(period?.taxYear).toBe(currentYear);
      expect(period?.taxMonth).toBe(month + 1);
    }
  });
});
