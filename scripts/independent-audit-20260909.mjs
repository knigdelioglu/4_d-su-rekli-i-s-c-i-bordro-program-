// Fixtures exported by the new Rust audit tests, never production/user data.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { initSync, calculate_payroll_json } from '../src/wasm/pkg/payroll_wasm.js';
const directory = process.env.PAYROLL_AUDIT_FIXTURES;
if (!directory) throw new Error('Set PAYROLL_AUDIT_FIXTURES to the Rust audit fixture directory.');
initSync({ module: readFileSync(new URL('../src/wasm/pkg/payroll_wasm_bg.wasm', import.meta.url)) });
for (const name of ['normal', 'retro-meal']) {
  const { request, expected } = JSON.parse(readFileSync(resolve(directory, `${name}.json`), 'utf8'));
  assert.deepEqual(JSON.parse(calculate_payroll_json(JSON.stringify(request))), expected, name);
  console.log(`PASS — Rust/WASM exact output parity: ${name}`);
}
