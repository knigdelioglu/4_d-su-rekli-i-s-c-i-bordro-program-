import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = process.cwd();
const modelsPath = resolve(root, 'crates/payroll-core/src/models.rs');
const generatedPath = resolve(
  root,
  'src/services/payrollEngine/generated/payrollContract.ts'
);

const source = readFileSync(modelsPath, 'utf8');

// This is intentionally a small Rust item parser rather than a collection of
// field-name regexes. It does not attempt to parse Rust expressions; it only
// accepts the public serde models in models.rs and fails closed when their
// shape changes beyond the contract surface we understand.
function stripComments(value) {
  return value
    .replace(/\/\*[\s\S]*?\*\//gu, '')
    .replace(/(^|[^:])\/\/.*$/gmu, '$1');
}

function findMatchingBrace(value, openingIndex) {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = openingIndex; index < value.length; index += 1) {
    const character = value[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (character === '\\') {
        escaped = true;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }
    if (character === '"') {
      inString = true;
      continue;
    }
    if (character === '{') depth += 1;
    if (character === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error('Rust contract parser: kapanmayan model bloğu.');
}

function splitTopLevel(value) {
  const parts = [];
  let start = 0;
  let angleDepth = 0;
  let braceDepth = 0;
  let bracketDepth = 0;
  let parenDepth = 0;
  let inString = false;
  let escaped = false;
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (character === '\\') {
        escaped = true;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }
    if (character === '"') {
      inString = true;
      continue;
    }
    if (character === '<') angleDepth += 1;
    if (character === '>') angleDepth -= 1;
    if (character === '{') braceDepth += 1;
    if (character === '}') braceDepth -= 1;
    if (character === '[') bracketDepth += 1;
    if (character === ']') bracketDepth -= 1;
    if (character === '(') parenDepth += 1;
    if (character === ')') parenDepth -= 1;
    if (
      character === ',' &&
      angleDepth === 0 &&
      braceDepth === 0 &&
      bracketDepth === 0 &&
      parenDepth === 0
    ) {
      parts.push(value.slice(start, index));
      start = index + 1;
    }
  }
  parts.push(value.slice(start));
  return parts.map((part) => part.trim()).filter(Boolean);
}

function serdeAttributes(value) {
  const result = { renameAll: null, rename: null, default: false };
  for (const [, content] of value.matchAll(/#\[serde\(([^\]]*)\)\]/gu)) {
    const renameAll = content.match(/\brename_all\s*=\s*"([^"]+)"/u);
    const rename = content.match(/\brename\s*=\s*"([^"]+)"/u);
    if (renameAll) result.renameAll = renameAll[1];
    if (rename) result.rename = rename[1];
    if (/\bdefault\b/u.test(content)) result.default = true;
  }
  return result;
}

function rustToSerdeName(name, renameAll) {
  if (!renameAll) return name;
  if (renameAll === 'camelCase') {
    return name.replace(/_([a-z])/gu, (_match, character) => character.toUpperCase());
  }
  if (renameAll === 'snake_case') {
    return name
      .replace(/([A-Z]+)([A-Z][a-z])/gu, '$1_$2')
      .replace(/([a-z\d])([A-Z])/gu, '$1_$2')
      .replace(/-/gu, '_')
      .toLowerCase();
  }
  if (renameAll === 'SCREAMING_SNAKE_CASE') {
    return rustToSerdeName(name, 'snake_case').toUpperCase();
  }
  throw new Error(`Rust contract parser: desteklenmeyen serde rename_all: ${renameAll}`);
}

function typeMetadata(typeText, optionalByDefault) {
  const normalized = typeText.replace(/\s+/gu, ' ').trim();
  const isOption = /^Option\s*</u.test(normalized);
  const isDecimal = /\bDecimal\b/u.test(normalized);
  const nestedTypes = [...normalized.matchAll(/\b([A-Z][A-Za-z0-9_]*)\b/gu)]
    .map(([, name]) => name)
    .filter((name) => !['Option', 'Vec', 'HashMap', 'String', 'Decimal'].includes(name));
  return {
    rustType: normalized,
    required: !isOption && !optionalByDefault,
    optional: isOption || optionalByDefault,
    nullable: isOption,
    decimal: isDecimal,
    nestedTypes: [...new Set(nestedTypes)],
  };
}

function parseStructFields(body, structDefault, renameAll) {
  return splitTopLevel(body).map((entry) => {
    const attributes = serdeAttributes(entry);
    const withoutAttributes = entry.replace(/#\[[^\]]*\]/gu, '').trim();
    const match = withoutAttributes.match(/^pub\s+([\p{L}_][\p{L}\p{N}_]*)\s*:\s*(.+)$/su);
    if (!match) {
      throw new Error(`Rust contract parser: struct alanı okunamadı: ${entry}`);
    }
    const [, rustName, rustType] = match;
    const name = attributes.rename ?? rustToSerdeName(rustName, renameAll);
    return {
      rustName,
      name,
      ...typeMetadata(rustType, structDefault || attributes.default),
    };
  });
}

function parseEnumVariants(body, renameAll) {
  return splitTopLevel(body).map((entry) => {
    const attributes = serdeAttributes(entry);
    const withoutAttributes = entry.replace(/#\[[^\]]*\]/gu, '').trim();
    const match = withoutAttributes.match(/^([A-Za-z_][A-Za-z0-9_]*)(?:\s*=\s*.+)?$/su);
    if (!match) {
      throw new Error(`Rust contract parser: enum variantı okunamadı: ${entry}`);
    }
    const [, rustName] = match;
    return {
      rustName,
      name: attributes.rename ?? rustToSerdeName(rustName, renameAll),
    };
  });
}

function parseModels(value) {
  const cleanSource = stripComments(value);
  const lines = cleanSource.split('\n');
  const lineOffsets = [];
  let offset = 0;
  for (const line of lines) {
    lineOffsets.push(offset);
    offset += line.length + 1;
  }

  const structs = {};
  const enums = {};
  let pendingAttributes = [];
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const trimmed = lines[lineIndex].trim();
    if (!trimmed || trimmed.startsWith('///')) continue;
    if (trimmed.startsWith('#[')) {
      pendingAttributes.push(trimmed);
      continue;
    }
    const item = trimmed.match(/^pub\s+(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/u);
    if (!item) {
      pendingAttributes = [];
      continue;
    }

    const [, kind, rustName] = item;
    const openingIndex = lineOffsets[lineIndex] + lines[lineIndex].indexOf('{');
    const closingIndex = findMatchingBrace(cleanSource, openingIndex);
    const body = cleanSource.slice(openingIndex + 1, closingIndex);
    const attributes = serdeAttributes(pendingAttributes.join('\n'));
    pendingAttributes = [];
    const renameAll = attributes.renameAll;
    if (kind === 'struct') {
      if (structs[rustName]) throw new Error(`Rust contract parser: duplicate struct ${rustName}`);
      structs[rustName] = {
        renameAll,
        default: attributes.default,
        fields: parseStructFields(body, attributes.default, renameAll),
      };
    } else {
      if (enums[rustName]) throw new Error(`Rust contract parser: duplicate enum ${rustName}`);
      const variants = parseEnumVariants(body, renameAll);
      enums[rustName] = {
        renameAll,
        values: variants.map((variant) => variant.name),
        variants,
      };
    }
    while (lineIndex < lines.length && lineOffsets[lineIndex] <= closingIndex) lineIndex += 1;
    lineIndex -= 1;
  }
  if (Object.keys(structs).length === 0 || Object.keys(enums).length === 0) {
    throw new Error('Rust contract parser: models.rs içinde struct/enum bulunamadı.');
  }
  return { structs, enums };
}

function buildGeneratedContract() {
  const parsed = parseModels(source);
  const decimalKeys = [...new Set(
    Object.values(parsed.structs)
      .flatMap((struct) => struct.fields)
      .filter((field) => field.decimal)
      .map((field) => field.name)
  )].sort((left, right) => left.localeCompare(right));
  if (decimalKeys.length === 0) {
    throw new Error('Rust model contract içinde Decimal alanı bulunamadı.');
  }

  const enumValues = Object.fromEntries(
    Object.entries(parsed.enums).map(([name, item]) => [name, item.values])
  );
  const structContract = Object.fromEntries(
    Object.entries(parsed.structs).map(([name, item]) => [name, {
      renameAll: item.renameAll,
      default: item.default,
      fields: Object.fromEntries(item.fields.map((field) => [field.name, {
        rustName: field.rustName,
        rustType: field.rustType,
        required: field.required,
        optional: field.optional,
        nullable: field.nullable,
        decimal: field.decimal,
        nestedTypes: field.nestedTypes,
      }])),
    }])
  );

  return [
    '/**',
    ' * GENERATED FILE — run `bun scripts/generate-payroll-contract.mjs --write`.',
    ' * Source of truth: crates/payroll-core/src/models.rs',
    ' */',
    `export const RUST_DECIMAL_KEYS = ${JSON.stringify(decimalKeys, null, 2)} as const;`,
    '',
    `export const RUST_ENUM_VALUES = ${JSON.stringify(enumValues, null, 2)} as const;`,
    '',
    `export const RUST_STRUCT_CONTRACT = ${JSON.stringify(structContract, null, 2)} as const;`,
    '',
    'export const RUST_DOMAIN_CONTRACT = {',
    '  enums: RUST_ENUM_VALUES,',
    '  structs: RUST_STRUCT_CONTRACT,',
    '} as const;',
    '',
  ].join('\n');
}

const generated = buildGeneratedContract();

if (process.argv.includes('--write')) {
  writeFileSync(generatedPath, generated);
  console.log('payroll-contract: wrote generated Rust domain contract.');
  process.exit(0);
}

const current = readFileSync(generatedPath, 'utf8');
if (current !== generated) {
  console.error(
    'payroll-contract: generated TypeScript contract is stale. Run `bun scripts/generate-payroll-contract.mjs --write`.'
  );
  process.exit(1);
}
console.log('payroll-contract: PASS — Rust domain contract is synchronized.');
