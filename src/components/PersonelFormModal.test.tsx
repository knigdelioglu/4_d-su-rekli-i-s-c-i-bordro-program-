import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { PersonelFormModal } from './PersonelFormModal';

describe('PersonelFormModal validation controls', () => {
  test('uses editable text drafts for amounts and leaves incomplete IBAN visible for correction', () => {
    const markup = renderToStaticMarkup(
      <PersonelFormModal
        isOpen
        onClose={() => undefined}
        onSave={() => undefined}
        isPrimiGruplari={[{ id: 'group-1', ad: '1. Grup', oran: 9, aktif: true }]}
      />
    );

    expect(markup).toMatch(/<form[^>]*novalidate/i);
    expect(markup).toContain('value="TR"');
    expect(markup).toContain('data-testid="personel-form-tab-deductions"');
    expect(markup).toContain('placeholder="TR00 0000 0000 0000 0000 0000 00"');
    expect(markup).toContain('aria-invalid="false"');
  });
});
