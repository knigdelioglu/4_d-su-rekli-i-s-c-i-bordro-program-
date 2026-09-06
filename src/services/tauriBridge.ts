import {
  BordroDonemi,
  BordroKaydi,
  BordroStatus,
  DönemselKurumDegerleri,
  AnnualPayrollParameters,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  SickLeaveRecord,
  ManualPayrollIncomeInput,
  PayrollAccrualInput,
  CompensationRevision,
  CompensationRevisionOverride,
  RetroAdjustmentBatch,
  RetroAllocation,
} from '../types/payroll';
import type { RetroCalculationRequest, RetroCalculationResult } from './payrollEngine/types';
import { PayrollNotice } from '../types/payrollNotice';
import { decodeDecimalValues, encodeDecimalValues } from './payrollEngine/decimalBoundary';
import type { MutationImpact, PayrollMutation } from './payrollEngine/types';

// Type-safe IPC invoke helper with window fallback detection
async function invokeTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const win = typeof window !== 'undefined' ? (window as any) : null;
  if (win && win.__TAURI_INTERNALS__ && typeof win.__TAURI_INTERNALS__.invoke === 'function') {
    return decodeDecimalValues(
      await win.__TAURI_INTERNALS__.invoke(cmd, args)
    ) as T;
  }
  if (win && win.__TAURI__ && win.__TAURI__.core && typeof win.__TAURI__.core.invoke === 'function') {
    return decodeDecimalValues(await win.__TAURI__.core.invoke(cmd, args)) as T;
  }
  throw new Error(`Tauri IPC context not available for command "${cmd}".`);
}

function emitPayrollDataChanged(): void {
  if (
    typeof window !== 'undefined' &&
    typeof window.dispatchEvent === 'function' &&
    typeof CustomEvent === 'function'
  ) {
    window.dispatchEvent(new CustomEvent('payroll:data-changed'));
  }
}

async function mutateTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const result = await invokeTauri<T>(cmd, args);
  emitPayrollDataChanged();
  return result;
}

export const tauriBridge = {
  async deletePayrollAccrual(personnelId: string, periodId: string, accrualId: string): Promise<void> {
    return mutateTauri<void>('delete_payroll_accrual', { personnelId, periodId, accrualId });
  },
  isTauriAvailable(): boolean {
    const win = typeof window !== 'undefined' ? (window as any) : null;
    return Boolean(
      win &&
        ((win.__TAURI_INTERNALS__ && typeof win.__TAURI_INTERNALS__.invoke === 'function') ||
          (win.__TAURI__ && win.__TAURI__.core && typeof win.__TAURI__.core.invoke === 'function'))
    );
  },

  async getPersonnelList(): Promise<Personel[]> {
    return invokeTauri<Personel[]>('get_personnel_list');
  },

  async savePersonnel(personel: Personel): Promise<void> {
    return mutateTauri<void>('save_personnel', { personel: encodeDecimalValues(personel) });
  },

  async deletePersonnel(id: string): Promise<void> {
    return mutateTauri<void>('delete_personnel', { id });
  },

  async getTaxOpenings(): Promise<PersonelTaxOpening[]> {
    return invokeTauri<PersonelTaxOpening[]>('get_tax_openings');
  },

  async saveTaxOpening(taxOpening: PersonelTaxOpening): Promise<void> {
    return mutateTauri<void>('save_tax_opening', { taxOpening: encodeDecimalValues(taxOpening) });
  },

  async getPeriods(): Promise<BordroDonemi[]> {
    return invokeTauri<BordroDonemi[]>('get_periods');
  },

  async savePeriod(period: BordroDonemi): Promise<void> {
    return mutateTauri<void>('save_period', { period });
  },

  async savePeriodWithSettings(
    period: BordroDonemi,
    settings: DönemselKurumDegerleri
  ): Promise<void> {
    return mutateTauri<void>('save_period_with_settings', {
      period,
      settings: encodeDecimalValues(settings),
    });
  },

  async getAttendanceList(): Promise<PersonelPuantaj[]> {
    return invokeTauri<PersonelPuantaj[]>('get_attendance_list');
  },

  async saveAttendance(attendance: PersonelPuantaj): Promise<void> {
    return mutateTauri<void>('save_attendance', { attendance });
  },

  async getPayrollList(): Promise<BordroKaydi[]> {
    return invokeTauri<BordroKaydi[]>('get_payroll_list');
  },

  async getPayrollNotices(periodId: string): Promise<PayrollNotice[]> {
    return invokeTauri<PayrollNotice[]>('get_payroll_notices', { periodId });
  },

  async calculatePayroll(
    personnelId: string,
    periodId: string,
    manualIncome?: ManualPayrollIncomeInput,
    accrual?: PayrollAccrualInput
  ): Promise<BordroKaydi> {
    return mutateTauri<BordroKaydi>('calculate_payroll', {
      personnelId,
      periodId,
      manualIncome: manualIncome ? encodeDecimalValues(manualIncome) : null,
      accrual: accrual ? encodeDecimalValues(accrual) : null,
    });
  },

  async finalizePayroll(
    personnelId: string,
    periodId: string,
    accrualId?: string
  ): Promise<BordroKaydi> {
    return mutateTauri<BordroKaydi>('finalize_payroll', { personnelId, periodId, accrualId: accrualId ?? null });
  },

  async evaluateMutationPolicy(
    mutation: PayrollMutation,
    dataset: unknown
  ): Promise<MutationImpact> {
    // The native command reads its own SQLite snapshot. The dataset argument
    // keeps the cross-platform engine interface uniform and is intentionally
    // not sent across the native persistence boundary.
    void dataset;
    return invokeTauri<MutationImpact>('evaluate_mutation_policy', { mutation });
  },

  async setPayrollStatus(
    personnelId: string,
    periodId: string,
    status: BordroStatus,
    accrualId?: string
  ): Promise<void> {
    return mutateTauri<void>('set_payroll_status', {
      personnelId,
      periodId,
      status,
      accrualId: accrualId ?? null,
    });
  },

  async getInstitutionSettings(): Promise<Record<string, DönemselKurumDegerleri>> {
    return invokeTauri<Record<string, DönemselKurumDegerleri>>('get_institution_settings');
  },

  async saveInstitutionSettings(settings: DönemselKurumDegerleri): Promise<void> {
    return mutateTauri<void>('save_institution_settings', {
      settings: encodeDecimalValues(settings),
    });
  },

  async getAppSetting(key: string): Promise<string | null> {
    return invokeTauri<string | null>('get_app_setting', { key });
  },

  async setAppSetting(key: string, value: string): Promise<void> {
    return mutateTauri<void>('set_app_setting', { key, value });
  },

  async checkLegacyMigrated(): Promise<boolean> {
    return invokeTauri<boolean>('check_legacy_migrated');
  },

  async migrateLegacyPayload(payloadJson: string): Promise<void> {
    return mutateTauri<void>('migrate_legacy_payload', { payloadJson });
  },

  async replaceBackupPayload(payloadJson: string): Promise<void> {
    return mutateTauri<void>('replace_backup_payload', { payloadJson });
  },

  async getAnnualPayrollParameters(): Promise<AnnualPayrollParameters[]> {
    return invokeTauri<AnnualPayrollParameters[]>('get_annual_payroll_parameters');
  },

  async saveAnnualPayrollParameters(parameters: AnnualPayrollParameters): Promise<void> {
    return mutateTauri<void>('save_annual_payroll_parameters', {
      parameters: encodeDecimalValues(parameters),
    });
  },

  async getSickLeaveRecords(personnelId?: string): Promise<SickLeaveRecord[]> {
    return invokeTauri<SickLeaveRecord[]>('get_sick_leave_records', { personnelId: personnelId || null });
  },

  async saveSickLeaveRecord(record: SickLeaveRecord): Promise<void> {
    return mutateTauri<void>('save_sick_leave_record', { record });
  },

  async deleteSickLeaveRecord(id: string): Promise<void> {
    return mutateTauri<void>('delete_sick_leave_record', { id });
  },

  async getCompensationRevisions(): Promise<CompensationRevision[]> {
    return invokeTauri<CompensationRevision[]>('get_compensation_revisions');
  },

  async getCompensationRevisionOverrides(): Promise<CompensationRevisionOverride[]> {
    return invokeTauri<CompensationRevisionOverride[]>('get_compensation_revision_overrides');
  },

  async getRetroAdjustmentBatches(): Promise<RetroAdjustmentBatch[]> {
    return invokeTauri<RetroAdjustmentBatch[]>('get_retro_adjustment_batches');
  },

  async getRetroAdjustmentAllocations(): Promise<RetroAllocation[]> {
    return invokeTauri<RetroAllocation[]>('get_retro_adjustment_allocations');
  },

  async saveCompensationRevision(
    revision: CompensationRevision,
    overrides: CompensationRevisionOverride[]
  ): Promise<void> {
    return mutateTauri<void>('save_compensation_revision', {
      revision: encodeDecimalValues(revision),
      overrides: encodeDecimalValues(overrides),
    });
  },

  async calculateRetroPreview(request: RetroCalculationRequest): Promise<RetroCalculationResult> {
    return invokeTauri<RetroCalculationResult>('calculate_retro_preview', {
      batchId: request.batchId,
      revision: encodeDecimalValues(request.revision),
      overrides: encodeDecimalValues(request.overrides),
      personnelId: request.personnelId,
      paymentDate: request.paymentDate,
      calculatedAt: request.calculatedAt,
      description: request.description ?? null,
    });
  },

  async saveRetroAdjustmentBatch(
    batch: RetroAdjustmentBatch,
    allocations: RetroAllocation[]
  ): Promise<void> {
    return mutateTauri<void>('save_retro_adjustment_batch', {
      batch: encodeDecimalValues(batch),
      allocations: encodeDecimalValues(allocations),
    });
  },

  async createRetroPayment(
    batch: RetroAdjustmentBatch,
    allocations: RetroAllocation[],
    paymentPeriodId: string,
    sequence: number
  ): Promise<BordroKaydi> {
    return mutateTauri<BordroKaydi>('create_retro_payment', {
      batch: encodeDecimalValues(batch),
      allocations: encodeDecimalValues(allocations),
      paymentPeriodId,
      sequence,
    });
  },
};
