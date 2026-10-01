import type { DönemselKurumDegerleri } from '../../types/payroll';

export type PeriodSettingsSaveOutcome =
  | { kind: 'success' }
  | {
      kind: 'error' | 'partial-error';
      message: string;
      paramsForm: DönemselKurumDegerleri;
      zamAylariForm: number[];
    };

export function formatPeriodSettingsSaveError(error: unknown, context = 'Ücret ayarları'): string {
  let detail: string | undefined;

  if (typeof error === 'string') {
    detail = error.trim();
  } else if (error instanceof Error) {
    detail = error.message.trim();
  } else if (error && typeof error === 'object') {
    const tagged = error as { type?: unknown; message?: unknown };
    if (tagged.type === 'PayrollFinalized') {
      const detail = typeof tagged.message === 'string' ? ` ${tagged.message.trim()}` : '';
      return `Kesinleştirilmiş bordro nedeniyle ${context.toLocaleLowerCase('tr-TR')} değiştirilemedi.${detail}`;
    }
    if (typeof tagged.message === 'string') {
      detail = tagged.message.trim();
    } else if (tagged.message != null) {
      try {
        detail = JSON.stringify(tagged.message);
      } catch {
        // Use the standard fallback below for non-serializable native errors.
      }
    }
  }

  return detail
    ? `${context} kaydedilemedi: ${detail}`
    : `${context} kaydedilemedi. Lütfen tekrar deneyin.`;
}

export async function savePeriodSettings(
  paramsForm: DönemselKurumDegerleri,
  zamAylariForm: number[],
  persistedParams: DönemselKurumDegerleri,
  persistedZamAylari: number[],
  onSaveParams: (settings: DönemselKurumDegerleri) => Promise<void> | void,
  onSaveZamAylari: (months: number[]) => Promise<void> | void
): Promise<PeriodSettingsSaveOutcome> {
  // Each persisted save is a payroll mutation: period settings invalidate the
  // period and every later period, and the zam schedule invalidates ALL
  // payrolls. Unchanged sections must therefore not be re-saved; otherwise a
  // single "Kaydet" click on one field marks the entire ledger STALE (or is
  // rejected because some unrelated history is FINALIZED).
  const paramsChanged = !sameSettingsValue(paramsForm, persistedParams);
  const zamAylariChanged = !sameZamAylari(zamAylariForm, persistedZamAylari);
  let paramsSaved = false;
  try {
    if (paramsChanged) await onSaveParams(paramsForm);
    paramsSaved = true;
    if (zamAylariChanged) await onSaveZamAylari(zamAylariForm);
    return { kind: 'success' };
  } catch (saveError) {
    return {
      kind: paramsSaved ? 'partial-error' : 'error',
      message: paramsSaved
        ? `Ücret ayarları kaydedildi. ${formatPeriodSettingsSaveError(saveError, 'Zam takvimi')}`
        : formatPeriodSettingsSaveError(saveError),
      paramsForm: paramsSaved ? paramsForm : persistedParams,
      zamAylariForm: [...persistedZamAylari],
    };
  }
}

function stableSettingsJson(value: unknown): string {
  const normalize = (item: unknown): unknown => {
    if (Array.isArray(item)) return item.map(normalize);
    if (item && typeof item === 'object') {
      return Object.fromEntries(
        Object.entries(item as Record<string, unknown>)
          .filter(([, entry]) => entry !== undefined)
          .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
          .map(([key, entry]) => [key, normalize(entry)])
      );
    }
    // UI forms keep numbers while the persisted boundary may keep exact
    // Decimal strings; compare their numeric meaning.
    if (typeof item === 'string' && /^-?\d+(\.\d+)?$/.test(item)) return Number(item);
    return item;
  };
  return JSON.stringify(normalize(value));
}

export function sameSettingsValue(left: unknown, right: unknown): boolean {
  return stableSettingsJson(left) === stableSettingsJson(right);
}

export function sameZamAylari(left: number[], right: number[]): boolean {
  const normalize = (months: number[]) =>
    [...new Set(months.filter((month) => Number.isInteger(month)))].sort((a, b) => a - b);
  const a = normalize(left);
  const b = normalize(right);
  return a.length === b.length && a.every((month, index) => month === b[index]);
}
