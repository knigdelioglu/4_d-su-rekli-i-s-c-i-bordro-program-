import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const root = process.cwd();
const sourceDir = resolve(root, 'src');
const legacyFixture = resolve(sourceDir, 'utils/payrollUtils.ts');
const legacyNames = [
  'calculateGelirToplam',
  'calculateKesintiToplam',
  'calculateNetOdeme',
  'calculateZam',
  'autoFillGelirlerFromPuantaj',
  'calculateIsPrimiDetayi',
  'calculateTotalTaxForCumulativeMatrah',
  'calculateGelirVergisi2026',
  'calculateAylikAsgariUcretGvMatrahi',
  'calculateGvMatrah',
  'calculateGvHesapDetayi',
  'calculatePrimeEsasKazanc',
  'calculateStatutoryDeductions',
  'calculateIncomingDevredenPek',
  'calculatePreviousCumulativeGvMatrah',
  'calculatePreviousCumulativeAsgariUcretGvMatrah',
];

function walk(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

const violations = [];
for (const path of walk(sourceDir)) {
  if (!/\.(?:ts|tsx)$/.test(path) || /\.test\.(?:ts|tsx)$/.test(path)) continue;
  if (path === legacyFixture || path.includes('/wasm/pkg/')) continue;

  const source = readFileSync(path, 'utf8');
  if (/(?:from\s+|import\(\s*)["'][^"']*payrollUtils["']/.test(source)) {
    violations.push(`${relative(root, path)} imports test-only payrollUtils`);
  }
  for (const name of legacyNames) {
    if (new RegExp(`\\b${name}\\b`).test(source)) {
      violations.push(`${relative(root, path)} references legacy payroll calculation ${name}`);
    }
  }
}

if (violations.length > 0) {
  console.error('verify:production-graph: FAIL — legacy TypeScript payroll code entered production source:');
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

console.log('verify:production-graph: PASS — production source uses PayrollEngine; legacy TS payroll helpers are test-only.');
