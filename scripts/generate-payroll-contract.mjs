import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = process.cwd();
const modelsPath = resolve(root, 'crates/payroll-core/src/models.rs');
const generatedPath = resolve(
  root,
  'src/services/payrollEngine/generated/payrollContract.ts'
);

const modelsSource = readFileSync(modelsPath, 'utf8');
const decimalKeys = [...new Set(
  [...modelsSource.matchAll(
    /\bpub\s+([^\s:]+)\s*:\s*(?:Option\s*<\s*)?Decimal\b/gu
  )].map(([, key]) => key)
)].sort((left, right) => left.localeCompare(right));

if (decimalKeys.length === 0) {
  throw new Error('Rust model contract içinde Decimal alanı bulunamadı.');
}

const generated = [
  '/**',
  ' * GENERATED FILE — run `bun scripts/generate-payroll-contract.mjs --write`.',
  ' * Source of truth: crates/payroll-core/src/models.rs',
  ' */',
  'export const RUST_DECIMAL_KEYS = [',
  ...decimalKeys.map((key) => `  ${JSON.stringify(key)},`),
  '] as const;',
  '',
].join('\n');

if (process.argv.includes('--write')) {
  writeFileSync(generatedPath, generated);
  console.log(`payroll-contract: wrote ${decimalKeys.length} Decimal keys.`);
  process.exit(0);
}

const current = readFileSync(generatedPath, 'utf8');
if (current !== generated) {
  console.error(
    'payroll-contract: generated TypeScript contract is stale. Run `bun scripts/generate-payroll-contract.mjs --write`.'
  );
  process.exit(1);
}
console.log(`payroll-contract: PASS — ${decimalKeys.length} Decimal keys are synchronized.`);
