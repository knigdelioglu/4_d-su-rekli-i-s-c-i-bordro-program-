import { isExactDecimalString } from '../payrollEngine/decimalBoundary';

/** Mirrors payroll-core's SQLite-safe open-ended bracket sentinel. */
export const OPEN_ENDED_TAX_BRACKET_LIMIT = 1_000_000_000_000_000;
export const OPEN_ENDED_TAX_BRACKET_LIMIT_TEXT = String(OPEN_ENDED_TAX_BRACKET_LIMIT);

type UnknownRecord = Record<string, unknown>;

interface DecimalParts {
  negative: boolean;
  digits: string;
  scale: number;
}

export interface AnnualPayrollParameterSemanticIssue {
  field: string;
  message: string;
}

function isRecord(value: unknown): value is UnknownRecord {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function decimalParts(value: unknown): DecimalParts | null {
  if (!isExactDecimalString(value)) return null;
  const negative = value.startsWith('-');
  const unsigned = negative ? value.slice(1) : value;
  const [whole, fraction = ''] = unsigned.split('.');
  const digits = `${whole}${fraction}`.replace(/^0+/, '') || '0';
  return {
    negative: digits !== '0' && negative,
    digits,
    scale: fraction.length,
  };
}

/** Exact decimal comparison without converting persisted values to JS Number. */
function compareDecimals(left: unknown, right: unknown): number | null {
  const a = decimalParts(left);
  const b = decimalParts(right);
  if (!a || !b) return null;
  if (a.negative !== b.negative) return a.negative ? -1 : 1;

  const scale = Math.max(a.scale, b.scale);
  const aDigits = `${a.digits}${'0'.repeat(scale - a.scale)}`.replace(/^0+/, '') || '0';
  const bDigits = `${b.digits}${'0'.repeat(scale - b.scale)}`.replace(/^0+/, '') || '0';
  const magnitude =
    aDigits.length === bDigits.length
      ? aDigits === bDigits
        ? 0
        : aDigits < bDigits
          ? -1
          : 1
      : aDigits.length < bDigits.length
        ? -1
        : 1;
  return a.negative ? -magnitude : magnitude;
}

/**
 * Browser-side persistence policy for the annual tariff. It intentionally
 * performs only exact ordering/range checks; tax calculations remain in Rust.
 */
export function annualPayrollParameterSemanticIssue(
  value: unknown
): AnnualPayrollParameterSemanticIssue | null {
  if (!isRecord(value)) {
    return { field: '', message: 'plain object olmalıdır.' };
  }

  if (
    typeof value.year !== 'number' ||
    !Number.isInteger(value.year) ||
    value.year <= 0 ||
    value.year > 2_147_483_647
  ) {
    return { field: 'year', message: 'sıfırdan büyük geçerli bir tam sayı olmalıdır.' };
  }

  const brackets = value.gelirVergisiDilimleri;
  if (!Array.isArray(brackets) || brackets.length === 0) {
    return { field: 'gelirVergisiDilimleri', message: 'en az bir vergi dilimi içermelidir.' };
  }

  let previousLimit: unknown = '0';
  for (const [index, bracket] of brackets.entries()) {
    const field = `gelirVergisiDilimleri[${index}]`;
    if (!isRecord(bracket)) {
      return { field, message: 'plain object olmalıdır.' };
    }
    if (!isExactDecimalString(bracket.limit)) {
      return { field: `${field}.limit`, message: 'exact Decimal metni olmalıdır.' };
    }
    if (!isExactDecimalString(bracket.oran)) {
      return { field: `${field}.oran`, message: 'exact Decimal metni olmalıdır.' };
    }

    const limitVsZero = compareDecimals(bracket.limit, '0');
    if (limitVsZero === null || limitVsZero <= 0) {
      return { field: `${field}.limit`, message: 'sıfırdan büyük olmalıdır.' };
    }
    const limitVsPrevious = compareDecimals(bracket.limit, previousLimit);
    if (limitVsPrevious === null || limitVsPrevious <= 0) {
      return { field: `${field}.limit`, message: 'önceki limitten büyük olmalıdır.' };
    }
    if (compareDecimals(bracket.limit, OPEN_ENDED_TAX_BRACKET_LIMIT_TEXT)! > 0) {
      return {
        field: `${field}.limit`,
        message: 'SQLite-safe üst sınırı aşamaz.',
      };
    }

    const rateVsZero = compareDecimals(bracket.oran, '0');
    const rateVsOne = compareDecimals(bracket.oran, '1');
    if (rateVsZero === null || rateVsOne === null || rateVsZero < 0 || rateVsOne > 0) {
      return {
        field: `${field}.oran`,
        message: '0 ile 1 arasında olmalıdır.',
      };
    }
    previousLimit = bracket.limit;
  }

  const insuranceCap = value.sigortaGvYillikBrutAsgariUcretTavani;
  if (insuranceCap !== undefined && insuranceCap !== null) {
    const capVsZero = compareDecimals(insuranceCap, '0');
    if (capVsZero === null || capVsZero <= 0) {
      return {
        field: 'sigortaGvYillikBrutAsgariUcretTavani',
        message: 'sıfırdan büyük olmalıdır.',
      };
    }
  }

  return null;
}
