/** Formats errors raised while saving personnel records for the form UI. */
export function formatPersonnelSaveError(error: unknown): string {
  if (error && typeof error === 'object') {
    const tagged = error as { type?: unknown; message?: unknown };

    if (tagged.type === 'PayrollFinalized') {
      return 'Bu değişiklik kesinleştirilmiş bir bordroyu etkilediği için personel bilgileri kaydedilemedi.';
    }

    if (typeof tagged.message === 'string') return tagged.message;

    try {
      return JSON.stringify(error) || 'Beklenmeyen bir hata oluştu.';
    } catch {
      return 'Beklenmeyen bir hata oluştu.';
    }
  }

  if (typeof error === 'string') return error;
  return 'Beklenmeyen bir hata oluştu.';
}
