import { expect, test, describe } from 'bun:test';
import { tauriBridge } from './tauriBridge';
import {
  ManualPayrollIncomeInput,
  PayrollAccrualInput,
  PersonelPuantaj,
  PersonelTaxOpening,
} from '../types/payroll';

/**
 * Regression testi: tauriBridge IPC arg anahtarları, Tauri command wrapper'ın
 * varsayılan camelCase anahtarlarıyla birebir eşleşmelidir. Eşleşmezlerse Tauri runtime invoke'u
 * deserialize edemez ve command hiç çalışmaz (örn. calculate_payroll için
 * "kayıtlı puantaj bulunamadı" hatası UI'da görünür ama DB'de puantaj vardır).
 */
describe('tauriBridge IPC arg anahtarları Tauri camelCase parametreleriyle eşleşmeli', () => {
  const installMock = (result: unknown = {}) => {
    let capturedCmd: string | null = null;
    let capturedArgs: Record<string, any> | null = null;

    (globalThis as any).window = {
      __TAURI_INTERNALS__: {
        invoke: async (cmd: string, args: Record<string, any>) => {
          capturedCmd = cmd;
          capturedArgs = args;
          return result;
        },
      },
    };

    return {
      cmd: () => capturedCmd,
      args: () => capturedArgs,
    };
  };

  test('calculatePayroll: manuel gelir verilmezse manualIncome=null göndermeli', async () => {
    const mock = installMock();
    await tauriBridge.calculatePayroll('p-1', '2026-05');
    expect(mock.cmd()).toBe('calculate_payroll');
    expect(mock.args()).toEqual({
      personnelId: 'p-1',
      periodId: '2026-05',
      manualIncome: null,
      accrual: null,
    });
  });

  test('calculatePayroll: manuel Tediye/TİS değerlerini değiştirmeden göndermeli', async () => {
    const mock = installMock();
    const manualIncome: ManualPayrollIncomeInput = {
      tediye: 1000.25,
      tisIkramiyesi: 2000.75,
    };
    await tauriBridge.calculatePayroll('p-1', '2026-05', manualIncome);
    expect(mock.cmd()).toBe('calculate_payroll');
    expect(mock.args()).toEqual({
      personnelId: 'p-1',
      periodId: '2026-05',
      manualIncome: {
        tediye: '1000.25',
        tisIkramiyesi: '2000.75',
      },
      accrual: null,
    });
  });

  test('calculatePayroll: explicit NORMAL ödeme tarihini exact accrual olarak göndermeli', async () => {
    const mock = installMock();
    const accrual: PayrollAccrualInput = {
      accrualId: 'p-1_2026-05',
      accrualType: 'NORMAL',
      paymentDate: '2026-06-13',
      sequence: 0,
      grossAmount: null,
      description: 'Normal maaş',
    };
    await tauriBridge.calculatePayroll('p-1', '2026-05', undefined, accrual);
    expect(mock.args()).toEqual({
      personnelId: 'p-1',
      periodId: '2026-05',
      manualIncome: null,
      accrual: {
        accrualId: 'p-1_2026-05',
        accrualType: 'NORMAL',
        paymentDate: '2026-06-13',
        sequence: 0,
        grossAmount: null,
        description: 'Normal maaş',
      },
    });
  });

  test('setPayrollStatus: personnelId / periodId / status göndermeli', async () => {
    const mock = installMock();
    await tauriBridge.setPayrollStatus('p-1', '2026-05', 'FINALIZED');
    expect(mock.cmd()).toBe('set_payroll_status');
    expect(mock.args()).toEqual({
      personnelId: 'p-1',
      periodId: '2026-05',
      status: 'FINALIZED',
      accrualId: null,
    });
  });

  test('saveTaxOpening: taxOpening anahtarıyla göndermeli (Rust: tax_opening)', async () => {
    const mock = installMock();
    const opening: PersonelTaxOpening = {
      id: 'p-1_2026',
      personnelId: 'p-1',
      year: 2026,
      gvCumulativeOpening: 120000,
      effectiveFromPeriodId: '2026-05',
    };
    await tauriBridge.saveTaxOpening(opening);
    expect(mock.cmd()).toBe('save_tax_opening');
    expect(mock.args()).toEqual({
      taxOpening: { ...opening, gvCumulativeOpening: '120000' },
    });
  });

  test('migrateLegacyPayload: payloadJson anahtarıyla göndermeli (Rust: payload_json)', async () => {
    const mock = installMock();
    const payload = '{"donemler":[],"personeller":[]}';
    await tauriBridge.migrateLegacyPayload(payload);
    expect(mock.cmd()).toBe('migrate_legacy_payload');
    expect(mock.args()).toEqual({ payloadJson: payload });
  });

  test('exportBackup: native file save command should receive the serialized JSON and suggested name', async () => {
    const mock = installMock(true);
    const saved = await tauriBridge.exportBackup('{"backupVersion":5}', 'backup.json');
    expect(saved).toBe(true);
    expect(mock.cmd()).toBe('export_backup');
    expect(mock.args()).toEqual({
      payloadJson: '{"backupVersion":5}',
      fileName: 'backup.json',
    });
  });

  test('exportPdf: native file save command should receive unchanged bytes and suggested name', async () => {
    const mock = installMock(true);
    const pdfBytes = [0x25, 0x50, 0x44, 0x46, 0xff];
    const saved = await tauriBridge.exportPdf(pdfBytes, 'bordro.pdf');
    expect(saved).toBe(true);
    expect(mock.cmd()).toBe('export_pdf');
    expect(mock.args()).toEqual({ pdfBytes, fileName: 'bordro.pdf' });
  });

  test('exportPdf: native save cancellation is returned as false', async () => {
    const mock = installMock(false);
    expect(await tauriBridge.exportPdf([0x25, 0x50, 0x44, 0x46], 'bordro.pdf')).toBe(false);
    expect(mock.cmd()).toBe('export_pdf');
  });

  test('saveAttendance: attendance anahtarı değişmemeli (kaydet zinciri)', async () => {
    const mock = installMock();
    const puantaj: PersonelPuantaj = {
      id: 'p-1_2026-05',
      personelId: 'p-1',
      donemId: '2026-05',
      gunler: { '2026-05-15': 'Ç' },
    };
    await tauriBridge.saveAttendance(puantaj);
    expect(mock.cmd()).toBe('save_attendance');
    expect(mock.args()).toEqual({ attendance: puantaj });
  });

  test('getSickLeaveRecords: personnelId anahtarıyla göndermeli (Rust: personnel_id)', async () => {
    const mock = installMock();
    await tauriBridge.getSickLeaveRecords('p-1');
    expect(mock.cmd()).toBe('get_sick_leave_records');
    expect(mock.args()).toEqual({ personnelId: 'p-1' });
  });
});
