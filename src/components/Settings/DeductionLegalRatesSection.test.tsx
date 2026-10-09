import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import type { PayrollPeriodInstitutionSettings } from '../../types/payroll';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from '../../utils/payrollPresentation';
import {
  applySendikaAidatiYontemi,
  DeductionLegalRatesSection,
  getSendikaAidatiYontemi,
} from './DeductionLegalRatesSection';

const periodId = '2026-10';

const renderSection = (overrides: Partial<PayrollPeriodInstitutionSettings> = {}) => {
  const paramsForm: PayrollPeriodInstitutionSettings = {
    ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
    donemId: periodId,
    ...overrides,
  };

  return renderToStaticMarkup(
    <DeductionLegalRatesSection
      aktifDonemId={periodId}
      paramsForm={paramsForm}
      setParamsForm={() => undefined}
      savedSuccess={false}
      onSubmit={() => undefined}
    />
  );
};

describe('DeductionLegalRatesSection union dues method', () => {
  test('shows only the active rate input when the fixed amount is zero', () => {
    const markup = renderSection({ sendikaAidatiYuzde: 65, sabitSendikaAidati: 0 });

    expect(getSendikaAidatiYontemi(0)).toBe('oran');
    expect(markup).toContain('Kurum genelinde uygulanan yöntem: Oran');
    expect(markup).toContain('id="sendika-aidati-orani"');
    expect(markup).not.toContain('id="sendika-aidati-sabit-tutar"');
    expect(markup).toMatch(/name="sendikaAidatiYontemi"[^>]*checked=""[^>]*value="oran"/);
  });

  test('shows only the active fixed amount input when a positive fixed amount is saved', () => {
    const markup = renderSection({ sabitSendikaAidati: 1700.57 });

    expect(getSendikaAidatiYontemi(1700.57)).toBe('sabit');
    expect(markup).toContain('Kurum genelinde uygulanan yöntem: Sabit tutar');
    expect(markup).toContain('id="sendika-aidati-sabit-tutar"');
    expect(markup).toContain('value="1700.57"');
    expect(markup).not.toContain('id="sendika-aidati-orani"');
    expect(markup).toMatch(/name="sendikaAidatiYontemi"[^>]*checked=""[^>]*value="sabit"/);
  });

  test('clears the fixed amount when switching to rate and preserves the configured rate', () => {
    const current: PayrollPeriodInstitutionSettings = {
      donemId: periodId,
      sendikaAidatiYuzde: 65,
      sabitSendikaAidati: 1700.57,
    };

    const next = applySendikaAidatiYontemi(current, 'oran');

    expect(next.sabitSendikaAidati).toBe(0);
    expect(next.sendikaAidatiYuzde).toBe(65);
    expect(applySendikaAidatiYontemi(current, 'sabit')).toBe(current);
  });
});
