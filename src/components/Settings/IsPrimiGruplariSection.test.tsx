import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { IsPrimiGruplariSection } from './IsPrimiGruplariSection';

describe('IsPrimiGruplariSection', () => {
  test('shows global group controls without period or calculation-policy details', () => {
    const markup = renderToStaticMarkup(
      <IsPrimiGruplariSection
        groups={[{ id: '1. Grup', ad: '1. Grup', oran: 9, aktif: true }]}
        onSave={() => undefined}
      />
    );

    expect(markup).toContain('İş Primi Grupları');
    expect(markup).toContain('Personel kodu');
    expect(markup).toContain('Oran (%)');
    expect(markup).not.toContain('Başlangıç tarihi');
    expect(markup).not.toContain('Kısa açıklama');
    expect(markup).not.toContain('Hesaplama ayrıntıları');
    expect(markup).not.toContain('Hangi ücretler değişiyor?');
    expect(markup).not.toContain('Yol tutarı nasıl yuvarlansın?');
    expect(markup).not.toContain('Etkileri önizle');
    expect(markup).not.toContain('Yeni Dönem Aç');
  });
});
