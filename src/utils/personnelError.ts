/** Formats errors raised while saving personnel records for the form UI. */
export function isDuplicateTcNoError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false;
  const tagged = error as { type?: unknown; code?: unknown };
  return tagged.type === 'DUPLICATE_TC_NO' || tagged.code === 'DUPLICATE_TC_NO';
}

export function formatPersonnelSaveError(error: unknown): string {
  if (error && typeof error === 'object') {
    const tagged = error as { type?: unknown; message?: unknown };

    if (tagged.type === 'DUPLICATE_TC_NO') {
      return 'Bu T.C. kimlik numarasıyla kayıtlı bir personel zaten bulunuyor. Lütfen numarayı kontrol edin.';
    }

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
