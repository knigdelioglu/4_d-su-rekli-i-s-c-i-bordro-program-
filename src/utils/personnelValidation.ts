export const INVALID_TURKISH_IBAN_MESSAGE =
  'Türk IBAN’ı 26 karakter olmalı, TR ile başlamalı ve kontrol basamağı geçerli olmalıdır.';
export const NEGATIVE_PERSONNEL_AMOUNT_MESSAGE = 'Kesinti tutarı negatif olamaz.';
export const INVALID_PERSONNEL_AMOUNT_MESSAGE =
  'Tutar geçerli bir sayı olmalı ve en fazla iki ondalık basamak içermelidir.';

const MONEY_FIELDS = [
  'sabitSendikaAidati',
  'sabitBesTutar',
  'icraTutar',
  'nafakaTutar',
  'kisiBorcuTutar',
  'dogumAskerlikBorclanmasiTutar',
  'hayatSaglikSigortasiTutar',
  'digerKesintiTutar',
] as const;

const GV_MONEY_FIELDS = [
  'dogumAskerlikGvIndirimTutar',
  'hayatSigortasiPrimiTutar',
  'saglikSigortasiPrimiTutar',
] as const;

/** Accepts ordinary grouping spaces and returns the canonical compact IBAN. */
export function normalizeTurkishIban(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const normalized = value.replace(/\s/gu, '').toUpperCase();
  if (!/^TR\d{2}\d{5}0[A-Z0-9]{16}$/.test(normalized)) return null;

  let remainder = 0;
  for (const character of `${normalized.slice(4)}${normalized.slice(0, 4)}`) {
    const code = character.charCodeAt(0);
    if (code >= 48 && code <= 57) {
      remainder = (remainder * 10 + code - 48) % 97;
    } else {
      const numericValue = code - 55;
      remainder = (remainder * 100 + numericValue) % 97;
    }
  }
  return remainder === 1 ? normalized : null;
}

export function isValidTurkishIban(value: unknown): boolean {
  return normalizeTurkishIban(value) !== null;
}

export type AmountInputResult =
  | { valid: true; value: number | undefined }
  | { valid: false; message: string };

/** Parses user-entered TL amounts without turning invalid or negative text into zero/positive. */
export function parseNonNegativeAmountInput(raw: string): AmountInputResult {
  const value = raw.trim();
  if (value === '') return { valid: true, value: undefined };
  if (value.startsWith('-')) {
    const signed = Number(value.replace(',', '.'));
    if (Number.isFinite(signed) && signed < 0) {
      return { valid: false, message: NEGATIVE_PERSONNEL_AMOUNT_MESSAGE };
    }
  }

  if (!/^(?:\d+(?:[.,]\d*)?|[.,]\d+)$/.test(value)) {
    return { valid: false, message: INVALID_PERSONNEL_AMOUNT_MESSAGE };
  }
  const fraction = value.replace(',', '.').split('.')[1] ?? '';
  if (fraction.replace(/0+$/, '').length > 2) {
    return { valid: false, message: INVALID_PERSONNEL_AMOUNT_MESSAGE };
  }
  const parsed = Number(value.replace(',', '.'));
  if (!Number.isFinite(parsed)) {
    return { valid: false, message: INVALID_PERSONNEL_AMOUNT_MESSAGE };
  }
  return { valid: true, value: parsed };
}

function isValidStoredMoney(value: unknown): boolean {
  if (value === undefined || value === null) return true;
  if (typeof value !== 'number' && typeof value !== 'string') return false;
  const text = String(value);
  if (!/^-?(?:0|[1-9]\d*)(?:\.\d+)?$/.test(text)) return false;
  const amount = Number(text);
  if (!Number.isFinite(amount) || amount < 0) return false;
  const fraction = text.split('.')[1]?.replace(/0+$/, '') ?? '';
  return fraction.length <= 2;
}

export interface PersonnelValidationError {
  field: string;
  message: string;
}

/** Save-boundary validation. Snapshot parsing intentionally does not call this. */
export function validatePersonnelForSave(personnel: {
  iban?: unknown;
  kesintiler?: unknown;
}): PersonnelValidationError | null {
  if (!normalizeTurkishIban(personnel.iban)) {
    return { field: 'iban', message: INVALID_TURKISH_IBAN_MESSAGE };
  }

  const deductions = personnel.kesintiler as Record<string, unknown> | undefined;
  for (const field of MONEY_FIELDS) {
    const value = deductions?.[field];
    if (!isValidStoredMoney(value)) {
      return {
        field: `kesintiler.${field}`,
        message: typeof value === 'string' && value.startsWith('-')
          ? NEGATIVE_PERSONNEL_AMOUNT_MESSAGE
          : INVALID_PERSONNEL_AMOUNT_MESSAGE,
      };
    }
  }
  const gvDeductions = deductions?.gvIndirimleri as Record<string, unknown> | undefined;
  for (const field of GV_MONEY_FIELDS) {
    const value = gvDeductions?.[field];
    if (!isValidStoredMoney(value)) {
      return {
        field: `kesintiler.gvIndirimleri.${field}`,
        message: typeof value === 'string' && value.startsWith('-')
          ? 'GV indirim girdisi negatif olamaz.'
          : INVALID_PERSONNEL_AMOUNT_MESSAGE,
      };
    }
  }
  return null;
}

export const DUPLICATE_TC_NO_CODE = 'DUPLICATE_TC_NO';
export const DUPLICATE_TC_NO_MESSAGE =
  'Bu T.C. kimlik numarasıyla kayıtlı bir personel zaten bulunuyor. Lütfen numarayı kontrol edin.';

export class PersonnelSaveError extends Error {
  constructor(readonly type: string, message: string) {
    super(message);
    this.name = 'PersonnelSaveError';
  }
}

export function preparePersonnelForSave<T extends { tcNo: string; iban: string }>(personnel: T): T {
  const validation = validatePersonnelForSave(personnel);
  if (validation) throw new PersonnelSaveError('ValidationError', validation.message);
  return {
    ...personnel,
    tcNo: personnel.tcNo.trim(),
    iban: normalizeTurkishIban(personnel.iban)!,
  };
}

export function assertUniqueTcNo(
  personnel: readonly { id: string; tcNo: string }[],
  candidate: { id: string; tcNo: string }
): void {
  const tcNo = candidate.tcNo.trim();
  if (personnel.some((item) => item.id !== candidate.id && item.tcNo.trim() === tcNo)) {
    throw new PersonnelSaveError(DUPLICATE_TC_NO_CODE, DUPLICATE_TC_NO_MESSAGE);
  }
}

/** Detects only records changed by this write, preserving malformed old snapshots as readable data. */
export function validatePersonnelSnapshotMutation<T extends {
  personeller: Array<{ id: string; tcNo: string; iban: string; kesintiler?: unknown }>;
}>(
  previous: T | null,
  next: T
): T {
  const oldById = new Map((previous?.personeller ?? []).map((person) => [person.id, person]));
  const changedIds = new Set(
    next.personeller
      .filter((person) => JSON.stringify(oldById.get(person.id)) !== JSON.stringify(person))
      .map((person) => person.id)
  );
  if (changedIds.size === 0) return next;

  const personeller = next.personeller.map((person) => {
    if (!changedIds.has(person.id)) return person;
    const prepared = preparePersonnelForSave(person);
    assertUniqueTcNo(next.personeller, prepared);
    return prepared;
  });
  return { ...next, personeller } as T;
}
