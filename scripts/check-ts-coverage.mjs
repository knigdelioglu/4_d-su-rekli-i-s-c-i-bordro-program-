import { readFileSync } from 'node:fs';

const [reportPath, baselinePath] = process.argv.slice(2);

function fail(message) {
  console.error(`TypeScript coverage ratchet: ${message}`);
  process.exit(2);
}

if (!reportPath || !baselinePath) {
  fail('Kullanım: node scripts/check-ts-coverage.mjs <lcov.info> <baseline.json>');
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    fail(`${path} okunamadı: ${error instanceof Error ? error.message : error}`);
  }
}

function parseLcov(path) {
  let text;
  try {
    text = readFileSync(path, 'utf8');
  } catch (error) {
    fail(`${path} okunamadı: ${error instanceof Error ? error.message : error}`);
  }

  const records = new Map();
  let current = null;
  const finishRecord = () => {
    if (!current) return;
    if (!current.filename) fail('LCOV SF kaydı eksik');
    if (records.has(current.filename)) fail(`duplicate LCOV source kaydı: ${current.filename}`);
    records.set(current.filename, current.metrics);
    current = null;
  };

  for (const line of text.split(/\r?\n/)) {
    if (line.startsWith('SF:')) {
      finishRecord();
      current = { filename: line.slice(3), metrics: {} };
      continue;
    }
    if (line === 'end_of_record') {
      finishRecord();
      continue;
    }
    if (!current && line.startsWith('TN:')) continue;
    if (!current) {
      if (line.trim()) fail(`SF öncesi beklenmeyen LCOV satırı: ${line}`);
      continue;
    }
    const fields = [
      ['LF:', 'lines', 'count'],
      ['LH:', 'lines', 'covered'],
      ['FNF:', 'functions', 'count'],
      ['FNH:', 'functions', 'covered'],
    ];
    for (const [prefix, metric, field] of fields) {
      if (!line.startsWith(prefix)) continue;
      const raw = line.slice(prefix.length);
      if (!/^\d+$/.test(raw)) fail(`${current.filename}: ${prefix.slice(0, -1)} geçersiz`);
      current.metrics[metric] ??= {};
      if (current.metrics[metric][field] !== undefined) {
        fail(`${current.filename}: duplicate ${prefix.slice(0, -1)} değeri`);
      }
      current.metrics[metric][field] = Number(raw);
      break;
    }
  }
  finishRecord();
  if (records.size === 0) fail('LCOV raporunda kaynak dosya kaydı yok');
  return records;
}

function normalize(path) {
  return path.replaceAll('\\', '/').replace(/^\.\//, '');
}

function validateMetric(metric, value, label) {
  if (!value || !Number.isSafeInteger(value.count) || value.count <= 0) {
    fail(`${label}: count geçersiz`);
  }
  if (
    !Number.isSafeInteger(value.covered) ||
    value.covered < 0 ||
    value.covered > value.count
  ) {
    fail(`${label}: covered geçersiz`);
  }
  if (value.percent !== undefined &&
      (typeof value.percent !== 'number' || !Number.isFinite(value.percent) || value.percent < 0 || value.percent > 100)) {
    fail(`${label}: percent geçersiz`);
  }
  return value;
}

const baseline = readJson(baselinePath);
if (!baseline.files || typeof baseline.files !== 'object' || Array.isArray(baseline.files)) {
  fail('baseline.files object eksik');
}
const report = parseLcov(reportPath);
const normalizedEntries = [...report.entries()].map(([path, metrics]) => [normalize(path), metrics]);
const failures = [];
const checked = [];

for (const [path, baselineFile] of Object.entries(baseline.files)) {
  const normalizedPath = normalize(path);
  const matches = normalizedEntries.filter(([name]) =>
    name === normalizedPath || name.endsWith(`/${normalizedPath}`)
  );
  if (matches.length !== 1) {
    failures.push(`${path}: LCOV raporunda ${matches.length === 0 ? 'dosya yok' : 'birden fazla eşleşme var'}`);
    continue;
  }
  const [sourcePath, currentFile] = matches[0];
  for (const metric of ['lines', 'functions']) {
    const expected = validateMetric(metric, baselineFile?.[metric], `${path} ${metric} baseline`);
    const actual = validateMetric(metric, currentFile[metric], `${sourcePath} ${metric} current`);
    checked.push({ path, metric, percent: (actual.covered / actual.count) * 100 });
    if (BigInt(actual.covered) * BigInt(expected.count) < BigInt(expected.covered) * BigInt(actual.count)) {
      failures.push(
        `${path} ${metric}: ${(actual.covered / actual.count * 100).toFixed(2)}% < baseline ${(expected.covered / expected.count * 100).toFixed(2)}%`
      );
    }
  }
}

if (failures.length > 0) {
  console.error('TypeScript coverage ratchet: FAIL');
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`TypeScript coverage ratchet: PASS — ${checked.length} kritik dosya metriği baseline'ın altında değil.`);
for (const item of checked) console.log(`  ${item.path} ${item.metric}: ${item.percent.toFixed(2)}%`);
