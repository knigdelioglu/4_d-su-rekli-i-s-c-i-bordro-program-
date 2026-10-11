import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { BordroDonemi, Personel, PersonelPuantaj, SickLeaveRecord } from '../types/payroll';
import { generateDefaultPuantajGunler } from '../utils/payrollPresentation';
import { PuantajGrid } from './PuantajGrid';

const period: BordroDonemi = {
  id: '2027-01', yil: 2027, ay: 1, baslangicTarihi: '2027-01-15',
  bitisTarihi: '2027-02-14', donemAdi: 'Ocak 2027', taxYear: 2027, taxMonth: 2,
};
const personnel: Personel[] = [
  { id: 'person-a', tcNo: '10000000001', ad: 'Ada', soyad: 'Bir', grup: '1. Grup', sgkSicilNo: '', iban: '', hizmetYili: 1 },
  { id: 'person-b', tcNo: '10000000002', ad: 'Bora', soyad: 'İki', grup: '1. Grup', sgkSicilNo: '', iban: '', hizmetYili: 2 },
];
const startingAttendance: PersonelPuantaj[] = personnel.map((person) => {
  const gunler = generateDefaultPuantajGunler(period.baslangicTarihi, period.bitisTarihi);
  gunler['2027-01-16'] = 'R';
  if (person.id === 'person-a') gunler['2027-01-17'] = 'R';
  return { id: `${person.id}_${period.id}`, personelId: person.id, donemId: period.id, gunler };
});
const sickLeaveRecords: SickLeaveRecord[] = [
  { id: 'report-a', personnelId: 'person-a', startDate: '2027-01-16', endDate: '2027-01-17' },
  { id: 'report-b', personnelId: 'person-b', startDate: '2027-01-16', endDate: '2027-01-16' },
  { id: 'report-b-mismatch', personnelId: 'person-b', startDate: '2027-01-20', endDate: '2027-01-20' },
];

declare global {
  interface Window {
    __puantajBulkTest: {
      calls: number;
      failNext: boolean;
      snapshot: () => PersonelPuantaj[];
      attempts: () => number;
    };
  }
}

function Harness() {
  const [attendances, setAttendances] = useState(startingAttendance);
  const [attempts, setAttempts] = useState(0);
  const [failNext, setFailNext] = useState(true);
  window.__puantajBulkTest = {
    calls: attempts,
    failNext,
    snapshot: () => structuredClone(attendances),
    attempts: () => attempts,
  };
  const saveOne = async (updated: PersonelPuantaj) => {
    setAttendances((current) => current.map((item) => item.personelId === updated.personelId ? updated : item));
  };
  const saveBatch = async (updated: PersonelPuantaj[]) => {
    setAttempts((current) => current + 1);
    if (failNext) {
      setFailNext(false);
      throw new Error('sentetik yazma hatası');
    }
    setAttendances((current) => current.map((item) => updated.find((next) => next.personelId === item.personelId) ?? item));
  };
  return <PuantajGrid
    aktifDonem={period}
    personeller={personnel}
    puantajlar={attendances}
    sickLeaveRecords={sickLeaveRecords}
    onSavePuantaj={saveOne}
    onSavePuantajBatch={saveBatch}
  />;
}

const root = document.getElementById('root');
if (!root) throw new Error('Puantaj grid harness root is missing.');
createRoot(root).render(<Harness />);
