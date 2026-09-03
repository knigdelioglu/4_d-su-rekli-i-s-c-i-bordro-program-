import { tauriBridge } from '../tauriBridge';
import {
  assertExactDecimalDto,
  parseWasmPayrollBoundaryResult,
  serializePayrollRequestForWasm,
  toPayrollBoundaryDto,
} from './decimalBoundary';
import { getWasmRuntime } from './wasmRuntime';
import {
  MutationImpact,
  PayrollBoundaryPayroll,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
  PayrollEngine,
  PayrollMutation,
} from './types';

const tauriEngine: PayrollEngine = {
  kind: 'tauri',
  calculatePayroll: async (request) =>
    toPayrollBoundaryDto(
      await tauriBridge.calculatePayroll(
        request.personnelId,
        request.periodId,
        toPayrollBoundaryDto(request.manualIncome ?? null) as unknown as Parameters<
          typeof tauriBridge.calculatePayroll
        >[2]
      )
    ) as PayrollBoundaryPayroll,
  // Tauri's command performs the authoritative database-backed preflight.
  validatePayroll: async () => undefined,
  getPayrollNotices: (periodId) => tauriBridge.getPayrollNotices(periodId),
  getPayrolls: async () =>
    (await tauriBridge.getPayrollList()).map((payroll) =>
      toPayrollBoundaryDto(payroll)
    ) as PayrollBoundaryPayroll[],
  finalizePayroll: async (personnelId, periodId) =>
    toPayrollBoundaryDto(await tauriBridge.finalizePayroll(personnelId, periodId)) as PayrollBoundaryPayroll,
  evaluateMutationPolicy: (mutation, dataset) =>
    tauriBridge.evaluateMutationPolicy(mutation, dataset),
};

function mutationPolicyRequest(
  mutation: PayrollMutation,
  dataset: PayrollDatasetSnapshot
): string {
  const value = { dataset, mutation };
  assertExactDecimalDto(value);
  return JSON.stringify(value);
}

function parseMutationImpact(json: string): MutationImpact {
  const value: unknown = JSON.parse(json);
  if (!value || typeof value !== 'object') {
    throw new Error('WASM mutation policy sonucu geçersiz.');
  }
  return value as MutationImpact;
}

const wasmEngine: PayrollEngine = {
  kind: 'wasm',
  calculatePayroll: async (request: PayrollCalculationRequest) => {
    const runtime = await getWasmRuntime();
    const json = runtime.calculate_payroll_json(serializePayrollRequestForWasm(request));
    return parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(json);
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
        calculatedAt: '1970-01-01T00:00:00.000Z',
        manualIncome: null,
        dataset,
      })
    );
    return JSON.parse(json) as Awaited<ReturnType<PayrollEngine['getPayrollNotices']>>;
  },
  getPayrolls: async (dataset) => dataset.payrolls,
  finalizePayroll: async (personnelId, periodId, dataset) => {
    const current = dataset.payrolls.find(
      (item) => item.personelId === personnelId && item.donemId === periodId
    );
    if (!current) throw new Error('Bordro kaydı bulunamadı.');
    const runtime = await getWasmRuntime();
    const json = runtime.finalize_payroll_json(
      serializePayrollRequestForWasm({
        personnelId,
        periodId,
        calculatedAt: new Date().toISOString(),
        manualIncome: {
          tediye: current.gelirler.tediye,
          tisIkramiyesi: current.gelirler.tisIkramiyesi,
        },
        dataset,
      })
    );
    return parseWasmPayrollBoundaryResult<PayrollBoundaryPayroll>(json);
  },
  evaluateMutationPolicy: async (mutation, dataset) => {
    const runtime = await getWasmRuntime();
    return parseMutationImpact(
      runtime.evaluate_mutation_policy_json(mutationPolicyRequest(mutation, dataset))
    );
  },
};

/** The only platform selector used by payroll UI/runtime code. */
export function getPayrollEngine(): PayrollEngine {
  return tauriBridge.isTauriAvailable() ? tauriEngine : wasmEngine;
}

export type {
  MutationImpact,
  PayrollBoundaryManualIncomeInput,
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollCalculationRequest,
  PayrollDatasetSnapshot,
  PayrollDatasetSnapshotModel,
  PayrollEngine,
  PayrollKey,
  PayrollMutation,
  PayrollUiModel,
} from './types';
