import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = process.cwd();
const artifactPath = resolve(root, process.argv[2] ?? 'src/wasm/pkg/payroll_wasm_bg.wasm');
const referencePath = process.argv[3] ? resolve(root, process.argv[3]) : null;
const referenceRevisionPath = 'HEAD:src/wasm/pkg/payroll_wasm_bg.wasm';

function fail(message) {
  console.error(`verify:wasm-binary: FAIL — ${message}`);
  process.exit(1);
}

function readFile(path) {
  try {
    return readFileSync(path);
  } catch (error) {
    fail(`${path} okunamadı: ${error instanceof Error ? error.message : error}`);
  }
}

function readReference() {
  if (referencePath) return readFile(referencePath);
  try {
    return execFileSync('git', ['show', referenceRevisionPath], {
      cwd: root,
      maxBuffer: 32 * 1024 * 1024,
    });
  } catch (error) {
    fail(`checked-in WASM referansı okunamadı: ${error instanceof Error ? error.message : error}`);
  }
}

function readUnsignedLeb128(bytes, cursor, label) {
  let value = 0;
  let shift = 0;
  while (cursor.index < bytes.length && shift <= 28) {
    const byte = bytes[cursor.index++];
    value += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return value;
    shift += 7;
  }
  fail(`geçersiz ${label} unsigned LEB128 değeri`);
}

function readSignedLeb128(bytes, cursor, label) {
  while (cursor.index < bytes.length) {
    const byte = bytes[cursor.index++];
    if ((byte & 0x80) === 0) return;
  }
  fail(`geçersiz ${label} signed LEB128 değeri`);
}

function encodeUnsignedLeb128(value) {
  const encoded = [];
  let remaining = value;
  do {
    let byte = remaining & 0x7f;
    remaining = Math.floor(remaining / 128);
    if (remaining !== 0) byte |= 0x80;
    encoded.push(byte);
  } while (remaining !== 0);
  return Buffer.from(encoded);
}

function readCustomName(payload) {
  const cursor = { index: 0 };
  const length = readUnsignedLeb128(payload, cursor, 'custom section name');
  const end = cursor.index + length;
  if (end > payload.length) fail('custom section name payload sınırını aşıyor');
  return Buffer.from(payload.subarray(cursor.index, end)).toString('utf8');
}

function parseSections(bytes) {
  if (bytes.length < 8 || !bytes.subarray(0, 8).equals(Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]))) {
    fail('WASM magic/version geçersiz');
  }

  const sections = [];
  const cursor = { index: 8 };
  while (cursor.index < bytes.length) {
    const id = bytes[cursor.index++];
    const size = readUnsignedLeb128(bytes, cursor, `section ${id} size`);
    const start = cursor.index;
    const end = start + size;
    if (end > bytes.length) fail(`section ${id} payload sınırını aşıyor`);
    const payload = Buffer.from(bytes.subarray(start, end));
    sections.push({ id, name: id === 0 ? readCustomName(payload) : '', payload });
    cursor.index = end;
  }
  return sections;
}

function replaceWithCount(value, pattern, replacement) {
  let count = 0;
  const result = value.replace(pattern, (...args) => {
    count += 1;
    return typeof replacement === 'function' ? replacement(...args) : replacement;
  });
  return { value: result, count };
}

function normalizeMetadataPaths(bytes) {
  let value = Buffer.from(bytes).toString('latin1');
  let replacements = 0;
  const replacementsToApply = [
    [
      /(?:\/[^/\0\x00-\x1f]+\/)+crates\/(payroll-core|payroll-wasm)\/src\//g,
      (_, crate) => `crates/${crate}/src/`,
    ],
    [
      /(?:\/[^/\0\x00-\x1f]+\/)+registry\/src\/[^/\0\x00-\x1f]+\//g,
      '/cargo-home/registry/src/<registry>/',
    ],
    [
      /(?:\/[^/\0\x00-\x1f]+\/)*rustc\/[0-9a-f]{40}\//g,
      '/rustc/<rustc>/',
    ],
    [
      /(?:\/[^/\0\x00-\x1f]+\/)+rustlib\/src\/rust\/library\//g,
      '/rustc/<rustc>/library/',
    ],
    [
      /(?:\/[^/\0\x00-\x1f]+\/)+rust\/deps\//g,
      '/rust/deps/',
    ],
  ];

  for (const [pattern, replacement] of replacementsToApply) {
    const result = replaceWithCount(value, pattern, replacement);
    value = result.value;
    replacements += result.count;
  }

  return { bytes: Buffer.from(value, 'latin1'), replacements };
}

function skipOffsetExpression(bytes, cursor) {
  while (cursor.index < bytes.length) {
    const opcode = bytes[cursor.index++];
    switch (opcode) {
      case 0x0b:
        return;
      case 0x23:
      case 0x41:
      case 0x42:
      case 0xd0:
        readSignedLeb128(bytes, cursor, 'offset expression immediate');
        break;
      case 0x43:
        cursor.index += 4;
        break;
      case 0x44:
        cursor.index += 8;
        break;
      default:
        fail(`data section offset expression opcode desteklenmiyor: 0x${opcode.toString(16)}`);
    }
    if (cursor.index > bytes.length) fail('data section offset expression sınırını aşıyor');
  }
  fail('data section offset expression end opcode içermiyor');
}

function normalizeDataSection(payload) {
  const cursor = { index: 0 };
  const segmentCountStart = cursor.index;
  const segmentCount = readUnsignedLeb128(payload, cursor, 'data segment count');
  const chunks = [payload.subarray(segmentCountStart, cursor.index)];
  let replacements = 0;

  for (let index = 0; index < segmentCount; index += 1) {
    const segmentStart = cursor.index;
    const flags = readUnsignedLeb128(payload, cursor, `data segment ${index} flags`);
    if (flags === 0) {
      skipOffsetExpression(payload, cursor);
    } else if (flags === 1) {
      // Passive segment: no memory index or offset expression.
    } else if (flags === 2) {
      readUnsignedLeb128(payload, cursor, `data segment ${index} memory index`);
      skipOffsetExpression(payload, cursor);
    } else {
      fail(`data segment ${index} flags desteklenmiyor: ${flags}`);
    }

    const initLengthStart = cursor.index;
    const initLength = readUnsignedLeb128(payload, cursor, `data segment ${index} init length`);
    const initStart = cursor.index;
    const initEnd = initStart + initLength;
    if (initEnd > payload.length) fail(`data segment ${index} init payload sınırını aşıyor`);

    const normalized = normalizeMetadataPaths(payload.subarray(initStart, initEnd));
    chunks.push(payload.subarray(segmentStart, initLengthStart));
    chunks.push(encodeUnsignedLeb128(normalized.bytes.length));
    chunks.push(normalized.bytes);
    replacements += normalized.replacements;
    cursor.index = initEnd;
  }

  if (cursor.index !== payload.length) fail('data section içinde beklenmeyen trailing payload var');
  return { payload: Buffer.concat(chunks), replacements };
}

function equalBytes(left, right) {
  return left.length === right.length && left.equals(right);
}

function sectionLabel(section, index) {
  return `${index} (id=${section.id}${section.name ? `, name=${section.name}` : ''})`;
}

function normalizedSections(bytes) {
  return parseSections(bytes).map((section, index) => {
    if (section.id !== 11) return { ...section, replacements: 0, index };
    const normalized = normalizeDataSection(section.payload);
    return { ...section, payload: normalized.payload, replacements: normalized.replacements, index };
  });
}

function hashSections(sections) {
  const hash = createHash('sha256');
  hash.update(Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]));
  for (const section of sections) {
    hash.update(Buffer.from([section.id]));
    hash.update(encodeUnsignedLeb128(section.payload.length));
    hash.update(section.payload);
  }
  return hash.digest('hex');
}

const current = readFile(artifactPath);
const reference = readReference();
const currentHash = createHash('sha256').update(current).digest('hex');
const referenceHash = createHash('sha256').update(reference).digest('hex');

if (equalBytes(current, reference)) {
  console.log(`verify:wasm-binary: PASS — exact byte match (${currentHash}).`);
  process.exit(0);
}

const currentSections = normalizedSections(current);
const referenceSections = normalizedSections(reference);
if (currentSections.length !== referenceSections.length) {
  fail(`section sayısı değişti: current=${currentSections.length}, reference=${referenceSections.length}; current=${currentHash}, reference=${referenceHash}`);
}

let pathReplacements = 0;
for (let index = 0; index < currentSections.length; index += 1) {
  const currentSection = currentSections[index];
  const referenceSection = referenceSections[index];
  pathReplacements += currentSection.replacements + referenceSection.replacements;
  if (currentSection.id !== referenceSection.id || currentSection.name !== referenceSection.name) {
    fail(`WASM section topology drifti: current=${sectionLabel(currentSection, index)}, reference=${sectionLabel(referenceSection, index)}`);
  }
  if (!equalBytes(currentSection.payload, referenceSection.payload)) {
    fail(`semantic WASM section drifti: ${sectionLabel(currentSection, index)}; current=${currentHash}, reference=${referenceHash}`);
  }
}

const currentNormalizedHash = hashSections(currentSections);
const referenceNormalizedHash = hashSections(referenceSections);
if (currentNormalizedHash !== referenceNormalizedHash) {
  fail(`normalize edilmiş WASM hash eşleşmiyor: current=${currentNormalizedHash}, reference=${referenceNormalizedHash}`);
}

console.log(
  `verify:wasm-binary: PASS — drift yalnız canonicalized compiler/source path metadata ile sınırlı; normalizedHash=${currentNormalizedHash}, pathReplacements=${pathReplacements}.`,
);
