import type {
  Personel,
  PersonelPuantaj,
  PuantajKodu,
  SickLeaveRecord,
} from '../types/payroll';
import { generateDefaultPuantajGunler } from './payrollPresentation';

export type PuantajBulkRowStatus = 'APPLY' | 'PRESERVED_R' | 'REPORT_MISMATCH';

export interface PuantajBulkPreviewRow {
  personnelId: string;
  personnelName: string;
  date: string;
  previousCode: PuantajKodu;
  nextCode: PuantajKodu;
  status: PuantajBulkRowStatus;
}

export interface PuantajBulkPlan {
  updates: PersonelPuantaj[];
  rows: PuantajBulkPreviewRow[];
  reportMismatches: PuantajBulkPreviewRow[];
  preservedRows: PuantajBulkPreviewRow[];
}

/**
 * Creates a read-only preview and the exact attendance records for one atomic
 * batch save. R codes and dates covered by sick-leave records are never
 * replaced; an existing report/code mismatch blocks the whole batch.
 */
export function createPuantajBulkPlan(args: {
  personnel: Personel[];
  attendances: PersonelPuantaj[];
  sickLeaveRecords: SickLeaveRecord[];
  periodId: string;
  periodStartInclusive: string;
  periodEndInclusive: string;
  dates: string[];
  code: PuantajKodu;
}): PuantajBulkPlan {
  const { personnel, attendances, sickLeaveRecords, periodId, periodStartInclusive, periodEndInclusive, dates, code } = args;
  const periodDates = [...new Set(dates)].filter((date) => date >= periodStartInclusive && date <= periodEndInclusive);
  const attendanceByPersonnel = new Map(
    attendances
      .filter((attendance) => attendance.donemId === periodId)
      .map((attendance) => [attendance.personelId, attendance] as const)
  );
  const rows: PuantajBulkPreviewRow[] = [];
  const updates: PersonelPuantaj[] = [];

  for (const person of personnel) {
    const existing = attendanceByPersonnel.get(person.id);
    const days = { ...(existing?.gunler ?? generateDefaultPuantajGunler(periodStartInclusive, periodEndInclusive)) };
    let changed = false;
    for (const date of periodDates) {
      const previousCode = days[date] as PuantajKodu | undefined;
      if (!previousCode) continue;
      const coveredByReport = sickLeaveRecords.some((record) =>
        record.personnelId === person.id && record.startDate <= date && record.endDate >= date
      );
      if (previousCode === code && !(coveredByReport && previousCode !== 'R')) continue;
      const status: PuantajBulkRowStatus = coveredByReport && previousCode !== 'R'
        ? 'REPORT_MISMATCH'
        : previousCode === 'R' || coveredByReport
          ? 'PRESERVED_R'
          : 'APPLY';
      const row: PuantajBulkPreviewRow = {
        personnelId: person.id,
        personnelName: `${person.ad} ${person.soyad}`.trim(),
        date,
        previousCode,
        nextCode: code,
        status,
      };
      rows.push(row);
      if (status === 'APPLY') {
        days[date] = code;
        changed = true;
      }
    }
    if (changed) {
      updates.push({
        id: existing?.id ?? `${person.id}_${periodId}`,
        personelId: person.id,
        donemId: periodId,
        gunler: days,
      });
    }
  }

  return {
    updates,
    rows,
    reportMismatches: rows.filter((row) => row.status === 'REPORT_MISMATCH'),
    preservedRows: rows.filter((row) => row.status === 'PRESERVED_R'),
  };
}
