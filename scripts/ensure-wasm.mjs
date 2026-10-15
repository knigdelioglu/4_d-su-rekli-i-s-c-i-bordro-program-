import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function checkNodeScript(script) {
  const result = spawnSync(process.execPath, [script], {
    cwd: root,
    encoding: 'utf8',
  });
  return {
    ok: !result.error && result.status === 0,
    error: result.error,
    output: `${result.stdout ?? ''}${result.stderr ?? ''}`,
  };
}

const packageCheck = checkNodeScript('scripts/verify-wasm-package.mjs');
const freshnessCheck = checkNodeScript('scripts/verify-wasm-freshness.mjs');

if (packageCheck.ok && freshnessCheck.ok) {
  process.stdout.write(packageCheck.output);
  process.stdout.write(freshnessCheck.output);
  console.log('WASM package is current; skipping compilation.');
  process.exit(0);
}

if (packageCheck.error || freshnessCheck.error) {
  const error = packageCheck.error ?? freshnessCheck.error;
  console.error(`WASM verification failed to start: ${error.message}`);
  process.exit(1);
}

console.log('WASM package is missing or stale; rebuilding it.');

const build = spawnSync(process.execPath, ['scripts/build-wasm.mjs'], {
  cwd: root,
  stdio: 'inherit',
});

if (build.error) {
  console.error(`WASM build failed to start: ${build.error.message}`);
  process.exit(1);
}
process.exit(build.status ?? 1);
