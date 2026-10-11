import { comparePaymentEvents } from '../../services/payrollEngine/paymentEventOrder';
import type { AccrualType, BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { getDefaultAccrualPaymentDate } from '../../utils/payrollPresentation';
import { isValidTurkishIban } from '../../utils/personnelValidation';

export const ACCRUAL_TYPE_LABELS: Record<AccrualType, string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
  RETRO_ADJUSTMENT: 'Geriye Dönük Fark',
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

/** Discovery only: other periods' records never enter active-period totals. */
export function getOtherPeriodAccruals(
  payrolls: BordroKaydi[],
  periods: BordroDonemi[],
  personnelId: string,
  accrualType: AccrualType,
  activePeriodId: string
): Array<{ payroll: BordroKaydi; periodLabel: string }> {
  const periodsById = new Map(periods.map((period) => [period.id, period]));
  return payrolls
    .filter((payroll) => payroll.personelId === personnelId &&
      payroll.accrualType === accrualType && payroll.donemId !== activePeriodId)
    .sort((left, right) => right.paymentDate.localeCompare(left.paymentDate) || right.sequence - left.sequence)
    .map((payroll) => ({ payroll, periodLabel: periodsById.get(payroll.donemId)?.donemAdi || payroll.donemId }));
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
      const eventOrder = comparePaymentEvents(left.bordro, right.bordro, period);
      if (eventOrder !== 0) return eventOrder;
      return left.personel.id.localeCompare(right.personel.id);
    });
}

/** Only finalized accruals are ready to be transferred to the bank. */
export function getBankTransferableAccrualRows(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[]
): AuthoritativeAccrualRow[] {
  return getAuthoritativeAccrualRows(period, personnel, payrolls)
    .filter(({ personel, bordro }) =>
      bordro.status === 'FINALIZED' && isValidTurkishIban(personel.iban)
    );
}

export function describeBankExcludedAccruals(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[]
): string | null {
  const personnelById = new Map(personnel.map((person) => [person.id, person]));
  const excludedByStatus = payrolls.filter((payroll) =>
    payroll.donemId === period.id && payroll.status !== 'FINALIZED'
  );
  const invalidIbanRows = getAuthoritativeAccrualRows(period, personnel, payrolls)
    .filter(({ personel, bordro }) =>
      bordro.status === 'FINALIZED' && !isValidTurkishIban(personel.iban)
    );
  if (excludedByStatus.length === 0 && invalidIbanRows.length === 0) return null;

  const calculatedCount = excludedByStatus.filter((payroll) => payroll.status === 'CALCULATED').length;
  const recalculationCount = excludedByStatus.length - calculatedCount;
  const statusExamples = excludedByStatus.slice(0, 4).map((payroll) => {
    const person = personnelById.get(payroll.personelId);
    const name = person ? `${person.ad} ${person.soyad}` : payroll.personelId;
    return `${name} – ${ACCRUAL_TYPE_LABELS[payroll.accrualType]} (${PAYROLL_STATUS_LABELS[payroll.status]})`;
  });
  const invalidIbanExamples = invalidIbanRows
    .slice(0, Math.max(0, 4 - statusExamples.length))
    .map(({ personel, bordro }) =>
      `${personel.ad} ${personel.soyad} – ${ACCRUAL_TYPE_LABELS[bordro.accrualType]} (geçersiz IBAN)`
    );
  const examples = [...statusExamples, ...invalidIbanExamples];
  const totalExcluded = excludedByStatus.length + invalidIbanRows.length;
  const more = totalExcluded > examples.length ? ` ve ${totalExcluded - examples.length} kayıt daha` : '';
  const reasons = [
    calculatedCount > 0
      ? `${calculatedCount} hesaplanmış tahakkuk henüz kesinleştirilmedi ve banka aktarımına hazır değil`
      : '',
    recalculationCount > 0
      ? `${recalculationCount} taslak veya güncelliğini yitirmiş tahakkuk yeniden hesaplanmalı`
      : '',
    invalidIbanRows.length > 0
      ? `${invalidIbanRows.length} kesinleşmiş tahakkukun Türk IBAN doğrulaması başarısız; banka talimatına aktarılmadı`
      : '',
  ].filter(Boolean).join('; ');
  const exampleText = examples.length > 0 ? `: ${examples.join(', ')}${more}` : '';
  return `${reasons}${exampleText}. Geçerli IBAN ve kesinleştirme koşullarını sağlayan kayıtlar banka listesine alınır.`;
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

/**
 * Period accruals that are excluded from official lists because they are not
 * authoritative (STALE/DRAFT). Lists show this so a missing row is explained.
 */
export function describeExcludedAccruals(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[]
): string | null {
  const personnelById = new Map(personnel.map((person) => [person.id, person]));
  const excluded = payrolls.filter(
    (payroll) => payroll.donemId === period.id && !isAuthoritativePayroll(payroll)
  );
  if (excluded.length === 0) return null;
  const examples = excluded.slice(0, 4).map((payroll) => {
    const person = personnelById.get(payroll.personelId);
    const name = person ? `${person.ad} ${person.soyad}` : payroll.personelId;
    return `${name} – ${ACCRUAL_TYPE_LABELS[payroll.accrualType]} (${PAYROLL_STATUS_LABELS[payroll.status]})`;
  });
  const more = excluded.length > examples.length ? ` ve ${excluded.length - examples.length} kayıt daha` : '';
  return `${excluded.length} tahakkuk yeniden hesaplanması gerektiği için bu listede yer almıyor: ${examples.join(', ')}${more}. Bordro Hesaplama ekranından yeniden hesaplayın.`;
}
