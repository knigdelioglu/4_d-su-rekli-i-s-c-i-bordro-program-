import { describe, expect, test } from 'bun:test';
import {
  formatPayrollError,
  formatStalePayrollMessage,
  isPayrollTaxOpeningConfigurationError,
  classifyBatchAttendance,
  findFirstStalePriorEvent,
  isStaleChainError,
  summarizeBatchPayrollOutcomes,
  tryAcquireSupplementaryPaymentScope,
  STALE_CHAIN_HINT,
} from './useBordroCalculationController';
import { paymentEventSequenceScopeKey } from '../services/payrollEngine/paymentEventOrder';
import { getPeriodDaysList } from '../utils/payrollPresentation';
import type { BordroDonemi, BordroKaydi, PersonelPuantaj, PuantajKodu } from '../types/payroll';

const leapPeriod = {
  baslangicTarihi: '2024-02-15',
  bitisTarihi: '2024-03-14',
};

function fullPeriodAttendance(): PersonelPuantaj {
  const gunler: Record<string, PuantajKodu> = Object.fromEntries(
    getPeriodDaysList(leapPeriod.baslangicTarihi, leapPeriod.bitisTarihi).map((day) => [
      day.dateStr,
      'Ç' as PuantajKodu,
    ])
  );
  return { id: 'person-1_2024-02', personelId: 'person-1', donemId: '2024-02', gunler };
}

describe('batch payroll input classification', () => {
  test('distinguishes no attendance from a present but incomplete calendar', () => {
    expect(classifyBatchAttendance(undefined, leapPeriod)).toBe('missing');
    expect(
      classifyBatchAttendance(
        { id: 'person-1_2024-02', personelId: 'person-1', donemId: '2024-02', gunler: {} },
        leapPeriod
      )
    ).toBe('incomplete');
  });

  test('uses the period calendar dates for full and partial leap-period coverage', () => {
    const attendance = fullPeriodAttendance();
    expect(Object.keys(attendance.gunler).length).toBe(29);
    expect(attendance.gunler['2024-02-29']).toBe('Ç');
    expect(classifyBatchAttendance(attendance, leapPeriod)).toBe('complete');

    delete attendance.gunler['2024-02-29'];
    expect(classifyBatchAttendance(attendance, leapPeriod)).toBe('incomplete');
  });

  test('batch outcome categories account for every personnel exactly once', () => {
    const summary = summarizeBatchPayrollOutcomes([
      'success',
      'success',
      'finalized-skipped',
      'attendance-missing',
      'attendance-incomplete',
      'not-applicable',
      'calculation-error',
    ]);

    expect(summary).toEqual({
      total: 7,
      success: 2,
      finalizedSkipped: 1,
      attendanceMissing: 1,
      attendanceIncomplete: 1,
      notApplicable: 1,
      calculationErrors: 1,
    });
    expect(
      summary.success +
        summary.finalizedSkipped +
        summary.attendanceMissing +
        summary.attendanceIncomplete +
        summary.notApplicable +
        summary.calculationErrors
    ).toBe(summary.total);
  });
});

describe('payroll error messages', () => {
  test('explains that stale payroll source data changed without blaming a prior period', () => {
    const message = formatStalePayrollMessage('Ayşe Kaya');

    expect(message).toContain('Ayşe Kaya bordrosu kaynak verilerindeki değişiklik nedeniyle güncelliğini yitirdi');
    expect(message).toContain('yeniden hesaplayın');
    expect(message).not.toContain('önceki dönem değişikliği');
  });

  test('turns missing annual parameters into an actionable message', () => {
    expect(
      formatPayrollError(
        new Error('2026 vergi yılı yıllık bordro parametreleri bulunamadı; bordro hesaplanamaz.')
      )
    ).toBe('2026 yılı gelir vergisi tarifesi henüz tanımlı değil. Yıllık Parametreler bölümünü tamamlayın.');
  });

  test('turns missing statutory parameters into an actionable message', () => {
    expect(
      formatPayrollError(new Error('2026 dönemi için zorunlu yasal parametre eksik: gunlukAsgariUcret.'))
    ).toBe('Dönem yasal parametreleri henüz tamamlanmamış. Dönem Parametreleri bölümünü tamamlayın.');
  });

  test('formats structured Tauri domain errors instead of showing object identity', () => {
    expect(
      formatPayrollError({
        type: 'ValidationError',
        message: 'p-1_2026-09 tahakkuku STALE durumda; önceki zincir çözümlenemiyor.',
      })
    ).toBe(`p-1_2026-09 tahakkuku STALE durumda; önceki zincir çözümlenemiyor. ${STALE_CHAIN_HINT}`);
  });

  test('formats attendance save errors safely across native, Error, and string variants', () => {
    expect(
      formatPayrollError({
        type: 'PayrollFinalized',
        message: 'Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez.',
      })
    ).toBe('Kesinleştirilmiş bordro tarihçesini etkileyen veri değiştirilemez.');
    expect(formatPayrollError(new Error('Kayıt reddedildi.'))).toBe('Kayıt reddedildi.');
    expect(formatPayrollError('Kayıt reddedildi.')).toBe('Kayıt reddedildi.');
  });

  test('does not expose object identity for an unprintable error object', () => {
    const error: Record<string, unknown> = {};
    error.self = error;

    expect(formatPayrollError(error)).toBe('Beklenmeyen bir hata oluştu.');
    expect(formatPayrollError(error).includes('[object Object]')).toBe(false);
  });

  test('formats a structured retro preview failure as its readable message', () => {
    const error = {
      type: 'ValidationError',
      message: '2027-01 dönemindeki tahakkuk authoritative değil; retro hesap durduruldu.',
    };

    expect(formatPayrollError(error)).toBe(error.message);
    expect(formatPayrollError(error)).not.toBe(String(error));
  });

  test('turns an unresolved legacy tax opening into an actionable message', () => {
    const message = formatPayrollError(
      new Error(
        'Legacy GV opening için legacy başlangıç ayı period ID ile çözülemiyor; explicit effectiveFromPeriodId girin.'
      )
    );

    expect(message).toBe(
      'Kümülatif gelir vergisi açılışının başlangıç dönemi eksik veya geçersiz. Bordro ekranındaki "Önceki Kümülatif Matrah Girişi" bölümünü açıp tutarı aktif dönemle kaydedin; ardından bordroyu yeniden hesaplayın.'
    );
    expect(isPayrollTaxOpeningConfigurationError(message)).toBe(false);
    expect(
      isPayrollTaxOpeningConfigurationError(
        'Hesaplama hatası: Legacy GV opening için explicit effectiveFromPeriodId girin.'
      )
    ).toBe(true);
    expect(
      isPayrollTaxOpeningConfigurationError(
        'Mevcut normal GV opening effective dönemi eksik; kayıt güvenli biçimde güncellenemiyor.'
      )
    ).toBe(true);
  });
});

test('does not allocate one payment sequence scope to overlapping submissions', () => {
  const pending = new Set<string>();
  const scope = paymentEventSequenceScopeKey('person-1', 2026, 8, '2026-08-10');
  const release = tryAcquireSupplementaryPaymentScope(pending, scope);

  expect(release !== null).toBe(true);
  expect(tryAcquireSupplementaryPaymentScope(pending, scope)).toBe(null);
  release!();
  release!();
  const releaseAfterRetry = tryAcquireSupplementaryPaymentScope(pending, scope);
  expect(releaseAfterRetry !== null).toBe(true);
  releaseAfterRetry!();
});

describe('stale payment-event chain replay selection', () => {
  const period = (id: string, taxYear: number, taxMonth: number): BordroDonemi =>
    ({ id, donemAdi: id, yil: taxYear, ay: taxMonth, taxYear, taxMonth,
      baslangicTarihi: `${id}-01`, bitisTarihi: `${id}-28` }) as unknown as BordroDonemi;
  const periods = new Map<string, BordroDonemi>([
    ['2026-01', period('2026-01', 2026, 1)],
    ['2026-02', period('2026-02', 2026, 2)],
    ['2026-03', period('2026-03', 2026, 3)],
    ['2026-05', period('2026-05', 2026, 5)],
  ]);
  const payroll = (personelId: string, donemId: string, status: BordroKaydi['status']): BordroKaydi =>
    ({ id: `${personelId}_${donemId}`, accrualId: `${personelId}_${donemId}`, personelId, donemId,
      accrualType: 'NORMAL', paymentDate: `${donemId}-28`, sequence: 0, status }) as unknown as BordroKaydi;
  const target = { paymentDate: '2026-05-28', sequence: 0, accrualId: 'p-1_2026-05', id: 'p-1_2026-05' };

  test('returns the earliest prior STALE event of the same person (p-1_2026-01 before May)', () => {
    const found = findFirstStalePriorEvent(
      [
        payroll('p-1', '2026-03', 'STALE'),
        payroll('p-1', '2026-01', 'STALE'),
        payroll('p-1', '2026-02', 'CALCULATED'),
        payroll('p-2', '2025-12', 'STALE'),
      ],
      periods,
      'p-1',
      target,
      periods.get('2026-05')!
    );
    expect(found?.payroll.accrualId).toBe('p-1_2026-01');
  });

  test('ignores the target itself, later events, FINALIZED and CALCULATED history', () => {
    const found = findFirstStalePriorEvent(
      [
        payroll('p-1', '2026-01', 'FINALIZED'),
        payroll('p-1', '2026-02', 'CALCULATED'),
        payroll('p-1', '2026-05', 'STALE'),
      ],
      periods,
      'p-1',
      target,
      periods.get('2026-05')!
    );
    expect(found).toBe(null);
  });

  test('stale-chain engine errors get an actionable hint', () => {
    const message = formatPayrollError({
      type: 'ValidationError',
      message: 'Payment-event/PEK zinciri çözülemez: p-1_2026-01 tahakkuku STALE durumda; authoritative state belirlenemiyor.',
    });
    expect(isStaleChainError(message)).toBe(true);
    expect(message.includes('otomatik yeniden hesaplanır')).toBe(true);
  });
});
