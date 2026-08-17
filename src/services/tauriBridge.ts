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
} from '../types/payroll';
import { PayrollNotice } from '../types/payrollNotice';

// Type-safe IPC invoke helper with window fallback detection
async function invokeTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const win = typeof window !== 'undefined' ? (window as any) : null;
  if (win && win.__TAURI_INTERNALS__ && typeof win.__TAURI_INTERNALS__.invoke === 'function') {
    return await win.__TAURI_INTERNALS__.invoke(cmd, args);
  }
  if (win && win.__TAURI__ && win.__TAURI__.core && typeof win.__TAURI__.core.invoke === 'function') {
    return await win.__TAURI__.core.invoke(cmd, args);
  }
  throw new Error(`Tauri IPC context not available for command "${cmd}".`);
}

function emitPayrollDataChanged(): void {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('payroll:data-changed'));
  }
}

async function mutateTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const result = await invokeTauri<T>(cmd, args);
  emitPayrollDataChanged();
  return result;
}

export const tauriBridge = {
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
    return mutateTauri<void>('save_personnel', { personel });
  },

  async deletePersonnel(id: string): Promise<void> {
    return mutateTauri<void>('delete_personnel', { id });
  },

  async getTaxOpenings(): Promise<PersonelTaxOpening[]> {
    return invokeTauri<PersonelTaxOpening[]>('get_tax_openings');
  },

  async saveTaxOpening(taxOpening: PersonelTaxOpening): Promise<void> {
    return mutateTauri<void>('save_tax_opening', { taxOpening });
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
    return mutateTauri<void>('save_period_with_settings', { period, settings });
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
    manualIncome?: ManualPayrollIncomeInput
  ): Promise<BordroKaydi> {
    return mutateTauri<BordroKaydi>('calculate_payroll', {
      personnelId,
      periodId,
      manualIncome: manualIncome ?? null,
    });
  },

  async setPayrollStatus(personnelId: string, periodId: string, status: BordroStatus): Promise<void> {
    return mutateTauri<void>('set_payroll_status', { personnelId, periodId, status });
  },

  async getInstitutionSettings(): Promise<Record<string, DönemselKurumDegerleri>> {
    return invokeTauri<Record<string, DönemselKurumDegerleri>>('get_institution_settings');
  },

  async saveInstitutionSettings(settings: DönemselKurumDegerleri): Promise<void> {
    return mutateTauri<void>('save_institution_settings', { settings });
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
    return mutateTauri<void>('save_annual_payroll_parameters', { parameters });
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
};
