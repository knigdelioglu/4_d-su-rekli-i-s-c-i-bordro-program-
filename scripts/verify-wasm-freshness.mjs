import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const root = process.cwd();
const manifestPath = resolve(root, 'src/wasm/pkg/source-hash.txt');
const packageManifestPath = resolve(root, 'src/wasm/pkg/package.json');
const sourceDirectories = [
  'crates/payroll-core/src',
  'crates/payroll-wasm/src',
  // payroll-core imports these shared domain files by path for native/WASM parity.
  'src-tauri/src/domain',
];
const manifestSources = [
  'Cargo.toml',
  'Cargo.lock',
  'crates/payroll-core/Cargo.toml',
  'crates/payroll-wasm/Cargo.toml',
  'src-tauri/Cargo.toml',
];

function walk(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

const files = [
  ...sourceDirectories.flatMap((directory) => walk(resolve(root, directory))),
  ...manifestSources.map((path) => resolve(root, path)).filter(existsSync),
].sort();

const hash = createHash('sha256');
for (const path of files) {
  hash.update(relative(root, path));
  hash.update('\0');
  hash.update(readFileSync(path));
  hash.update('\0');
}
const currentHash = hash.digest('hex');
const shouldWrite = process.argv.includes('--write');

if (shouldWrite) {
  writeFileSync(manifestPath, `${currentHash}\n`);
  if (existsSync(packageManifestPath)) {
    const packageManifest = JSON.parse(readFileSync(packageManifestPath, 'utf8'));
    if (Array.isArray(packageManifest.files) && !packageManifest.files.includes('source-hash.txt')) {
      packageManifest.files.push('source-hash.txt');
      writeFileSync(packageManifestPath, `${JSON.stringify(packageManifest, null, 2)}\n`);
    }
  }
  console.log(`verify:wasm-freshness: UPDATED — ${currentHash}`);
  process.exit(0);
}

if (!existsSync(manifestPath)) {
  console.error('verify:wasm-freshness: FAIL — src/wasm/pkg/source-hash.txt bulunamadı; bun run wasm:build çalıştırın.');
  process.exit(1);
}

const committedHash = readFileSync(manifestPath, 'utf8').trim();
if (committedHash !== currentHash) {
  console.error('verify:wasm-freshness: FAIL — Rust/WASM source ile committed generated WASM provenance eşleşmiyor.');
  console.error(`  committed: ${committedHash || '<empty>'}`);
  console.error(`  current:   ${currentHash}`);
  console.error('  Çözüm: bun run wasm:build');
  process.exit(1);
}

console.log(`verify:wasm-freshness: PASS — ${currentHash}`);
