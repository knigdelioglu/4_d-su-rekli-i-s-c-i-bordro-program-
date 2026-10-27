import type { BordroDonemi, BordroKaydi, RetroAdjustmentBatch } from '../../types/payroll';
import type { PayrollCalculationRequest, PayrollBoundaryAccrualInput, PayrollDatasetSnapshot } from './types';
import { comparePaymentEvents } from './paymentEventOrder';
import { isSupersededZeroPayableRetroEvent } from '../../components/retroEventRecovery';
import { getDefaultAccrualPaymentDate } from '../../utils/payrollPresentation';
import { describeError } from '../../utils/errorMessage';
const MAX_CHAIN_REPLAY_STEPS = 240;
const ACCRUAL_TYPE_LABELS = { NORMAL: 'Normal bordro', TEDIYE: 'Tediye', TIS_IKRAMIYE: 'TİS ikramiyesi', SUPPLEMENTAL: 'Ek ödeme', RETRO_ADJUSTMENT: 'Geriye dönük fark' };
const formatPayrollError = describeError;

/** A current payment row cannot make a stale entitlement ledger authoritative. */
export function payrollNeedsReplay(event: BordroKaydi, dataset: PayrollDatasetSnapshot): boolean {
  const staleLedger = event.accrualType === 'RETRO_ADJUSTMENT'
    && dataset.retroBatches?.some((batch) => batch.id === (event.accrualId || event.id) && batch.status === 'STALE');
  return staleLedger || event.status === 'STALE' || event.status === 'DRAFT';
}

export function assertRetroRecovered(batchId: string, dataset: PayrollDatasetSnapshot): void {
  const batch = dataset.retroBatches?.find((item) => item.id === batchId);
  const events = dataset.payrolls.filter((event) => (event.accrualId || event.id) === batchId);
  if (!batch || !['CALCULATED', 'FINALIZED'].includes(batch.status)
    || events.some((event) => !['CALCULATED', 'FINALIZED'].includes(event.status))
    || (Number(batch.payableSettlementAmount ?? 0) > 0 && !events.length)) {
    throw new Error(`Geriye dönük fark (${batchId}) yeniden hesaplandı ancak güncel kayıt doğrulanamadı. Verileri yeniden yükleyin; sonraki ödeme hesaplanmadı.`);
  }
}

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
  targetPeriod: BordroDonemi,
  retroBatches: ReadonlyArray<RetroAdjustmentBatch> = []
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
  return prior.find((item) =>
    (item.payroll.status === 'STALE' || item.payroll.status === 'DRAFT'
      || (item.payroll.accrualType === 'RETRO_ADJUSTMENT' && retroBatches.some((batch) => batch.id === (item.payroll.accrualId || item.payroll.id) && batch.status === 'STALE'))) &&
    // A stale retro event retired by a zero-payable recalculated ledger is an
    // audit record; the core engine skips it, so it must not block the chain.
    !isSupersededZeroPayableRetroEvent(item.payroll, retroBatches)
  ) ?? null;
}

/** Replay in payment order, keeping retro recovery in its dedicated ledger
 * workflow. Never attempt a later payment using an unresolved retro event. */
export async function replaySamePeriodPayrollFollowers(options: {
  personnelId: string;
  period: BordroDonemi;
  getDataset: () => PayrollDatasetSnapshot;
  recalculate: (event: BordroKaydi, dataset: PayrollDatasetSnapshot) => Promise<void>;
  recoverRetro?: (batchId: string) => Promise<void>;
  afterEvent?: BordroKaydi;
  formatError?: (error: unknown) => string;
}): Promise<ChainReplayResult> {
  let replayed = 0;
  const attempted = new Set<string>();
  for (let step = 0; step < MAX_CHAIN_REPLAY_STEPS; step++) {
    const dataset = options.getDataset();
    const next = (dataset.payrolls as unknown as BordroKaydi[])
      .filter((payroll) =>
        payroll.personelId === options.personnelId &&
        payroll.donemId === options.period.id &&
        (options.afterEvent ? comparePaymentEvents(payroll, options.afterEvent, options.period) > 0 : payroll.accrualType !== 'NORMAL') &&
        payrollNeedsReplay(payroll, dataset) &&
        !isSupersededZeroPayableRetroEvent(payroll, (dataset.retroBatches ?? []) as unknown as RetroAdjustmentBatch[])
      )
      .sort((left, right) => comparePaymentEvents(left, right, options.period))[0];
    if (!next) return { ok: true, replayed };
    const eventId = next.accrualId || next.id;
    if (attempted.has(eventId)) return { ok: false, replayed,
      error: `${ACCRUAL_TYPE_LABELS[next.accrualType]} (${eventId}) yeniden hesaplandı ancak güncelliğini yitirmiş durumda kaldı. Sonraki tahakkuklar hesaplanmadı; kaynak bağımlılıklarını kontrol edin.` };
    attempted.add(eventId);
    try {
      if (next.accrualType === 'RETRO_ADJUSTMENT') {
        if (!options.recoverRetro) throw new Error('Geriye Dönük Farklar ekranından yeniden hesaplayın; sonraki tahakkuklar bu kayda bağlı.');
        await options.recoverRetro(eventId);
        assertRetroRecovered(eventId, options.getDataset());
      } else {
        await options.recalculate(next, dataset);
        const saved = options.getDataset().payrolls.find((event) => (event.accrualId || event.id) === eventId);
        if (!saved || payrollNeedsReplay(saved as unknown as BordroKaydi, options.getDataset())) {
          throw new Error('Tahakkuk güncel duruma geçmedi. Kaynak bağımlılıkları çözülmeden sonraki ödeme hesaplanamaz.');
        }
      }
      replayed++;
    } catch (err) {
      return {
        ok: false, replayed,
        error: `${ACCRUAL_TYPE_LABELS[next.accrualType] ?? next.accrualType} (${eventId}) yeniden hesaplanamadı: ${(options.formatError ?? formatPayrollError)(err)}`,
      };
    }
  }
  return { ok: false, replayed, error: 'Tahakkuk zinciri çok uzun; yeniden hesaplama güvenli sınırda durduruldu.' };
}

export function storedManualIncome(event: BordroKaydi): PayrollCalculationRequest['manualIncome'] {
  if (event.accrualType !== 'NORMAL') return null;
  const tediye = event.gelirler?.tediye ?? null;
  const tisIkramiyesi = event.gelirler?.tisIkramiyesi ?? null;
  if (tediye === null && tisIkramiyesi === null) return null;
  return { tediye, tisIkramiyesi } as unknown as PayrollCalculationRequest['manualIncome'];
}

export function storedAccrualInput(event: BordroKaydi, period: BordroDonemi): PayrollBoundaryAccrualInput {
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


/** Rebuild the existing payment chain in canonical order; never invent a missing historical source record. */
export async function replayThroughRetroBatch(options: {
  batchId: string; getDataset: () => PayrollDatasetSnapshot;
  recalculate: (event: BordroKaydi, period: BordroDonemi, dataset: PayrollDatasetSnapshot) => Promise<void>;
  recoverRetro: (batchId: string) => Promise<void>;
}): Promise<ChainReplayResult> {
  let replayed = 0;
  try {
    const initial = options.getDataset();
    const batch = initial.retroBatches?.find((item) => item.id === options.batchId);
    if (!batch) throw new Error('Geriye dönük fark kaydı bulunamadı. Verileri yeniden yükleyin.');
    if (batch.status === 'FINALIZED' || ['PAID', 'SETTLED_BY_OFFSET'].includes(batch.settlementStatus ?? '')) {
      throw new Error('Kesinleşmiş, ödenmiş veya mahsupla kapatılmış geriye dönük fark değiştirilemez. Doğrulanmış geçmiş için yeni bir düzeltme revizyonu kullanın.');
    }
    const linked = initial.payrolls.find((item) => (item.accrualId || item.id) === batch.id);
    if (linked?.status === 'FINALIZED') throw new Error('Bağlı ödeme kesinleşmiş; aynı kayıt yeniden yazılamaz. Yeni bir düzeltme revizyonu gerekir.');
    const periods = initial.periods.filter((period) => linked ? period.id === linked.donemId
      : period.taxYear === Number(batch.paymentDate.slice(0, 4)) && period.taxMonth === Number(batch.paymentDate.slice(5, 7)));
    if (periods.length !== 1) throw new Error(`${batch.paymentDate} ödeme tarihi için tek bir bordro dönemi belirlenemedi. Ödeme dönemi kaydını kontrol edin.`);
    const period = periods[0] as unknown as BordroDonemi;
    const target = (linked ?? { id: batch.id, accrualId: batch.id, personelId: batch.personnelId, donemId: period.id,
      accrualType: 'RETRO_ADJUSTMENT', paymentDate: batch.paymentDate,
      sequence: Math.max(-1, ...initial.payrolls.filter((item) => item.personelId === batch.personnelId && item.paymentDate === batch.paymentDate).map((item) => item.sequence)) + 1,
    }) as unknown as BordroKaydi;
    const attempted = new Set<string>();
    for (let step = 0; step < MAX_CHAIN_REPLAY_STEPS; step++) {
      const dataset = options.getDataset();
      const prior = findFirstStalePriorEvent(dataset.payrolls as unknown as BordroKaydi[],
        new Map(dataset.periods.map((item) => [item.id, item as unknown as BordroDonemi])), batch.personnelId, target, period,
        dataset.retroBatches as unknown as RetroAdjustmentBatch[]);
      if (!prior) {
        await options.recoverRetro(batch.id);
        assertRetroRecovered(batch.id, options.getDataset());
        const followers = await replaySamePeriodPayrollFollowers({ personnelId: batch.personnelId, period, afterEvent: target,
          getDataset: options.getDataset, recoverRetro: options.recoverRetro,
          recalculate: (event, snapshot) => options.recalculate(event, period, snapshot) });
        if (followers.ok) {
          try { assertRetroRecovered(batch.id, options.getDataset()); }
          catch { throw new Error(`Bağlı tahakkukun yeniden hesabı ${batch.id} geriye dönük farkını yeniden geçersiz kıldı. Kaynak dönemler ile ${batch.paymentDate} ödeme sırası arasında bağımlılık döngüsü var. Revizyonun kendi sonraki ödeme tahakkuklarını kaynak kapsamına alıp almadığını kontrol edin; kapsam veya ödeme tarihini yalnız doğrulanmış dayanağa göre düzeltin.`); }
        }
        return { ...followers, replayed: replayed + 1 + followers.replayed };
      }
      const id = prior.payroll.accrualId || prior.payroll.id;
      if (attempted.has(id)) throw new Error(`${prior.period.donemAdi || prior.period.id} (${id}) güncel duruma geçmedi. Zincirdeki kaynak bağımlılığı çözülmeden sonraki ödeme hesaplanamaz.`);
      attempted.add(id);
      if (prior.payroll.accrualType === 'RETRO_ADJUSTMENT') {
        await options.recoverRetro(id); assertRetroRecovered(id, options.getDataset());
      } else await options.recalculate(prior.payroll, prior.period, dataset);
      replayed++;
    }
    throw new Error('Tahakkuk zinciri güvenli işlem sınırını aştı. Kaynak kayıtlarını kontrol edin.');
  } catch (error) { return { ok: false, replayed, error: describeError(error) }; }
}
