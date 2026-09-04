import type { AccrualType, BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { getDefaultAccrualPaymentDate } from '../../utils/payrollPresentation';

export const ACCRUAL_TYPE_LABELS: Record<AccrualType, string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
};

export const PAYROLL_STATUS_LABELS: Record<BordroKaydi['status'], string> = {
  DRAFT: 'Taslak',
  CALCULATED: 'Hesaplandı',
  STALE: 'Yeniden Hesaplanmalı',
  FINALIZED: 'Kesinleştirildi',
};

export function getPayrollStatusLabel(status: BordroKaydi['status']): string {
  return PAYROLL_STATUS_LABELS[status];
}

export interface AuthoritativeAccrualRow {
  personel: Personel;
  bordro: BordroKaydi;
  accrualId: string;
  paymentDate: string;
  accrualTypeLabel: string;
  sequence: number;
}

export function isAuthoritativePayroll(payroll: BordroKaydi): boolean {
  return payroll.status === 'CALCULATED' || payroll.status === 'FINALIZED';
}

export function getAccrualPaymentDate(
  payroll: BordroKaydi,
  period: BordroDonemi
): string {
  return payroll.paymentDate || getDefaultAccrualPaymentDate(period);
}

export function getAuthoritativeAccrualRows(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[]
): AuthoritativeAccrualRow[] {
  const personnelById = new Map(personnel.map((person) => [person.id, person]));

  return payrolls
    .filter(
      (payroll) =>
        payroll.donemId === period.id && isAuthoritativePayroll(payroll)
    )
    .flatMap((payroll) => {
      const personel = personnelById.get(payroll.personelId);
      if (!personel) return [];
      return [
        {
          personel,
          bordro: payroll,
          accrualId: payroll.accrualId || payroll.id,
          paymentDate: getAccrualPaymentDate(payroll, period),
          accrualTypeLabel: ACCRUAL_TYPE_LABELS[payroll.accrualType],
          sequence: payroll.sequence,
        },
      ];
    })
    .sort((left, right) => {
      const dateOrder = left.paymentDate.localeCompare(right.paymentDate);
      if (dateOrder !== 0) return dateOrder;
      if (left.sequence !== right.sequence) return left.sequence - right.sequence;
      const accrualOrder = left.accrualId.localeCompare(right.accrualId);
      if (accrualOrder !== 0) return accrualOrder;
      return left.personel.id.localeCompare(right.personel.id);
    });
}

export function getPaymentDateOptions(rows: AuthoritativeAccrualRow[]): string[] {
  return [...new Set(rows.map((row) => row.paymentDate))].sort((left, right) =>
    left.localeCompare(right)
  );
}

export function filterAccrualRowsByPaymentDate(
  rows: AuthoritativeAccrualRow[],
  paymentDate: string
): AuthoritativeAccrualRow[] {
  if (paymentDate === 'all') return rows;
  return rows.filter((row) => row.paymentDate === paymentDate);
}

export function countAuthoritativeNormalPersonnel(
  payrolls: BordroKaydi[],
  periodId: string
): number {
  return new Set(
    payrolls
      .filter(
        (payroll) =>
          payroll.donemId === periodId &&
          payroll.accrualType === 'NORMAL' &&
          isAuthoritativePayroll(payroll)
      )
      .map((payroll) => payroll.personelId)
  ).size;
}
