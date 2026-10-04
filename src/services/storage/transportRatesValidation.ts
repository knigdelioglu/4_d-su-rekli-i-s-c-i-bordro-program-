import type { DönemselKurumDegerleri } from '../../types/payroll';

export { validateTransportRatesForPeriod } from './payrollPayloadSchema';

/** New periods inherit the final rate, never the previous period's dated rows. */
export function transportRateForNewPeriod(settings: DönemselKurumDegerleri, start: string): number {
  return (settings.transportRateChanges ?? []).filter((change) => change.effectiveFrom <= start)
    .at(-1)?.gunlukVasitaYol ?? settings.gunlukVasitaYol;
}
