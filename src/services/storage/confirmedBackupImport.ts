export type BackupFileReader = (file: File) => Promise<string>;

/** Reads and imports a backup only after the caller has obtained confirmation. */
export async function importConfirmedBackupFile(
  file: File,
  confirmed: boolean,
  onImport: (json: string) => void | Promise<void>,
  readFile: BackupFileReader = (selectedFile) => selectedFile.text()
): Promise<boolean> {
  if (!confirmed) return false;
  const json = await readFile(file);
  await onImport(json);
  return true;
}
