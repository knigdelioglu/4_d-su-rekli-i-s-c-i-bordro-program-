import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { dirname, resolve, sep } from 'node:path';
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
  CARGO_ENCODED_RUSTFLAGS: rustFlags.join(encodedSeparator),
};
delete environment.RUSTFLAGS;

const build = spawnSync(
  'wasm-pack',
  [
    'build',
    'crates/payroll-wasm',
    '--target',
    'web',
    '--out-dir',
    '../../src/wasm/pkg',
    '--release',
    '--no-opt',
    '--no-typescript',
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
