export interface RetroPreviewRefreshGuard {
  suppressUntilPreviewVisible: boolean;
}

/** Keep save-triggered refreshes from clearing the result until it is rendered. */
export function consumeRetroPreviewRefreshSuppression(
  guard: RetroPreviewRefreshGuard,
  previewVisible: boolean
): boolean {
  if (!guard.suppressUntilPreviewVisible) return false;
  if (previewVisible) guard.suppressUntilPreviewVisible = false;
  return true;
}
