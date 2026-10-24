import { describe, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';

describe('attendance calendar dates across device time zones', () => {
  for (const zone of ['Europe/Istanbul', 'America/Havana', 'America/Santiago', 'Pacific/Apia']) {
    test(`keeps every date and weekday in ${zone}`, () => {
      const script = `
        import { getPeriodDaysList as presentation } from ${JSON.stringify(new URL('./payrollPresentation.ts', import.meta.url).href)};
        import { getPeriodDaysList as legacy } from ${JSON.stringify(new URL('./payrollUtils.ts', import.meta.url).href)};
        console.log(JSON.stringify([
          ['2026-02-15', '2026-03-14'],
          ['2026-08-15', '2026-09-14'],
          ['2024-02-15', '2024-03-14'],
          ['2011-12-15', '2012-01-14'],
        ].map(([start, end]) => ({ start, end, days: presentation(start, end), legacy: legacy(start, end) }))));
      `;
      const result = spawnSync(process.execPath, ['-e', script], {
        env: { TZ: zone }, encoding: 'utf8', timeout: 5000, maxBuffer: 128 * 1024,
      });
      expect(result.status).toBe(0);
      const periods = JSON.parse(result.stdout) as Array<{
        start: string; end: string;
        days: Array<{ dateStr: string; dayOfWeek: number; dayNumber: number; isWeekend: boolean; isSunday: boolean }>;
        legacy: unknown;
      }>;
      for (const period of periods) {
        const expectedCount = (Date.parse(period.end + 'T00:00:00Z') - Date.parse(period.start + 'T00:00:00Z')) / 86400000 + 1;
        expect(period.days).toHaveLength(expectedCount);
        expect(period.days[0].dateStr).toBe(period.start);
        expect(period.days.at(-1)?.dateStr).toBe(period.end);
        expect(new Set(period.days.map((day) => day.dateStr)).size).toBe(expectedCount);
        expect(period.legacy).toEqual(period.days);
        for (const day of period.days) {
          const date = new Date(day.dateStr + 'T00:00:00Z');
          expect(day.dayOfWeek).toBe(date.getUTCDay());
          expect(day.dayNumber).toBe(date.getUTCDate());
          expect(day.isSunday).toBe(day.dayOfWeek === 0);
          expect(day.isWeekend).toBe(day.dayOfWeek === 0 || day.dayOfWeek === 6);
        }
      }
    });
  }
});
