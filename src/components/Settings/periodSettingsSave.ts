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
  let paramsSaved = false;
  try {
    await onSaveParams(paramsForm);
    paramsSaved = true;
    await onSaveZamAylari(zamAylariForm);
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
