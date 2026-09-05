import { describe, expect, test } from 'bun:test';
import { getPeriodSummaryNextTask, type PeriodSummaryDecisionState } from './periodSummaryLogic';

const readyState: PeriodSummaryDecisionState = {
  periodParametersReady: true,
  annualParametersReady: true,
  missingAttendanceCount: 0,
  staleNormalCount: 0,
  missingNormalCount: 0,
  missingTis: 0,
  missingTediye: 0,
  finalizedPending: 0,
};

function state(overrides: Partial<PeriodSummaryDecisionState>): PeriodSummaryDecisionState {
  return { ...readyState, ...overrides };
}

describe('Dönem özeti sonraki iş kararı', () => {
  test('period institution parameters take priority and open Ücretler', () => {
    expect(
      getPeriodSummaryNextTask(state({ periodParametersReady: false }))
    ).toEqual({
      label: 'Dönem ücret parametrelerini tamamla',
      tab: 'parametrelar',
      parametreSection: 'gelir',
    });
  });

  test('missing annual parameters open the annual tax section', () => {
    expect(getPeriodSummaryNextTask(state({ annualParametersReady: false }))).toEqual({
      label: 'Yıllık vergi tarifesini tamamla',
      tab: 'parametrelar',
      parametreSection: 'annualTax',
    });
  });

  test('stale normal payrolls take priority over never-calculated payrolls', () => {
    expect(
      getPeriodSummaryNextTask(state({ staleNormalCount: 1, missingNormalCount: 0 }))
    ).toEqual({
      label: 'Yeniden hesaplanması gereken bordrolara git',
      tab: 'bordro',
      payrollView: 'normal',
    });
  });

  test('never-calculated normal payrolls remain a separate next task', () => {
    expect(
      getPeriodSummaryNextTask(state({ staleNormalCount: 0, missingNormalCount: 1 }))
    ).toEqual({ label: 'Normal maaşları hesapla', tab: 'bordro', payrollView: 'normal' });
  });

  test('returns no task only when all period work is complete', () => {
    expect(getPeriodSummaryNextTask(readyState)).toBe(null);
  });
});
