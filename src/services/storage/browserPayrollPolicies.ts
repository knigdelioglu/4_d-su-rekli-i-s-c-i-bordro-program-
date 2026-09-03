import { BordroDonemi, BordroKaydi } from '../../types/payroll';

export type BrowserPayrollMutation =
  | { kind: 'PERSON'; personnelId: string }
  | { kind: 'PERSON_PERIOD'; personnelId: string; periodId: string }
  | { kind: 'PERSON_TAX_YEAR'; personnelId: string; taxYear: number }
  | { kind: 'TAX_YEAR'; taxYear: number }
  | { kind: 'PERIOD'; periodId: string }
  | { kind: 'ALL' };

function isFinalized(payroll: BordroKaydi): boolean {
  return payroll.status === 'FINALIZED';
}

function periodFor(payroll: BordroKaydi, periods: BordroDonemi[]): BordroDonemi | undefined {
  return periods.find((period) => period.id === payroll.donemId);
}

function isPeriodDependent(candidate: BordroDonemi, source: BordroDonemi): boolean {
  return (
    candidate.id === source.id ||
    candidate.baslangicTarihi > source.baslangicTarihi ||
    (candidate.taxYear === source.taxYear && candidate.taxMonth > source.taxMonth)
  );
}

function isAffectedByMutation(
  payroll: BordroKaydi,
  periods: BordroDonemi[],
  mutation: BrowserPayrollMutation
): boolean {
  const period = periodFor(payroll, periods);
  switch (mutation.kind) {
    case 'PERSON':
      return payroll.personelId === mutation.personnelId;
    case 'PERSON_PERIOD':
      return (
        payroll.personelId === mutation.personnelId &&
        !!period &&
        !!periods.find((candidate) => candidate.id === mutation.periodId) &&
        isPeriodDependent(period, periods.find((candidate) => candidate.id === mutation.periodId)!)
      );
    case 'PERSON_TAX_YEAR':
      return payroll.personelId === mutation.personnelId && period?.taxYear === mutation.taxYear;
    case 'TAX_YEAR':
      return period?.taxYear === mutation.taxYear;
    case 'PERIOD': {
      const source = periods.find((candidate) => candidate.id === mutation.periodId);
      return !!period && !!source && isPeriodDependent(period, source);
    }
    case 'ALL':
      return true;
  }
}

/** Mirrors native invalidation: only mutable CALCULATED snapshots become STALE. */
export function invalidateBrowserPayrolls(
  payrolls: BordroKaydi[],
  periods: BordroDonemi[],
  mutation: BrowserPayrollMutation
): BordroKaydi[] {
  return payrolls.map((payroll) =>
    payroll.status === 'CALCULATED' && isAffectedByMutation(payroll, periods, mutation)
      ? { ...payroll, status: 'STALE' }
      : payroll
  );
}

export function invalidateBrowserPayrollsFromPeriodPosition(
  payrolls: BordroKaydi[],
  periods: BordroDonemi[],
  source: BordroDonemi
): BordroKaydi[] {
  return payrolls.map((payroll) => {
    const period = periodFor(payroll, periods);
    const affects =
      period &&
      (period.baslangicTarihi >= source.baslangicTarihi ||
        (period.taxYear === source.taxYear && period.taxMonth >= source.taxMonth));
    return payroll.status === 'CALCULATED' && affects
      ? { ...payroll, status: 'STALE' }
      : payroll;
  });
}

export function invalidateBrowserPayrollsAfterCalculation(
  payrolls: BordroKaydi[],
  periods: BordroDonemi[],
  calculated: BordroKaydi
): BordroKaydi[] {
  const source = periods.find((period) => period.id === calculated.donemId);
  if (!source) return payrolls;
  return payrolls.map((payroll) => {
    const candidate = periodFor(payroll, periods);
    const affects =
      payroll.id !== calculated.id &&
      payroll.personelId === calculated.personelId &&
      !!candidate &&
      isPeriodDependent(candidate, source) &&
      candidate.id !== source.id;
    return payroll.status === 'CALCULATED' && affects
      ? { ...payroll, status: 'STALE' }
      : payroll;
  });
}

/** FINALIZED snapshots are an immutable boundary for their source records. */
export function assertBrowserMutationAllowed(
  payrolls: BordroKaydi[],
  periods: BordroDonemi[],
  mutation: BrowserPayrollMutation
): void {
  if (payrolls.some((payroll) => isFinalized(payroll) && isAffectedByMutation(payroll, periods, mutation))) {
    throw new Error('Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez.');
  }
}

