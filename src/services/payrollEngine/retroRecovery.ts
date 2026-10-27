import type { PayrollDatasetSnapshot, PayrollEngine, PayrollMutation, RetroCalculationResult } from './types';
import type { BordroDonemi, BordroKaydi } from '../../types/payroll';
import { storedAccrualInput, storedManualIncome, payrollNeedsReplay } from './replayChain';
import { comparePaymentEvents, nextPaymentSequence } from './paymentEventOrder';
import { applyBrowserPayrollImpact, applyBrowserRetroBatchImpact, assertBrowserMutationImpactAllowed } from '../storage/browserPayrollPolicies';
import { isDecimalBoundaryKey, isExactDecimalString } from './decimalBoundary';

const positive = (value: string) => {
  if (!isExactDecimalString(value)) throw new Error('Geriye dönük fark tutarı doğrulanamadı.');
  return !value.startsWith('-') && /[1-9]/.test(value);
};
const zero = (value: string | null | undefined) => value == null || /^-?0(?:\.0+)?$/.test(value);

function exactKey(value: unknown, key = ''): string {
  if (typeof value === 'string' && isDecimalBoundaryKey(key) && isExactDecimalString(value)) {
    const [whole, rawFraction = ''] = value.split('.');
    const negative = whole.startsWith('-');
    const integer = whole.replace(/^-/, '').replace(/^0+(?=\d)/, '');
    const fraction = rawFraction.replace(/0+$/, '');
    return JSON.stringify(`${negative && (integer !== '0' || fraction) ? '-' : ''}${integer}${fraction ? `.${fraction}` : ''}`);
  }
  if (Array.isArray(value)) return `[${value.map((item) => exactKey(item, key)).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.entries(value).filter(([, item]) => item !== undefined).sort(([a], [b]) => a.localeCompare(b))
    .map(([field, item]) => `${JSON.stringify(field)}:${exactKey(item, field)}`).join(',')}}`;
  return JSON.stringify(value);
}

/** Pure WASM-side preparation; the caller commits all three collections with the existing authoritative CAS. */
export async function recoverRetroBatchInSnapshot(engine: PayrollEngine, dataset: PayrollDatasetSnapshot, batchId: string, expected?: RetroCalculationResult): Promise<Pick<PayrollDatasetSnapshot, 'payrolls' | 'retroBatches' | 'retroAllocations'>> {
  if (engine.kind !== 'wasm') throw new Error('Yerel geriye dönük fark kurtarması yetkili veritabanı servisini kullanmalıdır.');
  const existingBatch = dataset.retroBatches?.find((batch) => batch.id === batchId);
  const original = existingBatch ?? expected?.batch;
  if (original && (original.id !== batchId || (expected && (expected.batch.personnelId !== original.personnelId || expected.batch.revisionId !== original.revisionId || expected.batch.paymentDate !== original.paymentDate)))) throw new Error('Geriye dönük fark kimliği farklı bir kayıtla eşleşiyor.');
  if (!original) throw new Error('Geriye dönük fark kaydı bulunamadı. Verileri yeniden yükleyin.');
  if (existingBatch && (original.status === 'FINALIZED' || ['PAID', 'SETTLED_BY_OFFSET'].includes(original.settlementStatus ?? ''))) {
    throw new Error('Kesinleşmiş, ödenmiş veya mahsupla kapatılmış geriye dönük fark yeniden yazılamaz. Yeni bir düzeltme revizyonu gerekir.');
  }
  const linked = dataset.payrolls.filter((event) => (event.accrualId || event.id) === batchId);
  if (linked.length > 1) throw new Error('Aynı geriye dönük farka birden fazla ödeme bağlı; kayıt kimliklerini inceleyin.');
  const event = linked[0];
  if (event?.status === 'FINALIZED') throw new Error('Bağlı geriye dönük fark ödemesi kesinleşmiş; aynı kayıt yeniden yazılamaz. Yeni bir düzeltme revizyonu gerekir.');
  if (event && (event.personelId !== original.personnelId || event.paymentDate !== original.paymentDate || event.accrualType !== 'RETRO_ADJUSTMENT')) {
    throw new Error('Geriye dönük fark ile ödeme olayı eşleşmiyor. Personel, tarih ve kayıt kimliğini inceleyin.');
  }
  const revision = dataset.compensationRevisions?.find((item) => item.id === original.revisionId);
  if (!revision) throw new Error(`Geriye dönük fark revizyonu bulunamadı: ${original.revisionId}. Özgün revizyonu yedekten kontrol edin.`);
  const replay = await engine.calculateRetroPreview({ batchId, revision,
    overrides: dataset.compensationRevisionOverrides?.filter((item) => item.revisionId === revision.id) ?? [],
    personnelId: original.personnelId, paymentDate: original.paymentDate,
    calculatedAt: original.calculatedAt || original.createdAt || new Date().toISOString(), description: original.description ?? null,
    dataset: { ...dataset, retroBatches: dataset.retroBatches.filter((batch) => batch.id !== batchId),
      retroAllocations: dataset.retroAllocations.filter((allocation) => allocation.batchId !== batchId) },
  });
  if (expected) {
    const finances = (result: RetroCalculationResult) => Object.fromEntries(['totalGrossDelta', 'payableSettlementAmount', 'offsetSettlementAmount', 'recoveredAmount', 'recoverableAmount', 'outstandingReceivable', 'settlementStatus'].map((field) => [field, result.batch[field] ?? (field === 'settlementStatus' ? null : '0')]));
    const allocations = (result: RetroCalculationResult) => [...result.allocations].sort((a, b) => a.id.localeCompare(b.id));
    if (exactKey(finances(replay)) !== exactKey(finances(expected)) || exactKey(allocations(replay)) !== exactKey(allocations(expected))) throw new Error('Geriye dönük fark önizlemesi güncel kaynaklarla eşleşmiyor. Yeniden önizleyin; kayıt yapılmadı.');
  }
  const batch = replay.batch;
  const withBatch: PayrollDatasetSnapshot = { ...dataset,
    retroBatches: [...dataset.retroBatches.filter((item) => item.id !== batchId), batch],
    retroAllocations: [...dataset.retroAllocations.filter((item) => item.batchId !== batchId), ...replay.allocations] };
  let staged = withBatch;
  const applyMutation = async (mutation: PayrollMutation) => {
    const impact = await engine.evaluateMutationPolicy(mutation, staged);
    assertBrowserMutationImpactAllowed(impact, { period: (id) => dataset.periods.find((period) => period.id === id)?.donemAdi });
    staged = { ...staged, payrolls: applyBrowserPayrollImpact(staged.payrolls, impact), retroBatches: applyBrowserRetroBatchImpact(staged.retroBatches, impact) };
  };
  if (existingBatch) {
    const previousImpact = await engine.evaluateMutationPolicy({ kind: 'RETRO_BATCH_SAVE', personnelId: batch.personnelId, batchId, paymentDate: batch.paymentDate }, dataset);
    assertBrowserMutationImpactAllowed(previousImpact, { period: (id) => dataset.periods.find((period) => period.id === id)?.donemAdi });
    staged = { ...staged, payrolls: applyBrowserPayrollImpact(staged.payrolls, previousImpact), retroBatches: applyBrowserRetroBatchImpact(staged.retroBatches, previousImpact) };
  }
  await applyMutation({ kind: 'RETRO_BATCH_SAVE', personnelId: batch.personnelId, batchId, paymentDate: batch.paymentDate });
  const hasPayable = positive(batch.payableSettlementAmount);
  const isZeroDifference = zero(batch.totalGrossDelta) && zero(batch.payableSettlementAmount) && zero(batch.offsetSettlementAmount)
    && batch.settlementStatus === 'UNSETTLED';
  const replayPredecessors = async (target: { id: string; accrualId: string; paymentDate: string; sequence: number }, period: PayrollDatasetSnapshot['periods'][number]) => {
    const attempted = new Set<string>();
    // The staged source-PEK ledger can invalidate an earlier same-day NORMAL.
    // Rebuild it before its dependent retro payment, keeping the entire update in one CAS.
    for (let step = 0; ; step++) {
      const prior = staged.payrolls.filter((record) => record.personelId === batch.personnelId
        && record.paymentDate >= batch.paymentDate && (record.accrualId || record.id) !== batchId && payrollNeedsReplay(record as unknown as BordroKaydi, staged))
        .map((record) => ({ record, period: staged.periods.find((item) => item.id === record.donemId) }))
        .filter((item) => item.period && comparePaymentEvents(item.record, target, item.period, period) < 0)
        .sort((a, b) => comparePaymentEvents(a.record, b.record, a.period!, b.period!))[0];
      if (!prior) break;
      const id = prior.record.accrualId || prior.record.id;
      if (step >= 240 || attempted.has(id)) throw new Error(`Önceki tahakkuk (${id}) güncel duruma geçmedi. Kaynak bağımlılıklarını inceleyin; onarım kaydedilmedi.`);
      attempted.add(id);
      const input = storedAccrualInput(prior.record as unknown as BordroKaydi, prior.period as unknown as BordroDonemi);
      if (prior.record.accrualType === 'RETRO_ADJUSTMENT') {
        const ledger = staged.retroBatches.find((item) => item.id === id);
        if (!ledger || ledger.status !== 'CALCULATED' || !positive(ledger.payableSettlementAmount)) throw new Error(`Önce ${id} geriye dönük farkını kaynaklarıyla yeniden hesaplayın.`);
        input.grossAmount = ledger.payableSettlementAmount;
      }
      const refreshed = await engine.calculatePayroll({ personnelId: batch.personnelId, periodId: prior.period!.id,
        calculatedAt: new Date().toISOString(), manualIncome: storedManualIncome(prior.record as unknown as BordroKaydi), accrual: input, dataset: staged });
      await applyMutation({ kind: 'ACCRUAL_CALCULATION', personnelId: batch.personnelId, periodId: prior.period!.id, accrualId: id });
      staged = { ...staged, payrolls: staged.payrolls.map((item) => (item.accrualId || item.id) === id ? refreshed : item) };
      if (staged.retroBatches.find((item) => item.id === batchId)?.status !== 'CALCULATED') throw new Error(`${prior.period!.id} kaynak döneminin yeniden hesabı ${batchId} geriye dönük farkını yeniden geçersiz kıldı. Kaynak kapsamı ile ödeme sırası arasında bağımlılık döngüsü var; revizyon kapsamını ve ödeme tarihini doğrulanmış dayanakla inceleyin. Onarım kaydedilmedi.`);
    }
  };
  if (hasPayable) {
    const periods = dataset.periods.filter((period) => event ? period.id === event.donemId
      : period.taxYear === Number(batch.paymentDate.slice(0, 4)) && period.taxMonth === Number(batch.paymentDate.slice(5, 7)));
    if (periods.length !== 1) throw new Error(`${batch.paymentDate} için tek bir ödeme dönemi belirlenemedi. Ödeme dönemi kaydını kontrol edin.`);
    const period = periods[0];
    const sequence = event?.sequence ?? nextPaymentSequence(dataset, batch.personnelId, period, batch.paymentDate);
    const target = { id: batchId, accrualId: batchId, paymentDate: batch.paymentDate, sequence };
    await replayPredecessors(target, period);
    const calculated = await engine.calculatePayroll({ personnelId: batch.personnelId, periodId: period.id, calculatedAt: new Date().toISOString(), manualIncome: null,
      accrual: { accrualId: batchId, accrualType: 'RETRO_ADJUSTMENT', paymentDate: batch.paymentDate, sequence, grossAmount: batch.payableSettlementAmount, description: batch.description || 'Geriye dönük hakediş farkı' }, dataset: staged });
    await applyMutation(event ? { kind: 'ACCRUAL_CALCULATION', personnelId: batch.personnelId, periodId: period.id, accrualId: batchId }
      : { kind: 'ACCRUAL_INSERT', personnelId: batch.personnelId, periodId: period.id, accrualId: batchId, paymentDate: batch.paymentDate, sequence });
    staged = { ...staged, payrolls: [...staged.payrolls.filter((item) => (item.accrualId || item.id) !== batchId), calculated] };
  } else {
    if (!isZeroDifference && !['OVERPAYMENT', 'SETTLED_BY_OFFSET'].includes(batch.settlementStatus ?? '')) throw new Error('Ödeme oluşturmayan geriye dönük fark sonucu doğrulanamadı.');
    if (event && event.status !== 'STALE' && !isZeroDifference) throw new Error('Mevcut ödeme henüz güncel durumda. Fazla tahakkuk kaydından önce ödeme olayını ayrıca inceleyin.');
    if (event) {
      await applyMutation({ kind: 'ACCRUAL_DELETE', personnelId: event.personelId, periodId: event.donemId, accrualId: batchId });
      staged = { ...staged, payrolls: staged.payrolls.filter((item) => (item.accrualId || item.id) !== batchId) };
    }
    staged = { ...staged, retroBatches: [...staged.retroBatches.filter((item) => item.id !== batchId), batch] };
    const sameDay = staged.payrolls.filter((item) => item.personelId === batch.personnelId && item.paymentDate === batch.paymentDate);
    const last = [...sameDay].sort((a, b) => b.sequence - a.sequence)[0];
    const period = last && staged.periods.find((item) => item.id === last.donemId);
    if (last && period) await replayPredecessors({ id: `${batchId}_settlement_carry`, accrualId: `${batchId}_settlement_carry`, paymentDate: batch.paymentDate, sequence: last.sequence + 1 }, period);
  }
  return { payrolls: staged.payrolls, retroBatches: [...staged.retroBatches.filter((item) => item.id !== batchId), batch], retroAllocations: staged.retroAllocations };

}
