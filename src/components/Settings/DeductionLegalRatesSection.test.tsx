import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import type {
  AnnualPayrollParameters,
  BordroDonemi,
  PayrollPeriodInstitutionSettings,
} from '../../types/payroll';
import { DEFAULT_PRODUCTION_KURUM_DEGERLERI } from '../../utils/payrollPresentation';
import {
  applySendikaAidatiYontemi,
  DeductionLegalRatesSection,
  getSendikaAidatiYontemi,
} from './DeductionLegalRatesSection';

const periodId = '2026-10';

const renderSection = (
  overrides: Partial<PayrollPeriodInstitutionSettings> = {},
  context: { period?: BordroDonemi; annual?: AnnualPayrollParameters[] } = {}
) => {
  const activePeriodId = context.period?.id ?? periodId;
  const paramsForm: PayrollPeriodInstitutionSettings = {
    ...DEFAULT_PRODUCTION_KURUM_DEGERLERI,
    donemId: activePeriodId,
    ...overrides,
  };

  return renderToStaticMarkup(
    <DeductionLegalRatesSection
      aktifDonemId={activePeriodId}
      activePeriodForParams={context.period}
      annualPayrollParameters={context.annual}
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

describe('DeductionLegalRatesSection legal parameter context', () => {
  const period = (serviceYear: number, serviceMonth: number, taxYear: number, taxMonth: number) => ({
    id: `${serviceYear}-${String(serviceMonth).padStart(2, '0')}`,
    yil: serviceYear,
    ay: serviceMonth,
    baslangicTarihi: `${serviceYear}-${String(serviceMonth).padStart(2, '0')}-15`,
    bitisTarihi: `${taxYear}-${String(taxMonth).padStart(2, '0')}-14`,
    donemAdi: 'UAT dönemi',
    taxYear,
    taxMonth,
  } as BordroDonemi);

  test('renders service and tax years from the active period without fixed 2026 guidance', () => {
    const samples = [
      { period: period(2026, 12, 2027, 1), expected: ['2026-12', '2026-12-15–2027-01-14', '2027-01 (2027 vergi yılı)'] },
      { period: period(2027, 12, 2028, 1), expected: ['2027-12', '2027-12-15–2028-01-14', '2028-01 (2028 vergi yılı)'] },
      { period: period(2028, 1, 2028, 2), expected: ['2028-01', '2028-01-15–2028-02-14', '2028-02 (2028 vergi yılı)'] },
    ];

    for (const sample of samples) {
      const markup = renderSection({}, { period: sample.period });
      for (const value of sample.expected) expect(markup).toContain(value);
      expect(markup).not.toContain('2026-08 dönemi');
      expect(markup).not.toContain('PEK Alt Sınırı = 1.101,00 TL');
      expect(markup).toContain('mevzuat kaynağı ve sürümü kayıtlı olmadığından');
      expect(markup).toContain(`${sample.period.taxYear} vergi yılı gelir vergisi parametre kaydı bulunmuyor`);
    }
  });

  test('reports the matching annual tax parameters and makes payment year distinct from service year', () => {
    const active = period(2026, 12, 2027, 1);
    const markup = renderSection({}, {
      period: active,
      annual: [{
        year: 2027,
        gelirVergisiDilimleri: [{ limit: 110000, oran: 0.15 }],
        updatedAt: '2027-01-02T10:00:00Z',
      } as AnnualPayrollParameters],
    });

    expect(markup).toContain('Hizmet dönemi 2026');
    expect(markup).toContain('Ödeme/vergi dönemi:');
    expect(markup).toContain('2027-01 (2027 vergi yılı)');
    expect(markup).toContain('2027 vergi yılı gelir vergisi parametre kaydı mevcut');
    expect(markup).toContain('2027-01-02T10:00:00Z');
  });

  test('shows the immutable engine snapshot and its mid-period effective segments', () => {
    const markup = renderSection({
      gunlukAsgariUcret: 1101,
      pekTavanKatsayisi: 7.5,
      statutoryParameterSegments: [],
      statutoryParameterSnapshot: {
        gunlukAsgariUcret: 1200,
        sgkIsciOraniYuzde: 14,
        issizlikIsciOraniYuzde: 1,
        pekTavanKatsayisi: 9,
        gunlukYemekIstisnasiSGK: 300,
        gunlukYemekIstisnasiGV: 250,
        statutoryParameterSegments: [{
          effectiveFrom: '2026-10-01',
          gunlukAsgariUcret: 1250,
          pekTavanKatsayisi: 10,
          gunlukYemekIstisnasiSGK: 320,
          gunlukYemekIstisnasiGV: 260,
        }],
      },
    });

    expect(markup).toContain('Bordro motoru kayıtlı dönem yasal parametre snapshot’ını kullanıyor');
    expect(markup).toContain('Snapshot günlük asgari ücret: 1.200,00 TL');
    expect(markup).toContain('Snapshot PEK tavan katsayısı: 9');
    expect(markup).toContain('2026-10-01 yürürlük');
    expect(markup).toContain('1.250,00 TL');
    expect(markup).toContain('Dönem ayarı günlük tutarı: 1.101,00 TL');
    expect(markup).toContain('kaynak bağlantısı veya sürüm bilgisi kayıtlı değil');
  });
});
