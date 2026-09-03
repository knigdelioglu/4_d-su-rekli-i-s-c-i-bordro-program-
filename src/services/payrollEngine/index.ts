import { tauriBridge } from '../tauriBridge';
import {
  parseWasmPayrollResult,
  serializePayrollRequestForWasm,
} from './decimalBoundary';
import { getWasmRuntime } from './wasmRuntime';
import { PayrollCalculationRequest, PayrollDatasetSnapshot, PayrollEngine } from './types';

const tauriEngine: PayrollEngine = {
  kind: 'tauri',
  calculatePayroll: (request) =>
    tauriBridge.calculatePayroll(
      request.personnelId,
      request.periodId,
      request.manualIncome ?? null
    ),
  // Tauri's command performs the authoritative database-backed preflight.
  validatePayroll: async () => undefined,
  getPayrollNotices: (periodId) => tauriBridge.getPayrollNotices(periodId),
  getPayrolls: () => tauriBridge.getPayrollList(),
  setPayrollStatus: async (personnelId, periodId, status) => {
    await tauriBridge.setPayrollStatus(personnelId, periodId, status);
    const payroll = (await tauriBridge.getPayrollList()).find(
      (item) => item.personelId === personnelId && item.donemId === periodId
    );
    if (!payroll) throw new Error('Güncellenen bordro kaydı bulunamadı.');
    return payroll;
  },
};

const wasmEngine: PayrollEngine = {
  kind: 'wasm',
  calculatePayroll: async (request: PayrollCalculationRequest) => {
    const runtime = await getWasmRuntime();
    const json = runtime.calculate_payroll_json(serializePayrollRequestForWasm(request));
    return parseWasmPayrollResult(json);
  },
  validatePayroll: async (request: PayrollCalculationRequest) => {
    const runtime = await getWasmRuntime();
    runtime.validate_payroll_json(serializePayrollRequestForWasm(request));
  },
  getPayrollNotices: async (periodId: string, dataset: PayrollDatasetSnapshot) => {
    const runtime = await getWasmRuntime();
    const json = runtime.get_payroll_notices_json(
      serializePayrollRequestForWasm({
        personnelId: '',
        periodId,
        manualIncome: null,
        dataset,
      })
    );
    return JSON.parse(json) as Awaited<ReturnType<PayrollEngine['getPayrollNotices']>>;
  },
  getPayrolls: async (dataset) => dataset.payrolls,
  setPayrollStatus: async (personnelId, periodId, status, dataset) => {
    const current = dataset.payrolls.find(
      (item) => item.personelId === personnelId && item.donemId === periodId
    );
    if (!current) throw new Error('Bordro kaydı bulunamadı.');
    if (current.status === 'FINALIZED' && status !== 'FINALIZED') {
      throw new Error('Kesinleştirilmiş bordronun durumu değiştirilemez.');
    }
    if (status === 'FINALIZED' && current.status !== 'CALCULATED') {
      throw new Error(`Bordro CALCULATED durumda değil: ${current.status}.`);
    }
    if (status === 'FINALIZED') {
      const period = dataset.periods.find((item) => item.id === periodId);
      if (!period) throw new Error(`Bordro dönemi bulunamadı: ${periodId}.`);
      const hasNonFinalizedPrior = dataset.payrolls.some((item) => {
        if (item.personelId !== personnelId || item.status === 'FINALIZED') return false;
        const candidate = dataset.periods.find((candidatePeriod) => candidatePeriod.id === item.donemId);
        return (
          candidate?.taxYear === period.taxYear && candidate.taxMonth < period.taxMonth
        );
      });
      if (hasNonFinalizedPrior) {
        throw new Error(
          'Bu bordro kesinleştirilemez: aynı vergi yılındaki önceki mevcut bordrolar önce FINALIZED olmalıdır.'
        );
      }
      await wasmEngine.validatePayroll({
        personnelId,
        periodId,
        manualIncome: {
          tediye: current.gelirler.tediye,
          tisIkramiyesi: current.gelirler.tisIkramiyesi,
        },
        dataset,
      });
    }
    return { ...current, status };
  },
};

/** The only platform selector used by payroll UI/runtime code. */
export function getPayrollEngine(): PayrollEngine {
  return tauriBridge.isTauriAvailable() ? tauriEngine : wasmEngine;
}

export type { PayrollCalculationRequest, PayrollDatasetSnapshot, PayrollEngine } from './types';
