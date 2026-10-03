import type { BordroKaydi, RetroAdjustmentBatch, RetroAllocation } from '../types/payroll';

export interface RetroRecoveryAssessment {
  /** True when the row may offer the deterministic "Yeniden Hesapla" recovery. */
  eligible: boolean;
  /**
   * Source service periods whose NORMAL/supplementary payroll is DRAFT/STALE.
   * They must be recalculated on the payroll screen before the retro ledger
   * can be replayed. A period with no source payroll at all is not listed:
   * the core engine decides whether it is still in the revision window and,
   * if so, rejects the replay with an explicit missing-NORMAL error instead
   * of the UI hiding the recovery action forever (BUG-RETRO-002).
   */
  blockedSourcePeriods: string[];
  /** Human readable reason when a stale/unsettled row cannot be recovered. */
  reason?: string;
}

const AUTHORITATIVE = ['CALCULATED', 'FINALIZED'];
const CLOSED_SETTLEMENTS = ['PAID', 'SETTLED_BY_OFFSET'];

export function retroPaymentEventNeedsReplay(
  batch: RetroAdjustmentBatch,
  payroll: BordroKaydi | undefined,
  allocations: RetroAllocation[],
  payrolls: BordroKaydi[]
): RetroRecoveryAssessment {
  const blockedSourcePeriods = [...new Set(allocations
    .filter((allocation) => allocation.batchId === batch.id)
    .map((allocation) => allocation.sourcePeriodId))]
    .filter((periodId) => payrolls.some((event) =>
      event.personelId === batch.personnelId &&
      event.donemId === periodId &&
      event.accrualType !== 'RETRO_ADJUSTMENT' &&
      !AUTHORITATIVE.includes(event.status)
    ));

  if (batch.status === 'FINALIZED' || CLOSED_SETTLEMENTS.includes(batch.settlementStatus ?? '')) {
    return { eligible: false, blockedSourcePeriods: [] };
  }

  const linkedEventMatches = payroll === undefined || (
    payroll.accrualType === 'RETRO_ADJUSTMENT' &&
    payroll.accrualId === batch.id &&
    payroll.personelId === batch.personnelId &&
    payroll.paymentDate === batch.paymentDate
  );
  if (!linkedEventMatches) {
    return {
      eligible: false,
      blockedSourcePeriods,
      reason: 'Bağlı ödeme olayı bu batch ile eşleşmiyor (personel/ödeme tarihi); kayıt elle incelenmelidir.',
    };
  }
  if (payroll?.status === 'FINALIZED') {
    return {
      eligible: false,
      blockedSourcePeriods,
      reason: 'Ödeme olayı kesinleşmiş; düzeltme için yeni bir retro revision/batch oluşturun.',
    };
  }
  if (blockedSourcePeriods.length) {
    return {
      eligible: false,
      blockedSourcePeriods,
      reason: `Önce Bordro Hesaplama ekranında kaynak dönem bordrolarını yeniden hesaplayın: ${blockedSourcePeriods.join(', ')}.`,
    };
  }

  if (batch.status === 'STALE') {
    // A stale ledger is never an active payment candidate. It is always
    // recoverable by a deterministic replay with the same batch id: the
    // linked event (STALE, CALCULATED or deleted) is rebuilt in place, or a
    // zero/negative result is stored as a settlement-only ledger.
    return { eligible: true, blockedSourcePeriods };
  }

  if (payroll !== undefined && isSupersededZeroPayableRetroEvent(payroll, [batch])) {
    // Legacy split-brain left by older builds: the ledger already says
    // "0 TL / no payment" but the old STALE event row is still persisted.
    // The same deterministic replay retires that row natively (atomic with
    // the ledger rewrite), so the recovery must stay reachable.
    return { eligible: true, blockedSourcePeriods };
  }

  const eligible = batch.status === 'CALCULATED' &&
    (batch.payableSettlementAmount ?? 0) > 0 &&
    batch.settlementStatus !== 'OVERPAYMENT' &&
    payroll !== undefined &&
    payroll.status === 'STALE';
  return { eligible, blockedSourcePeriods };
}

/**
 * Mirrors the core engine's `is_superseded_zero_payable_retro`: a STALE retro
 * event whose batch was recalculated to a CALCULATED ledger without any
 * payable amount is not a payment obligation. The native settlement-only save
 * now retires such an event in the same transaction as the ledger rewrite, so
 * this only matches legacy rows written by older builds; those must neither
 * block the NORMAL payroll chain nor be shown as a net payment (BUG-RETRO-002).
 */
export function isSupersededZeroPayableRetroEvent(
  payroll: Pick<BordroKaydi, 'accrualType' | 'status' | 'accrualId' | 'id' | 'personelId' | 'paymentDate'>,
  batches: ReadonlyArray<Pick<RetroAdjustmentBatch, 'id' | 'personnelId' | 'paymentDate' | 'status' | 'payableSettlementAmount' | 'totalGrossDelta' | 'settlementStatus'>>
): boolean {
  if (payroll.accrualType !== 'RETRO_ADJUSTMENT' || payroll.status !== 'STALE') return false;
  const batchId = payroll.accrualId || payroll.id;
  return batches.some((batch) =>
    batch.id === batchId &&
    batch.personnelId === payroll.personelId &&
    batch.paymentDate === payroll.paymentDate &&
    batch.status === 'CALCULATED' &&
    Number(batch.payableSettlementAmount ?? 0) <= 0 &&
    (Number(batch.totalGrossDelta) <= 0 || batch.settlementStatus === 'SETTLED_BY_OFFSET')
  );
}
