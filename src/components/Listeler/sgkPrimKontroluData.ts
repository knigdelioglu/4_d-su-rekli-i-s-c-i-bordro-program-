import type {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  RetroAdjustmentBatch,
  RetroAllocation,
} from '../../types/payroll';
import { isAuthoritativePayroll } from './accrualListData';
import { DEFAULT_KURUM_DEGERLERI } from '../../utils/payrollPresentation';

export type SgkPrimKontroluRowStatus =
  | 'authoritative'
  | 'stale'
  | 'draft'
  | 'missingSnapshot'
  | 'notCalculated';

export interface SgkPrimKontroluRateCandidates {
  isverenSgkOranlari: Array<number | undefined>;
  isverenIssizlikOranlari: Array<number | undefined>;
  isciSgkOranlari: Array<number | undefined>;
  isciIssizlikOranlari: Array<number | undefined>;
}

export interface SgkPrimKontroluRow {
  personel: Personel;
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
  /** Source-period PEK added by authoritative retro allocations. */
  retroPekDelta: number;
  pekAltSinirTamamlamaIsverenPrimi: number;
  dortPrimToplami: number;
  toplam: number;
  status: SgkPrimKontroluRowStatus;
  rateCandidates: SgkPrimKontroluRateCandidates;
}

export interface SgkPrimKontroluTotals {
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
  pekAltSinirTamamlamaIsverenPrimi: number;
  dortPrimToplami: number;
  sgkMutabakatToplami: number;
  /** Backward-compatible alias for callers that still use the old name. */
  genelPrimToplami: number;
  hazirOlmayanPersonelSayisi: number;
  reconciliationReady: boolean;
}

export interface SgkPrimKontroluRateLabels {
  isverenSgk: string;
  isverenIssizlik: string;
  isciSgk: string;
  isciIssizlik: string;
}

type SgkPrimKontroluExcelValue = string | number;

export interface SgkPrimKontroluExcelPayload {
  columns: Array<{ header: string; key: string; width: number }>;
  data: Array<Record<string, SgkPrimKontroluExcelValue>>;
  summaryRows: Array<Record<string, SgkPrimKontroluExcelValue>>;
}

export type SgkPrimComparisonStatus =
  | 'empty'
  | 'invalid'
  | 'incomplete'
  | 'compatible'
  | 'programHigher'
  | 'programLower';

export interface SgkPrimComparison {
  status: SgkPrimComparisonStatus;
  sgkTutarKurus: number | null;
  farkKurus: number | null;
}

const DEFAULT_SGK_RATES = {
  isverenSgkOraniYuzde: DEFAULT_KURUM_DEGERLERI.sgkIsverenOraniYuzde,
  isverenIssizlikOraniYuzde: DEFAULT_KURUM_DEGERLERI.issizlikIsverenOraniYuzde,
  isciSgkOraniYuzde: DEFAULT_KURUM_DEGERLERI.sgkIsciOraniYuzde,
  isciIssizlikOraniYuzde: DEFAULT_KURUM_DEGERLERI.issizlikIsciOraniYuzde,
};

type SgkPrimKontroluResolvedRates = typeof DEFAULT_SGK_RATES;
type SgkPeriodRateKey =
  | 'sgkIsverenOraniYuzde'
  | 'issizlikIsverenOraniYuzde'
  | 'sgkIsciOraniYuzde'
  | 'issizlikIsciOraniYuzde';

const EMPTY_SNAPSHOT_TOTALS = {
  isverenSgkPrimi: 0,
  isverenIssizlikPrimi: 0,
  isciSgkPrimi: 0,
  isciIssizlikPrimi: 0,
  pekAltSinirTamamlamaIsverenPrimi: 0,
};

function isNonNegativeFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function isValidRate(value: unknown): value is number {
  return isNonNegativeFiniteNumber(value) && value <= 100;
}

function resolvePeriodRate(
  institutionSettings: Partial<DönemselKurumDegerleri> | undefined,
  key: SgkPeriodRateKey,
  fallback: number
): number {
  const value = institutionSettings?.[key];
  return isValidRate(value) ? value : fallback;
}

function resolveSgkRates(
  institutionSettings: Partial<DönemselKurumDegerleri> | undefined
): SgkPrimKontroluResolvedRates {
  return {
    isverenSgkOraniYuzde: resolvePeriodRate(
      institutionSettings,
      'sgkIsverenOraniYuzde',
      DEFAULT_SGK_RATES.isverenSgkOraniYuzde
    ),
    isverenIssizlikOraniYuzde: resolvePeriodRate(
      institutionSettings,
      'issizlikIsverenOraniYuzde',
      DEFAULT_SGK_RATES.isverenIssizlikOraniYuzde
    ),
    isciSgkOraniYuzde: resolvePeriodRate(
      institutionSettings,
      'sgkIsciOraniYuzde',
      DEFAULT_SGK_RATES.isciSgkOraniYuzde
    ),
    isciIssizlikOraniYuzde: resolvePeriodRate(
      institutionSettings,
      'issizlikIsciOraniYuzde',
      DEFAULT_SGK_RATES.isciIssizlikOraniYuzde
    ),
  };
}

export function hasCompleteSgkSnapshot(payroll: BordroKaydi | null | undefined): boolean {
  return [
    payroll?.pekDetay?.isverenSgkPrimi,
    payroll?.pekDetay?.isverenIssizlikPrimi,
    payroll?.kesintiler?.isciSgkPrimi,
    payroll?.kesintiler?.isciIssizlikPrimi,
  ].every(isNonNegativeFiniteNumber);
}

function decimalTextToKurus(value: string): number {
  const match = value.match(/^(?<sign>-?)(?<whole>\d+)(?:\.(?<fraction>\d+))?(?:e(?<exponent>[+-]?\d+))?$/i);
  if (!match?.groups) throw new Error(`Geçersiz parasal değer: ${value}`);

  const sign = match.groups.sign === '-' ? -1n : 1n;
  const whole = match.groups.whole;
  const fraction = match.groups.fraction ?? '';
  const exponent = Number(match.groups.exponent ?? '0');
  const decimalPlaces = fraction.length - exponent;
  const digits = BigInt(`${whole}${fraction}`);
  let cents: bigint;

  if (decimalPlaces <= 2) {
    cents = digits * 10n ** BigInt(2 - decimalPlaces);
  } else {
    const divisor = 10n ** BigInt(decimalPlaces - 2);
    const wholeCents = digits / divisor;
    const remainder = digits % divisor;
    cents = wholeCents + (remainder * 2n >= divisor ? 1n : 0n);
  }

  const result = Number(sign * cents);
  if (!Number.isSafeInteger(result)) {
    throw new Error('Parasal değer güvenli kuruş aralığını aşıyor.');
  }
  return result;
}

/** Converts an existing snapshot amount to integer kuruş for exact comparisons. */
export function amountToKurus(value: number | null | undefined): number {
  if (value === null || value === undefined) return 0;
  if (!Number.isFinite(value)) throw new Error('Parasal snapshot değeri sonlu değil.');
  return decimalTextToKurus(value.toString());
}

export function kurusToAmount(value: number): number {
  return value / 100;
}

/**
 * Parses the Turkish TL input convention (for example 757.876,39) directly
 * into kuruş. The value is transient UI input and is never persisted.
 */
export function parseSgkTutarToKurus(value: string): number | null {
  const input = value.trim().replace(/\s+/g, '');
  if (!input) return null;
  if (!/^\d[\d.,]*$/.test(input)) return null;

  let integerPart = input;
  let fractionPart = '';

  if (input.includes(',')) {
    const parts = input.split(',');
    if (parts.length !== 2 || parts[1].length > 2) return null;
    integerPart = parts[0];
    fractionPart = parts[1];
    if (!/^\d+$|^\d{1,3}(?:\.\d{3})+$/.test(integerPart)) return null;
    integerPart = integerPart.replace(/\./g, '');
  } else if (input.includes('.')) {
    const parts = input.split('.');
    const lastPart = parts[parts.length - 1];
    const isDecimalNotation = parts.length === 2 && lastPart.length <= 2;
    if (isDecimalNotation) {
      integerPart = parts[0];
      fractionPart = lastPart;
      if (!/^\d+$/.test(integerPart)) return null;
    } else {
      if (!parts.every((part, index) => index === 0 ? /^\d{1,3}$/.test(part) : /^\d{3}$/.test(part))) {
        return null;
      }
      integerPart = parts.join('');
    }
  }

  if (!/^\d+$/.test(integerPart)) return null;
  const normalizedFraction = `${fractionPart}00`.slice(0, 2);
  const result = Number(BigInt(integerPart) * 100n + BigInt(normalizedFraction));
  return Number.isSafeInteger(result) ? result : null;
}

interface SgkSnapshotKurus {
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
  pekAltSinirTamamlamaIsverenPrimi: number;
}

interface RetroLedgerKurus {
  retroPekDelta: number;
  isverenSgkPrimi: number;
  isverenIssizlikPrimi: number;
  isciSgkPrimi: number;
  isciIssizlikPrimi: number;
}

const EMPTY_RETRO_LEDGER: RetroLedgerKurus = {
  retroPekDelta: 0,
  isverenSgkPrimi: 0,
  isverenIssizlikPrimi: 0,
  isciSgkPrimi: 0,
  isciIssizlikPrimi: 0,
};

function getRetroLedgerKurus(
  periodId: string,
  personnelId: string,
  batches: RetroAdjustmentBatch[],
  allocations: RetroAllocation[]
): RetroLedgerKurus {
  const authoritativeBatchIds = new Set(
    batches
      .filter(
        (batch) =>
          batch.personnelId === personnelId &&
          (batch.status === 'CALCULATED' || batch.status === 'FINALIZED')
      )
      .map((batch) => batch.id)
  );
  return allocations
    .filter(
      (allocation) =>
        allocation.personnelId === personnelId &&
        allocation.sourcePeriodId === periodId &&
        authoritativeBatchIds.has(allocation.batchId)
    )
    .reduce(
      (totals, allocation) => ({
        retroPekDelta: totals.retroPekDelta + amountToKurus(allocation.retroPekDelta),
        isverenSgkPrimi: totals.isverenSgkPrimi + amountToKurus(allocation.employerSgkDelta),
        isverenIssizlikPrimi:
          totals.isverenIssizlikPrimi + amountToKurus(allocation.employerUnemploymentDelta),
        isciSgkPrimi: totals.isciSgkPrimi + amountToKurus(allocation.workerSgkDelta),
        isciIssizlikPrimi:
          totals.isciIssizlikPrimi + amountToKurus(allocation.workerUnemploymentDelta),
      }),
      { ...EMPTY_RETRO_LEDGER }
    );
}

function getSgkSnapshotKurus(payroll: BordroKaydi): SgkSnapshotKurus | null {
  if (!hasCompleteSgkSnapshot(payroll)) return null;

  const pekAltSinirTamamlama = payroll.pekDetay?.pekAltSinirTamamlamaIsverenPrimi;
  if (
    pekAltSinirTamamlama !== null &&
    pekAltSinirTamamlama !== undefined &&
    !isNonNegativeFiniteNumber(pekAltSinirTamamlama)
  ) {
    return null;
  }

  try {
    return {
      isverenSgkPrimi: amountToKurus(payroll.pekDetay?.isverenSgkPrimi),
      isverenIssizlikPrimi: amountToKurus(payroll.pekDetay?.isverenIssizlikPrimi),
      isciSgkPrimi: amountToKurus(payroll.kesintiler?.isciSgkPrimi),
      isciIssizlikPrimi: amountToKurus(payroll.kesintiler?.isciIssizlikPrimi),
      // The optional field is absent in legacy snapshots; that means no
      // lower-bound completion was recorded, so it is safely treated as zero.
      pekAltSinirTamamlamaIsverenPrimi: amountToKurus(pekAltSinirTamamlama),
    };
  } catch {
    return null;
  }
}

function sumSnapshotValues(snapshots: SgkSnapshotKurus[]): SgkSnapshotKurus {
  return snapshots.reduce(
    (totals, snapshot) => ({
      isverenSgkPrimi: totals.isverenSgkPrimi + snapshot.isverenSgkPrimi,
      isverenIssizlikPrimi: totals.isverenIssizlikPrimi + snapshot.isverenIssizlikPrimi,
      isciSgkPrimi: totals.isciSgkPrimi + snapshot.isciSgkPrimi,
      isciIssizlikPrimi: totals.isciIssizlikPrimi + snapshot.isciIssizlikPrimi,
      pekAltSinirTamamlamaIsverenPrimi:
        totals.pekAltSinirTamamlamaIsverenPrimi +
        snapshot.pekAltSinirTamamlamaIsverenPrimi,
    }),
    { ...EMPTY_SNAPSHOT_TOTALS }
  );
}

function getRateCandidates(authoritativePayrolls: BordroKaydi[]): SgkPrimKontroluRateCandidates {
  return {
    isverenSgkOranlari: authoritativePayrolls.map(
      (payroll) => payroll.pekDetay?.sgkIsverenOraniYuzde ?? undefined
    ),
    isverenIssizlikOranlari: authoritativePayrolls.map(
      (payroll) => payroll.pekDetay?.isverenIssizlikOraniYuzde ?? undefined
    ),
    // Worker-side rates are not persisted in the payroll snapshot; these are
    // resolved from the active period settings when the header is built.
    isciSgkOranlari: authoritativePayrolls.map(() => undefined),
    isciIssizlikOranlari: authoritativePayrolls.map(() => undefined),
  };
}

export function getSgkPrimKontroluRows(
  period: BordroDonemi,
  personnel: Personel[],
  payrolls: BordroKaydi[],
  retroBatches: RetroAdjustmentBatch[] = [],
  retroAllocations: RetroAllocation[] = []
): SgkPrimKontroluRow[] {
  const payrollsByPerson = new Map<string, BordroKaydi[]>();

  payrolls
    .filter((payroll) => payroll.donemId === period.id)
    .forEach((payroll) => {
      const personPayrolls = payrollsByPerson.get(payroll.personelId) ?? [];
      personPayrolls.push(payroll);
      payrollsByPerson.set(payroll.personelId, personPayrolls);
    });

  return personnel.map((personel) => {
    const personPayrolls = payrollsByPerson.get(personel.id) ?? [];
    const authoritativePayrolls = personPayrolls.filter(isAuthoritativePayroll);
    const hasStalePayroll = personPayrolls.some((payroll) => payroll.status === 'STALE');
    const hasDraftPayroll = personPayrolls.some((payroll) => payroll.status === 'DRAFT');
    const snapshots = authoritativePayrolls.map(getSgkSnapshotKurus);
    const hasMissingSnapshot = snapshots.some((snapshot) => snapshot === null);
    const retroLedger = getRetroLedgerKurus(
      period.id,
      personel.id,
      retroBatches,
      retroAllocations
    );
    const hasAuthoritativeSourcePayroll =
      authoritativePayrolls.length > 0 && !hasMissingSnapshot;
    const status: SgkPrimKontroluRowStatus = hasStalePayroll
      ? 'stale'
      : hasMissingSnapshot
        ? 'missingSnapshot'
        : hasDraftPayroll
          ? 'draft'
          : hasAuthoritativeSourcePayroll
            ? 'authoritative'
            : 'notCalculated';
    const baseSnapshotTotals = sumSnapshotValues(
      snapshots.filter((snapshot): snapshot is SgkSnapshotKurus => snapshot !== null)
    );
    const snapshotTotals =
      status === 'authoritative'
        ? {
            ...baseSnapshotTotals,
            isverenSgkPrimi: baseSnapshotTotals.isverenSgkPrimi + retroLedger.isverenSgkPrimi,
            isverenIssizlikPrimi:
              baseSnapshotTotals.isverenIssizlikPrimi + retroLedger.isverenIssizlikPrimi,
            isciSgkPrimi: baseSnapshotTotals.isciSgkPrimi + retroLedger.isciSgkPrimi,
            isciIssizlikPrimi: baseSnapshotTotals.isciIssizlikPrimi + retroLedger.isciIssizlikPrimi,
          }
        : { ...EMPTY_SNAPSHOT_TOTALS };

    const isverenSgkPrimi = kurusToAmount(snapshotTotals.isverenSgkPrimi);
    const isverenIssizlikPrimi = kurusToAmount(snapshotTotals.isverenIssizlikPrimi);
    const isciSgkPrimi = kurusToAmount(snapshotTotals.isciSgkPrimi);
    const isciIssizlikPrimi = kurusToAmount(snapshotTotals.isciIssizlikPrimi);
    const pekAltSinirTamamlamaIsverenPrimi = kurusToAmount(
      snapshotTotals.pekAltSinirTamamlamaIsverenPrimi
    );
    const dortPrimToplami = kurusToAmount(
      snapshotTotals.isverenSgkPrimi +
        snapshotTotals.isverenIssizlikPrimi +
        snapshotTotals.isciSgkPrimi +
        snapshotTotals.isciIssizlikPrimi
    );
    const sgkMutabakatToplami = kurusToAmount(
      snapshotTotals.isverenSgkPrimi +
        snapshotTotals.isverenIssizlikPrimi +
        snapshotTotals.isciSgkPrimi +
        snapshotTotals.isciIssizlikPrimi +
        snapshotTotals.pekAltSinirTamamlamaIsverenPrimi
    );

    return {
      personel,
      isverenSgkPrimi,
      isverenIssizlikPrimi,
      isciSgkPrimi,
      isciIssizlikPrimi,
      retroPekDelta: kurusToAmount(retroLedger.retroPekDelta),
      pekAltSinirTamamlamaIsverenPrimi,
      dortPrimToplami,
      toplam: sgkMutabakatToplami,
      status,
      rateCandidates: getRateCandidates(authoritativePayrolls),
    };
  });
}

export function getSgkPrimKontroluTotals(rows: SgkPrimKontroluRow[]): SgkPrimKontroluTotals {
  const totalsKurus = rows.reduce(
    (totals, row) => {
      if (row.status !== 'authoritative') return totals;
      totals.isverenSgkPrimi += amountToKurus(row.isverenSgkPrimi);
      totals.isverenIssizlikPrimi += amountToKurus(row.isverenIssizlikPrimi);
      totals.isciSgkPrimi += amountToKurus(row.isciSgkPrimi);
      totals.isciIssizlikPrimi += amountToKurus(row.isciIssizlikPrimi);
      totals.pekAltSinirTamamlamaIsverenPrimi += amountToKurus(
        row.pekAltSinirTamamlamaIsverenPrimi
      );
      return totals;
    },
    {
      isverenSgkPrimi: 0,
      isverenIssizlikPrimi: 0,
      isciSgkPrimi: 0,
      isciIssizlikPrimi: 0,
      pekAltSinirTamamlamaIsverenPrimi: 0,
    }
  );

  const dortPrimToplami =
    totalsKurus.isverenSgkPrimi +
    totalsKurus.isverenIssizlikPrimi +
    totalsKurus.isciSgkPrimi +
    totalsKurus.isciIssizlikPrimi;
  const sgkMutabakatToplami = dortPrimToplami + totalsKurus.pekAltSinirTamamlamaIsverenPrimi;
  const hazirOlmayanPersonelSayisi = rows.filter((row) => row.status !== 'authoritative').length;

  return {
    isverenSgkPrimi: kurusToAmount(totalsKurus.isverenSgkPrimi),
    isverenIssizlikPrimi: kurusToAmount(totalsKurus.isverenIssizlikPrimi),
    isciSgkPrimi: kurusToAmount(totalsKurus.isciSgkPrimi),
    isciIssizlikPrimi: kurusToAmount(totalsKurus.isciIssizlikPrimi),
    pekAltSinirTamamlamaIsverenPrimi: kurusToAmount(
      totalsKurus.pekAltSinirTamamlamaIsverenPrimi
    ),
    dortPrimToplami: kurusToAmount(dortPrimToplami),
    sgkMutabakatToplami: kurusToAmount(sgkMutabakatToplami),
    genelPrimToplami: kurusToAmount(sgkMutabakatToplami),
    hazirOlmayanPersonelSayisi,
    reconciliationReady: hazirOlmayanPersonelSayisi === 0,
  };
}

function uniqueResolvedRates(values: Array<number | undefined>, fallback: number): number[] {
  const resolvedValues = values.length
    ? values.map((value) => (isValidRate(value) ? value : fallback))
    : [fallback];
  return [...new Set(resolvedValues)];
}

function formatRate(value: number): string {
  return new Intl.NumberFormat('tr-TR', { maximumFractionDigits: 2 }).format(value);
}

function formatRateLabel(
  label: string,
  values: Array<number | undefined>,
  fallback: number
): string {
  const uniqueRates = uniqueResolvedRates(values, fallback);
  return uniqueRates.length === 1
    ? `${label} %${formatRate(uniqueRates[0])}`
    : `${label} (Değişken Oran)`;
}

export function getSgkPrimKontroluRateLabels(
  rows: SgkPrimKontroluRow[],
  institutionSettings?: Partial<DönemselKurumDegerleri>
): SgkPrimKontroluRateLabels {
  const fallbackRates = resolveSgkRates(institutionSettings);
  const authoritativeRows = rows.filter((row) => row.status === 'authoritative');
  const rateCandidates = authoritativeRows.reduce(
    (candidates, row) => ({
      isverenSgkOranlari: [
        ...candidates.isverenSgkOranlari,
        ...row.rateCandidates.isverenSgkOranlari,
      ],
      isverenIssizlikOranlari: [
        ...candidates.isverenIssizlikOranlari,
        ...row.rateCandidates.isverenIssizlikOranlari,
      ],
      isciSgkOranlari: [...candidates.isciSgkOranlari, ...row.rateCandidates.isciSgkOranlari],
      isciIssizlikOranlari: [
        ...candidates.isciIssizlikOranlari,
        ...row.rateCandidates.isciIssizlikOranlari,
      ],
    }),
    {
      isverenSgkOranlari: [] as Array<number | undefined>,
      isverenIssizlikOranlari: [] as Array<number | undefined>,
      isciSgkOranlari: [] as Array<number | undefined>,
      isciIssizlikOranlari: [] as Array<number | undefined>,
    }
  );

  return {
    isverenSgk: formatRateLabel(
      'SGK İşveren',
      rateCandidates.isverenSgkOranlari,
      fallbackRates.isverenSgkOraniYuzde
    ),
    isverenIssizlik: formatRateLabel(
      'İşveren İşsizlik',
      rateCandidates.isverenIssizlikOranlari,
      fallbackRates.isverenIssizlikOraniYuzde
    ),
    isciSgk: formatRateLabel(
      'SGK İşçi',
      rateCandidates.isciSgkOranlari,
      fallbackRates.isciSgkOraniYuzde
    ),
    isciIssizlik: formatRateLabel(
      'İşçi İşsizlik',
      rateCandidates.isciIssizlikOranlari,
      fallbackRates.isciIssizlikOraniYuzde
    ),
  };
}

export function compareSgkPrimTotals(
  programSgkMutabakatToplami: number,
  input: string,
  reconciliationReady = true
): SgkPrimComparison {
  if (!input.trim()) {
    return { status: 'empty', sgkTutarKurus: null, farkKurus: null };
  }

  const sgkTutarKurus = parseSgkTutarToKurus(input);
  if (sgkTutarKurus === null) {
    return { status: 'invalid', sgkTutarKurus: null, farkKurus: null };
  }

  const farkKurus = amountToKurus(programSgkMutabakatToplami) - sgkTutarKurus;
  return {
    status: !reconciliationReady
      ? 'incomplete'
      : farkKurus === 0
        ? 'compatible'
        : farkKurus > 0
          ? 'programHigher'
          : 'programLower',
    sgkTutarKurus,
    farkKurus,
  };
}

export function getSgkPrimKontroluStatusLabel(status: SgkPrimKontroluRowStatus): string {
  switch (status) {
    case 'authoritative':
      return 'Hazır';
    case 'stale':
      return 'Yeniden hesaplanmalı';
    case 'draft':
      return 'Bordro tamamlanmamış';
    case 'missingSnapshot':
      return 'SGK verisi eksik';
    case 'notCalculated':
      return 'Bordro hesaplanmadı';
  }
}

export function buildSgkPrimKontroluExcelPayload(
  rows: SgkPrimKontroluRow[],
  totals: SgkPrimKontroluTotals,
  rateLabels: SgkPrimKontroluRateLabels,
  comparison: SgkPrimComparison
): SgkPrimKontroluExcelPayload {
  const columns = [
    { header: 'S.No', key: 'siraNo', width: 8 },
    { header: 'T.C. Kimlik No', key: 'tcNo', width: 18 },
    { header: 'SGK Sicil No', key: 'sgkSicilNo', width: 18 },
    { header: 'Ad Soyad', key: 'adSoyad', width: 28 },
    { header: 'Durum', key: 'durum', width: 24 },
    { header: rateLabels.isverenSgk, key: 'isverenSgkPrimi', width: 20 },
    { header: rateLabels.isverenIssizlik, key: 'isverenIssizlikPrimi', width: 20 },
    { header: rateLabels.isciSgk, key: 'isciSgkPrimi', width: 18 },
    { header: rateLabels.isciIssizlik, key: 'isciIssizlikPrimi', width: 18 },
    { header: 'Retro kaynak PEK farkı', key: 'retroPekDelta', width: 20 },
    {
      header: 'PEK Alt Sınır İşveren Tamamlama',
      key: 'pekAltSinirTamamlamaIsverenPrimi',
      width: 28,
    },
    { header: 'Toplam', key: 'toplam', width: 18 },
  ];

  const data = rows.map((row, index) => {
    const isReady = row.status === 'authoritative';
    return {
      siraNo: index + 1,
      tcNo: row.personel.tcNo,
      sgkSicilNo: row.personel.sgkSicilNo,
      adSoyad: `${row.personel.ad} ${row.personel.soyad}`,
      durum: getSgkPrimKontroluStatusLabel(row.status),
      isverenSgkPrimi: isReady ? row.isverenSgkPrimi : '',
      isverenIssizlikPrimi: isReady ? row.isverenIssizlikPrimi : '',
      isciSgkPrimi: isReady ? row.isciSgkPrimi : '',
      isciIssizlikPrimi: isReady ? row.isciIssizlikPrimi : '',
      retroPekDelta: isReady ? row.retroPekDelta : '',
      pekAltSinirTamamlamaIsverenPrimi: isReady
        ? row.pekAltSinirTamamlamaIsverenPrimi
        : '',
      toplam: isReady ? row.toplam : '',
    };
  });

  const emptySummaryRow = (label: string): Record<string, SgkPrimKontroluExcelValue> => ({
    siraNo: '',
    tcNo: '',
    sgkSicilNo: '',
    adSoyad: label,
    durum: '',
    isverenSgkPrimi: '',
    isverenIssizlikPrimi: '',
    isciSgkPrimi: '',
    isciIssizlikPrimi: '',
    retroPekDelta: '',
    pekAltSinirTamamlamaIsverenPrimi: '',
    toplam: '',
  });

  const summaryRows = [
    {
      ...emptySummaryRow('SGK İşveren Toplamı'),
      isverenSgkPrimi: totals.isverenSgkPrimi,
    },
    {
      ...emptySummaryRow('İşveren İşsizlik Toplamı'),
      isverenIssizlikPrimi: totals.isverenIssizlikPrimi,
    },
    {
      ...emptySummaryRow('SGK İşçi Toplamı'),
      isciSgkPrimi: totals.isciSgkPrimi,
    },
    {
      ...emptySummaryRow('İşçi İşsizlik Toplamı'),
      isciIssizlikPrimi: totals.isciIssizlikPrimi,
    },
    {
      ...emptySummaryRow('Dört Ana Prim Toplamı'),
      toplam: totals.dortPrimToplami,
    },
    {
      ...emptySummaryRow('PEK Alt Sınır İşveren Tamamlama Toplamı'),
      pekAltSinirTamamlamaIsverenPrimi: totals.pekAltSinirTamamlamaIsverenPrimi,
    },
    {
      ...emptySummaryRow('SGK Mutabakat Toplamı'),
      toplam: totals.sgkMutabakatToplami,
    },
    {
      ...emptySummaryRow("SGK'dan Girilen Tutar"),
      toplam: comparison.sgkTutarKurus === null ? '' : kurusToAmount(comparison.sgkTutarKurus),
    },
    {
      ...emptySummaryRow('Fark'),
      toplam: comparison.farkKurus === null ? '' : kurusToAmount(comparison.farkKurus),
    },
    {
      ...emptySummaryRow('Hazır Olmayan Personel Sayısı'),
      toplam: totals.hazirOlmayanPersonelSayisi,
    },
  ];

  return { columns, data, summaryRows };
}
