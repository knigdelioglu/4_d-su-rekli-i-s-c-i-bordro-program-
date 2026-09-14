import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const IGNORED_CUSTOM_SECTIONS = new Set(['name', 'producers']);
const PATH_MARKERS = [
  '/rustc/',
  '/cargo-home/',
  '/rust/deps/',
  '/workspace/',
  '/home/',
  '/Users/',
  'crates/',
  'src-tauri/',
];

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');

function fail(message) {
  throw new Error(message);
}

function readUleb(bytes, offset) {
  let value = 0;
  let shift = 0;
  let cursor = offset;
  while (cursor < bytes.length) {
    const byte = bytes[cursor++];
    value += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return [value, cursor];
    shift += 7;
    if (shift > 35) fail('ULEB128 değer aralığı aşıldı.');
  }
  fail('Kesilmiş ULEB128 değeri.');
}

function readSleb(bytes, offset, bits = 32) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  let byte = 0;
  while (cursor < bytes.length) {
    byte = bytes[cursor++];
    value |= BigInt(byte & 0x7f) << shift;
    shift += 7n;
    if ((byte & 0x80) === 0) break;
    if (shift > BigInt(bits + 7)) fail('SLEB128 değer aralığı aşıldı.');
  }
  if (byte & 0x40) value |= (-1n) << shift;
  const numeric = Number(value);
  if (!Number.isSafeInteger(numeric)) fail('SLEB128 değeri JavaScript güvenli aralığını aşıyor.');
  return [numeric, cursor];
}

function equalBytes(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function parseSections(bytes) {
  if (bytes.length < 8 || bytes.subarray(0, 4).toString('hex') !== '0061736d') {
    fail('WASM magic header geçersiz.');
  }
  if (bytes.readUInt32LE(4) !== 1) fail('Desteklenmeyen WASM binary version.');

  const sections = [];
  let cursor = 8;
  while (cursor < bytes.length) {
    const id = bytes[cursor++];
    const [size, payloadStart] = readUleb(bytes, cursor);
    const payloadEnd = payloadStart + size;
    if (payloadEnd > bytes.length) fail(`Section ${id} payload sınırını aşıyor.`);
    const payload = bytes.subarray(payloadStart, payloadEnd);
    let name = null;
    let body = payload;
    if (id === 0) {
      const [nameLength, nameStart] = readUleb(payload, 0);
      const nameEnd = nameStart + nameLength;
      if (nameEnd > payload.length) fail('Custom section adı payload sınırını aşıyor.');
      name = Buffer.from(payload.subarray(nameStart, nameEnd)).toString('utf8');
      body = payload.subarray(nameEnd);
    }
    sections.push({ id, name, payload, body });
    cursor = payloadEnd;
  }
  return sections;
}

function readConstOffset(bytes, offset) {
  if (bytes[offset] !== 0x41) fail('Aktif data segment offset’i i32.const değil.');
  const [value, afterValue] = readSleb(bytes, offset + 1, 32);
  if (bytes[afterValue] !== 0x0b) fail('Aktif data segment offset expression kapanmıyor.');
  return [value >>> 0, afterValue + 1];
}

function parseDataSegments(bytes) {
  const payload = parseSections(bytes).find(({ id }) => id === 11)?.payload;
  if (!payload) fail('WASM data section bulunamadı.');
  const [count, firstSegment] = readUleb(payload, 0);
  const segments = [];
  let cursor = firstSegment;
  for (let index = 0; index < count; index += 1) {
    const [flags, afterFlags] = readUleb(payload, cursor);
    cursor = afterFlags;
    let memoryIndex = null;
    let offset = null;
    if (flags === 0 || flags === 2) {
      if (flags === 2) [memoryIndex, cursor] = readUleb(payload, cursor);
      else memoryIndex = 0;
      [offset, cursor] = readConstOffset(payload, cursor);
    } else if (flags !== 1) {
      fail(`Desteklenmeyen data segment flags değeri: ${flags}.`);
    }
    const [size, dataStart] = readUleb(payload, cursor);
    const dataEnd = dataStart + size;
    if (dataEnd > payload.length) fail(`Data segment ${index} payload sınırını aşıyor.`);
    segments.push({
      index,
      flags,
      memoryIndex,
      offset,
      bytes: payload.subarray(dataStart, dataEnd),
    });
    cursor = dataEnd;
  }
  if (cursor !== payload.length) fail('Data section sonunda beklenmeyen bytes bulundu.');
  return segments;
}

function canonicalPath(value) {
  const normalized = value.replaceAll('\\', '/');
  const repositoryPath = normalized.match(/(?:^|\/)(crates\/.*)$/);
  if (repositoryPath) return `repo/${repositoryPath[1]}`;
  const tauriPath = normalized.match(/(?:^|\/)(src-tauri\/.*)$/);
  if (tauriPath) return `repo/${tauriPath[1]}`;
  const rustcPath = normalized.match(/^\/rustc\/[^/]+\/(.*)$/);
  if (rustcPath) return `rustc/${rustcPath[1]}`;
  const cargoPath = normalized.match(/^\/cargo-home\/registry\/src\/[^/]+\/(.*)$/);
  if (cargoPath) return `cargo-registry/${cargoPath[1]}`;
  const rustDependencyPath = normalized.match(/^\/rust\/deps\/(.*)$/);
  if (rustDependencyPath) return `rust-deps/${rustDependencyPath[1]}`;
  return normalized;
}

function findPathSpans(bytes) {
  const spans = new Map();
  let cursor = 0;
  while (cursor < bytes.length) {
    if (bytes[cursor] < 0x20 || bytes[cursor] > 0x7e) {
      cursor += 1;
      continue;
    }
    const runStart = cursor;
    while (cursor < bytes.length && bytes[cursor] >= 0x20 && bytes[cursor] <= 0x7e) {
      cursor += 1;
    }
    const run = Buffer.from(bytes.subarray(runStart, cursor)).toString('utf8');
    for (const marker of PATH_MARKERS) {
      let markerOffset = run.indexOf(marker);
      while (markerOffset >= 0) {
        const end = run.indexOf('.rs', markerOffset);
        if (end >= 0) {
          const start = runStart + markerOffset;
          const finish = runStart + end + 3;
          const text = Buffer.from(bytes.subarray(start, finish)).toString('utf8');
          const existing = spans.get(start);
          if (!existing || existing.end < finish) {
            spans.set(start, { start, end: finish, key: canonicalPath(text) });
          }
        }
        markerOffset = run.indexOf(marker, markerOffset + marker.length);
      }
    }
  }
  return [...spans.values()].sort((left, right) => left.start - right.start);
}

function dataFingerprint(segments) {
  return segments
    .map((segment) => {
      const spans = findPathSpans(segment.bytes);
      const nonPathParts = [];
      let cursor = 0;
      for (const span of spans) {
        nonPathParts.push(segment.bytes.subarray(cursor, span.start));
        cursor = span.end;
      }
      nonPathParts.push(segment.bytes.subarray(cursor));
      return {
        flags: segment.flags,
        memoryIndex: segment.memoryIndex,
        nonPathLength: nonPathParts.reduce((total, part) => total + part.length, 0),
        nonPathSha256: digest(Buffer.concat(nonPathParts)),
        paths: spans.map(({ key }) => key).sort(),
      };
    })
    .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function dataFingerprintSummary(fingerprint) {
  return fingerprint.map(({ flags, memoryIndex, nonPathLength, nonPathSha256, paths }) => ({
    flags,
    memoryIndex,
    nonPathLength,
    nonPathSha256,
    pathCount: paths.length,
    pathsSha256: digest(Buffer.from(paths.join('\0'))),
  }));
}

function maskedDataBytes(segment) {
  const masked = Buffer.from(segment.bytes);
  for (const span of findPathSpans(segment.bytes)) masked.fill(0, span.start, span.end);
  return masked;
}

function dataDiffSummary(referenceSegments, generatedSegments) {
  const summary = [];
  const count = Math.min(referenceSegments.length, generatedSegments.length);
  for (let index = 0; index < count; index += 1) {
    const referenceBytes = maskedDataBytes(referenceSegments[index]);
    const generatedBytes = maskedDataBytes(generatedSegments[index]);
    const firstDifferences = [];
    let differenceCount = 0;
    let firstDifferenceOffset = null;
    const common = Math.min(referenceBytes.length, generatedBytes.length);
    for (let byteIndex = 0; byteIndex < common; byteIndex += 1) {
      if (referenceBytes[byteIndex] !== generatedBytes[byteIndex]) {
        differenceCount += 1;
        firstDifferenceOffset ??= byteIndex;
        if (firstDifferences.length < 24) {
          firstDifferences.push({
            offset: byteIndex,
            reference: referenceBytes[byteIndex],
            generated: generatedBytes[byteIndex],
          });
        }
      }
    }
    summary.push({
      segment: index,
      referenceLength: referenceBytes.length,
      generatedLength: generatedBytes.length,
      differenceCount,
      firstDifferences,
      ...(firstDifferenceOffset === null
        ? {}
        : {
            firstDifferenceContext: (() => {
              const start = Math.max(0, firstDifferenceOffset - 16);
              const end = Math.min(
                Math.max(referenceSegments[index].bytes.length, generatedSegments[index].bytes.length),
                firstDifferenceOffset + 48,
              );
              return {
                start,
                referenceHex: referenceSegments[index].bytes.subarray(start, end).toString('hex'),
                generatedHex: generatedSegments[index].bytes.subarray(start, end).toString('hex'),
              };
            })(),
          }),
    });
  }
  return summary;
}

class PathAddressMap {
  constructor(segments) {
    this.entries = segments.flatMap((segment) => {
      if (segment.offset === null) return [];
      return findPathSpans(segment.bytes).map((span) => ({
        start: segment.offset + span.start,
        end: segment.offset + span.end,
        key: span.key,
      }));
    });
  }

  lookup(value) {
    const unsigned = value >>> 0;
    const entry = this.entries.find(({ start, end }) => unsigned >= start && unsigned < end);
    if (!entry) return null;
    return `${entry.key}+${unsigned - entry.start}`;
  }
}

function skipBlockType(bytes, offset) {
  const first = bytes[offset];
  if ([0x40, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f].includes(first)) return offset + 1;
  return readSleb(bytes, offset, 33)[1];
}

function skipHeapType(bytes, offset) {
  return readSleb(bytes, offset, 33)[1];
}

function skipRefType(bytes, offset) {
  if (bytes[offset] === 0x63 || bytes[offset] === 0x64) return skipHeapType(bytes, offset + 1);
  return offset + 1;
}

function skipMemArg(bytes, offset) {
  const [, afterAlignment] = readUleb(bytes, offset);
  return readUleb(bytes, afterAlignment)[1];
}

function skipBulkMemoryInstruction(bytes, offset, subopcode) {
  if (subopcode <= 7) return offset;
  switch (subopcode) {
    case 8:
      return readUleb(bytes, readUleb(bytes, offset)[1])[1];
    case 9:
      return readUleb(bytes, offset)[1];
    case 10:
      return readUleb(bytes, readUleb(bytes, offset)[1])[1];
    case 11:
      return readUleb(bytes, offset)[1];
    case 12:
      return readUleb(bytes, readUleb(bytes, offset)[1])[1];
    case 13:
      return readUleb(bytes, offset)[1];
    case 14:
      return readUleb(bytes, readUleb(bytes, offset)[1])[1];
    case 15:
    case 16:
    case 17:
      return readUleb(bytes, offset)[1];
    default:
      fail(`Desteklenmeyen 0xfc opcode alt değeri: ${subopcode}.`);
  }
}

function readInstruction(body, offset, pathAddresses) {
  const start = offset;
  const opcode = body[offset++];
  if (opcode === undefined) fail('Instruction body sınırı aşıldı.');
  const readIndex = () => readUleb(body, offset)[1];
  switch (opcode) {
    case 0x02:
    case 0x03:
    case 0x04:
      offset = skipBlockType(body, offset);
      break;
    case 0x0c:
    case 0x0d:
    case 0x10:
    case 0x12:
    case 0x18:
    case 0x20:
    case 0x21:
    case 0x22:
    case 0x23:
    case 0x24:
    case 0x25:
    case 0x26:
    case 0xd2:
    case 0xd5:
    case 0xd6:
      offset = readIndex();
      break;
    case 0x0e: {
      const [count, afterCount] = readUleb(body, offset);
      offset = afterCount;
      for (let index = 0; index < count + 1; index += 1) offset = readIndex();
      break;
    }
    case 0x11:
    case 0x13:
      offset = readIndex();
      offset = readIndex();
      break;
    case 0x14:
    case 0x15:
      offset = readIndex();
      break;
    case 0x1c: {
      const [count, afterCount] = readUleb(body, offset);
      offset = afterCount;
      for (let index = 0; index < count; index += 1) offset = skipRefType(body, offset);
      break;
    }
    case 0x28:
    case 0x29:
    case 0x2a:
    case 0x2b:
    case 0x2c:
    case 0x2d:
    case 0x2e:
    case 0x2f:
    case 0x30:
    case 0x31:
    case 0x32:
    case 0x33:
    case 0x34:
    case 0x35:
    case 0x36:
    case 0x37:
    case 0x38:
    case 0x39:
    case 0x3a:
    case 0x3b:
    case 0x3c:
    case 0x3d:
    case 0x3e:
      offset = skipMemArg(body, offset);
      break;
    case 0x3f:
    case 0x40:
      offset = readIndex();
      break;
    case 0x41: {
      const [value, afterValue] = readSleb(body, offset, 32);
      offset = afterValue;
      const path = pathAddresses.lookup(value);
      if (path) return { next: offset, token: `i32.const:data-path:${path}` };
      break;
    }
    case 0x42:
      offset = readSleb(body, offset, 64)[1];
      break;
    case 0x43:
      offset += 4;
      break;
    case 0x44:
      offset += 8;
      break;
    case 0xd0:
      offset = skipHeapType(body, offset);
      break;
    case 0xfc: {
      const [subopcode, afterSubopcode] = readUleb(body, offset);
      offset = skipBulkMemoryInstruction(body, afterSubopcode, subopcode);
      break;
    }
    default:
      if (opcode >= 0xfb && opcode <= 0xff) {
        fail(`Desteklenmeyen prefixed opcode: 0x${opcode.toString(16)}.`);
      }
  }
  if (offset > body.length) fail('Instruction immediate body sınırını aşıyor.');
  return { next: offset, token: body.subarray(start, offset).toString('hex') };
}

function canonicalBody(body, pathAddresses) {
  let cursor = 0;
  const [localCount, afterLocalCount] = readUleb(body, cursor);
  cursor = afterLocalCount;
  for (let index = 0; index < localCount; index += 1) {
    const [, afterCount] = readUleb(body, cursor);
    cursor = afterCount;
    cursor = skipRefType(body, cursor);
  }
  const tokens = [body.subarray(0, cursor).toString('hex')];
  while (cursor < body.length) {
    const instruction = readInstruction(body, cursor, pathAddresses);
    tokens.push(instruction.token);
    cursor = instruction.next;
  }
  return tokens.join('|');
}

function parseCodeBodies(bytes) {
  const payload = parseSections(bytes).find(({ id }) => id === 10)?.payload;
  if (!payload) fail('WASM code section bulunamadı.');
  const [count, firstBody] = readUleb(payload, 0);
  const bodies = [];
  let cursor = firstBody;
  for (let index = 0; index < count; index += 1) {
    const [size, bodyStart] = readUleb(payload, cursor);
    const bodyEnd = bodyStart + size;
    if (bodyEnd > payload.length) fail(`Code body ${index} payload sınırını aşıyor.`);
    bodies.push(payload.subarray(bodyStart, bodyEnd));
    cursor = bodyEnd;
  }
  if (cursor !== payload.length) fail('Code section sonunda beklenmeyen bytes bulundu.');
  return bodies;
}

function canonicalCode(bytes, pathAddresses) {
  return parseCodeBodies(bytes).map((body) => {
    try {
      return canonicalBody(body, pathAddresses);
    } catch {
      // Unsupported instructions are fail-closed: only an exact body match can pass.
      return `raw:${digest(body)}`;
    }
  });
}

function compareSections(reference, generated) {
  const referenceSections = parseSections(reference).filter(
    ({ id, name }) => id !== 0 || !IGNORED_CUSTOM_SECTIONS.has(name),
  );
  const generatedSections = parseSections(generated).filter(
    ({ id, name }) => id !== 0 || !IGNORED_CUSTOM_SECTIONS.has(name),
  );
  if (referenceSections.length !== generatedSections.length) {
    fail(
      `Metadata dışı section sayısı değişti: reference=${referenceSections.length}, generated=${generatedSections.length}.`,
    );
  }

  const referenceData = parseDataSegments(reference);
  const generatedData = parseDataSegments(generated);
  const referenceDataFingerprint = JSON.stringify(dataFingerprint(referenceData));
  const generatedDataFingerprint = JSON.stringify(dataFingerprint(generatedData));
  if (referenceDataFingerprint !== generatedDataFingerprint) {
    fail(
      `WASM data section metadata-normalized karşılaştırmada değişti: reference=${JSON.stringify(dataFingerprintSummary(JSON.parse(referenceDataFingerprint)))} generated=${JSON.stringify(dataFingerprintSummary(JSON.parse(generatedDataFingerprint)))} diff=${JSON.stringify(dataDiffSummary(referenceData, generatedData))}`,
    );
  }

  const referencePaths = new PathAddressMap(referenceData);
  const generatedPaths = new PathAddressMap(generatedData);
  const referenceCode = canonicalCode(reference, referencePaths);
  const generatedCode = canonicalCode(generated, generatedPaths);
  if (referenceCode.length !== generatedCode.length) {
    fail(`WASM function body sayısı değişti: reference=${referenceCode.length}, generated=${generatedCode.length}.`);
  }
  for (let index = 0; index < referenceCode.length; index += 1) {
    if (referenceCode[index] !== generatedCode[index]) {
      fail(`WASM function body ${index} metadata-normalized karşılaştırmada değişti.`);
    }
  }

  for (let index = 0; index < referenceSections.length; index += 1) {
    const left = referenceSections[index];
    const right = generatedSections[index];
    if (left.id !== right.id || left.name !== right.name) {
      fail(`WASM section topology değişti: index=${index}.`);
    }
    if (left.id === 10 || left.id === 11) continue;
    if (!equalBytes(left.payload, right.payload)) {
      fail(`WASM section değişti: id=${left.id}${left.name ? ` name=${left.name}` : ''}.`);
    }
  }
}

const argumentsList = process.argv.slice(2);
let reference;
let generatedPath;
if (argumentsList.length === 0) {
  generatedPath = 'src/wasm/pkg/payroll_wasm_bg.wasm';
  reference = execFileSync('git', ['show', `HEAD:${generatedPath}`], {
    maxBuffer: 8 * 1024 * 1024,
  });
} else if (argumentsList.length === 2) {
  reference = readFileSync(argumentsList[0]);
  generatedPath = argumentsList[1];
} else {
  console.error('Kullanım: node scripts/verify-wasm-binary.mjs [<reference.wasm> <generated.wasm>]');
  process.exit(2);
}

try {
  const generated = readFileSync(generatedPath);
  compareSections(reference, generated);
  console.log(
    `verify:wasm-binary: PASS — exact standard sections; only name/producers and source-location metadata normalized. generated=${digest(generated)}`,
  );
} catch (error) {
  console.error(`verify:wasm-binary: FAIL — ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
