import { describe, expect, test } from 'bun:test';
import type { PersonelPuantaj, SickLeaveRecord } from '../types/payroll';
import { createBordroDonemi } from './payrollPresentation';
import {
  findSickLeaveConflicts,
  syncPuantajForSickLeaveDelete,
  syncPuantajForSickLeaveSave,
} from './sickLeaveSync';

describe('sickLeaveSync', () => {
  const period1 = createBordroDonemi(2025, 12, 2026, 1);
  const period2 = createBordroDonemi(2026, 1, 2026, 2);
  const donemler = [period1, period2];

  test('detects conflicts across 15-14 period boundary when dates have Ç or T', () => {
    const puantaj1: PersonelPuantaj = {
      id: 'p1_2025-12',
      personelId: 'p1',
      donemId: '2025-12',
      gunler: {
        '2026-01-12': 'Ç',
        '2026-01-13': 'Ç',
        '2026-01-14': 'Ç',
      },
    };

    const puantaj2: PersonelPuantaj = {
      id: 'p1_2026-01',
      personelId: 'p1',
      donemId: '2026-01',
      gunler: {
        '2026-01-15': 'Ç',
        '2026-01-16': 'Ç',
        '2026-01-17': 'T',
      },
    };

    const conflicts = findSickLeaveConflicts({
      personnelId: 'p1',
      startDate: '2026-01-13',
      endDate: '2026-01-16',
      donemler,
      puantajlar: [puantaj1, puantaj2],
    });

    expect(conflicts.length).toBe(4);
    expect(conflicts.map((c) => c.date)).toEqual([
      '2026-01-13',
      '2026-01-14',
      '2026-01-15',
      '2026-01-16',
    ]);
    expect(conflicts.map((c) => c.currentCode)).toEqual(['Ç', 'Ç', 'Ç', 'Ç']);
    expect(conflicts.map((c) => c.periodId)).toEqual([
      '2025-12',
      '2025-12',
      '2026-01',
      '2026-01',
    ]);
  });

  test('does not report conflict if dates are already R or not set', () => {
    const puantaj1: PersonelPuantaj = {
      id: 'p1_2025-12',
      personelId: 'p1',
      donemId: '2025-12',
      gunler: {
        '2026-01-12': 'R',
        '2026-01-13': 'R',
      },
    };

    const conflicts = findSickLeaveConflicts({
      personnelId: 'p1',
      startDate: '2026-01-12',
      endDate: '2026-01-13',
      donemler,
      puantajlar: [puantaj1],
    });

    expect(conflicts.length).toBe(0);
  });

  test('syncPuantajForSickLeaveSave sets R across 15-14 period boundary', () => {
    const puantaj1: PersonelPuantaj = {
      id: 'p1_2025-12',
      personelId: 'p1',
      donemId: '2025-12',
      gunler: {
        '2026-01-13': 'Ç',
        '2026-01-14': 'Ç',
      },
    };

    const puantaj2: PersonelPuantaj = {
      id: 'p1_2026-01',
      personelId: 'p1',
      donemId: '2026-01',
      gunler: {
        '2026-01-15': 'Ç',
        '2026-01-16': 'Ç',
      },
    };

    const record: SickLeaveRecord = {
      id: 'sick1',
      personnelId: 'p1',
      startDate: '2026-01-14',
      endDate: '2026-01-15',
    };

    const updated = syncPuantajForSickLeaveSave({
      record,
      donemler,
      puantajlar: [puantaj1, puantaj2],
    });

    const p1Updated = updated.find((p) => p.donemId === '2025-12');
    const p2Updated = updated.find((p) => p.donemId === '2026-01');

    expect(p1Updated?.gunler['2026-01-13']).toBe('Ç');
    expect(p1Updated?.gunler['2026-01-14']).toBe('R');

    expect(p2Updated?.gunler['2026-01-15']).toBe('R');
    expect(p2Updated?.gunler['2026-01-16']).toBe('Ç');
  });

  test('syncPuantajForSickLeaveDelete restores R to default, but preserves manual changes', () => {
    // 2026-01-17 is Saturday (weekend), 2026-01-16 is Friday (weekday)
    const puantaj: PersonelPuantaj = {
      id: 'p1_2026-01',
      personelId: 'p1',
      donemId: '2026-01',
      gunler: {
        '2026-01-15': 'R', // still R -> should restore to Ç
        '2026-01-16': 'Ç', // user changed from R to Ç -> MUST PRESERVE Ç!
        '2026-01-17': 'R', // Saturday, still R -> should restore to T
      },
    };

    const deletedRecord: SickLeaveRecord = {
      id: 'sick1',
      personnelId: 'p1',
      startDate: '2026-01-15',
      endDate: '2026-01-17',
    };

    const updated = syncPuantajForSickLeaveDelete({
      deletedRecord,
      donemler,
      puantajlar: [puantaj],
    });

    const pUpdated = updated.find((p) => p.donemId === '2026-01');
    expect(pUpdated?.gunler['2026-01-15']).toBe('Ç'); // restored weekday
    expect(pUpdated?.gunler['2026-01-16']).toBe('Ç'); // preserved manual change!
    expect(pUpdated?.gunler['2026-01-17']).toBe('T'); // restored weekend
  });

  test('syncPuantajForSickLeaveSave on edit preserves manual changes and restores trimmed R dates', () => {
    // Old: Jan 15-18. User changed Jan 17 to 'İ' (İzin).
    // New: Jan 15-16 (shortened). Jan 17 and Jan 18 are removed.
    // Jan 17 should stay 'İ' (not overwritten). Jan 18 (Sunday) should restore to 'T'.
    const puantaj: PersonelPuantaj = {
      id: 'p1_2026-01',
      personelId: 'p1',
      donemId: '2026-01',
      gunler: {
        '2026-01-15': 'R',
        '2026-01-16': 'R',
        '2026-01-17': 'İ', // manual change!
        '2026-01-18': 'R', // was R, now trimmed -> restore to T (Sunday)
      },
    };

    const existingRecord: SickLeaveRecord = {
      id: 'sick1',
      personnelId: 'p1',
      startDate: '2026-01-15',
      endDate: '2026-01-18',
    };

    const newRecord: SickLeaveRecord = {
      id: 'sick1',
      personnelId: 'p1',
      startDate: '2026-01-15',
      endDate: '2026-01-16',
    };

    const updated = syncPuantajForSickLeaveSave({
      record: newRecord,
      existingRecord,
      donemler,
      puantajlar: [puantaj],
    });

    const pUpdated = updated.find((p) => p.donemId === '2026-01');
    expect(pUpdated?.gunler['2026-01-15']).toBe('R');
    expect(pUpdated?.gunler['2026-01-16']).toBe('R');
    expect(pUpdated?.gunler['2026-01-17']).toBe('İ'); // preserved!
    expect(pUpdated?.gunler['2026-01-18']).toBe('T'); // restored to Sunday rest!
  });
});
