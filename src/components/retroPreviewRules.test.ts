import { describe, expect, test } from 'bun:test';
import { consumeRetroPreviewRefreshSuppression } from './retroPreviewRules';

describe('retro preview refresh rules', () => {
  test('keeps the preview through persistence refreshes until visible, then invalidates later refreshes', () => {
    const guard = { suppressUntilPreviewVisible: true };
    let preview: { totalGrossDelta: number } | null = null;

    // Revision persistence can cause more than one authoritative prop refresh.
    expect(consumeRetroPreviewRefreshSuppression(guard, preview !== null)).toBe(true);
    expect(consumeRetroPreviewRefreshSuppression(guard, preview !== null)).toBe(true);
    expect(guard.suppressUntilPreviewVisible).toBe(true);

    // The preview state update itself commits after the save/reload cycle.
    preview = { totalGrossDelta: 2543.28 };
    expect(consumeRetroPreviewRefreshSuppression(guard, preview !== null)).toBe(true);
    expect(guard.suppressUntilPreviewVisible).toBe(false);
    expect(preview).toEqual({ totalGrossDelta: 2543.28 });

    // A later authoritative change must invalidate the now-visible preview.
    expect(consumeRetroPreviewRefreshSuppression(guard, preview !== null)).toBe(false);
    preview = null;
    expect(preview).toEqual(null);
  });
});
