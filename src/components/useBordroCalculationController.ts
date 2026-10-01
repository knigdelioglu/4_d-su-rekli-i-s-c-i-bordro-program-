import { useLayoutEffect, useRef, useState, type FormEvent, type MouseEvent } from 'react';
import {
  comparePaymentEvents,
  nextPaymentSequence,
  paymentEventSequenceScopeKey,
} from '../services/payrollEngine/paymentEventOrder';
import {
  assertPayrollCalculationSnapshotCurrentForEngine,
} from '../services/payrollEngine/calculationSnapshot';
import {
  AccrualType,
  BordroDonemi,
  BordroKaydi,
  Personel,
  PersonelPuantaj,
} from '../types/payroll';
import { formatTL, getDefaultAccrualPaymentDate, getPeriodDaysList } from '../utils/payrollPresentation';
import {
  getPayrollEngine,
  PayrollBoundaryAccrualInput,
  PayrollBoundaryPayroll,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
} from '../services/payrollEngine';
import {
  isExactDecimalString,
  toPayrollUiModel,
} from '../services/payrollEngine/decimalBoundary';

function formatActionableParameterError(message: string): string {
  const annualYear = message.match(/\b(20\d{2})\b/)?.[1];
  if (isPayrollTaxOpeningConfigurationError(message)) {
    return 'Kümülatif gelir vergisi açılışının başlangıç dönemi eksik veya geçersiz. Bordro ekranındaki "Önceki Kümülatif Matrah Girişi" bölümünü açıp tutarı aktif dönemle kaydedin; ardından bordroyu yeniden hesaplayın.';
  }
  if (
    annualYear &&
    /(yıllık bordro parametreleri|sigorta gv yıllık)/i.test(message) &&
    /(eksik|bulunamadı|yok)/i.test(message)
  ) {
    return `${annualYear} yılı gelir vergisi tarifesi henüz tanımlı değil. Yıllık Parametreler bölümünü tamamlayın.`;
  }
  if (
    /(zorunlu yasal parametre|yasal parametresi|kurum ayarları|yasal parametre baseline)/i.test(message) &&
    /(eksik|yok|bulunamadı)/i.test(message)
  ) {
    return 'Dönem yasal parametreleri henüz tamamlanmamış. Dönem Parametreleri bölümünü tamamlayın.';
  }
  if (/(günlük taban ücret|iş primi grupları)/i.test(message) && /(eksik|geçerli)/i.test(message)) {
    return 'Dönem kurum ücretleri henüz tamamlanmamış. Ücretler bölümünde kurum değerlerini tamamlayın.';
  }
  if (isStaleChainError(message)) {
    return `${message} ${STALE_CHAIN_HINT}`;
  }
  return message;
}

export const STALE_CHAIN_HINT =
  'Çözüm: Bordro Hesaplama ekranında ilgili kişinin bordrosunu "Hesapla" ile veya "Tüm Hesaplanabilir Bordroları Hesapla" ile yeniden hesaplayın; önceki güncelliğini yitirmiş tahakkuklar sırasıyla otomatik yeniden hesaplanır.';

export function isStaleChainError(message: string): boolean {
  return /(DRAFT\/STALE|tahakkuku (STALE|DRAFT) durumda)/.test(message);
}

export function isPayrollTaxOpeningConfigurationError(message: string): boolean {
  return /(?:legacy\s+(?:asgari\s+)?gv\s+opening|(?:normal|asgari)\s+gv\s+opening)[\s\S]*(?:effective|başlangıç\s+(?:ayı|dönemi))/i.test(
    message
  );
}

export function formatPayrollError(err: unknown): string {
  if (err && typeof err === 'object') {
    const tagged = err as { type?: string; message?: unknown };
    if (tagged.type === 'NegativeNetPayment' && tagged.message && typeof tagged.message === 'object') {
      const details = tagged.message as { gelir?: number; kesinti?: number; fark?: number };
      return `Kesintiler geliri aşıyor. Gelir: ${formatTL(details.gelir ?? 0)}, kesinti: ${formatTL(details.kesinti ?? 0)}, açık: ${formatTL(details.fark ?? 0)}.`;
    }
    if (typeof tagged.message === 'string') return formatActionableParameterError(tagged.message);
    try {
      return JSON.stringify(err);
    } catch {
      return 'Beklenmeyen bir hata oluştu.';
    }
  }
  return formatActionableParameterError(String(err));
}

export function formatStalePayrollMessage(personName: string): string {
  return `${personName} bordrosu kaynak verilerindeki değişiklik nedeniyle güncelliğini yitirdi. Bordro zarfını açmadan/yazdırmadan önce yeniden hesaplayın.`;
}

export type BatchPayrollOutcome =
  | 'success'
  | 'finalized-skipped'
  | 'attendance-missing'
  | 'attendance-incomplete'
  | 'not-applicable'
  | 'calculation-error';

export interface BatchPayrollOutcomeSummary {
  total: number;
  success: number;
  finalizedSkipped: number;
  attendanceMissing: number;
  attendanceIncomplete: number;
  /** Bu dönem için yapısal olarak hesaplanamayan personel (ör. sonraki kesinleşmiş geçmiş, ileri tarihli GV devri). */
  notApplicable: number;
  calculationErrors: number;
}

export function classifyBatchAttendance(
  attendance: PersonelPuantaj | undefined,
  period: Pick<BordroDonemi, 'baslangicTarihi' | 'bitisTarihi'>
): 'missing' | 'incomplete' | 'complete' {
  if (!attendance) return 'missing';

  const recordedDates = Object.keys(attendance.gunler ?? {});
  const periodDates = getPeriodDaysList(period.baslangicTarihi, period.bitisTarihi).map(
    (day) => day.dateStr
  );
  if (periodDates.length === 0) return 'complete';

  const recordedDateSet = new Set(recordedDates);
  const hasExactCoverage =
    recordedDates.length === periodDates.length &&
    periodDates.every((date) => recordedDateSet.has(date));
  return hasExactCoverage ? 'complete' : 'incomplete';
}

export function summarizeBatchPayrollOutcomes(
  outcomes: BatchPayrollOutcome[]
): BatchPayrollOutcomeSummary {
  const summary: BatchPayrollOutcomeSummary = {
    total: outcomes.length,
    success: 0,
    finalizedSkipped: 0,
    attendanceMissing: 0,
    attendanceIncomplete: 0,
    notApplicable: 0,
    calculationErrors: 0,
  };
  for (const outcome of outcomes) {
    switch (outcome) {
      case 'success':
        summary.success++;
        break;
      case 'finalized-skipped':
        summary.finalizedSkipped++;
        break;
      case 'attendance-missing':
        summary.attendanceMissing++;
        break;
      case 'attendance-incomplete':
        summary.attendanceIncomplete++;
        break;
      case 'not-applicable':
        summary.notApplicable++;
        break;
      case 'calculation-error':
        summary.calculationErrors++;
        break;
    }
  }
  return summary;
}

export const ACCRUAL_TYPE_LABELS: Record<AccrualType, string> = {
  NORMAL: 'Normal Maaş',
  TEDIYE: 'Tediye',
  TIS_IKRAMIYE: 'TİS İkramiyesi',
  SUPPLEMENTAL: 'Ek Ödeme',
  RETRO_ADJUSTMENT: 'Geriye Dönük Fark',
};

export type SupplementaryAccrualType = Exclude<AccrualType, 'NORMAL'>;

export type PayrollRowFilter =
  | 'all'
  | 'attendanceMissing'
  | 'notCalculated'
  | 'stale'
  | 'calculated'
  | 'finalized';

export interface SupplementaryAccrualDraft {
  accrualType: SupplementaryAccrualType;
  paymentDate: string;
  grossAmount: string;
  description: string;
}

interface UseBordroCalculationControllerOptions {
  aktifDonem: BordroDonemi;
  activeAccrualType: AccrualType;
  activeViewTitle: string;
  authoritativeDataset: PayrollDatasetSnapshot;
  bordrolar: BordroKaydi[];
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  isSupplementaryView: boolean;
  onSaveBordro: (
    bordro: PayrollBoundaryPayroll,
    calculationSnapshot?: PayrollDatasetSnapshot
  ) => Promise<void> | void;
}

const DATASET_COMMIT_TIMEOUT_MS = 5000;
const MAX_CHAIN_REPLAY_STEPS = 240;

export interface ChainReplayResult {
  ok: boolean;
  replayed: number;
  error?: string;
}

type PaymentEventKey = { paymentDate: string; sequence: number; accrualId: string; id: string };

/**
 * Returns the person's first (canonical payment-event order) DRAFT/STALE event
 * that precedes the target event. Pure; exported for regression tests.
 */
export function findFirstStalePriorEvent(
  payrolls: BordroKaydi[],
  periodsById: Map<string, BordroDonemi>,
  personId: string,
  target: PaymentEventKey,
  targetPeriod: BordroDonemi
): { payroll: BordroKaydi; period: BordroDonemi } | null {
  const targetId = target.accrualId || target.id;
  const prior = payrolls
    .filter((payroll) => payroll.personelId === personId && (payroll.accrualId || payroll.id) !== targetId)
    .map((payroll) => ({ payroll, period: periodsById.get(payroll.donemId) }))
    .filter((item): item is { payroll: BordroKaydi; period: BordroDonemi } =>
      item.period !== undefined &&
      comparePaymentEvents(item.payroll, target, item.period, targetPeriod) < 0
    )
    .sort((left, right) => comparePaymentEvents(left.payroll, right.payroll, left.period, right.period));
  return prior.find((item) => item.payroll.status === 'STALE' || item.payroll.status === 'DRAFT') ?? null;
}

const TAX_OPENING_LATER_PATTERN = /opening başlangıç vergi ayı (\d+) aktif vergi ayı (\d+) sonrasında olamaz/i;

export function isTaxOpeningAfterActiveMonthError(message: string): boolean {
  return TAX_OPENING_LATER_PATTERN.test(message);
}

/** Engine errors that mean "no payroll can exist here", not a failed calculation. */
export function structuralNotApplicableReason(message: string): string | null {
  if (isTaxOpeningAfterActiveMonthError(message)) {
    return 'kümülatif GV devri daha sonraki bir vergi ayından başlıyor';
  }
  if (/Kesinleştirilmiş bordro\/retro tarihçesini etkileyen/i.test(message)) {
    return 'sonraki dönemlerde kesinleşmiş bordro/retro geçmişi var';
  }
  return null;
}

/**
 * Explains why a person cannot have a NORMAL payroll in the active period at
 * all (as opposed to a calculation error). Pure; exported for tests.
 */
export function getBatchNotApplicableReason(
  dataset: {
    payrolls: BordroKaydi[];
    periods: BordroDonemi[];
    taxOpenings?: Array<{
      personnelId: string;
      year: number;
      gvCumulativeOpening?: unknown;
      effectiveFromPeriodId?: string;
      asgariGvCumulativeOpening?: unknown;
      asgariGvEffectiveFromPeriodId?: string;
    }>;
  },
  person: Pick<Personel, 'id' | 'devirKumulatifGvMatrahi' | 'devirKumulatifGvMatrahiYili' | 'devirKumulatifGvMatrahiBaslangicAyi'>,
  activePeriod: BordroDonemi,
  target: PaymentEventKey
): string | null {
  const periodsById = new Map(dataset.periods.map((period) => [period.id, period]));
  const targetId = target.accrualId || target.id;
  const laterFinalized = dataset.payrolls.find((payroll) => {
    if (payroll.personelId !== person.id || payroll.status !== 'FINALIZED') return false;
    if ((payroll.accrualId || payroll.id) === targetId) return false;
    const period = periodsById.get(payroll.donemId);
    return period !== undefined && comparePaymentEvents(payroll, target, period, activePeriod) > 0;
  });
  if (laterFinalized) {
    const period = periodsById.get(laterFinalized.donemId);
    return `sonraki ${period?.donemAdi || laterFinalized.donemId} döneminde kesinleşmiş bordrosu var; bu döneme yeni bordro eklenemez`;
  }

  const opening = dataset.taxOpenings?.find(
    (item) => item.personnelId === person.id && item.year === activePeriod.taxYear
  );
  const openingStarts = [
    opening?.gvCumulativeOpening != null ? opening.effectiveFromPeriodId : undefined,
    opening?.asgariGvCumulativeOpening != null ? opening.asgariGvEffectiveFromPeriodId : undefined,
  ]
    .filter((id): id is string => Boolean(id))
    .map((id) => periodsById.get(id))
    .filter((period): period is BordroDonemi => period !== undefined && period.taxYear === activePeriod.taxYear);
  const laterOpening = openingStarts.find((period) => period.taxMonth > activePeriod.taxMonth);
  if (laterOpening) {
    return `kümülatif GV devri ${laterOpening.donemAdi || laterOpening.id} döneminden başlıyor; daha önceki dönem bordrosu hesaplanamaz`;
  }
  if (
    !opening &&
    (person.devirKumulatifGvMatrahi ?? 0) > 0 &&
    person.devirKumulatifGvMatrahiYili === activePeriod.taxYear &&
    (person.devirKumulatifGvMatrahiBaslangicAyi ?? 1) > activePeriod.taxMonth
  ) {
    return `kümülatif GV devri ${person.devirKumulatifGvMatrahiBaslangicAyi}. vergi ayından başlıyor; daha önceki dönem bordrosu hesaplanamaz`;
  }
  return null;
}

function storedManualIncome(event: BordroKaydi): PayrollCalculationRequest['manualIncome'] {
  if (event.accrualType !== 'NORMAL') return null;
  const tediye = event.gelirler?.tediye ?? null;
  const tisIkramiyesi = event.gelirler?.tisIkramiyesi ?? null;
  if (tediye === null && tisIkramiyesi === null) return null;
  return { tediye, tisIkramiyesi } as unknown as PayrollCalculationRequest['manualIncome'];
}

function storedAccrualInput(event: BordroKaydi, period: BordroDonemi): PayrollBoundaryAccrualInput {
  const gross =
    event.accrualType === 'TEDIYE'
      ? event.gelirler?.tediye
      : event.accrualType === 'TIS_IKRAMIYE'
        ? event.gelirler?.tisIkramiyesi
        : event.gelirler?.ekOdeme;
  return {
    accrualId: event.accrualId || event.id,
    accrualType: event.accrualType,
    paymentDate: event.paymentDate || getDefaultAccrualPaymentDate(period),
    sequence: event.sequence,
    grossAmount: event.accrualType === 'NORMAL' ? null : String(gross ?? 0),
    description: event.accrualDescription ?? null,
  } as PayrollBoundaryAccrualInput;
}

export function tryAcquireSupplementaryPaymentScope(
  pendingScopes: Set<string>,
  scope: string
): (() => void) | null {
  if (pendingScopes.has(scope)) return null;
  pendingScopes.add(scope);
  let released = false;
  return () => {
    if (released) return;
    released = true;
    pendingScopes.delete(scope);
  };
}

export function useBordroCalculationController({
  aktifDonem,
  activeAccrualType,
  activeViewTitle,
  authoritativeDataset,
  bordrolar,
  personeller,
  puantajlar,
  isSupplementaryView,
  onSaveBordro,
}: UseBordroCalculationControllerOptions) {
  const payrollEngine = getPayrollEngine();
  const authoritativeDatasetRef = useRef(authoritativeDataset);
  const datasetCommitWaitersRef = useRef(new Set<() => void>());
  useLayoutEffect(() => {
    authoritativeDatasetRef.current = authoritativeDataset;
    for (const notify of [...datasetCommitWaitersRef.current]) notify();
  }, [authoritativeDataset]);

  /**
   * A save only schedules a React state update; the next calculation must not
   * start from the pre-save snapshot. Sequential flows (batch, chain replay)
   * wait here until the saved dataset has been committed to the ref.
   */
  const waitForDatasetCommit = (previous: PayrollDatasetSnapshot): Promise<void> =>
    new Promise((resolve) => {
      if (authoritativeDatasetRef.current !== previous) {
        resolve();
        return;
      }
      let timer: ReturnType<typeof setTimeout> | undefined;
      const done = () => {
        if (timer !== undefined) clearTimeout(timer);
        datasetCommitWaitersRef.current.delete(done);
        resolve();
      };
      datasetCommitWaitersRef.current.add(done);
      timer = setTimeout(done, DATASET_COMMIT_TIMEOUT_MS);
    });
  const [searchTerm, setSearchTerm] = useState('');
  const [rowFilter, setRowFilter] = useState<PayrollRowFilter>('all');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [deletingAccrualId, setDeletingAccrualId] = useState<string | null>(null);
  const [isBatchProcessing, setIsBatchProcessing] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [normalPaymentDateMap, setNormalPaymentDateMap] = useState<Record<string, string>>({});
  const [newAccrualPersonId, setNewAccrualPersonId] = useState<string | null>(null);
  const pendingSupplementaryPaymentScopes = useRef(new Set<string>());
  const [visiblePendingSupplementaryPaymentScopes, setVisiblePendingSupplementaryPaymentScopes] =
    useState<Set<string>>(() => new Set());
  const [expandedTimelinePersonId, setExpandedTimelinePersonId] = useState<string | null>(null);
  const [supplementaryAccrualDraft, setSupplementaryAccrualDraft] = useState<SupplementaryAccrualDraft>({
    accrualType: 'TEDIYE',
    paymentDate: getDefaultAccrualPaymentDate(aktifDonem),
    grossAmount: '',
    description: '',
  });
  const [manualKumulatifGvMap, setManualKumulatifGvMap] = useState<Record<string, string>>({});
  const [manualKumulatifAsgariGvMap, setManualKumulatifAsgariGvMap] = useState<Record<string, string>>({});
  const [isKumulatifModalOpen, setIsKumulatifModalOpen] = useState(false);
  const firstBatchCalculationErrorRef = useRef<string | null>(null);
  const batchCalculationErrorsRef = useRef<string[]>([]);

  const getAccrualId = (payroll: BordroKaydi): string => payroll.accrualId || payroll.id;

  const getPersonAccruals = (personId: string): BordroKaydi[] =>
    bordrolar
      .filter((item) => item.personelId === personId && item.donemId === aktifDonem.id)
      .sort((a, b) => comparePaymentEvents(a, b, aktifDonem));

  const getNormalPayroll = (personId: string): BordroKaydi | undefined =>
    getPersonAccruals(personId).find((item) => item.accrualType === 'NORMAL');

  const getActiveViewAccruals = (personId: string): BordroKaydi[] =>
    getPersonAccruals(personId).filter((item) => item.accrualType === activeAccrualType);

  const getActiveViewPayroll = (personId: string): BordroKaydi | undefined =>
    getActiveViewAccruals(personId)[0];

  const getNormalAccrualInput = (
    personId: string,
    dataset: PayrollDatasetSnapshot = authoritativeDatasetRef.current
  ): PayrollBoundaryAccrualInput => {
    const exactPayroll = dataset.payrolls.find(
      (item) =>
        item.personelId === personId &&
        item.donemId === aktifDonem.id &&
        item.accrualType === 'NORMAL'
    );
    const existingPayroll = dataset.payrolls
      .filter((item) => item.personelId === personId && item.donemId === aktifDonem.id)
      .sort((a, b) => comparePaymentEvents(a, b, aktifDonem))
      .find((item) => item.accrualType === 'NORMAL');
    return {
      accrualId:
        exactPayroll?.accrualId ||
        exactPayroll?.id ||
        existingPayroll?.accrualId ||
        existingPayroll?.id ||
        `${personId}_${aktifDonem.id}`,
      accrualType: 'NORMAL',
      paymentDate:
        exactPayroll?.paymentDate ||
        existingPayroll?.paymentDate ||
        (exactPayroll || existingPayroll
          ? getDefaultAccrualPaymentDate(aktifDonem)
          : normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)),
      sequence: exactPayroll?.sequence ?? existingPayroll?.sequence ?? nextPaymentSequence(
        dataset,
        personId,
        aktifDonem,
        normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)
      ),
      grossAmount: null,
      description: exactPayroll?.accrualDescription ?? existingPayroll?.accrualDescription ?? null,
    };
  };

  const getLegacyManualIncomeInput = (
    personId: string,
    dataset: PayrollDatasetSnapshot = authoritativeDatasetRef.current
  ): PayrollCalculationRequest['manualIncome'] => {
    const exactPayroll = dataset.payrolls.find(
      (item) =>
        item.personelId === personId &&
        item.donemId === aktifDonem.id &&
        item.accrualType === 'NORMAL'
    );
    if (!exactPayroll) return null;
    const tediye = exactPayroll.gelirler.tediye ?? null;
    const tisIkramiyesi = exactPayroll.gelirler.tisIkramiyesi ?? null;
    if (tediye === null && tisIkramiyesi === null) return null;
    return { tediye, tisIkramiyesi };
  };

  const getDevirGvMatrahiForActiveYear = (person: Personel): number => {
    const activeTaxYear = aktifDonem.taxYear ?? (aktifDonem.ay === 12 ? aktifDonem.yil + 1 : aktifDonem.yil);
    const openingYear = person.devirKumulatifGvMatrahiYili;
    const opening = person.devirKumulatifGvMatrahi ?? 0;
    return opening > 0 && (!openingYear || openingYear === activeTaxYear) ? opening : 0;
  };

  const buildDataset = (): PayrollDatasetSnapshot => authoritativeDatasetRef.current;

  const calculateAndSaveForPerson = async (person: Personel): Promise<BordroKaydi | null> => {
    const calculationSnapshot = buildDataset();
    const pPuantaj = calculationSnapshot.attendances.find(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );
    if (!pPuantaj || !pPuantaj.gunler || Object.keys(pPuantaj.gunler).length === 0) return null;

    const existingBordro = calculationSnapshot.payrolls.find(
      (payroll) =>
        payroll.personelId === person.id &&
        payroll.donemId === aktifDonem.id &&
        payroll.accrualType === 'NORMAL'
    );
    if (existingBordro?.status === 'FINALIZED') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildiği için yeniden hesaplanamaz.`);
      return null;
    }

    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: getLegacyManualIncomeInput(person.id, calculationSnapshot),
        accrual: getNormalAccrualInput(person.id, calculationSnapshot),
        dataset: calculationSnapshot,
      });
      assertPayrollCalculationSnapshotCurrentForEngine(
        payrollEngine.kind,
        calculationSnapshot,
        authoritativeDatasetRef.current
      );
      await onSaveBordro(calculated, calculationSnapshot);
      await waitForDatasetCommit(calculationSnapshot);
      return toPayrollUiModel(calculated) as unknown as BordroKaydi;
    } catch (err) {
      console.error('Payroll engine calculation failed:', err);
      const formattedError = `Hesaplama hatası: ${formatPayrollError(err)}`;
      if (firstBatchCalculationErrorRef.current === null) {
        firstBatchCalculationErrorRef.current = formattedError;
      }
      batchCalculationErrorsRef.current.push(`${person.ad} ${person.soyad}: ${formattedError}`);
      setErrorMessage(formattedError);
      return null;
    }
  };

  const recordBatchError = (person: Personel, message: string) => {
    if (firstBatchCalculationErrorRef.current === null) {
      firstBatchCalculationErrorRef.current = message;
    }
    batchCalculationErrorsRef.current.push(`${person.ad} ${person.soyad}: ${message}`);
  };

  /**
   * Brings the person's earlier payment-event chain back to an authoritative
   * state before the active period is calculated. Any earlier DRAFT/STALE
   * event (typically left behind by a personnel-card or settings change) is
   * recalculated in canonical payment-event order with its own stored inputs.
   * FINALIZED history is never touched; the shared mutation policy still
   * rejects a replay that would affect FINALIZED records.
   */
  const replayStalePriorChain = async (person: Personel): Promise<ChainReplayResult> => {
    let replayed = 0;
    let lastAttempt: string | null = null;
    for (let step = 0; step < MAX_CHAIN_REPLAY_STEPS; step++) {
      const dataset = buildDataset();
      const periodsById = new Map(
        dataset.periods.map((period) => [period.id, period as unknown as BordroDonemi])
      );
      const target = getNormalAccrualInput(person.id, dataset);
      const targetEvent = {
        paymentDate: target.paymentDate,
        sequence: target.sequence,
        accrualId: target.accrualId,
        id: target.accrualId,
      };
      const nextStale = findFirstStalePriorEvent(
        dataset.payrolls as unknown as BordroKaydi[],
        periodsById,
        person.id,
        targetEvent,
        aktifDonem
      );
      if (!nextStale) return { ok: true, replayed };

      const { payroll: event, period } = nextStale;
      const eventId = event.accrualId || event.id;
      const eventLabel = `${period.donemAdi || period.id} ${ACCRUAL_TYPE_LABELS[event.accrualType] ?? event.accrualType} (${eventId})`;
      if (lastAttempt === eventId) {
        return {
          ok: false,
          replayed,
          error: `${eventLabel} yeniden hesaplandı ancak güncel duruma geçmedi; sayfayı yenileyip tekrar deneyin.`,
        };
      }
      lastAttempt = eventId;

      if (event.accrualType === 'RETRO_ADJUSTMENT') {
        return {
          ok: false,
          replayed,
          error: `Önceki ${eventLabel} güncelliğini yitirmiş bir geriye dönük fark ödemesi. Önce Geriye Dönük Farklar ekranından yeniden hesaplayın.`,
        };
      }
      if (event.accrualType === 'NORMAL') {
        const attendance = dataset.attendances.find(
          (item) => item.personelId === person.id && item.donemId === event.donemId
        );
        if (!attendance || !attendance.gunler || Object.keys(attendance.gunler).length === 0) {
          return {
            ok: false,
            replayed,
            error: `Önceki ${eventLabel} güncelliğini yitirmiş ancak o dönemin puantajı yok; zincir yeniden hesaplanamıyor.`,
          };
        }
      }

      try {
        const calculated = await payrollEngine.calculatePayroll({
          personnelId: person.id,
          periodId: event.donemId,
          calculatedAt: new Date().toISOString(),
          manualIncome: storedManualIncome(event),
          accrual: storedAccrualInput(event, period),
          dataset,
        });
        assertPayrollCalculationSnapshotCurrentForEngine(
          payrollEngine.kind,
          dataset,
          authoritativeDatasetRef.current
        );
        await onSaveBordro(calculated, dataset);
        await waitForDatasetCommit(dataset);
        replayed++;
      } catch (err) {
        return {
          ok: false,
          replayed,
          error: `Önceki ${eventLabel} yeniden hesaplanamadı: ${formatPayrollError(err)}`,
        };
      }
    }
    return {
      ok: false,
      replayed,
      error: 'Önceki tahakkuk zinciri çok uzun; yeniden hesaplama güvenli sınırda durduruldu.',
    };
  };

  /**
   * After the active NORMAL payroll is (re)calculated, the same period's later
   * supplementary events (tediye, TİS, ek ödeme) are invalidated by the shared
   * mutation policy. Recalculate them with their stored gross amounts so they
   * do not silently drop out of official lists.
   */
  const replayStaleSamePeriodFollowers = async (person: Personel): Promise<ChainReplayResult> => {
    let replayed = 0;
    const attempted = new Set<string>();
    for (let step = 0; step < MAX_CHAIN_REPLAY_STEPS; step++) {
      const dataset = buildDataset();
      const next = (dataset.payrolls as unknown as BordroKaydi[])
        .filter(
          (payroll) =>
            payroll.personelId === person.id &&
            payroll.donemId === aktifDonem.id &&
            payroll.accrualType !== 'NORMAL' &&
            payroll.accrualType !== 'RETRO_ADJUSTMENT' &&
            (payroll.status === 'STALE' || payroll.status === 'DRAFT') &&
            !attempted.has(payroll.accrualId || payroll.id)
        )
        .sort((left, right) => comparePaymentEvents(left, right, aktifDonem))[0];
      if (!next) return { ok: true, replayed };
      const eventId = next.accrualId || next.id;
      attempted.add(eventId);
      try {
        const calculated = await payrollEngine.calculatePayroll({
          personnelId: person.id,
          periodId: aktifDonem.id,
          calculatedAt: new Date().toISOString(),
          manualIncome: null,
          accrual: storedAccrualInput(next, aktifDonem),
          dataset,
        });
        assertPayrollCalculationSnapshotCurrentForEngine(
          payrollEngine.kind,
          dataset,
          authoritativeDatasetRef.current
        );
        await onSaveBordro(calculated, dataset);
        await waitForDatasetCommit(dataset);
        replayed++;
      } catch (err) {
        return {
          ok: false,
          replayed,
          error: `${ACCRUAL_TYPE_LABELS[next.accrualType] ?? next.accrualType} (${eventId}) yeniden hesaplanamadı: ${formatPayrollError(err)}`,
        };
      }
    }
    return { ok: true, replayed };
  };

  const handleCalculateAll = async () => {
    firstBatchCalculationErrorRef.current = null;
    batchCalculationErrorsRef.current = [];
    setIsBatchProcessing(true);
    const outcomes: BatchPayrollOutcome[] = [];
    const successPersons: Personel[] = [];
    const missingPuantajPersons: string[] = [];
    const incompletePuantajPersons: string[] = [];
    const notApplicablePersons: string[] = [];
    let replayedChainEvents = 0;
    let replayedFollowerEvents = 0;
    let followerFailures = 0;
    try {
      for (const person of personeller) {
        const currentDataset = buildDataset();
        const existing = currentDataset.payrolls.find(
          (item) => item.personelId === person.id && item.donemId === aktifDonem.id && item.accrualType === 'NORMAL'
        );
        if (existing?.status === 'FINALIZED') {
          outcomes.push('finalized-skipped');
          continue;
        }
        const notApplicableReason = getBatchNotApplicableReason(
          currentDataset as unknown as Parameters<typeof getBatchNotApplicableReason>[0],
          person,
          aktifDonem,
          (() => {
            const target = getNormalAccrualInput(person.id, currentDataset);
            return {
              paymentDate: target.paymentDate,
              sequence: target.sequence,
              accrualId: target.accrualId,
              id: target.accrualId,
            };
          })()
        );
        if (notApplicableReason) {
          notApplicablePersons.push(`${person.ad} ${person.soyad} (${notApplicableReason})`);
          outcomes.push('not-applicable');
          continue;
        }
        const attendance = currentDataset.attendances.find(
          (item) => item.personelId === person.id && item.donemId === aktifDonem.id
        );
        const attendanceCoverage = classifyBatchAttendance(attendance, aktifDonem);
        if (attendanceCoverage === 'missing') {
          missingPuantajPersons.push(`${person.ad} ${person.soyad}`);
          outcomes.push('attendance-missing');
          continue;
        }
        if (attendanceCoverage === 'incomplete') {
          incompletePuantajPersons.push(`${person.ad} ${person.soyad}`);
          outcomes.push('attendance-incomplete');
          continue;
        }
        const chain = await replayStalePriorChain(person);
        replayedChainEvents += chain.replayed;
        if (!chain.ok) {
          recordBatchError(person, `Hesaplama hatası: ${chain.error ?? 'önceki tahakkuk zinciri yeniden hesaplanamadı.'}`);
          outcomes.push('calculation-error');
          continue;
        }
        const errorCountBefore = batchCalculationErrorsRef.current.length;
        const res = await calculateAndSaveForPerson(person);
        if (res) {
          outcomes.push('success');
          successPersons.push(person);
          const followers = await replayStaleSamePeriodFollowers(person);
          replayedFollowerEvents += followers.replayed;
          if (!followers.ok) {
            recordBatchError(person, `Ek tahakkuk: ${followers.error ?? 'yeniden hesaplanamadı.'}`);
            followerFailures++;
          }
        } else if (
          batchCalculationErrorsRef.current.length > errorCountBefore &&
          structuralNotApplicableReason(batchCalculationErrorsRef.current[batchCalculationErrorsRef.current.length - 1])
        ) {
          // Engine-confirmed structural case not caught by the pre-check
          // (e.g. legacy opening): report as not applicable, not as an error.
          const reason = structuralNotApplicableReason(
            batchCalculationErrorsRef.current[batchCalculationErrorsRef.current.length - 1]
          );
          batchCalculationErrorsRef.current.pop();
          if (batchCalculationErrorsRef.current.length === 0) firstBatchCalculationErrorRef.current = null;
          notApplicablePersons.push(`${person.ad} ${person.soyad} (${reason})`);
          outcomes.push('not-applicable');
        } else {
          outcomes.push('calculation-error');
        }
      }
    } finally {
      setIsBatchProcessing(false);
    }
    // The summary must describe what is actually persisted, not what the loop
    // believed it saved: re-check every counted success against the final
    // committed dataset so the counter can never disagree with the rows.
    const finalPayrolls = buildDataset().payrolls;
    for (const person of successPersons) {
      const saved = finalPayrolls.find(
        (item) =>
          item.personelId === person.id &&
          item.donemId === aktifDonem.id &&
          item.accrualType === 'NORMAL'
      );
      if (saved?.status !== 'CALCULATED' && saved?.status !== 'FINALIZED') {
        const index = outcomes.indexOf('success');
        if (index >= 0) outcomes[index] = 'calculation-error';
        recordBatchError(
          person,
          `Hesaplama hatası: bordro kaydedildi ancak son durumda ${saved?.status ?? 'kayıt yok'}; yeniden hesaplayın.`
        );
      }
    }
    const replayNote =
      `${replayedChainEvents > 0 ? ` Önceki dönemlerden güncelliğini yitirmiş ${replayedChainEvents} tahakkuk otomatik yeniden hesaplandı.` : ''}` +
      `${replayedFollowerEvents > 0 ? ` Bu dönemin ${replayedFollowerEvents} ek tahakkuku (tediye/TİS/ek ödeme) yeniden hesaplandı.` : ''}`;
    const summary = summarizeBatchPayrollOutcomes(outcomes);
    const calculableTotal = summary.total - summary.finalizedSkipped - summary.notApplicable;
    const skippedNote =
      `${summary.finalizedSkipped > 0 ? ` Bu dönemi zaten kesinleşmiş ${summary.finalizedSkipped} bordro atlandı.` : ''}` +
      `${summary.notApplicable > 0 ? ` Bu dönem için hesaplanamayan ${summary.notApplicable} personel: ${notApplicablePersons.slice(0, 3).join('; ')}${notApplicablePersons.length > 3 ? '…' : ''}.` : ''}`;
    const outcomeSummary = `Hesaplanan bordro: ${summary.success}/${calculableTotal} (toplam personel ${summary.total}).`;
    const attendanceIssueCount = summary.attendanceMissing + summary.attendanceIncomplete;
    if (summary.calculationErrors === 0 && attendanceIssueCount === 0 && followerFailures === 0) {
      setErrorMessage(null);
      setSuccessMessage(
        `Hesaplanabilir ${calculableTotal} personelin ${summary.success} bordrosu başarıyla güncellendi.${skippedNote}${replayNote}`
      );
      setTimeout(() => setSuccessMessage(null), summary.notApplicable > 0 ? 12000 : 3500);
    } else {
      setSuccessMessage(
        summary.success > 0
          ? `${outcomeSummary}${skippedNote}${replayNote}`
          : null
      );
      const problems: string[] = [];
      if (missingPuantajPersons.length > 0) {
        problems.push(`Puantaj yok (${missingPuantajPersons.length}): ${missingPuantajPersons.slice(0, 3).join(', ')}${missingPuantajPersons.length > 3 ? '…' : ''}.`);
      }
      if (incompletePuantajPersons.length > 0) {
        problems.push(`Puantaj eksik/tamamlanmamış (${incompletePuantajPersons.length}): ${incompletePuantajPersons.slice(0, 3).join(', ')}${incompletePuantajPersons.length > 3 ? '…' : ''}.`);
      }
      if (summary.calculationErrors > 0 || followerFailures > 0) {
        const failures = batchCalculationErrorsRef.current.slice(0, 3).join(' | ');
        problems.push(
          `${summary.calculationErrors + followerFailures} hesaplama hatası${failures ? `: ${failures}` : firstBatchCalculationErrorRef.current ? `: ${firstBatchCalculationErrorRef.current}` : '.'}`
        );
      }
      setErrorMessage(`${outcomeSummary} ${problems.join(' ')}`);
    }
  };

  const handleOpenPaySlip = async (person: Personel, requestedBordro?: BordroKaydi) => {
    let bordro = requestedBordro || getActiveViewPayroll(person.id);
    if (bordro?.status === 'STALE') {
      setErrorMessage(formatStalePayrollMessage(`${person.ad} ${person.soyad}`));
      return;
    }
    if (bordro?.status === 'DRAFT') {
      setErrorMessage(`${person.ad} ${person.soyad} bordrosu taslak durumda. Önce bordroyu hesaplayın.`);
      return;
    }
    if (!bordro) {
      if (isSupplementaryView) {
        setErrorMessage(`${person.ad} ${person.soyad} için ${activeViewTitle.toLocaleLowerCase('tr-TR')} kaydı henüz yok. Önce tahakkuk ekleyin.`);
        return;
      }
      const hasPuantaj = puantajlar.some(
        (p) => p.personelId === person.id && p.donemId === aktifDonem.id
      );
      if (!hasPuantaj) {
        setErrorMessage(`HATA: ${person.ad} ${person.soyad} için bu dönemde kayıtlı puantaj bulunmadığından bordro zarfı açılamıyor.`);
        return;
      }
      bordro = (await calculateAndSaveForPerson(person)) || undefined;
    }
    if (bordro) setActivePaySlip({ personel: person, bordro });
  };

  const openSupplementaryAccrualForm = (person: Personel) => {
    setNewAccrualPersonId(person.id);
    setExpandedTimelinePersonId(person.id);
    setSupplementaryAccrualDraft({
      accrualType: activeAccrualType === 'NORMAL' ? 'TEDIYE' : (activeAccrualType as SupplementaryAccrualType),
      paymentDate: getDefaultAccrualPaymentDate(aktifDonem),
      grossAmount: '',
      description: '',
    });
    setErrorMessage(null);
  };

  const handleRecalculateAccrual = async (person: Personel, accrual: BordroKaydi, event: MouseEvent) => {
    event.stopPropagation();
    if (accrual.status === 'FINALIZED') {
      setErrorMessage(`${person.ad} ${person.soyad} için kesinleştirilmiş kayıt yeniden hesaplanamaz.`);
      return;
    }
    try {
      const calculationSnapshot = buildDataset();
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: null,
        accrual: {
          accrualId: getAccrualId(accrual),
          accrualType: accrual.accrualType,
          paymentDate: accrual.paymentDate || getDefaultAccrualPaymentDate(aktifDonem),
          sequence: accrual.sequence,
          grossAmount: String(
            accrual.accrualType === 'TEDIYE'
              ? accrual.gelirler.tediye ?? 0
              : accrual.accrualType === 'TIS_IKRAMIYE'
                ? accrual.gelirler.tisIkramiyesi ?? 0
                : accrual.gelirler.ekOdeme ?? 0
          ),
          description: accrual.accrualDescription ?? null,
        },
        dataset: calculationSnapshot,
      });
      assertPayrollCalculationSnapshotCurrentForEngine(
        payrollEngine.kind,
        calculationSnapshot,
        authoritativeDatasetRef.current
      );
      await onSaveBordro(calculated, calculationSnapshot);
      setSuccessMessage(`${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} yeniden hesaplandı.`);
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    }
  };

  const handleCalculateSupplementary = async (person: Personel, event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const grossAmount = supplementaryAccrualDraft.grossAmount.trim();
    const paymentDate = supplementaryAccrualDraft.paymentDate.trim();
    if (!isExactDecimalString(grossAmount) || grossAmount.startsWith('-')) {
      setErrorMessage('Ek ödeme brüt tutarı geçerli ve negatif olmayan bir tutar olmalıdır.');
      return;
    }
    if (!/^\d{4}-\d{2}-\d{2}$/.test(paymentDate)) {
      setErrorMessage('Ödeme/tahakkuk tarihi YYYY-AA-GG biçiminde olmalıdır.');
      return;
    }
    const paymentScope = paymentEventSequenceScopeKey(
      person.id,
      aktifDonem.taxYear,
      aktifDonem.taxMonth,
      paymentDate
    );
    const releasePaymentScope = tryAcquireSupplementaryPaymentScope(
      pendingSupplementaryPaymentScopes.current,
      paymentScope
    );
    if (!releasePaymentScope) {
      setErrorMessage('Bu kişi ve ödeme tarihi için başka bir ek tahakkuk hesaplaması sürüyor.');
      return;
    }
    setVisiblePendingSupplementaryPaymentScopes((current) => {
      const next = new Set(current);
      next.add(paymentScope);
      return next;
    });
    try {
      const calculationSnapshot = buildDataset();
      const nextSequence = nextPaymentSequence(
        calculationSnapshot,
        person.id,
        aktifDonem,
        paymentDate
      );
      const accrual: PayrollBoundaryAccrualInput = {
        accrualId: `${person.id}_${aktifDonem.id}_${supplementaryAccrualDraft.accrualType.toLowerCase()}_${paymentDate}_${nextSequence}`,
        accrualType: supplementaryAccrualDraft.accrualType,
        paymentDate,
        sequence: nextSequence,
        grossAmount,
        description: supplementaryAccrualDraft.description.trim() || null,
      };
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: null,
        accrual,
        dataset: calculationSnapshot,
      });
      assertPayrollCalculationSnapshotCurrentForEngine(
        payrollEngine.kind,
        calculationSnapshot,
        authoritativeDatasetRef.current
      );
      await onSaveBordro(calculated, calculationSnapshot);
      setNewAccrualPersonId(null);
      setSuccessMessage(`${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} tahakkuku hesaplandı.`);
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      console.error('Supplementary payroll calculation failed:', err);
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    } finally {
      releasePaymentScope();
      setVisiblePendingSupplementaryPaymentScopes((current) => {
        if (!current.has(paymentScope)) return current;
        const next = new Set(current);
        next.delete(paymentScope);
        return next;
      });
    }
  };

  const isSupplementaryPaymentScopePending = (
    personId: string,
    paymentDate: string
  ): boolean =>
    visiblePendingSupplementaryPaymentScopes.has(
      paymentEventSequenceScopeKey(
        personId,
        aktifDonem.taxYear,
        aktifDonem.taxMonth,
        paymentDate
      )
    );

  const handleCalculateSingle = async (person: Personel, event: MouseEvent, requestedAccrual?: BordroKaydi) => {
    event.stopPropagation();
    if (isSupplementaryView) {
      if (requestedAccrual) await handleRecalculateAccrual(person, requestedAccrual, event);
      else openSupplementaryAccrualForm(person);
      return;
    }
    const hasPuantaj = puantajlar.some(
      (p) => p.personelId === person.id && p.donemId === aktifDonem.id
    );
    if (!hasPuantaj) {
      setSuccessMessage(null);
      setErrorMessage(`HATA: ${person.ad} ${person.soyad} için bu dönemde (${aktifDonem.donemAdi}) kayıtlı puantaj bulunamadı! Puantajsız bordro hesaplanamaz. Lütfen önce Puantaj Cetvelinden puantaj girişi yapın.`);
      return;
    }
    const singleDataset = buildDataset();
    const singleTarget = getNormalAccrualInput(person.id, singleDataset);
    const singleNotApplicable = getBatchNotApplicableReason(
      singleDataset as unknown as Parameters<typeof getBatchNotApplicableReason>[0],
      person,
      aktifDonem,
      {
        paymentDate: singleTarget.paymentDate,
        sequence: singleTarget.sequence,
        accrualId: singleTarget.accrualId,
        id: singleTarget.accrualId,
      }
    );
    if (singleNotApplicable) {
      setSuccessMessage(null);
      setErrorMessage(`${person.ad} ${person.soyad} için ${aktifDonem.donemAdi} bordrosu hesaplanamaz: ${singleNotApplicable}.`);
      return;
    }
    const chain = await replayStalePriorChain(person);
    if (!chain.ok) {
      setSuccessMessage(null);
      setErrorMessage(`Hesaplama hatası: ${chain.error ?? 'önceki tahakkuk zinciri yeniden hesaplanamadı.'}`);
      return;
    }
    const res = await calculateAndSaveForPerson(person);
    if (res) {
      const followers = await replayStaleSamePeriodFollowers(person);
      if (!followers.ok) {
        setSuccessMessage(null);
        setErrorMessage(`${person.ad} ${person.soyad} normal bordrosu hesaplandı ancak ek tahakkuk yeniden hesaplanamadı: ${followers.error}`);
        return;
      }
      setErrorMessage(null);
      setSuccessMessage(
        `${person.ad} ${person.soyad} bordrosu başarıyla hesaplandı.${chain.replayed > 0 ? ` Önceki dönemlerden ${chain.replayed} güncelliğini yitirmiş tahakkuk da yeniden hesaplandı.` : ''}${followers.replayed > 0 ? ` Bu dönemin ${followers.replayed} ek tahakkuku da yeniden hesaplandı.` : ''}`
      );
      setTimeout(() => setSuccessMessage(null), 3000);
    }
  };

  const handleFinalizeSuccess = async (person: Personel, finalizedBordro: PayrollBoundaryPayroll) => {
    await onSaveBordro(finalizedBordro);
    setErrorMessage(null);
    setSuccessMessage(`${person.ad} ${person.soyad} bordrosu kesinleştirildi.`);
    setTimeout(() => setSuccessMessage(null), 3500);
  };

  const matchesSupplementaryAccrualStatus = (payroll: BordroKaydi): boolean => {
    if (rowFilter === 'all' || rowFilter === 'attendanceMissing') return true;
    if (rowFilter === 'notCalculated') return payroll.status === 'DRAFT';
    if (rowFilter === 'stale') return payroll.status === 'STALE';
    if (rowFilter === 'calculated') return payroll.status === 'CALCULATED';
    return payroll.status === 'FINALIZED';
  };

  const matchesRowFilter = (person: Personel): boolean => {
    if (rowFilter === 'all') return true;
    const attendance = puantajlar.find(
      (item) => item.personelId === person.id && item.donemId === aktifDonem.id
    );
    const hasAttendance = Boolean(attendance?.gunler && Object.keys(attendance.gunler).length > 0);
    if (rowFilter === 'attendanceMissing') return !hasAttendance;
    if (isSupplementaryView) {
      const payrolls = getActiveViewAccruals(person.id);
      if (rowFilter === 'notCalculated') {
        return payrolls.length === 0 || payrolls.some(matchesSupplementaryAccrualStatus);
      }
      return payrolls.some(matchesSupplementaryAccrualStatus);
    }
    const payroll = getActiveViewPayroll(person.id);
    if (rowFilter === 'notCalculated') return hasAttendance && (!payroll || payroll.status === 'DRAFT');
    if (rowFilter === 'stale') return payroll?.status === 'STALE';
    if (rowFilter === 'calculated') return payroll?.status === 'CALCULATED';
    return payroll?.status === 'FINALIZED';
  };

  return {
    payrollEngine,
    searchTerm,
    setSearchTerm,
    rowFilter,
    setRowFilter,
    activePaySlip,
    setActivePaySlip,
    deletingAccrualId,
    setDeletingAccrualId,
    isBatchProcessing,
    successMessage,
    setSuccessMessage,
    errorMessage,
    setErrorMessage,
    normalPaymentDateMap,
    setNormalPaymentDateMap,
    newAccrualPersonId,
    setNewAccrualPersonId,
    expandedTimelinePersonId,
    setExpandedTimelinePersonId,
    supplementaryAccrualDraft,
    setSupplementaryAccrualDraft,
    isSupplementaryPaymentScopePending,
    manualKumulatifGvMap,
    setManualKumulatifGvMap,
    manualKumulatifAsgariGvMap,
    setManualKumulatifAsgariGvMap,
    isKumulatifModalOpen,
    setIsKumulatifModalOpen,
    getAccrualId,
    getPersonAccruals,
    getActiveViewAccruals,
    getActiveViewPayroll,
    getDevirGvMatrahiForActiveYear,
    buildDataset,
    handleCalculateAll,
    handleOpenPaySlip,
    openSupplementaryAccrualForm,
    handleRecalculateAccrual,
    handleCalculateSupplementary,
    handleCalculateSingle,
    handleFinalizeSuccess,
    matchesSupplementaryAccrualStatus,
    matchesRowFilter,
  };
}
