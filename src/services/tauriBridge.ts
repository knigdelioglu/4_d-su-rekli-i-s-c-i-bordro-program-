import {
  BordroDonemi,
  BordroKaydi,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
} from '../types/payroll';

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
    return invokeTauri<void>('save_personnel', { personel });
  },

  async deletePersonnel(id: string): Promise<void> {
    return invokeTauri<void>('delete_personnel', { id });
  },

  async getTaxOpenings(): Promise<PersonelTaxOpening[]> {
    return invokeTauri<PersonelTaxOpening[]>('get_tax_openings');
  },

  async saveTaxOpening(taxOpening: PersonelTaxOpening): Promise<void> {
    return invokeTauri<void>('save_tax_opening', { taxOpening });
  },

  async getPeriods(): Promise<BordroDonemi[]> {
    return invokeTauri<BordroDonemi[]>('get_periods');
  },

  async savePeriod(period: BordroDonemi): Promise<void> {
    return invokeTauri<void>('save_period', { period });
  },

  async getAttendanceList(): Promise<PersonelPuantaj[]> {
    return invokeTauri<PersonelPuantaj[]>('get_attendance_list');
  },

  async saveAttendance(attendance: PersonelPuantaj): Promise<void> {
    return invokeTauri<void>('save_attendance', { attendance });
  },

  async getPayrollList(): Promise<BordroKaydi[]> {
    return invokeTauri<BordroKaydi[]>('get_payroll_list');
  },

  async calculatePayroll(personnelId: string, periodId: string): Promise<BordroKaydi> {
    return invokeTauri<BordroKaydi>('calculate_payroll', { personnelId, periodId });
  },

  async setPayrollStatus(personnelId: string, periodId: string, status: string): Promise<void> {
    return invokeTauri<void>('set_payroll_status', { personnelId, periodId, status });
  },

  async getInstitutionSettings(): Promise<Record<string, DönemselKurumDegerleri>> {
    return invokeTauri<Record<string, DönemselKurumDegerleri>>('get_institution_settings');
  },

  async saveInstitutionSettings(settings: DönemselKurumDegerleri): Promise<void> {
    return invokeTauri<void>('save_institution_settings', { settings });
  },

  async getAppSetting(key: string): Promise<string | null> {
    return invokeTauri<string | null>('get_app_setting', { key });
  },

  async setAppSetting(key: string, value: string): Promise<void> {
    return invokeTauri<void>('set_app_setting', { key, value });
  },

  async checkLegacyMigrated(): Promise<boolean> {
    return invokeTauri<boolean>('check_legacy_migrated');
  },

  async migrateLegacyPayload(payloadJson: string): Promise<void> {
    return invokeTauri<void>('migrate_legacy_payload', { payloadJson });
  },
};
