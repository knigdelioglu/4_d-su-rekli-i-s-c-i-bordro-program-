type StatutorySettingsLike = {
  gunlukAsgariUcret?: number | string;
  sgkIsciOraniYuzde?: number | string;
  issizlikIsciOraniYuzde?: number | string;
  pekTavanKatsayisi?: number | string;
  gunlukYemekIstisnasiSGK?: number | string;
  gunlukYemekIstisnasiGV?: number | string;
  statutoryParameterSegments?: unknown[];
  statutoryParameterSnapshot?: unknown;
};

type StatutoryDefinition = {
  gunlukAsgariUcret: number | string | undefined;
  sgkIsciOraniYuzde: number | string | undefined;
  issizlikIsciOraniYuzde: number | string | undefined;
  pekTavanKatsayisi: number | string | undefined;
  gunlukYemekIstisnasiSGK: number | string | undefined;
  gunlukYemekIstisnasiGV: number | string | undefined;
  statutoryParameterSegments: unknown[];
};

/** Compares only mutable statutory inputs; persisted snapshot is deliberately ignored. */
export function currentStatutoryDefinition(
  settings: StatutorySettingsLike
): StatutoryDefinition {
  return {
    gunlukAsgariUcret: settings.gunlukAsgariUcret,
    sgkIsciOraniYuzde: settings.sgkIsciOraniYuzde,
    issizlikIsciOraniYuzde: settings.issizlikIsciOraniYuzde,
    pekTavanKatsayisi: settings.pekTavanKatsayisi,
    gunlukYemekIstisnasiSGK: settings.gunlukYemekIstisnasiSGK,
    gunlukYemekIstisnasiGV:
      settings.gunlukYemekIstisnasiGV ?? settings.gunlukYemekIstisnasiSGK,
    statutoryParameterSegments: settings.statutoryParameterSegments ?? [],
  };
}

export function sameStatutoryDefinition(
  left: StatutorySettingsLike,
  right: StatutorySettingsLike
): boolean {
  return JSON.stringify(currentStatutoryDefinition(left)) ===
    JSON.stringify(currentStatutoryDefinition(right));
}

function snapshotMatchesCurrentDefinition(settings: StatutorySettingsLike): boolean {
  return settings.statutoryParameterSnapshot === undefined ||
    JSON.stringify(settings.statutoryParameterSnapshot) ===
      JSON.stringify(currentStatutoryDefinition(settings));
}

/**
 * Applies the browser equivalent of SettingsRepository's snapshot lifecycle.
 * FINALIZED dependency authorization is performed by the caller's mutation
 * policy before this helper is called.
 */
export function reconcileStatutorySnapshot<T extends StatutorySettingsLike>(
  previous: T | undefined,
  next: T
): T {
  if (!previous) return next;
  if (
    sameStatutoryDefinition(previous, next) &&
    snapshotMatchesCurrentDefinition(previous)
  ) {
    return previous.statutoryParameterSnapshot !== undefined
      ? { ...next, statutoryParameterSnapshot: previous.statutoryParameterSnapshot }
      : next;
  }
  const { statutoryParameterSnapshot: _discarded, ...withoutSnapshot } = next;
  return withoutSnapshot as T;
}
