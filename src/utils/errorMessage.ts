/**
 * Converts any thrown value (Error, string, WASM/Tauri tagged DomainError,
 * Map or plain object) into readable text. Never returns "[object Object]".
 */
export function describeError(error: unknown, fallback = 'Beklenmeyen bir hata oluştu.'): string {
  if (error == null) return fallback;
  if (typeof error === 'string') return error.trim() || fallback;
  if (error instanceof Error) {
    const message = error.message.trim();
    return message && message !== '[object Object]' ? message : fallback;
  }
  if (error instanceof Map) {
    return describeError(Object.fromEntries(error), fallback);
  }
  if (typeof error === 'object') {
    const tagged = error as { type?: unknown; message?: unknown; error?: unknown };
    if (typeof tagged.message === 'string' && tagged.message.trim()) return tagged.message.trim();
    if (typeof tagged.error === 'string' && tagged.error.trim()) return tagged.error.trim();
    if (tagged.message != null && typeof tagged.message === 'object') {
      const nested = describeError(tagged.message, '');
      if (nested) return typeof tagged.type === 'string' ? `${tagged.type}: ${nested}` : nested;
    }
    try {
      const json = JSON.stringify(error);
      if (json && json !== '{}') return json;
    } catch {
      // Fall through to the generic message for non-serializable values.
    }
    return typeof tagged.type === 'string' ? tagged.type : fallback;
  }
  return String(error);
}
