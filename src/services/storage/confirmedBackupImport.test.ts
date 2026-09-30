import { describe, expect, test } from 'bun:test';
import { importConfirmedBackupFile } from './confirmedBackupImport';

describe('confirmed backup file import', () => {
  test('cancel does not read the file or invoke import', async () => {
    let readCalled = false;
    let importCalled = false;
    const file = new File(['{}'], 'backup.json');

    const imported = await importConfirmedBackupFile(
      file,
      false,
      () => { importCalled = true; },
      async () => { readCalled = true; return '{}'; }
    );

    expect(imported).toBe(false);
    expect(readCalled).toBe(false);
    expect(importCalled).toBe(false);
  });

  test('confirmation reads the selected file and waits for import completion', async () => {
    let importedJson: string | null = null;
    let importCompleted = false;
    const file = new File(['backup-content'], 'backup.json');

    const imported = await importConfirmedBackupFile(
      file,
      true,
      async (json) => {
        importedJson = json;
        await Promise.resolve();
        importCompleted = true;
      }
    );

    expect(imported).toBe(true);
    expect(importedJson).toBe('backup-content');
    expect(importCompleted).toBe(true);
  });
});
