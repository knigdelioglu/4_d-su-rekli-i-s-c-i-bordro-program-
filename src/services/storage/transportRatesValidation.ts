import type { LegacyInstitutionSettings } from '../../types/payroll';

export { validateTransportRatesForPeriod } from './payrollPayloadSchema';

/** New periods inherit the final rate, never the previous period's dated rows. */
export function transportRateForNewPeriod(
  settings: Pick<LegacyInstitutionSettings, 'transportRateChanges' | 'gunlukVasitaYol'>,
  start: string
): number {
  return (settings.transportRateChanges ?? []).filter((change) => change.effectiveFrom <= start)
    .at(-1)?.gunlukVasitaYol ?? settings.gunlukVasitaYol;
}
