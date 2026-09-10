import { readFileSync } from 'node:fs';

const args = process.argv.slice(2);
const enforce = args.includes('--enforce');
const positional = args.filter((arg) => arg !== '--enforce');
const [summaryPath, baselinePath] = positional;

function fail(message) {
  console.error(`cargo-mutants baseline: ${message}`);
  process.exit(2);
}

if (!summaryPath || !baselinePath) {
  fail('Kullanım: node scripts/check-cargo-mutants-baseline.mjs [--enforce] <summary.json> <baseline.json>');
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    fail(`JSON okunamadı: ${path}\n${error instanceof Error ? error.message : error}`);
  }
}

const summary = readJson(summaryPath);
const baseline = readJson(baselinePath);
if (summary.status !== 'complete') fail('summary status complete değil');

const baselineComplete = baseline.status === 'complete' || baseline.fullRun?.status === 'complete';
if (!baselineComplete) {
  console.log(
    JSON.stringify(
      {
        status: 'pending_baseline',
        qualityRegression: null,
        blocking: false,
        mode: enforce ? 'enforced-but-baseline-pending' : 'advisory',
        message: 'Full mutation baseline yok; current summary yalnız advisory evidence olarak raporlandı.',
      },
      null,
      2
    )
  );
  process.exit(0);
}
// The lifecycle keeps the machine-readable counters at the document root;
// accepting counters in fullRun as well makes a future hand-updated artifact
// compatible without changing the pending sentinel contract.
const baselineRun = { ...(baseline.fullRun ?? {}), ...baseline };

const integerFields = ['total', 'caught', 'missed', 'timeout', 'unviable', 'meaningful'];
for (const field of integerFields) {
  if (!Number.isInteger(summary[field]) || summary[field] < 0) fail(`summary.${field} geçersiz`);
  if (!Number.isInteger(baselineRun[field]) || baselineRun[field] < 0) fail(`baseline.${field} geçersiz`);
}
if (!Number.isFinite(summary.scorePercent) || !Number.isFinite(baselineRun.scorePercent)) {
  fail('complete baseline/summary scorePercent finite olmalı');
}

const regressions = [];
if (summary.scorePercent < baselineRun.scorePercent) {
  regressions.push(`scorePercent ${summary.scorePercent} < baseline ${baselineRun.scorePercent}`);
}
if (summary.missed > baselineRun.missed) {
  regressions.push(`missed ${summary.missed} > baseline ${baselineRun.missed}`);
}
if (summary.timeout > baselineRun.timeout) {
  regressions.push(`timeout ${summary.timeout} > baseline ${baselineRun.timeout}`);
}

const baselineMissed = new Set(baselineRun.missedMutants ?? []);
const newMissed = (summary.missedMutants ?? []).filter((id) => !baselineMissed.has(id));
if (newMissed.length > 0) regressions.push(`new missed mutants: ${newMissed.join(', ')}`);

const result = {
  status: regressions.length === 0 ? 'pass' : 'quality_regression',
  qualityRegression: regressions.length > 0,
  blocking: enforce && regressions.length > 0,
  mode: enforce ? 'enforced' : 'advisory',
  regressions,
};
console.log(JSON.stringify(result, null, 2));
if (enforce && regressions.length > 0) process.exit(1);
