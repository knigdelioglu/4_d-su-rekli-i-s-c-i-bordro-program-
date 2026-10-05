import type { BordroDonemi } from '../../types/payroll';

/** Select the closest configured period that begins before the new period. */
export function findPeriodSettingsInheritanceSource(
  periods: readonly BordroDonemi[],
  settingsByPeriod: Readonly<Record<string, unknown>>,
  newPeriodStartDate: string
): BordroDonemi | undefined {
  return periods
    .filter(
      (period) =>
        period.baslangicTarihi < newPeriodStartDate &&
        settingsByPeriod[period.id] !== undefined
    )
    .sort(
      (left, right) =>
        right.baslangicTarihi.localeCompare(left.baslangicTarihi) ||
        right.yil - left.yil ||
        right.ay - left.ay ||
        left.id.localeCompare(right.id)
    )[0];
}
