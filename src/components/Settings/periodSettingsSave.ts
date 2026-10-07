import type { PayrollPeriodInstitutionSettings } from '../../types/payroll';

export type PeriodSettingsSaveOutcome =
  | { kind: 'success' }
  | { kind: 'error'; message: string; paramsForm: PayrollPeriodInstitutionSettings };

export function formatPeriodSettingsSaveError(error: unknown, context = 'Kurum ayarları'): string {
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
    if (typeof item === 'string' && /^-?\d+(\.\d+)?$/.test(item)) return Number(item);
    return item;
  };
  return JSON.stringify(normalize(value));
}

export function sameSettingsValue(left: unknown, right: unknown): boolean {
  return stableSettingsJson(left) === stableSettingsJson(right);
}

export async function savePeriodSettings(
  paramsForm: PayrollPeriodInstitutionSettings,
  persistedParams: PayrollPeriodInstitutionSettings,
  onSaveParams: (settings: PayrollPeriodInstitutionSettings) => Promise<void> | void
): Promise<PeriodSettingsSaveOutcome> {
  if (sameSettingsValue(paramsForm, persistedParams)) return { kind: 'success' };
  try {
    await onSaveParams(paramsForm);
    return { kind: 'success' };
  } catch (saveError) {
    return {
      kind: 'error',
      message: formatPeriodSettingsSaveError(saveError),
      paramsForm: persistedParams,
    };
  }
}
