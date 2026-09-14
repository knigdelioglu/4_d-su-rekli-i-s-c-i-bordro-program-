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
if (!Array.isArray(report.data)) {
  console.error('Coverage JSON data dizisi eksik');
  process.exit(2);
}
if (!baseline.files || typeof baseline.files !== 'object' || Array.isArray(baseline.files)) {
  console.error('Coverage baseline files object eksik');
  process.exit(2);
}

const reportFiles = report.data.flatMap((data) => (Array.isArray(data.files) ? data.files : []));
const normalize = (path) => path.replaceAll('\\', '/').replace(/^\.\//, '');
const reportFileNames = reportFiles.map((file) => normalize(file.filename ?? ''));
const duplicateReportFiles = [...new Set(reportFileNames.filter((name, index) => reportFileNames.indexOf(name) !== index))];
if (duplicateReportFiles.length > 0) {
  console.error('Coverage raporunda duplicate dosya yolları var:');
  for (const path of duplicateReportFiles) console.error(`  - ${path}`);
  process.exit(1);
}

const findReportFiles = (relativePath) => {
  const normalizedRelativePath = normalize(relativePath);
  return reportFiles.filter((file) => {
    const filename = normalize(file.filename ?? '');
    return filename === normalizedRelativePath || filename.endsWith(`/${normalizedRelativePath}`);
  });
};

const failures = [];
const checked = [];
const metrics = ['lines', 'functions', 'regions'];

function readMetric(metricValue, label) {
  if (!metricValue || typeof metricValue !== 'object' || Array.isArray(metricValue)) {
    failures.push(`${label}: metric object eksik`);
    return null;
  }

  const { count, covered, percent } = metricValue;
  if (
    !Number.isSafeInteger(count) ||
    count <= 0 ||
    !Number.isSafeInteger(covered) ||
    covered < 0 ||
    covered > count
  ) {
    failures.push(`${label}: count/covered geçersiz`);
    return null;
  }
  if (typeof percent !== 'number' || !Number.isFinite(percent) || percent < 0 || percent > 100) {
    failures.push(`${label}: percent geçersiz`);
    return null;
  }
  return { count, covered, percent };
}

for (const [relativePath, baselineFile] of Object.entries(baseline.files ?? {})) {
  const matches = findReportFiles(relativePath);

  if (matches.length === 0) {
    failures.push(`${relativePath}: coverage raporunda dosya yok`);
    continue;
  }
  if (matches.length > 1) {
    failures.push(`${relativePath}: coverage raporunda birden fazla eşleşme var`);
    continue;
  }
  const currentFile = matches[0];

  for (const metric of metrics) {
    const expected = baselineFile[metric];
    const actual = currentFile.summary?.[metric];

    const expectedMetric = readMetric(expected, `${relativePath} ${metric} baseline`);
    const actualMetric = readMetric(actual, `${relativePath} ${metric} current`);
    if (!expectedMetric || !actualMetric) {
      continue;
    }

    checked.push({
      relativePath,
      metric,
      percent: (actualMetric.covered / actualMetric.count) * 100,
    });
    // Compare the exact integer ratios instead of rounded/floating percentages.
    if (
      BigInt(actualMetric.covered) * BigInt(expectedMetric.count) <
      BigInt(expectedMetric.covered) * BigInt(actualMetric.count)
    ) {
      failures.push(
        `${relativePath} ${metric}: ${(actualMetric.covered / actualMetric.count * 100).toFixed(2)}% < baseline ${(expectedMetric.covered / expectedMetric.count * 100).toFixed(2)}%`
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
