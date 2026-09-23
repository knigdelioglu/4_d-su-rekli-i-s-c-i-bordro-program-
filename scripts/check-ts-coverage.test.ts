import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const checker = fileURLToPath(new URL('./check-ts-coverage.mjs', import.meta.url));

function runChecker(lcov: string, baseline: unknown) {
  const directory = mkdtempSync(join(tmpdir(), 'ts-coverage-check-'));
  const lcovPath = join(directory, 'lcov.info');
  const baselinePath = join(directory, 'baseline.json');
  writeFileSync(lcovPath, lcov);
  writeFileSync(baselinePath, JSON.stringify(baseline));
  try {
    return spawnSync('node', [checker, lcovPath, baselinePath], { encoding: 'utf8' });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

const baseline = {
  files: {
    'src/example.ts': {
      lines: { count: 4, covered: 3, percent: 75 },
      functions: { count: 2, covered: 1, percent: 50 },
    },
  },
};

test('TypeScript coverage ratchet accepts equal or improved exact ratios', () => {
  const result = runChecker(
    'TN:\nSF:src/example.ts\nLF:8\nLH:6\nFNF:4\nFNH:2\nend_of_record\n',
    baseline
  );
  expect(result.status).toBe(0);
});

test('TypeScript coverage ratchet rejects regressions and missing critical files', () => {
  const regressed = runChecker(
    'SF:src/example.ts\nLF:8\nLH:5\nFNF:4\nFNH:2\nend_of_record\n',
    baseline
  );
  expect(regressed.status).toBe(1);
  expect(regressed.stderr.includes('baseline')).toBe(true);

  const missing = runChecker('SF:src/other.ts\nLF:1\nLH:1\nFNF:1\nFNH:1\nend_of_record\n', baseline);
  expect(missing.status).toBe(1);
  expect(missing.stderr.includes('dosya yok')).toBe(true);
});

test('TypeScript coverage ratchet rejects duplicate source records', () => {
  const result = runChecker(
    'SF:src/example.ts\nLF:4\nLH:3\nFNF:2\nFNH:1\nend_of_record\nSF:src/example.ts\nLF:4\nLH:3\nFNF:2\nFNH:1\nend_of_record\n',
    baseline
  );
  expect(result.status).toBe(2);
  expect(result.stderr.includes('duplicate LCOV source')).toBe(true);
});
