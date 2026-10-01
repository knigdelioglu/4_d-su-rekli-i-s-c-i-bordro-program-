import { describe, expect, test } from 'bun:test';
import { describeError } from './errorMessage';

describe('describeError', () => {
  test('never renders object identity', () => {
    const values: unknown[] = [
      new Error('Hata.'),
      'Düz metin.',
      { type: 'ValidationError', message: 'Doğrulama.' },
      { type: 'NegativeNetPayment', message: { gelir: 1, kesinti: 2 } },
      new Map([['message', 'Harita mesajı.']]),
      new Error('[object Object]'),
      { unexpected: true },
      {},
      null,
      undefined,
    ];
    for (const value of values) {
      const text = describeError(value);
      expect(text.length > 0).toBe(true);
      expect(text.includes('[object Object]')).toBe(false);
    }
    expect(describeError({ type: 'ValidationError', message: 'Doğrulama.' })).toBe('Doğrulama.');
    expect(describeError(new Map([['message', 'Harita mesajı.']]))).toBe('Harita mesajı.');
  });
});
