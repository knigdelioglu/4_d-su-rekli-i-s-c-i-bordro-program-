import { expect, test } from 'bun:test';
import { legacySettingsToPolicyPrefillOnly } from './api';
import { draftForCompensationPolicyLoad } from '../../hooks/useCompensationPolicyController';

test('baseline prefill carries current global premium groups into a valid draft', () => {
  const groups = [{ id: 'A', ad: 'A', oran: '6.5', aktif: true }];
  const settings = { gunlukTabanUcret: '100', gunlukYemek: '10', birlestirilmisSosyalYardim: '10', gunlukVasitaYol: '10', giyimYardimi: '10', hizmetZammiBirimi: '10', geceCalismaPrimiYuzde: '0', geceCalismaTatiliPrimiYuzde: '0', isPrimiYuzde: null, ekOdeme: null, digerGelirVarsayilan: null };
  const prefill = legacySettingsToPolicyPrefillOnly({ legacyInstitutionSettings: { p: settings }, isPrimiGruplari: groups } as any, 'p');
  expect(prefill.kind).toBe('AVAILABLE');
  const draft = draftForCompensationPolicyLoad({ institutionScope: 'CURRENT_INSTITUTION', periodId: 'p', periodStartInclusive: '2026-01-01', periodEndInclusive: '2026-01-31', projectionDate: '2026-01-01' }, { initialBaselineRequired: true, activePolicyProjection: { kind: 'NONE' }, nonAuthoritativePrefill: prefill } as any);
  expect(draft.isPrimiGruplari).toEqual([{ ...groups[0], oran: '6,5' }]);
});
