import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const root = process.cwd();
const sourceDir = resolve(root, 'src');
const generatedWasmDir = resolve(sourceDir, 'wasm/pkg');
const forbiddenPatterns = [
  ['fetch()', /\bfetch\s*\(/],
  ['globalThis.fetch', /\bglobalThis\.fetch\s*\(/],
  ['window.fetch', /\bwindow\.fetch\s*\(/],
  ['axios', /\baxios\b/],
  ['XMLHttpRequest', /\bXMLHttpRequest\b/],
  ['new WebSocket', /\bnew\s+WebSocket\s*\(/],
  ['WebSocket', /\bWebSocket\b/],
  ['navigator.sendBeacon', /\bnavigator\.sendBeacon\s*\(/],
  ['sendBeacon', /\.sendBeacon\s*\(/],
  ['supabase', /\bsupabase\b/i],
  ['firebase', /\bfirebase\b/i],
  ['/api/', /["'`]\/api(?:\/|["'`])/],
  [
    'dynamic network-client import',
    /\bimport\s*\(\s*["'`][^"'`]*(?:axios|supabase|firebase|socket\.io|graphql-request)[^"'`]*["'`]/i,
  ],
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
  if (path.startsWith(generatedWasmDir)) continue;
  const source = readFileSync(path, 'utf8');
  for (const [label, pattern] of forbiddenPatterns) {
    if (pattern.test(source)) violations.push(`${relative(root, path)} contains ${label}`);
  }
}

const generatedGlue = resolve(generatedWasmDir, 'payroll_wasm.js');
if (!existsSync(generatedGlue) || !/\bfetch\s*\(/.test(readFileSync(generatedGlue, 'utf8'))) {
  violations.push('generated WASM glue is missing its allowlisted same-origin asset fetch');
}

if (violations.length > 0) {
  console.error('verify:privacy: FAIL — browser payroll network boundary violations:');
  for (const violation of violations) console.error(`  - ${violation}`);
  process.exit(1);
}

console.log('verify:privacy: PASS — no browser payroll network/backend path; only generated WASM asset loading is allowlisted.');
