import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, dirname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const cargoHome = resolve(process.env.CARGO_HOME || resolve(homedir(), '.cargo'));
const remappedRoot = root.split(sep).join('/');
const rustCargoHome = cargoHome.split(sep).join('/');

function splitRustFlags(value) {
  const flags = [];
  let current = '';
  let quote = null;
  let escaped = false;

  for (const character of value) {
    if (escaped) {
      current += character;
      escaped = false;
      continue;
    }
    if (character === '\\' && quote !== "'") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) quote = null;
      else current += character;
      continue;
    }
    if (character === "'" || character === '"') {
      quote = character;
    } else if (/\s/.test(character)) {
      if (current) {
        flags.push(current);
        current = '';
      }
    } else {
      current += character;
    }
  }

  if (escaped) current += '\\';
  if (quote) throw new Error('RUSTFLAGS içinde kapanmamış quote var.');
  if (current) flags.push(current);
  return flags;
}

const encodedSeparator = '\x1f';
// Cargo's encoded form keeps each remap flag intact even when the workspace
// path contains spaces (as it does on the local development volume).
const inheritedRustFlags = process.env.CARGO_ENCODED_RUSTFLAGS
  ? process.env.CARGO_ENCODED_RUSTFLAGS.split(encodedSeparator).filter(Boolean)
  : splitRustFlags(process.env.RUSTFLAGS ?? '');
const remapPath = (from, to) => `--remap-path-prefix=${from}=${to}`;
const rustFlags = [
  ...inheritedRustFlags,
  remapPath(remappedRoot, '/workspace'),
  remapPath(rustCargoHome, '/cargo-home'),
];

const environment = {
  ...process.env,
  CARGO_INCREMENTAL: '0',
  // A single release codegen unit and one Cargo worker keep wasm-ld's object
  // input order stable across host runners.
  CARGO_BUILD_JOBS: '1',
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS: '1',
  CARGO_ENCODED_RUSTFLAGS: rustFlags.join(encodedSeparator),
};
delete environment.RUSTFLAGS;

const wasmBindgenVersion = '0.2.127';
const wasmPackCache = resolve(
  homedir(),
  process.platform === 'darwin' ? 'Library/Caches/.wasm-pack' : '.cache/.wasm-pack',
);
const wasmBindgenBinDirs = [
  resolve(cargoHome, 'bin'),
  resolve(wasmPackCache, `wasm-bindgen-cargo-install-${wasmBindgenVersion}`),
].filter((directory) => existsSync(resolve(directory, 'wasm-bindgen')));
environment.PATH = [...wasmBindgenBinDirs, process.env.PATH ?? '']
  .filter(Boolean)
  .join(delimiter);

const wasmBindgen = spawnSync('wasm-bindgen', ['--version'], {
  cwd: root,
  env: environment,
  encoding: 'utf8',
});
if (
  wasmBindgen.error ||
  wasmBindgen.status !== 0 ||
  !wasmBindgen.stdout.trim().startsWith(`wasm-bindgen ${wasmBindgenVersion}`)
) {
  const detail = wasmBindgen.error?.message ?? wasmBindgen.stderr?.trim() ?? 'sürüm doğrulanamadı';
  console.error(`WASM build requires wasm-bindgen ${wasmBindgenVersion} on PATH: ${detail}`);
  process.exit(1);
}

const build = spawnSync(
  'wasm-pack',
  [
    'build',
    'crates/payroll-wasm',
    '--mode',
    'no-install',
    '--target',
    'web',
    '--out-dir',
    '../../src/wasm/pkg',
    '--release',
    '--no-opt',
    '--no-typescript',
    '--locked',
  ],
  { cwd: root, env: environment, stdio: 'inherit' },
);

if (build.error) {
  console.error(`WASM build failed to start: ${build.error.message}`);
  process.exit(1);
}
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

const freshness = spawnSync(
  process.execPath,
  ['scripts/verify-wasm-freshness.mjs', '--write'],
  { cwd: root, env: process.env, stdio: 'inherit' },
);

if (freshness.error) {
  console.error(`WASM freshness manifest update failed to start: ${freshness.error.message}`);
  process.exit(1);
}
process.exit(freshness.status ?? 1);
