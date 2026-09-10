import { readFileSync } from 'node:fs';

const [reportPath, baselinePath] = process.argv.slice(2);

if (!reportPath || !baselinePath) {
  console.error('Kullanım: node scripts/check-rust-coverage.mjs <report.json> <baseline.json>');
  process.exit(2);
}

const loadJson = (path) => {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    console.error(`Coverage JSON okunamadı: ${path}`);
    console.error(error instanceof Error ? error.message : error);
    process.exit(2);
  }
};

const report = loadJson(reportPath);
const baseline = loadJson(baselinePath);
const reportFiles = (report.data ?? []).flatMap((data) => data.files ?? []);
const normalize = (path) => path.replaceAll('\\', '/');
const findReportFile = (relativePath) =>
  reportFiles.find((file) => normalize(file.filename ?? '').endsWith(`/${relativePath}`));

const failures = [];
const checked = [];
const metrics = ['lines', 'functions', 'regions'];

for (const [relativePath, baselineFile] of Object.entries(baseline.files ?? {})) {
  const currentFile = findReportFile(relativePath);

  if (!currentFile) {
    failures.push(`${relativePath}: coverage raporunda dosya yok`);
    continue;
  }

  for (const metric of metrics) {
    const expected = baselineFile[metric];
    const actual = currentFile.summary?.[metric];

    if (!expected || !actual || typeof expected.percent !== 'number' || typeof actual.percent !== 'number') {
      failures.push(`${relativePath} ${metric}: metric eksik`);
      continue;
    }

    checked.push({ relativePath, metric, percent: actual.percent });
    if (actual.percent + Number.EPSILON < expected.percent) {
      failures.push(
        `${relativePath} ${metric}: ${actual.percent.toFixed(2)}% < baseline ${expected.percent.toFixed(2)}%`
      );
    }
  }
}

if (failures.length > 0) {
  console.error('Rust coverage ratchet: FAIL');
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`Rust coverage ratchet: PASS — ${checked.length} critical file metrici baseline'ın altında değil.`);
for (const item of checked) {
  console.log(`  ${item.relativePath} ${item.metric}: ${item.percent.toFixed(2)}%`);
}
