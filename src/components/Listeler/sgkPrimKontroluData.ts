import type { BordroDonemi, BordroKaydi, Personel } from '../../types/payroll';
import { isAuthoritativePayroll } from './accrualListData';

export type SgkPrimKontroluRowStatus = 'authoritative' | 'notCalculated' | 'stale';

export interface SgkPrimKontroluRow {
  personel: Personel;
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
  toplam: number;
  status: SgkPrimKontroluRowStatus;
}

export interface SgkPrimKontroluTotals {
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
  genelPrimToplami: number;
}

export type SgkPrimComparisonStatus =
  | 'empty'
  | 'invalid'
  | 'compatible'
  | 'programHigher'
  | 'programLower';

export interface SgkPrimComparison {
  status: SgkPrimComparisonStatus;
  sgkTutarKurus: number | null;
  farkKurus: number | null;
}

function decimalTextToKurus(value: string): number {
  const match = value.match(/^(?<sign>-?)(?<whole>\d+)(?:\.(?<fraction>\d+))?(?:e(?<exponent>[+-]?\d+))?$/i);
  if (!match?.groups) throw new Error(`Geçersiz parasal değer: ${value}`);

  const sign = match.groups.sign === '-' ? -1n : 1n;
  const whole = match.groups.whole;
  const fraction = match.groups.fraction ?? '';
  const exponent = Number(match.groups.exponent ?? '0');
  const decimalPlaces = fraction.length - exponent;
  const digits = BigInt(`${whole}${fraction}`);
  let cents: bigint;

  if (decimalPlaces <= 2) {
    cents = digits * 10n ** BigInt(2 - decimalPlaces);
  } else {
    const divisor = 10n ** BigInt(decimalPlaces - 2);
    const wholeCents = digits / divisor;
    const remainder = digits % divisor;
    cents = wholeCents + (remainder * 2n >= divisor ? 1n : 0n);
  }

  const result = Number(sign * cents);
  if (!Number.isSafeInteger(result)) {
    throw new Error('Parasal değer güvenli kuruş aralığını aşıyor.');
  }
  return result;
}

/** Converts an existing snapshot amount to integer kuruş for exact comparisons. */
export function amountToKurus(value: number | null | undefined): number {
  if (value === null || value === undefined) return 0;
  if (!Number.isFinite(value)) throw new Error('Parasal snapshot değeri sonlu değil.');
  return decimalTextToKurus(value.toString());
}

export function kurusToAmount(value: number): number {
  return value / 100;
}

/**
 * Parses the Turkish TL input convention (for example 757.876,39) directly
 * into kuruş. The value is transient UI input and is never persisted.
 */
export function parseSgkTutarToKurus(value: string): number | null {
  const input = value.trim().replace(/\s+/g, '');
  if (!input) return null;
  if (!/^\d[\d.,]*$/.test(input)) return null;

  let integerPart = input;
  let fractionPart = '';

  if (input.includes(',')) {
    const parts = input.split(',');
    if (parts.length !== 2 || parts[1].length > 2) return null;
    integerPart = parts[0];
    fractionPart = parts[1];
    if (!/^\d+$|^\d{1,3}(?:\.\d{3})+$/.test(integerPart)) return null;
    integerPart = integerPart.replace(/\./g, '');
  } else if (input.includes('.')) {
    const parts = input.split('.');
    const lastPart = parts[parts.length - 1];
    const isDecimalNotation = parts.length === 2 && lastPart.length <= 2;
    if (isDecimalNotation) {
      integerPart = parts[0];
      fractionPart = lastPart;
      if (!/^\d+$/.test(integerPart)) return null;
    } else {
      if (!parts.every((part, index) => index === 0 ? /^\d{1,3}$/.test(part) : /^\d{3}$/.test(part))) {
        return null;
      }
      integerPart = parts.join('');
    }
  }

  if (!/^\d+$/.test(integerPart)) return null;
  const normalizedFraction = `${fractionPart}00`.slice(0, 2);
  const result = Number(BigInt(integerPart) * 100n + BigInt(normalizedFraction));
  return Number.isSafeInteger(result) ? result : null;
}

function sumSnapshotValues(payrolls: BordroKaydi[]): {
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
} {
  const totals = {
    isverenSgkPrimi: 0,
    isverenIssizlikPrimi: 0,
    isciSgkPrimi: 0,
    isciIssizlikPrimi: 0,
  };

  for (const payroll of payrolls) {
    totals.isverenSgkPrimi += amountToKurus(payroll.pekDetay?.isverenSgkPrimi);
    totals.isverenIssizlikPrimi += amountToKurus(payroll.pekDetay?.isverenIssizlikPrimi);
    totals.isciSgkPrimi += amountToKurus(payroll.kesintiler?.isciSgkPrimi);
    totals.isciIssizlikPrimi += amountToKurus(payroll.kesintiler?.isciIssizlikPrimi);
  }

  return totals;
}

export function getSgkPrimKontroluRows(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[]
): SgkPrimKontroluRow[] {
  const payrollsByPerson = new Map<string, BordroKaydi[]>();

  payrolls
    .filter((payroll) => payroll.donemId === period.id)
    .forEach((payroll) => {
      const personPayrolls = payrollsByPerson.get(payroll.personelId) ?? [];
      personPayrolls.push(payroll);
      payrollsByPerson.set(payroll.personelId, personPayrolls);
    });

  return personnel.map((personel) => {
    const personPayrolls = payrollsByPerson.get(personel.id) ?? [];
    const authoritativePayrolls = personPayrolls.filter(isAuthoritativePayroll);
    const snapshotTotals = sumSnapshotValues(authoritativePayrolls);
    const hasStalePayroll = personPayrolls.some((payroll) => payroll.status === 'STALE');

    const isverenSgkPrimi = kurusToAmount(snapshotTotals.isverenSgkPrimi);
    const isverenIssizlikPrimi = kurusToAmount(snapshotTotals.isverenIssizlikPrimi);
    const isciSgkPrimi = kurusToAmount(snapshotTotals.isciSgkPrimi);
    const isciIssizlikPrimi = kurusToAmount(snapshotTotals.isciIssizlikPrimi);

    return {
      personel,
      isverenSgkPrimi,
      isverenIssizlikPrimi,
      isciSgkPrimi,
      isciIssizlikPrimi,
      toplam: kurusToAmount(
        snapshotTotals.isverenSgkPrimi +
          snapshotTotals.isverenIssizlikPrimi +
          snapshotTotals.isciSgkPrimi +
          snapshotTotals.isciIssizlikPrimi
      ),
      status:
        authoritativePayrolls.length > 0
          ? 'authoritative'
          : hasStalePayroll
            ? 'stale'
            : 'notCalculated',
    };
  });
}

export function getSgkPrimKontroluTotals(rows: SgkPrimKontroluRow[]): SgkPrimKontroluTotals {
  const totalsKurus = rows.reduce(
    (totals, row) => {
      if (row.status !== 'authoritative') return totals;
      totals.isverenSgkPrimi += amountToKurus(row.isverenSgkPrimi);
      totals.isverenIssizlikPrimi += amountToKurus(row.isverenIssizlikPrimi);
      totals.isciSgkPrimi += amountToKurus(row.isciSgkPrimi);
      totals.isciIssizlikPrimi += amountToKurus(row.isciIssizlikPrimi);
      return totals;
    },
    {
      isverenSgkPrimi: 0,
      isverenIssizlikPrimi: 0,
      isciSgkPrimi: 0,
      isciIssizlikPrimi: 0,
    }
  );

  const genelPrimToplami =
    totalsKurus.isverenSgkPrimi +
    totalsKurus.isverenIssizlikPrimi +
    totalsKurus.isciSgkPrimi +
    totalsKurus.isciIssizlikPrimi;

  return {
    isverenSgkPrimi: kurusToAmount(totalsKurus.isverenSgkPrimi),
    isverenIssizlikPrimi: kurusToAmount(totalsKurus.isverenIssizlikPrimi),
    isciSgkPrimi: kurusToAmount(totalsKurus.isciSgkPrimi),
    isciIssizlikPrimi: kurusToAmount(totalsKurus.isciIssizlikPrimi),
    genelPrimToplami: kurusToAmount(genelPrimToplami),
  };
}

export function compareSgkPrimTotals(
  programGenelPrimToplami: number,
  input: string
): SgkPrimComparison {
  if (!input.trim()) {
    return { status: 'empty', sgkTutarKurus: null, farkKurus: null };
  }

  const sgkTutarKurus = parseSgkTutarToKurus(input);
  if (sgkTutarKurus === null) {
    return { status: 'invalid', sgkTutarKurus: null, farkKurus: null };
  }

  const farkKurus = amountToKurus(programGenelPrimToplami) - sgkTutarKurus;
  return {
    status:
      farkKurus === 0
        ? 'compatible'
        : farkKurus > 0
          ? 'programHigher'
          : 'programLower',
    sgkTutarKurus,
    farkKurus,
  };
}
