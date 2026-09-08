import { describe, expect, test } from 'bun:test';
import { reconcileStatutorySnapshot, sameStatutoryDefinition } from './statutorySnapshotPolicy';

const baseSettings = {
  donemId: '2026-01',
  gunlukAsgariUcret: 1000,
  sgkIsciOraniYuzde: 14,
  issizlikIsciOraniYuzde: 1,
  pekTavanKatsayisi: 3,
  gunlukYemekIstisnasiSGK: 100,
  gunlukYemekIstisnasiGV: 100,
  statutoryParameterSegments: [],
};

const snapshot = {
  gunlukAsgariUcret: 1000,
  sgkIsciOraniYuzde: 14,
  issizlikIsciOraniYuzde: 1,
  pekTavanKatsayisi: 3,
  gunlukYemekIstisnasiSGK: 100,
  gunlukYemekIstisnasiGV: 100,
  statutoryParameterSegments: [],
};

describe('browser statutory snapshot lifecycle', () => {
  test('non-statutory edit preserves snapshot', () => {
    const previous = { ...baseSettings, statutoryParameterSnapshot: snapshot };
    const next = { ...previous, gunlukTabanUcret: 2500 };

    expect(sameStatutoryDefinition(previous, next)).toBe(true);
    expect(reconcileStatutorySnapshot(previous, next).statutoryParameterSnapshot).toEqual(snapshot);
  });

  test('statutory edit clears refreshable snapshot', () => {
    const previous = { ...baseSettings, statutoryParameterSnapshot: snapshot };
    const next = { ...previous, gunlukAsgariUcret: 1100 };

    expect(sameStatutoryDefinition(previous, next)).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(
      reconcileStatutorySnapshot(previous, next),
      'statutoryParameterSnapshot'
    )).toBe(false);
  });

  test('stale snapshot is cleared even when the mutable settings are unchanged', () => {
    const previous = {
      ...baseSettings,
      statutoryParameterSnapshot: { ...snapshot, gunlukAsgariUcret: 900 },
    };
    const next = { ...previous, gunlukTabanUcret: 2500 };

    expect(Object.prototype.hasOwnProperty.call(
      reconcileStatutorySnapshot(previous, next),
      'statutoryParameterSnapshot'
    )).toBe(false);
  });
});
