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

    if (
      !expected ||
      !actual ||
      typeof expected.percent !== 'number' ||
      !Number.isFinite(expected.percent) ||
      typeof actual.percent !== 'number' ||
      !Number.isFinite(actual.percent)
    ) {
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
