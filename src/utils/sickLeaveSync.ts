import type { BordroDonemi, PersonelPuantaj, PuantajKodu, SickLeaveRecord } from '../types/payroll';
import { generateDefaultPuantajGunler, getPeriodDaysList } from './payrollPresentation';

export interface SickLeaveConflict {
  date: string;
  currentCode: PuantajKodu;
  periodId: string;
  periodName?: string;
}

/**
 * Checks for existing puantaj codes on the requested sick leave dates.
 * A conflict exists if a date within the leave range has an existing code
 * other than 'R' (e.g. 'Ç', 'T', 'G', 'İ', etc.).
 */
export function findSickLeaveConflicts(params: {
  personnelId: string;
  startDate: string;
  endDate: string;
  donemler: BordroDonemi[];
  puantajlar: PersonelPuantaj[];
}): SickLeaveConflict[] {
  const { personnelId, startDate, endDate, donemler, puantajlar } = params;
  if (!personnelId || !startDate || !endDate || startDate > endDate) {
    return [];
  }

  const conflicts: SickLeaveConflict[] = [];
  const days = getPeriodDaysList(startDate, endDate);

  for (const day of days) {
    const period = donemler.find(
      (p) => day.dateStr >= p.baslangicTarihi && day.dateStr <= p.bitisTarihi
    );
    if (!period) continue;

    const attendance = puantajlar.find(
      (p) => p.personelId === personnelId && p.donemId === period.id
    );
    if (!attendance) continue;

    const currentCode = attendance.gunler[day.dateStr];
    if (currentCode && currentCode !== 'R') {
      conflicts.push({
        date: day.dateStr,
        currentCode,
        periodId: period.id,
        periodName: period.donemAdi || period.id,
      });
    }
  }

  return conflicts;
}

/**
 * Synchronizes puantaj records across 15-14 period boundaries when saving a sick leave.
 *
 * Rules:
 * 1. For all affected periods, sick leave dates are marked with 'R'.
 * 2. If editing an existing sick leave:
 *    Dates that were in the old record but NOT in the new record are restored to
 *    calendar defaults ('T' for weekend, 'Ç' for weekday) ONLY IF they are still 'R'.
 *    If the user manually changed a date to another code (e.g. 'Ç', 'İ'), that code is NEVER overwritten.
 */
export function syncPuantajForSickLeaveSave(params: {
  record: SickLeaveRecord;
  existingRecord?: SickLeaveRecord;
  donemler: BordroDonemi[];
  puantajlar: PersonelPuantaj[];
}): PersonelPuantaj[] {
  const { record, existingRecord, donemler, puantajlar } = params;
  const result = [...puantajlar];

  for (const period of donemler) {
    const pStart = period.baslangicTarihi;
    const pEnd = period.bitisTarihi;

    const overlapsNew = record.startDate <= pEnd && record.endDate >= pStart;
    const overlapsOld = existingRecord
      ? existingRecord.startDate <= pEnd && existingRecord.endDate >= pStart
      : false;

    if (!overlapsNew && !overlapsOld) continue;

    const existingIndex = result.findIndex(
      (p) => p.personelId === record.personnelId && p.donemId === period.id
    );

    const baseGunler: Record<string, PuantajKodu> =
      existingIndex >= 0
        ? { ...result[existingIndex].gunler }
        : generateDefaultPuantajGunler(pStart, pEnd);

    let changed = false;

    // If edit: restore dates in old record that are NOT in new record, ONLY if still 'R'
    if (existingRecord) {
      const oldDays = getPeriodDaysList(existingRecord.startDate, existingRecord.endDate);
      for (const day of oldDays) {
        if (day.dateStr >= pStart && day.dateStr <= pEnd) {
          if (day.dateStr < record.startDate || day.dateStr > record.endDate) {
            if (baseGunler[day.dateStr] === 'R') {
              baseGunler[day.dateStr] = day.isWeekend ? 'T' : 'Ç';
              changed = true;
            }
          }
        }
      }
    }

    // Set new record dates to 'R'
    if (overlapsNew) {
      const newDays = getPeriodDaysList(record.startDate, record.endDate);
      for (const day of newDays) {
        if (day.dateStr >= pStart && day.dateStr <= pEnd) {
          if (baseGunler[day.dateStr] !== 'R') {
            baseGunler[day.dateStr] = 'R';
            changed = true;
          }
        }
      }
    }

    if (changed || existingIndex < 0) {
      const updatedAttendance: PersonelPuantaj = {
        id: existingIndex >= 0 ? result[existingIndex].id : `${record.personnelId}_${period.id}`,
        personelId: record.personnelId,
        donemId: period.id,
        gunler: baseGunler,
      };

      if (existingIndex >= 0) {
        result[existingIndex] = updatedAttendance;
      } else {
        result.push(updatedAttendance);
      }
    }
  }

  return result;
}

/**
 * Synchronizes puantaj records across 15-14 period boundaries when deleting a sick leave.
 *
 * Rules:
 * 1. For all affected periods, dates that were part of this sick leave are restored
 *    to calendar defaults ('T' for weekend, 'Ç' for weekday) ONLY IF they are currently 'R'.
 * 2. If the user subsequently changed any date to another code (e.g. 'Ç', 'İ'), that code
 *    is strictly preserved ("kullanıcının sonradan değiştirdiği puantajı ezme").
 */
export function syncPuantajForSickLeaveDelete(params: {
  deletedRecord: SickLeaveRecord;
  donemler: BordroDonemi[];
  puantajlar: PersonelPuantaj[];
}): PersonelPuantaj[] {
  const { deletedRecord, donemler, puantajlar } = params;
  const result = [...puantajlar];

  for (const period of donemler) {
    const pStart = period.baslangicTarihi;
    const pEnd = period.bitisTarihi;

    const overlaps = deletedRecord.startDate <= pEnd && deletedRecord.endDate >= pStart;
    if (!overlaps) continue;

    const existingIndex = result.findIndex(
      (p) => p.personelId === deletedRecord.personnelId && p.donemId === period.id
    );
    if (existingIndex < 0) continue;

    const baseGunler = { ...result[existingIndex].gunler };
    let changed = false;

    const days = getPeriodDaysList(deletedRecord.startDate, deletedRecord.endDate);
    for (const day of days) {
      if (day.dateStr >= pStart && day.dateStr <= pEnd) {
        // Only restore if STILL 'R' (preserve user manual changes!)
        if (baseGunler[day.dateStr] === 'R') {
          baseGunler[day.dateStr] = day.isWeekend ? 'T' : 'Ç';
          changed = true;
        }
      }
    }

    if (changed) {
      result[existingIndex] = {
        ...result[existingIndex],
        gunler: baseGunler,
      };
    }
  }

  return result;
}
