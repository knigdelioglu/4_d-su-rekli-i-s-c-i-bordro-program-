import { spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import { dirname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const cargoHome = resolve(process.env.CARGO_HOME || resolve(homedir(), '.cargo'));
const rustCargoHome = cargoHome.split(sep).join('/');
const existingRustFlags = process.env.RUSTFLAGS?.trim();
const rustFlags = [
  existingRustFlags,
  `--remap-path-prefix=${rustCargoHome}=/cargo-home`,
]
  .filter(Boolean)
  .join(' ');

const environment = {
  ...process.env,
  RUSTFLAGS: rustFlags,
};

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
