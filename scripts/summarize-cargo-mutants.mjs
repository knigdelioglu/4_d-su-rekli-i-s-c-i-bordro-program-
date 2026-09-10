import { readFileSync } from 'node:fs';

const outcomesPaths = process.argv.slice(2);

if (outcomesPaths.length === 0) {
  console.error('Kullanım: node scripts/summarize-cargo-mutants.mjs <outcomes.json> [<outcomes.json> ...]');
  process.exit(2);
}

const reports = outcomesPaths.map((outcomesPath) => {
  try {
    return JSON.parse(readFileSync(outcomesPath, 'utf8'));
  } catch (error) {
    console.error(`cargo-mutants outcomes okunamadı: ${outcomesPath}`);
    console.error(error instanceof Error ? error.message : error);
    process.exit(2);
  }
});

const total = reports.reduce((sum, report) => sum + Number(report.total_mutants ?? 0), 0);
const caught = reports.reduce((sum, report) => sum + Number(report.caught ?? 0), 0);
const missed = reports.reduce((sum, report) => sum + Number(report.missed ?? 0), 0);
const timeout = reports.reduce((sum, report) => sum + Number(report.timeout ?? 0), 0);
const unviable = reports.reduce((sum, report) => sum + Number(report.unviable ?? 0), 0);
const meaningful = total - unviable;
const scorePercent = meaningful > 0 ? (caught / meaningful) * 100 : null;

const summary = {
  tool: 'cargo-mutants',
  version: reports[0]?.cargo_mutants_version ?? null,
  reports: reports.length,
  total,
  caught,
  missed,
  timeout,
  unviable,
  meaningful,
  scorePercent,
};

console.log(JSON.stringify(summary, null, 2));
if (missed > 0 || timeout > 0) {
  console.error('cargo-mutants: surviving veya timeout mutant var; rapor blocking gate için hazır değil.');
}
