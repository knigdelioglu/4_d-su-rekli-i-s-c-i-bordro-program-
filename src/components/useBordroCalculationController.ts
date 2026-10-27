import { useLayoutEffect, useRef, useState, useSyncExternalStore, type FormEvent, type MouseEvent } from 'react';
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
  RetroAdjustmentBatch,
} from '../types/payroll';
import { bulkPayrollJobs, type BulkPayrollJob, type BulkPayrollItem } from '../services/bulkPayrollJob';
import { assertRetroRecovered, findFirstStalePriorEvent, replaySamePeriodPayrollFollowers, storedManualIncome, storedAccrualInput, type ChainReplayResult } from '../services/payrollEngine/replayChain';
export { findFirstStalePriorEvent, replaySamePeriodPayrollFollowers } from '../services/payrollEngine/replayChain';
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
  if (isMissingCompensationPolicyBaseline(message)) {
    return 'Ücret politikası başlangıç sürümü eksik. NORMAL bordro hesaplanmadan önce Ücret Politikası ekranında açıkça onaylanmış bir başlangıç sürümü kaydedin.';
  }
  if (/road rounding mode|yol yuvarlama/i.test(message)) {
    return 'Yol yuvarlama biçimleri bu hizmet döneminde çakışıyor. Ücret Politikası ekranında “Bağlı düzeltmeleri önizle” adımıyla onarımı inceleyin.';
  }
  if (/UNSUPPORTED_HISTORICAL_REPLAY/i.test(message)) {
    return message.replace(/(?:Geçersiz veri:\s*)?UNSUPPORTED_HISTORICAL_REPLAY:\s*/i, '') + ' Kaynak bordronun özgün hesaplama kaydını veya yedeğini kontrol edin; geçmiş değerleri tahmin ederek doldurmayın.';
  }
  if (/CURRENT_ENGINE_SNAPSHOT_REQUIRED|authoritative (?:snapshot|değil)/i.test(message)) {
    return 'Bordro hesaplama bilgileri güncel değil. Verileri yeniden yükleyip ilgili bordroyu yeniden hesaplayın.';
  }
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

export function isMissingCompensationPolicyBaseline(message: string): boolean {
  return /(?:bu tarih için ücret politikası tanımlı değil|ücret politikası başlangıç state'i bulunamadı)/i.test(message);
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
  getAuthoritativeDataset?: () => PayrollDatasetSnapshot;
  bordrolar: BordroKaydi[];
  personeller: Personel[];
  puantajlar: PersonelPuantaj[];
  isSupplementaryView: boolean;
  onOpenCompensationPolicy?: () => void;
  onRecoverRetroBatch?: (batchId: string) => Promise<void>;
  onSaveBordro: (
    bordro: PayrollBoundaryPayroll,
    calculationSnapshot?: PayrollDatasetSnapshot,
    saveSource?: 'ENGINE_FINALIZATION'
  ) => Promise<void> | void;
}

const DATASET_COMMIT_TIMEOUT_MS = 5000;
const MAX_CHAIN_REPLAY_STEPS = 240;

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
  activePeriod: BordroDonemi
): string | null {
  const periodsById = new Map(dataset.periods.map((period) => [period.id, period]));
  // FINALIZED dependency decisions belong to the shared Rust policy. A later
  // event can be independent after a tax-year reset and expired PEK carry.

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
  getAuthoritativeDataset,
  bordrolar,
  personeller,
  puantajlar,
  isSupplementaryView,
  onSaveBordro,
  onOpenCompensationPolicy,
  onRecoverRetroBatch,
}: UseBordroCalculationControllerOptions) {
  const payrollEngine = getPayrollEngine();
  const authoritativeDatasetRef = useRef(authoritativeDataset);
  useLayoutEffect(() => {
    authoritativeDatasetRef.current = authoritativeDataset;
  }, [authoritativeDataset]);

  /**
   * A save only schedules a React state update; the next calculation must not
   * start from the pre-save snapshot. Sequential flows (batch, chain replay)
   * wait here until the saved dataset has been committed to the ref.
   */
  const currentDataset = () => getAuthoritativeDataset?.() ?? authoritativeDatasetRef.current;
  const waitForDatasetCommit = (previous: PayrollDatasetSnapshot): Promise<void> => new Promise((resolve, reject) => {
    const deadline = Date.now() + DATASET_COMMIT_TIMEOUT_MS;
    const check = () => {
      if (currentDataset().payrolls !== previous.payrolls) { resolve(); return; }
      if (Date.now() >= deadline) { reject(new Error('Kaydedilen bordro verileri yenilenemedi. Verileri yeniden yükleyip işlemi sürdürün.')); return; }
      setTimeout(check, 20);
    };
    check();
  });
  const [searchTerm, setSearchTerm] = useState('');
  const [rowFilter, setRowFilter] = useState<PayrollRowFilter>('all');
  const [activePaySlip, setActivePaySlip] = useState<{
    personel: Personel;
    bordro: BordroKaydi;
  } | null>(null);
  const [deletingAccrualId, setDeletingAccrualId] = useState<string | null>(null);
  const bulkState = useSyncExternalStore(bulkPayrollJobs.subscribe, bulkPayrollJobs.getSnapshot, bulkPayrollJobs.getSnapshot);
  const isBatchProcessing = bulkState.busy;
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
  const batchCalculationErrorsRef = useRef<string[]>([]);
  const isBatchProcessingRef = useRef(false);
  const missingBaselineDuringBatchRef = useRef<string | null>(null);

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
    dataset: PayrollDatasetSnapshot = currentDataset(),
    fallbackPaymentDate?: string
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
          : fallbackPaymentDate ?? normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)),
      sequence: exactPayroll?.sequence ?? existingPayroll?.sequence ?? nextPaymentSequence(
        dataset,
        personId,
        aktifDonem,
        fallbackPaymentDate ?? normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem)
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

  const buildDataset = (): PayrollDatasetSnapshot => currentDataset();

  const missingCompensationBaseline = async (
    person: Personel,
    dataset: PayrollDatasetSnapshot,
    accrual: PayrollBoundaryAccrualInput = getNormalAccrualInput(person.id, dataset),
    manualIncome: PayrollCalculationRequest['manualIncome'] = getLegacyManualIncomeInput(person.id, dataset)
  ): Promise<string | null> => {
    if (accrual.accrualType !== 'NORMAL') return null;
    try {
      await payrollEngine.validatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome,
        accrual,
        dataset,
      });
      return null;
    } catch (error) {
      const raw = error && typeof error === 'object' && typeof (error as { message?: unknown }).message === 'string'
        ? (error as { message: string }).message
        : String(error);
      return isMissingCompensationPolicyBaseline(raw) ? raw : null;
    }
  };

  const showMissingCompensationBaseline = (message: string) => {
    setSuccessMessage(null);
    setErrorMessage(formatPayrollError(message));
    onOpenCompensationPolicy?.();
  };

  const calculateAndSaveForPerson = async (person: Personel, fallbackPaymentDate?: string): Promise<BordroKaydi | null> => {
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

    const baselineError = await missingCompensationBaseline(person, calculationSnapshot);
    if (baselineError) {
      if (isBatchProcessingRef.current) missingBaselineDuringBatchRef.current = baselineError;
      else showMissingCompensationBaseline(baselineError);
      return null;
    }

    try {
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: getLegacyManualIncomeInput(person.id, calculationSnapshot),
        accrual: getNormalAccrualInput(person.id, calculationSnapshot, fallbackPaymentDate),
        dataset: calculationSnapshot,
      });
      assertPayrollCalculationSnapshotCurrentForEngine(
        payrollEngine.kind,
        calculationSnapshot,
        currentDataset()
      );
      await onSaveBordro(calculated, calculationSnapshot);
      await waitForDatasetCommit(calculationSnapshot);
      return toPayrollUiModel(calculated) as unknown as BordroKaydi;
    } catch (err) {
      console.error('Payroll engine calculation failed:', err);
      const raw = err && typeof err === 'object' && typeof (err as { message?: unknown }).message === 'string'
        ? (err as { message: string }).message
        : String(err);
      if (isMissingCompensationPolicyBaseline(raw)) {
        if (isBatchProcessingRef.current) missingBaselineDuringBatchRef.current = raw;
        else showMissingCompensationBaseline(raw);
        return null;
      }
      const formattedError = `Hesaplama hatası: ${formatPayrollError(err)}`;
      batchCalculationErrorsRef.current.push(`${person.ad} ${person.soyad}: ${formattedError}`);
      setErrorMessage(formattedError);
      return null;
    }
  };

  /**
   * Brings the person's earlier payment-event chain back to an authoritative
   * state before the active period is calculated. Any earlier DRAFT/STALE
   * event (typically left behind by a personnel-card or settings change) is
   * recalculated in canonical payment-event order with its own stored inputs.
   * FINALIZED history is never touched; the shared mutation policy still
   * rejects a replay that would affect FINALIZED records.
   */
  const replayStalePriorChain = async (person: Personel, fallbackPaymentDate?: string): Promise<ChainReplayResult> => {
    let replayed = 0;
    let lastAttempt: string | null = null;
    for (let step = 0; step < MAX_CHAIN_REPLAY_STEPS; step++) {
      const dataset = buildDataset();
      const periodsById = new Map(
        dataset.periods.map((period) => [period.id, period as unknown as BordroDonemi])
      );
      const target = getNormalAccrualInput(person.id, dataset, fallbackPaymentDate);
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
        aktifDonem,
        (dataset.retroBatches ?? []) as unknown as RetroAdjustmentBatch[]
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
        if (event.accrualType === 'RETRO_ADJUSTMENT') {
          if (!onRecoverRetroBatch) throw new Error('Geriye Dönük Farklar ekranından bu kaydı yeniden hesaplayın.');
          await onRecoverRetroBatch(eventId);
          assertRetroRecovered(eventId, buildDataset());
          replayed++;
          continue;
        }
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
          currentDataset()
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
    return replaySamePeriodPayrollFollowers({
      personnelId: person.id,
      period: aktifDonem,
      getDataset: buildDataset,
      recoverRetro: onRecoverRetroBatch,
      formatError: formatPayrollError,
      recalculate: async (next, dataset) => {
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
          currentDataset()
        );
        await onSaveBordro(calculated, dataset);
        await waitForDatasetCommit(dataset);
      },
    });
  };

  const executeBulkItem = async (item: BulkPayrollItem, job: BulkPayrollJob): Promise<{ status: 'success' | 'skipped'; message?: string }> => {
    const dataset = buildDataset();
    const person = dataset.personnel.find((entry) => entry.id === item.personnelId) as unknown as Personel;
    const period = dataset.periods.find((entry) => entry.id === job.periodId);
    if (!person || !period || job.periodId !== aktifDonem.id) throw new Error('Personel veya işlem dönemi bulunamadı. İlgili dönemi seçip tekrar deneyin.');
    if (job.accrualType === 'NORMAL') {
      const existing = dataset.payrolls.find((entry) => entry.personelId === person.id && entry.donemId === job.periodId && entry.accrualType === 'NORMAL');
      if (existing?.status === 'FINALIZED') return { status: 'skipped', message: 'Kesinleşmiş bordro korundu.' };
      const reason = getBatchNotApplicableReason(dataset as unknown as Parameters<typeof getBatchNotApplicableReason>[0], person, aktifDonem);
      if (reason) return { status: 'skipped', message: reason };
      const coverage = classifyBatchAttendance(dataset.attendances.find((entry) => entry.personelId === person.id && entry.donemId === job.periodId), aktifDonem);
      if (coverage !== 'complete') throw new Error(coverage === 'missing' ? 'Puantaj bulunamadı. Önce puantaj girin.' : 'Puantaj eksik. Dönemin tüm günlerini tamamlayın.');
      const chain = await replayStalePriorChain(person, job.paymentDate);
      if (!chain.ok) throw new Error(chain.error);
      const result = await calculateAndSaveForPerson(person, job.paymentDate);
      if (!result) throw new Error(batchCalculationErrorsRef.current.at(-1) ?? missingBaselineDuringBatchRef.current ?? 'Bordro hesaplanamadı.');
      const followers = await replayStaleSamePeriodFollowers(person);
      if (!followers.ok) throw new Error(`Normal bordro kaydedildi; bağlı tahakkuk zinciri tamamlanamadı: ${followers.error}`);
      const saved = buildDataset().payrolls.find((entry) => entry.personelId === person.id && entry.donemId === job.periodId && entry.accrualType === 'NORMAL');
      if (saved?.status !== 'CALCULATED' && saved?.status !== 'FINALIZED') throw new Error('Bordronun güncel kayıt durumu doğrulanamadı. İşlemi sürdürün.');
      return { status: 'success' };
    }
    const snapshot = buildDataset();
    const accrualId = `bulk_${job.id}_${person.id}`;
    const existing = snapshot.payrolls.find((entry) => (entry.accrualId || entry.id) === accrualId);
    if (existing && (existing.personelId !== person.id || existing.donemId !== job.periodId || existing.accrualType !== job.accrualType)) {
      throw new Error('Tahakkuk kimliği başka bir kayda ait. Toplu işlem durduruldu.');
    }
    // A crash between repository commit and checkpoint must never create a second accrual.
    if (existing?.status === 'CALCULATED' || existing?.status === 'FINALIZED') return { status: 'success', message: 'Önceden kaydedilen tahakkuk doğrulandı.' };
    const accrual: PayrollBoundaryAccrualInput = item.accrual ?? {
      accrualId, accrualType: job.accrualType, paymentDate: job.paymentDate!,
      sequence: existing?.sequence ?? nextPaymentSequence(snapshot, person.id, aktifDonem, job.paymentDate!),
      grossAmount: job.grossAmount!, description: job.description || null,
    };
    bulkPayrollJobs.checkpoint(person.id, { accrual });
    try {
      const calculated = await payrollEngine.calculatePayroll({ personnelId: person.id, periodId: job.periodId,
        calculatedAt: new Date().toISOString(), manualIncome: null, accrual, dataset: snapshot });
      assertPayrollCalculationSnapshotCurrentForEngine(payrollEngine.kind, snapshot, buildDataset());
      await onSaveBordro(calculated, snapshot);
      await waitForDatasetCommit(snapshot);
      return { status: 'success' };
    } catch (error) { throw new Error(formatPayrollError(error)); }
  };

  const runBulkJob = async (job: BulkPayrollJob) => {
    if (bulkPayrollJobs.getSnapshot().busy) return;
    isBatchProcessingRef.current = true;
    batchCalculationErrorsRef.current = [];
    missingBaselineDuringBatchRef.current = null;
    try {
      await bulkPayrollJobs.run(job, executeBulkItem);
      if (missingBaselineDuringBatchRef.current) showMissingCompensationBaseline(missingBaselineDuringBatchRef.current);
    }
    finally { isBatchProcessingRef.current = false; }
  };
  const handleCalculateAll = async () => {
    if (bulkPayrollJobs.getSnapshot().busy) return;
    const previous = bulkPayrollJobs.getSnapshot().job;
    if (previous && previous.status !== 'completed') {
      setErrorMessage('Yarım kalan toplu işlem var. Önce ilgili dönemde işlemi sürdürün.');
      return;
    }
    await runBulkJob({ version: 1, id: crypto.randomUUID(), periodId: aktifDonem.id, accrualType: 'NORMAL', status: 'paused',
      paymentDate: normalPaymentDateMap[aktifDonem.id] ?? getDefaultAccrualPaymentDate(aktifDonem),
      items: personeller.map((person) => ({ personnelId: person.id, status: 'pending' })) });
  };
  const handleResumeBulk = async () => {
    const job = bulkPayrollJobs.getSnapshot().job;
    if (!job || job.periodId !== aktifDonem.id || bulkPayrollJobs.getSnapshot().busy) return;
    const dataset = buildDataset();
    const items = job.items.map((item) => {
      if (item.status !== 'success') return item;
      const saved = dataset.payrolls.find((entry) => job.accrualType === 'NORMAL'
        ? entry.personelId === item.personnelId && entry.donemId === job.periodId && entry.accrualType === 'NORMAL'
        : (entry.accrualId || entry.id) === `bulk_${job.id}_${item.personnelId}`);
      return saved?.status === 'CALCULATED' || saved?.status === 'FINALIZED' ? item : { ...item, status: 'pending' as const };
    });
    await runBulkJob({ ...job, items });
  };
  const handleStartSupplementaryBulk = async (ids: string[], draft: SupplementaryAccrualDraft) => {
    if (!ids.length || draft.accrualType === 'RETRO_ADJUSTMENT') throw new Error('Tahakkuk türü ve personel seçimi geçersiz.');
    if (bulkPayrollJobs.getSnapshot().busy) throw new Error('Başka bir hesaplama sürüyor. Tamamlanmasını bekleyin.');
    const previous = bulkPayrollJobs.getSnapshot().job;
    if (previous && previous.status !== 'completed') throw new Error('Önce yarım kalan toplu işlemi sürdürün veya kapatın.');
    if (!isExactDecimalString(draft.grossAmount.trim()) || draft.grossAmount.trim().startsWith('-')) throw new Error('Brüt tutar geçerli ve negatif olmayan bir tutar olmalıdır.');
    void runBulkJob({ version: 1, id: crypto.randomUUID(), periodId: aktifDonem.id, accrualType: draft.accrualType,
      status: 'paused', paymentDate: draft.paymentDate, grossAmount: draft.grossAmount.trim(), description: draft.description.trim(),
      items: [...new Set(ids)].map((personnelId) => ({ personnelId, status: 'pending' })) });
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
      const accrualInput: PayrollBoundaryAccrualInput = {
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
      };
      const baselineError = await missingCompensationBaseline(person, calculationSnapshot, accrualInput, null);
      if (baselineError) {
        showMissingCompensationBaseline(baselineError);
        return;
      }
      const calculated = await payrollEngine.calculatePayroll({
        personnelId: person.id,
        periodId: aktifDonem.id,
        calculatedAt: new Date().toISOString(),
        manualIncome: null,
        accrual: accrualInput,
        dataset: calculationSnapshot,
      });
      assertPayrollCalculationSnapshotCurrentForEngine(
        payrollEngine.kind,
        calculationSnapshot,
        currentDataset()
      );
      await onSaveBordro(calculated, calculationSnapshot);
      setSuccessMessage(`${person.ad} ${person.soyad} için ${ACCRUAL_TYPE_LABELS[accrual.accrualType]} yeniden hesaplandı.`);
      setErrorMessage(null);
      setTimeout(() => setSuccessMessage(null), 3500);
    } catch (err) {
      const raw = err && typeof err === 'object' && typeof (err as { message?: unknown }).message === 'string'
        ? (err as { message: string }).message
        : String(err);
      if (isMissingCompensationPolicyBaseline(raw)) {
        showMissingCompensationBaseline(raw);
        return;
      }
      setErrorMessage(`Tahakkuk hesaplama hatası: ${formatPayrollError(err)}`);
    }
  };

  const handleCalculateSupplementary = async (person: Personel) => {
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
        currentDataset()
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
    const singleNotApplicable = getBatchNotApplicableReason(
      singleDataset as unknown as Parameters<typeof getBatchNotApplicableReason>[0],
      person,
      aktifDonem
    );
    if (singleNotApplicable) {
      setSuccessMessage(null);
      setErrorMessage(`${person.ad} ${person.soyad} için ${aktifDonem.donemAdi} bordrosu hesaplanamaz: ${singleNotApplicable}.`);
      return;
    }
    const baselineError = await missingCompensationBaseline(person, singleDataset);
    if (baselineError) {
      showMissingCompensationBaseline(baselineError);
      return;
    }
    const chain = await replayStalePriorChain(person);
    if (!chain.ok) {
      if (isMissingCompensationPolicyBaseline(chain.error ?? '')) {
        showMissingCompensationBaseline(chain.error ?? '');
        return;
      }
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
    await onSaveBordro(finalizedBordro, undefined, 'ENGINE_FINALIZATION');
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

  const withCalculationLock = async (work: () => Promise<void>) => {
    try { await bulkPayrollJobs.runManual(work); }
    catch (error) { setErrorMessage(formatPayrollError(error)); }
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
    bulkState,
    handleResumeBulk,
    handleStartSupplementaryBulk,
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
    handleOpenPaySlip: (person: Personel, requestedBordro?: BordroKaydi) =>
      requestedBordro || getActiveViewPayroll(person.id)
        ? handleOpenPaySlip(person, requestedBordro)
        : withCalculationLock(() => handleOpenPaySlip(person, requestedBordro)),
    openSupplementaryAccrualForm,
    handleRecalculateAccrual: (person: Personel, accrual: BordroKaydi, event: MouseEvent) => withCalculationLock(() => handleRecalculateAccrual(person, accrual, event)),
    handleCalculateSupplementary: (person: Personel, event: FormEvent<HTMLFormElement>) => {
      // Prevent native navigation synchronously, before awaiting the shared job lock.
      event.preventDefault();
      event.stopPropagation();
      return withCalculationLock(() => handleCalculateSupplementary(person));
    },
    handleCalculateSingle: (person: Personel, event: MouseEvent, requestedAccrual?: BordroKaydi) => withCalculationLock(() => handleCalculateSingle(person, event, requestedAccrual)),
    handleFinalizeSuccess,
    matchesSupplementaryAccrualStatus,
    matchesRowFilter,
  };
}
