import { describeError } from '../../utils/errorMessage';

type WasmExports = {
  default: () => Promise<unknown>;
  calculate_payroll_json: (requestJson: string) => string;
  calculate_legacy_backup_payroll_json: (replayJson: string) => string;
  calculate_compensation_policy_payroll_json: (requestJson: string) => string;
  resolve_compensation_policy_timeline_json: (requestJson: string) => string;
  find_compensation_policy_rounding_issues_json: (requestJson: string) => string;
  calculate_retro_preview_json: (requestJson: string) => string;
  finalize_payroll_json: (requestJson: string) => string;
  finalize_compensation_policy_payroll_json: (requestJson: string) => string;
  validate_payroll_json: (requestJson: string) => void;
  validate_payroll_backup_metadata_json: (payloadJson: string) => void;
  validate_annual_payroll_parameters_json: (parametersJson: string) => void;
  evaluate_mutation_policy_json: (requestJson: string) => string;
  get_payroll_notices_json: (requestJson: string) => string;
};

let runtimePromise: Promise<WasmExports> | null = null;

async function loadRuntime(): Promise<WasmExports> {
  // Keep this as a relative source import so Bun tests and Vite use the actual
  // wasm-bindgen module instead of resolving the ambient type declaration.
  const module = (await import('../../wasm/pkg/payroll_wasm.js')) as unknown as WasmExports;
  await module.default();
  return module;
}

export function getWasmRuntime(): Promise<WasmExports> {
  runtimePromise ??= loadRuntime().catch((error) => {
    runtimePromise = null;
    throw new Error(
      `Tarayıcı bordro motoru yüklenemedi. WASM paketini yeniden oluşturun: ${describeError(error)}`
    );
  });
  return runtimePromise;
}

export async function validatePayrollBackupMetadataWithWasm(payloadJson: string): Promise<void> {
  const runtime = await getWasmRuntime();
  if (typeof runtime.validate_payroll_backup_metadata_json !== 'function') {
    throw new Error(
      'V6 snapshot doğrulayıcısı WASM paketinde bulunamadı; paket consolidation sonrası yeniden üretilmelidir.'
    );
  }
  runtime.validate_payroll_backup_metadata_json(payloadJson);
}
