// Run with the fixture exported by audit_request_json_roundtrip_preserves_calculation.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import init, { calculate_payroll_json } from '../src/wasm/pkg/payroll_wasm.js';

const fixture = process.argv[2];
assert.ok(fixture, 'Pass the Rust-exported audit request JSON path.');
await init({ module_or_path: await readFile(new URL('../src/wasm/pkg/payroll_wasm_bg.wasm', import.meta.url)) });
const request = JSON.parse(await readFile(fixture, 'utf8'));
const normal = JSON.parse(calculate_payroll_json(JSON.stringify(request)));
assert.equal(normal.kesintiler.damgaVergisi, '219.88');
assert.equal(normal.netOdeme, '53126.45');
const invalid = structuredClone(request);
invalid.dataset.annualPayrollParameters[0].gelirVergisiDilimleri = [];
assert.throws(() => calculate_payroll_json(JSON.stringify(invalid)));
const extra = structuredClone(request);
extra.dataset.personnel[0].kesintiler.sabitBesTutar = '0';
extra.accrual = {
  accrualId: 'audit-extra', accrualType: 'TEDIYE', paymentDate: '2026-01-31',
  sequence: 1, grossAmount: '10000', description: null,
};
const result = JSON.parse(calculate_payroll_json(JSON.stringify(extra)));
assert.equal(result.kesintiler.bes, '300');
console.log('PASS: WASM meal DV/net, invalid tariff rejection, supplementary zero-fixed BES (3 scenarios).');
