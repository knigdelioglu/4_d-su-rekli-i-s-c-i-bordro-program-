import { existsSync, readdirSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const root = process.cwd();
const packageDir = resolve(root, 'src/wasm/pkg');
const expectedFiles = new Set([
  'package.json',
  'payroll_wasm.js',
  'payroll_wasm_bg.wasm',
  'source-hash.txt',
]);

if (!existsSync(packageDir)) {
  console.error('verify:wasm-package: FAIL — src/wasm/pkg bulunamadı.');
  process.exit(1);
}

const actualEntries = readdirSync(packageDir, { withFileTypes: true });
const actualFiles = new Set(actualEntries.filter((entry) => entry.isFile()).map((entry) => entry.name));
const missing = [...expectedFiles].filter((file) => !actualFiles.has(file));
const unexpected = actualEntries
  .filter((entry) => !expectedFiles.has(entry.name))
  .map((entry) => entry.name);

if (missing.length > 0 || unexpected.length > 0) {
  console.error('verify:wasm-package: FAIL — generated WASM artifact allowlist uyuşmuyor.');
  if (missing.length > 0) console.error(`  missing: ${missing.join(', ')}`);
  if (unexpected.length > 0) {
    console.error(`  unexpected: ${unexpected.map((file) => relative(root, join(packageDir, file))).join(', ')}`);
  }
  process.exit(1);
}

console.log(
  `verify:wasm-package: PASS — ${[...actualFiles].sort().join(', ')} allowlist içinde.`
);
