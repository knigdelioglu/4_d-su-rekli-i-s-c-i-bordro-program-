import type {
  CompensationPolicyBoundaryIntent,
  CompensationPolicyDraftValidation,
  CompensationPolicyNullableDecimalDraft,
  CompensationPolicyPageProps,
  CompensationPolicyStateDraft,
  CompensationPolicyStateWire,
  CompensationPolicyVersionWire,
  CompensationPolicyWriteOperation,
} from '../../services/compensationPolicy/contract';

const MAX_DECIMAL_COEFFICIENT = 79_228_162_514_264_337_593_543_950_335n;
const MAX_DECIMAL_SCALE = 28;
const DECIMAL_INPUT_PATTERN = /^([+-]?)(?:(\d+)(?:([.,])(\d+))?|([.,])(\d+))$/;

export type PolicyMoneyDraftField =
  | 'gunlukTabanUcret'
  | 'gunlukYemek'
  | 'birlestirilmisSosyalYardim'
  | 'gunlukVasitaYol'
  | 'giyimYardimi'
  | 'hizmetZammiBirimi'
  | 'ekOdeme'
  | 'digerGelirVarsayilan';

export type ExactDecimalResult =
  | { ok: true; value: string }
  | { ok: false; error: string };

export type CompensationPolicyDraftToStateResult =
  | { ok: true; state: CompensationPolicyStateWire }
  | { ok: false; validation: CompensationPolicyDraftValidation };

export type CompensationPolicyDraftToOperationResult =
  | { ok: true; operation: CompensationPolicyWriteOperation }
  | { ok: false; validation: CompensationPolicyDraftValidation };

export type CompensationPolicyPercentResult =
  | { ok: true; draft: CompensationPolicyStateDraft }
  | { ok: false; error: string };

function canonicalFromParts(sign: string, wholePart: string, fractionPart: string): ExactDecimalResult {
  const whole = wholePart.replace(/^0+(?=\d)/, '') || '0';
  let fraction = fractionPart;

  while (fraction.endsWith('0')) fraction = fraction.slice(0, -1);
  const isZero = /^0+$/.test(whole) && fraction.length === 0;
  const scale = fraction.length;
  const coefficient = BigInt(`${whole}${fraction}` || '0');

  if (scale > MAX_DECIMAL_SCALE) {
    return {
      ok: false,
      error: `Rust Decimal en çok ${MAX_DECIMAL_SCALE} anlamlı ondalık basamağı destekler.`,
    };
  }
  if (coefficient > MAX_DECIMAL_COEFFICIENT) {
    return { ok: false, error: 'Tutar Rust Decimal kapasitesini aşıyor.' };
  }

  return {
    ok: true,
    value: `${sign === '-' && !isZero ? '-' : ''}${whole}${fraction ? `.${fraction}` : ''}`,
  };
}

/** Converts Turkish comma or plain-dot input to a canonical exact decimal string. */
export function parseCompensationPolicyDecimal(raw: string): ExactDecimalResult {
  const text = raw.trim();
  const match = DECIMAL_INPUT_PATTERN.exec(text);
  if (!match) {
    return {
      ok: false,
      error: 'Ondalık değer girin; binlik ayırıcı kullanmayın. Örnek: 1234,56.',
    };
  }

  const sign = match[1] ?? '';
  const wholePart = match[2] ?? '0';
  const fractionPart = match[4] ?? match[6] ?? '';
  return canonicalFromParts(sign, wholePart, fractionPart);
}

/** Changes only the decimal separator for Turkish text inputs; no numeric coercion occurs. */
export function formatCompensationPolicyDecimalForUser(value: string): string {
  return value.replace('.', ',');
}

function decimalParts(canonical: string): { coefficient: bigint; scale: number } {
  const negative = canonical.startsWith('-');
  const unsigned = negative ? canonical.slice(1) : canonical;
  const [whole, fraction = ''] = unsigned.split('.');
  const coefficient = BigInt(`${whole}${fraction}` || '0');
  return { coefficient: negative ? -coefficient : coefficient, scale: fraction.length };
}

function exactDecimalFromCoefficient(coefficient: bigint, scale: number): ExactDecimalResult {
  const negative = coefficient < 0n;
  let magnitude = negative ? -coefficient : coefficient;
  let normalizedScale = scale;

  while (normalizedScale > 0 && magnitude % 10n === 0n) {
    magnitude /= 10n;
    normalizedScale -= 1;
  }

  if (normalizedScale > MAX_DECIMAL_SCALE) {
    return {
      ok: false,
      error: `Hesaplanan değer Rust Decimal ölçeğini (${MAX_DECIMAL_SCALE}) aşıyor; yuvarlama uygulanmadı.`,
    };
  }
  if (magnitude > MAX_DECIMAL_COEFFICIENT) {
    return { ok: false, error: 'Hesaplanan değer Rust Decimal kapasitesini aşıyor.' };
  }

  const digits = magnitude.toString().padStart(normalizedScale + 1, '0');
  const whole = normalizedScale === 0 ? digits : digits.slice(0, -normalizedScale);
  const fraction = normalizedScale === 0 ? '' : digits.slice(-normalizedScale);
  return {
    ok: true,
    value: `${negative && magnitude !== 0n ? '-' : ''}${whole}${fraction ? `.${fraction}` : ''}`,
  };
}

const requiredDecimalFields = [
  'gunlukTabanUcret',
  'gunlukYemek',
  'birlestirilmisSosyalYardim',
  'gunlukVasitaYol',
  'giyimYardimi',
  'hizmetZammiBirimi',
  'geceCalismaPrimiYuzde',
  'geceCalismaTatiliPrimiYuzde',
] as const satisfies readonly (keyof CompensationPolicyStateDraft)[];

type RequiredDecimalField = (typeof requiredDecimalFields)[number];

const moneyLabels: Record<PolicyMoneyDraftField, string> = {
  gunlukTabanUcret: 'Günlük taban ücret',
  gunlukYemek: 'Günlük yemek',
  birlestirilmisSosyalYardim: 'Birleştirilmiş sosyal yardım',
  gunlukVasitaYol: 'Günlük vasıta/yol',
  giyimYardimi: 'Giyim yardımı',
  hizmetZammiBirimi: 'Hizmet zammı birimi',
  ekOdeme: 'Ek ödeme',
  digerGelirVarsayilan: 'Diğer gelir varsayılanı',
};

function dateError(value: string): string | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return 'Geçerlilik tarihi YYYY-AA-GG biçiminde olmalıdır.';
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (year < 1 || month < 1 || month > 12 || day < 1) return 'Geçerlilik tarihi geçerli bir takvim günü olmalıdır.';
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const daysInMonth = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  return day <= daysInMonth ? null : 'Geçerlilik tarihi geçerli bir takvim günü olmalıdır.';
}

function readRequiredDecimal(raw: string, field: keyof CompensationPolicyStateDraft, errors: Partial<Record<keyof CompensationPolicyStateDraft, string>>, values: Map<string, string>): void {
  if (!raw.trim()) {
    errors[field] = 'Bu alan zorunludur; boş değer sıfır sayılmaz.';
    return;
  }
  const result = parseCompensationPolicyDecimal(raw);
  if (result.ok === false) {
    errors[field] = result.error;
    return;
  }
  values.set(field, result.value);
}

function readNullableDecimal(
  input: CompensationPolicyNullableDecimalDraft,
  field: keyof CompensationPolicyStateDraft,
  errors: Partial<Record<keyof CompensationPolicyStateDraft, string>>,
  values: Map<string, string | null>
): void {
  if (input.kind === 'ABSENT') {
    values.set(field, null);
    return;
  }
  if (!input.raw.trim()) {
    errors[field] = 'Değer girin veya alanı “Yok” olarak bırakın.';
    return;
  }
  const result = parseCompensationPolicyDecimal(input.raw);
  if (result.ok === false) {
    errors[field] = result.error;
    return;
  }
  values.set(field, result.value);
}

function invalidStateErrors(state: CompensationPolicyStateWire): string[] {
  const errors: string[] = [];
  const base = decimalParts(state.gunlukTabanUcret);
  if (base.coefficient <= 0n) errors.push('Günlük taban ücret sıfırdan büyük olmalıdır.');

  const nonNegativeFields = [
    ['gunlukYemek', state.gunlukYemek],
    ['birlestirilmisSosyalYardim', state.birlestirilmisSosyalYardim],
    ['gunlukVasitaYol', state.gunlukVasitaYol],
    ['giyimYardimi', state.giyimYardimi],
    ['hizmetZammiBirimi', state.hizmetZammiBirimi],
  ] as const;
  for (const [field, value] of nonNegativeFields) {
    if (decimalParts(value).coefficient < 0n) errors.push(`${moneyLabels[field]} negatif olamaz.`);
  }

  const percentageFields = [
    ['isPrimiYuzde', state.isPrimiYuzde],
    ['geceCalismaPrimiYuzde', state.geceCalismaPrimiYuzde],
    ['geceCalismaTatiliPrimiYuzde', state.geceCalismaTatiliPrimiYuzde],
  ] as const;
  for (const [field, value] of percentageFields) {
    if (value === null) continue;
    const coefficient = decimalParts(value).coefficient;
    const scale = decimalParts(value).scale;
    if (coefficient < 0n || coefficient > 100n * 10n ** BigInt(scale)) {
      errors.push(`${field === 'isPrimiYuzde' ? 'İş primi yüzdesi' : field === 'geceCalismaPrimiYuzde' ? 'Gece çalışma primi' : 'Gece çalışma tatili primi'} %0 ile %100 arasında olmalıdır.`);
    }
  }

  for (const [field, value] of [
    ['ekOdeme', state.ekOdeme],
    ['digerGelirVarsayilan', state.digerGelirVarsayilan],
  ] as const) {
    if (value !== null && decimalParts(value).coefficient < 0n) {
      errors.push(`${moneyLabels[field]} negatif olamaz.`);
    }
  }

  if (state.isPrimiGruplari.length === 0) errors.push('İş primi grupları tam ücret durumunda boş olamaz.');
  const activeOwners = new Map<string, number>();
  state.isPrimiGruplari.forEach((group, index) => {
    if (!group.id.trim() || !group.ad.trim() || group.id.trim() !== group.id || group.ad.trim() !== group.ad) {
      errors.push(`İş primi ${index + 1}: grup kodu ve adı boşluksuz, başında/sonunda boşluk olmadan girilmelidir.`);
    }
    const rate = decimalParts(group.oran);
    if (rate.coefficient < 0n || rate.coefficient > 100n * 10n ** BigInt(rate.scale)) {
      errors.push(`İş primi ${index + 1}: oran %0 ile %100 arasında olmalıdır.`);
    }
    if (!group.aktif) return;
    for (const identifier of [group.id, group.ad]) {
      const owner = activeOwners.get(identifier);
      if (owner !== undefined && owner !== index) {
        errors.push(`Aktif iş primi gruplarında yinelenen veya belirsiz kod/ad: ${identifier}.`);
      } else {
        activeOwners.set(identifier, index);
      }
    }
  });
  for (const [label, policy] of Object.entries(state.socialBenefitAccrualPolicies ?? {})) {
    if (!policy) continue;
    if ((policy.method === 'FULL_IF_ELIGIBLE_DAY' || policy.method === 'PROPORTIONAL') && policy.eligibleDayTypes.length === 0) {
      errors.push(`${label}: bu ödeme yöntemi için en az bir hak kazandıran gün türü seçilmelidir.`);
    }
    if (new Set(policy.eligibleDayTypes).size !== policy.eligibleDayTypes.length) {
      errors.push(`${label}: aynı hak kazandıran gün türü birden fazla seçilemez.`);
    }
  }
  return [...new Set(errors)];
}

function buildFullState(draft: CompensationPolicyStateDraft): CompensationPolicyDraftToStateResult {
  const fieldErrors: Partial<Record<keyof CompensationPolicyStateDraft, string>> = {};
  const requiredValues = new Map<string, string>();
  for (const field of requiredDecimalFields) readRequiredDecimal(draft[field], field, fieldErrors, requiredValues);

  const nullableValues = new Map<string, string | null>();
  readNullableDecimal(draft.isPrimiYuzde, 'isPrimiYuzde', fieldErrors, nullableValues);
  readNullableDecimal(draft.ekOdeme, 'ekOdeme', fieldErrors, nullableValues);
  readNullableDecimal(draft.digerGelirVarsayilan, 'digerGelirVarsayilan', fieldErrors, nullableValues);

  const groups = draft.isPrimiGruplari.map((group, index) => {
    const result = parseCompensationPolicyDecimal(group.oran);
    if (!group.oran.trim()) {
      fieldErrors.isPrimiGruplari = `İş primi ${index + 1}: oran zorunludur.`;
    } else if (result.ok === false) {
      fieldErrors.isPrimiGruplari = `İş primi ${index + 1}: ${result.error}`;
    }
    return { ...group, oran: result.ok === true ? result.value : '' };
  });
  const socialBenefitAccrualPolicies = Object.values(draft.socialBenefitAccrualPolicies).some(Boolean)
    ? draft.socialBenefitAccrualPolicies
    : null;

  if (Object.keys(fieldErrors).length > 0) {
    return {
      ok: false,
      validation: {
        draft: 'INVALID',
        fullState: { status: 'INCOMPLETE', fieldErrors },
        operationErrors: [],
      },
    };
  }

  const state = {
    gunlukTabanUcret: requiredValues.get('gunlukTabanUcret')!,
    gunlukYemek: requiredValues.get('gunlukYemek')!,
    birlestirilmisSosyalYardim: requiredValues.get('birlestirilmisSosyalYardim')!,
    gunlukVasitaYol: requiredValues.get('gunlukVasitaYol')!,
    giyimYardimi: requiredValues.get('giyimYardimi')!,
    hizmetZammiBirimi: requiredValues.get('hizmetZammiBirimi')!,
    isPrimiYuzde: nullableValues.get('isPrimiYuzde')!,
    isPrimiGruplari: groups,
    geceCalismaPrimiYuzde: requiredValues.get('geceCalismaPrimiYuzde')!,
    geceCalismaTatiliPrimiYuzde: requiredValues.get('geceCalismaTatiliPrimiYuzde')!,
    ekOdeme: nullableValues.get('ekOdeme')!,
    digerGelirVarsayilan: nullableValues.get('digerGelirVarsayilan')!,
    socialBenefitAccrualPolicies,
  } satisfies CompensationPolicyStateWire;
  const errors = invalidStateErrors(state);
  if (errors.length > 0) {
    return {
      ok: false,
      validation: { draft: 'INVALID', fullState: { status: 'INVALID', errors }, operationErrors: [] },
    };
  }
  return { ok: true, state };
}

function operationErrors(draft: CompensationPolicyPageProps['draft']): string[] {
  const errors: string[] = [];
  const invalidDate = dateError(draft.effectiveFrom);
  if (invalidDate) errors.push(invalidDate);
  if (!draft.reason.trim()) errors.push('Değişiklik gerekçesi boş bırakılamaz.');
  if (!['FULL_EARNING', 'ROAD_ONLY'].includes(draft.boundaryScope)) errors.push('Ücret değişikliği kapsamını seçin.');
  if (!['PER_COMPENSATION_SLICE', 'PERIOD_TOTAL'].includes(draft.roadRoundingMode)) errors.push('Yol tutarı yuvarlama biçimini seçin.');
  if (draft.operationKind === 'SAME_DATE_CORRECTION' && !draft.supersedesVersionId) {
    errors.push('Aynı gün düzeltmesi için o tarihteki etkin sürüm seçilmelidir.');
  }
  if (draft.operationKind === 'NEW_EFFECTIVE_DATE' && draft.supersedesVersionId !== null) {
    errors.push('Yeni geçerlilik tarihi önceki sürüm için düzeltme bağlantısı taşıyamaz.');
  }
  return errors;
}

export function validateCompensationPolicyDraft(draft: CompensationPolicyPageProps['draft']): CompensationPolicyDraftValidation {
  const stateResult = buildFullState(draft);
  const errors = operationErrors(draft);
  const fullState = stateResult.ok === true ? { status: 'VALID' as const, state: stateResult.state } : stateResult.validation.fullState;
  const stateValid = fullState.status === 'VALID';
  return {
    draft: stateValid && errors.length === 0 ? 'VALID' : 'INVALID',
    fullState,
    operationErrors: errors,
  };
}

/** Field-level form errors; operation-level errors stay in the page summary. */
export function getCompensationPolicyDraftFieldErrors(
  draft: CompensationPolicyStateDraft
): Partial<Record<keyof CompensationPolicyStateDraft, string>> {
  const result = buildFullState(draft);
  if (result.ok === false) {
    if (result.validation.fullState.status === 'INCOMPLETE') return result.validation.fullState.fieldErrors;
    if (result.validation.fullState.status === 'INVALID') {
      const fieldErrors: Partial<Record<keyof CompensationPolicyStateDraft, string>> = {};
      for (const error of result.validation.fullState.errors) {
        const lower = error.toLocaleLowerCase('tr-TR');
        const field: keyof CompensationPolicyStateDraft | null =
          lower.includes('taban ücret') ? 'gunlukTabanUcret'
            : lower.includes('günlük yemek') ? 'gunlukYemek'
              : lower.includes('birleştirilmiş sosyal yardım') ? 'birlestirilmisSosyalYardim'
                : lower.includes('vasıta/yol') ? 'gunlukVasitaYol'
                  : lower.includes('giyim') ? 'giyimYardimi'
                    : lower.includes('hizmet zammı') ? 'hizmetZammiBirimi'
                      : lower.includes('isprimiyuzde') || lower.includes('iş primi yüzdesi') ? 'isPrimiYuzde'
                        : lower.includes('gececalismatatili') || lower.includes('gece çalışma tatili') ? 'geceCalismaTatiliPrimiYuzde'
                          : lower.includes('gececalismaprimi') || lower.includes('gece çalışma primi') ? 'geceCalismaPrimiYuzde'
                            : lower.includes('ekodeme') || lower.includes('ek ödeme') ? 'ekOdeme'
                              : lower.includes('digergelirvarsayilan') || lower.includes('diğer gelir') ? 'digerGelirVarsayilan'
                                : lower.includes('iş primi') || lower.includes('grup') ? 'isPrimiGruplari'
                                  : null;
        if (field && !fieldErrors[field]) fieldErrors[field] = error;
      }
      return fieldErrors;
    }
  }
  return {};
}

/** Converts a fully validated raw form draft to exact wire state, or returns typed errors. */
export function draftToCompensationPolicyState(draft: CompensationPolicyStateDraft): CompensationPolicyDraftToStateResult {
  return buildFullState(draft);
}

/** Converts a fully validated draft to the frozen exact operation contract. */
export function draftToCompensationPolicyOperation(
  draft: CompensationPolicyPageProps['draft']
): CompensationPolicyDraftToOperationResult {
  const validation = validateCompensationPolicyDraft(draft);
  if (validation.fullState.status !== 'VALID' || validation.operationErrors.length > 0) {
    return { ok: false, validation };
  }
  const { state } = validation.fullState;
  const common = {
    effectiveFrom: draft.effectiveFrom,
    state,
    roadRoundingMode: draft.roadRoundingMode,
    reason: draft.reason.trim(),
  };
  if (draft.operationKind === 'INITIAL_BASELINE') {
    return { ok: true, operation: { ...common, kind: 'INITIAL_BASELINE', boundaryScope: draft.boundaryScope } };
  }
  const intent: CompensationPolicyBoundaryIntent = draft.boundaryScope === 'ROAD_ONLY'
    ? { boundaryScope: 'ROAD_ONLY', changeIntent: 'ROAD_AMOUNT_ONLY' }
    : { boundaryScope: 'FULL_EARNING', changeIntent: 'FULL_STATE_CHANGE' };
  if (draft.operationKind === 'NEW_EFFECTIVE_DATE') {
    return { ok: true, operation: { ...common, ...intent, kind: 'NEW_EFFECTIVE_DATE' } };
  }
  return {
    ok: true,
    operation: {
      ...common,
      ...intent,
      kind: 'SAME_DATE_CORRECTION',
      supersedesVersionId: draft.supersedesVersionId!,
    },
  };
}

function rawDecimal(value: unknown, field: string): string {
  if (typeof value !== 'string') {
    throw new Error(`${field} prefill değeri exact decimal string olmalıdır; sayısal değerler kabul edilmez.`);
  }
  return formatCompensationPolicyDecimalForUser(value);
}

function nullableDraftFromPartial(source: Partial<CompensationPolicyStateWire>, key: 'isPrimiYuzde' | 'ekOdeme' | 'digerGelirVarsayilan'): CompensationPolicyNullableDecimalDraft {
  if (!Object.prototype.hasOwnProperty.call(source, key) || source[key] === null) return { kind: 'ABSENT' };
  return { kind: 'VALUE', raw: rawDecimal(source[key], key) };
}

/** Maps exact partial prefill or full state into editable raw strings without financial defaults. */
export function compensationPolicyStateToDraft(source: Partial<CompensationPolicyStateWire>): CompensationPolicyStateDraft {
  const read = (key: RequiredDecimalField): string => {
    if (!Object.prototype.hasOwnProperty.call(source, key)) return '';
    return rawDecimal(source[key], key);
  };
  const groups = source.isPrimiGruplari;
  if (groups !== undefined && !Array.isArray(groups)) throw new Error('İş primi grupları prefill listesi geçersiz.');
  const savedAccrualPolicies = source.socialBenefitAccrualPolicies;
  const socialBenefitAccrualPolicies = {
    birlestirilmisSosyalYardim: savedAccrualPolicies?.birlestirilmisSosyalYardim ?? null,
    giyimYardimi: savedAccrualPolicies?.giyimYardimi ?? null,
    hizmetZammi: savedAccrualPolicies?.hizmetZammi ?? null,
    ekOdeme: savedAccrualPolicies?.ekOdeme ?? null,
    digerGelir: savedAccrualPolicies?.digerGelir ?? null,
  };
  return {
    gunlukTabanUcret: read('gunlukTabanUcret'),
    gunlukYemek: read('gunlukYemek'),
    birlestirilmisSosyalYardim: read('birlestirilmisSosyalYardim'),
    gunlukVasitaYol: read('gunlukVasitaYol'),
    giyimYardimi: read('giyimYardimi'),
    hizmetZammiBirimi: read('hizmetZammiBirimi'),
    isPrimiYuzde: nullableDraftFromPartial(source, 'isPrimiYuzde'),
    isPrimiGruplari: (groups ?? []).map((group, index) => {
      if (typeof group.id !== 'string' || typeof group.ad !== 'string' || typeof group.aktif !== 'boolean') {
        throw new Error(`İş primi ${index + 1} grup kimliği, adı ve etkin durumu eksik.`);
      }
      return { id: group.id, ad: group.ad, oran: rawDecimal(group.oran, `isPrimiGruplari[${index}].oran`), aktif: group.aktif };
    }),
    geceCalismaPrimiYuzde: read('geceCalismaPrimiYuzde'),
    geceCalismaTatiliPrimiYuzde: read('geceCalismaTatiliPrimiYuzde'),
    ekOdeme: nullableDraftFromPartial(source, 'ekOdeme'),
    digerGelirVarsayilan: nullableDraftFromPartial(source, 'digerGelirVarsayilan'),
    socialBenefitAccrualPolicies,
  };
}

/** Empty raw draft: required values stay blank and optional amounts stay absent. */
export function createEmptyCompensationPolicyStateDraft(): CompensationPolicyStateDraft {
  return {
    gunlukTabanUcret: '',
    gunlukYemek: '',
    birlestirilmisSosyalYardim: '',
    gunlukVasitaYol: '',
    giyimYardimi: '',
    hizmetZammiBirimi: '',
    isPrimiYuzde: { kind: 'ABSENT' },
    isPrimiGruplari: [],
    geceCalismaPrimiYuzde: '',
    geceCalismaTatiliPrimiYuzde: '',
    ekOdeme: { kind: 'ABSENT' },
    digerGelirVarsayilan: { kind: 'ABSENT' },
    socialBenefitAccrualPolicies: {
      birlestirilmisSosyalYardim: null,
      giyimYardimi: null,
      hizmetZammi: null,
      ekOdeme: null,
      digerGelir: null,
    },
  };
}

/** Deep-copies full form state while preserving absent/null and open numeric text. */
export function copyCompensationPolicyStateDraft(draft: CompensationPolicyStateDraft): CompensationPolicyStateDraft {
  return {
    ...draft,
    isPrimiYuzde: { ...draft.isPrimiYuzde },
    ekOdeme: { ...draft.ekOdeme },
    digerGelirVarsayilan: { ...draft.digerGelirVarsayilan },
    socialBenefitAccrualPolicies: Object.fromEntries(
      Object.entries(draft.socialBenefitAccrualPolicies).map(([key, policy]) => [
        key,
        policy ? { ...policy, eligibleDayTypes: [...policy.eligibleDayTypes] } : null,
      ])
    ) as CompensationPolicyStateDraft['socialBenefitAccrualPolicies'],
    isPrimiGruplari: draft.isPrimiGruplari.map((group) => ({ ...group })),
  };
}

/** Applies an exact percentage increase to selected monetary fields; no replay rule or rounding is stored. */
export function applyCompensationPolicyPercentageIncrease(
  draft: CompensationPolicyStateDraft,
  percentageRaw: string,
  fields: readonly ('gunlukTabanUcret' | 'gunlukYemek')[] = ['gunlukTabanUcret', 'gunlukYemek']
): CompensationPolicyPercentResult {
  const percentage = parseCompensationPolicyDecimal(percentageRaw);
  if (percentage.ok === false) return percentage;
  const percentParts = decimalParts(percentage.value);
  if (percentParts.coefficient < 0n || percentParts.coefficient > 100n * 10n ** BigInt(percentParts.scale)) {
    return { ok: false, error: 'Artış oranı %0 ile %100 arasında olmalıdır.' };
  }

  const result = copyCompensationPolicyStateDraft(draft);
  for (const field of fields) {
    const amount = parseCompensationPolicyDecimal(draft[field]);
    if (amount.ok === false) return { ok: false, error: `${moneyLabels[field]} geçerli olmadan artış uygulanamaz: ${amount.error}` };
    const amountParts = decimalParts(amount.value);
    if (amountParts.coefficient < 0n) return { ok: false, error: `${moneyLabels[field]} negatif olamaz.` };
    const scaleUnit = 10n ** BigInt(percentParts.scale);
    const multiplier = 100n * scaleUnit + percentParts.coefficient;
    const increased = exactDecimalFromCoefficient(
      amountParts.coefficient * multiplier,
      amountParts.scale + percentParts.scale + 2
    );
    if (increased.ok === false) return increased;
    result[field] = formatCompensationPolicyDecimalForUser(increased.value);
  }
  return { ok: true, draft: result };
}

/** Returns the active leaf from the scoped versions at one exact effective date. */
export function findScopedSameDateActiveLeaf(
  versions: readonly CompensationPolicyVersionWire[],
  effectiveFrom: string
): string | null {
  const sameDate = versions.filter((version) => version.effectiveFrom === effectiveFrom);
  if (sameDate.length === 0) return null;
  const supersededIds = new Set(sameDate.flatMap((version) => version.supersedesVersionId ? [version.supersedesVersionId] : []));
  const leaves = sameDate.filter((version) => !supersededIds.has(version.id));
  return leaves.length === 1 ? leaves[0]!.id : null;
}
