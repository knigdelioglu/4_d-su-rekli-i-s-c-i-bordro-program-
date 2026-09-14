import { readFileSync } from 'node:fs';

const USAGE =
  'Kullanım: node scripts/summarize-cargo-mutants.mjs [--expected-shards N] [--baseline path] [--shard ID=outcomes.json ...]';

function fail(message) {
  console.error(`cargo-mutants evidence: ${message}`);
  process.exit(2);
}

function parseInteger(value, name) {
  if (!/^\d+$/.test(value)) fail(`${name} sıfır veya pozitif bir tam sayı olmalı: ${value}`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) fail(`${name} güvenli bir tam sayı olmalı: ${value}`);
  return parsed;
}

function parseShard(value) {
  const separator = value.indexOf('=');
  if (separator <= 0 || separator === value.length - 1) {
    fail(`--shard ID=outcomes.json biçiminde olmalı: ${value}`);
  }
  return {
    id: parseInteger(value.slice(0, separator), '--shard id'),
    path: value.slice(separator + 1),
  };
}

function parseArguments() {
  const args = process.argv.slice(2);
  const outcomesPaths = [];
  const shardInputs = [];
  let expectedShards = null;
  let baselinePath = null;

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--expected-shards') {
      const value = args[++index];
      if (value === undefined) fail(USAGE);
      expectedShards = parseInteger(value, '--expected-shards');
    } else if (arg.startsWith('--expected-shards=')) {
      expectedShards = parseInteger(arg.slice('--expected-shards='.length), '--expected-shards');
    } else if (arg === '--shard') {
      const value = args[++index];
      if (value === undefined) fail(USAGE);
      shardInputs.push(parseShard(value));
    } else if (arg.startsWith('--shard=')) {
      shardInputs.push(parseShard(arg.slice('--shard='.length)));
    } else if (arg === '--baseline') {
      baselinePath = args[++index];
      if (baselinePath === undefined) fail(USAGE);
    } else if (arg.startsWith('-')) {
      fail(`bilinmeyen seçenek: ${arg}\n${USAGE}`);
    } else {
      outcomesPaths.push(arg);
    }
  }

  if (outcomesPaths.length > 0 && shardInputs.length > 0) {
    fail('outcomes yolları ile --shard girdileri birlikte kullanılamaz');
  }
  if (outcomesPaths.length === 0 && shardInputs.length === 0) fail(USAGE);

  const inputs =
    shardInputs.length > 0
      ? shardInputs
      : outcomesPaths.map((path, index) => ({ id: index, path }));

  if (expectedShards !== null && inputs.length !== expectedShards) {
    fail(`beklenen shard sayısı ${expectedShards}, bulunan outcomes sayısı ${inputs.length}`);
  }
  if (expectedShards !== null && expectedShards > 1 && shardInputs.length === 0) {
    fail('çoklu shard aggregate için her giriş --shard ID=outcomes.json ile verilmelidir');
  }

  const shardIds = inputs.map(({ id }) => id);
  if (new Set(shardIds).size !== shardIds.length) {
    fail('aynı shard ID birden fazla kez verildi');
  }
  if (expectedShards !== null) {
    const expectedIds = Array.from({ length: expectedShards }, (_, index) => index);
    const actualIds = [...shardIds].sort((left, right) => left - right);
    if (JSON.stringify(actualIds) !== JSON.stringify(expectedIds)) {
      fail(`shard ID kümesi 0..${expectedShards - 1} olmalı; bulunan ${actualIds.join(', ')}`);
    }
  }

  const inputPaths = inputs.map(({ path }) => path);
  if (new Set(inputPaths).size !== inputPaths.length) {
    fail('aynı outcomes.json yolu birden fazla kez verildi');
  }

  return { inputs, expectedShards, baselinePath };
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    fail(`${label} okunamadı veya JSON parse edilemedi: ${path}\n${error instanceof Error ? error.message : error}`);
  }
}

function integerField(report, field, path) {
  const value = report[field];
  if (!Number.isInteger(value) || value < 0) {
    fail(`${path}: ${field} non-negative integer olmalı`);
  }
  return value;
}

function isMutantScenario(scenario) {
  return Boolean(
    scenario &&
      typeof scenario === 'object' &&
      scenario.Mutant &&
      typeof scenario.Mutant === 'object' &&
      typeof scenario.Mutant.name === 'string' &&
      scenario.Mutant.name.length > 0
  );
}

function validateReport(report, path, shardId) {
  if (!report || typeof report !== 'object' || Array.isArray(report)) {
    fail(`${path}: outcomes kökü object olmalı`);
  }
  if (typeof report.cargo_mutants_version !== 'string' || report.cargo_mutants_version.length === 0) {
    fail(`${path}: cargo_mutants_version eksik`);
  }
  if (!Array.isArray(report.outcomes)) fail(`${path}: outcomes dizisi eksik`);
  if (typeof report.end_time !== 'string' || report.end_time.length === 0) {
    fail(`${path}: tamamlanmış end_time eksik; shard tamamlanmamış olabilir`);
  }

  const baselines = report.outcomes.filter((outcome) => outcome?.scenario === 'Baseline');
  if (baselines.length !== 1) {
    fail(`${path}: tam olarak bir Baseline outcome bekleniyor; bulunan ${baselines.length}`);
  }
  const baseline = baselines[0];
  if (baseline.summary !== 'Success') {
    fail(`${path}: baseline testleri başarılı değil (${baseline.summary ?? 'summary yok'})`);
  }
  if (!Array.isArray(baseline.phase_results) || baseline.phase_results.length === 0) {
    fail(`${path}: baseline phase_results eksik`);
  }
  const baselinePhases = new Set(baseline.phase_results.map((phase) => phase?.phase));
  if (!baselinePhases.has('Build') || !baselinePhases.has('Test')) {
    fail(`${path}: baseline Build ve Test fazlarını içermeli`);
  }
  if (baseline.phase_results.some((phase) => phase?.process_status !== 'Success')) {
    fail(`${path}: baseline Build/Test fazlarından biri başarılı değil`);
  }

  for (const field of ['shard', 'shard_id', 'shardId']) {
    if (report[field] !== undefined) {
      const reportedShard = Number(report[field]);
      if (!Number.isSafeInteger(reportedShard) || reportedShard !== shardId) {
        fail(`${path}: report shard ID ${shardId} ile uyuşmuyor`);
      }
    }
  }

  const counts = {
    total: integerField(report, 'total_mutants', path),
    caught: integerField(report, 'caught', path),
    missed: integerField(report, 'missed', path),
    timeout: integerField(report, 'timeout', path),
    unviable: integerField(report, 'unviable', path),
    success: integerField(report, 'success', path),
  };
  const mutantOutcomes = report.outcomes.filter((outcome) => isMutantScenario(outcome?.scenario));
  if (mutantOutcomes.length !== counts.total) {
    fail(`${path}: total_mutants=${counts.total}, mutant outcome sayısı=${mutantOutcomes.length}`);
  }

  const observed = {
    CaughtMutant: 0,
    MissedMutant: 0,
    Timeout: 0,
    Unviable: 0,
  };
  const mutants = [];
  for (const outcome of mutantOutcomes) {
    if (!Object.hasOwn(observed, outcome.summary)) {
      fail(`${path}: sınıflandırılamayan mutant sonucu: ${outcome.summary ?? 'summary yok'}`);
    }
    const mutant = outcome.scenario.Mutant;
    if (typeof mutant.file !== 'string' || mutant.file.length === 0) {
      fail(`${path}: mutant file eksik: ${mutant.name}`);
    }
    observed[outcome.summary] += 1;
    mutants.push({ id: mutant.name, file: mutant.file, summary: outcome.summary });
  }
  const unexpectedOutcomes = report.outcomes.filter(
    (outcome) => outcome?.scenario !== 'Baseline' && !isMutantScenario(outcome?.scenario)
  );
  if (unexpectedOutcomes.length > 0) {
    fail(`${path}: beklenmeyen scenario outcome bulundu`);
  }

  if (counts.caught !== observed.CaughtMutant) fail(`${path}: caught counter outcomes ile uyuşmuyor`);
  if (counts.missed !== observed.MissedMutant) fail(`${path}: missed counter outcomes ile uyuşmuyor`);
  if (counts.timeout !== observed.Timeout) fail(`${path}: timeout counter outcomes ile uyuşmuyor`);
  if (counts.unviable !== observed.Unviable) fail(`${path}: unviable counter outcomes ile uyuşmuyor`);
  if (counts.success !== 0) {
    fail(`${path}: sınıflandırılmamış başarılı mutant bulundu (${counts.success}); quality summary üretilemez`);
  }
  if (counts.caught + counts.missed + counts.timeout + counts.unviable !== counts.total) {
    fail(`${path}: outcome counter toplamı total_mutants ile uyuşmuyor`);
  }

  return { version: report.cargo_mutants_version, ...counts, mutants };
}

function validateBaseline(baselinePath, mutants, total, version) {
  if (!baselinePath) return;
  const baseline = readJson(baselinePath, 'mutation baseline');
  if (baseline.toolVersion && baseline.toolVersion !== version) {
    fail(`cargo-mutants tool sürümü baseline ile uyuşmuyor: ${version} != ${baseline.toolVersion}`);
  }
  const candidate = baseline.candidateMutants;
  const expectedTotal = candidate?.total ?? baseline.total;
  if (!Number.isInteger(expectedTotal) || expectedTotal < 0) {
    fail(`mutation baseline candidateMutants.total eksik: ${baselinePath}`);
  }
  if (total !== expectedTotal) {
    fail(`mutant denominator baseline ile uyuşmuyor: ${total} != ${expectedTotal}`);
  }

  const scope = baseline.scope ?? baseline.fullRun?.scope ?? [];
  if (!Array.isArray(scope) || scope.length === 0) fail(`mutation baseline scope eksik: ${baselinePath}`);
  const allowed = new Set(scope);
  const byFile = new Map();
  for (const mutant of mutants) {
    if (!allowed.has(mutant.file)) fail(`mutant scope dışında: ${mutant.file}`);
    byFile.set(mutant.file, (byFile.get(mutant.file) ?? 0) + 1);
  }

  const expectedByFile = candidate?.byFile;
  if (expectedByFile && typeof expectedByFile === 'object') {
    const expectedKeys = Object.keys(expectedByFile).sort();
    const actualKeys = [...byFile.keys()].sort();
    if (JSON.stringify(expectedKeys) !== JSON.stringify(actualKeys)) {
      fail('mutant file kapsamı baseline ile uyuşmuyor');
    }
    for (const file of expectedKeys) {
      if (byFile.get(file) !== expectedByFile[file]) {
        fail(`mutant file denominator baseline ile uyuşmuyor: ${file}`);
      }
    }
  }
}

const { inputs, expectedShards, baselinePath } = parseArguments();
const reports = inputs.map(({ id, path }) => ({
  shardId: id,
  ...validateReport(readJson(path, 'cargo-mutants outcomes'), path, id),
}));
const versions = new Set(reports.map((report) => report.version));
if (versions.size !== 1) fail('shard outcomes cargo-mutants sürümü aynı değil');

const allMutants = [];
const seenMutants = new Set();
for (const report of reports) {
  for (const mutant of report.mutants) {
    if (seenMutants.has(mutant.id)) fail(`duplicate mutant outcome: ${mutant.id}`);
    seenMutants.add(mutant.id);
    allMutants.push(mutant);
  }
}

const total = reports.reduce((sum, report) => sum + report.total, 0);
if (allMutants.length !== total) fail(`unique mutant sayısı total ile uyuşmuyor: ${allMutants.length} != ${total}`);
validateBaseline(baselinePath, allMutants, total, reports[0].version);

const caught = reports.reduce((sum, report) => sum + report.caught, 0);
const missed = reports.reduce((sum, report) => sum + report.missed, 0);
const timeout = reports.reduce((sum, report) => sum + report.timeout, 0);
const unviable = reports.reduce((sum, report) => sum + report.unviable, 0);
const meaningful = total - unviable;
const scorePercent = meaningful === 0 ? null : Number(((caught / meaningful) * 100).toFixed(2));

const summary = {
  schemaVersion: 1,
  status: 'complete',
  tool: 'cargo-mutants',
  toolVersion: reports[0].version,
  reports: reports.length,
  shards: expectedShards ?? reports.length,
  shardIds: reports.map((report) => report.shardId).sort((left, right) => left - right),
  total,
  caught,
  missed,
  timeout,
  unviable,
  meaningful,
  scorePercent,
  qualityMode: 'advisory',
  qualityStatus: missed > 0 || timeout > 0 ? 'findings' : 'clean',
  missedMutants: allMutants.filter((mutant) => mutant.summary === 'MissedMutant').map((mutant) => mutant.id),
  timeoutMutants: allMutants.filter((mutant) => mutant.summary === 'Timeout').map((mutant) => mutant.id),
};

console.log(JSON.stringify(summary, null, 2));
