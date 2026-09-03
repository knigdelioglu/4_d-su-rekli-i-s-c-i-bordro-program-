type WasmExports = {
  default: () => Promise<unknown>;
  calculate_payroll_json: (requestJson: string) => string;
  validate_payroll_json: (requestJson: string) => void;
  get_payroll_notices_json: (requestJson: string) => string;
};

let runtimePromise: Promise<WasmExports> | null = null;

async function loadRuntime(): Promise<WasmExports> {
  // The generated package is produced by `bun run wasm:build`. Keeping this
  // import static lets Vite bundle the wasm-bindgen JS glue and its .wasm file.
  const module = (await import('payroll-wasm-generated')) as unknown as WasmExports;
  await module.default();
  return module;
}

export function getWasmRuntime(): Promise<WasmExports> {
  runtimePromise ??= loadRuntime().catch((error) => {
    runtimePromise = null;
    throw new Error(
      `Tarayıcı bordro motoru yüklenemedi. WASM paketini yeniden oluşturun: ${String(error)}`
    );
  });
  return runtimePromise;
}

