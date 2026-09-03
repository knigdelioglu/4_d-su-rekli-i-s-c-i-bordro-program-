import { expect, test, type Page } from '@playwright/test';

type StoredPayroll = {
  id: string;
  personelId: string;
  donemId: string;
  status: string;
  gelirToplam: string;
  kesintiToplam: string;
  netOdeme: string;
};

type StoredPeriod = {
  id: string;
  yil: number;
  ay: number;
  taxYear: number;
  taxMonth: number;
};

type StoredAnnualPayrollParameters = {
  year: number;
  sigortaGvYillikBrutAsgariUcretTavani?: string;
};

type StoredSnapshot = {
  bordrolar: StoredPayroll[];
  puantajlar: Array<{ personelId: string; donemId: string; gunler: Record<string, string> }>;
  donemler?: StoredPeriod[];
  annualPayrollParameters?: StoredAnnualPayrollParameters[];
};

const databaseName = '4d-bordro-programi';
const objectStoreName = 'snapshots';

async function readStoredSnapshot(page: Page): Promise<StoredSnapshot | null> {
  return page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName }) =>
      new Promise<StoredSnapshot | null>((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          try {
            const request = database
              .transaction(storeName, 'readonly')
              .objectStore(storeName)
              .get('current');
            request.onerror = () => {
              database.close();
              reject(request.error ?? new Error('Snapshot okunamadı.'));
            };
            request.onsuccess = () => {
              const payload = request.result;
              database.close();
              resolve(typeof payload === 'string' ? (JSON.parse(payload) as StoredSnapshot) : null);
            };
          } catch (error) {
            database.close();
            reject(error);
          }
        };
      }),
    { databaseName, objectStoreName }
  );
}

async function installCalculableFixture(page: Page): Promise<void> {
  await page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName }) =>
      new Promise<void>((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const transaction = database.transaction(storeName, 'readwrite');
          const objectStore = transaction.objectStore(storeName);
          const readRequest = objectStore.get('current');
          readRequest.onerror = () => reject(readRequest.error ?? new Error('Fixture okunamadı.'));
          readRequest.onsuccess = () => {
            try {
              const snapshot = JSON.parse(readRequest.result as string) as StoredSnapshot;
              if (!snapshot.donemler || !snapshot.annualPayrollParameters) {
                throw new Error('Örnek fixture dönem/yıllık parametre içermiyor.');
              }

              // The production sample uses the end-month tax convention. For a
              // self-contained browser fixture, map the first eight periods to
              // tax months 1..8 so the shared Rust reference chain is complete.
              snapshot.donemler = snapshot.donemler.map((period) => ({
                ...period,
                taxYear: period.yil,
                taxMonth: period.ay,
              }));
              const activeYear = snapshot.donemler[0]?.taxYear;
              const annual = snapshot.annualPayrollParameters.find(
                (parameters) => parameters.year === activeYear
              );
              if (!annual) throw new Error('Örnek fixture yıllık vergi parametresi içermiyor.');
              annual.sigortaGvYillikBrutAsgariUcretTavani = '396360';

              const writeRequest = objectStore.put(JSON.stringify(snapshot), 'current');
              writeRequest.onerror = () =>
                reject(writeRequest.error ?? new Error('Fixture yazılamadı.'));
            } catch (error) {
              reject(error);
            }
          };
          transaction.oncomplete = () => {
            database.close();
            resolve();
          };
          transaction.onerror = () =>
            reject(transaction.error ?? new Error('Fixture transaction başarısız.'));
        };
      }),
    { databaseName, objectStoreName }
  );
  await page.reload();
  await expect(page.getByText(/4\/D Personel Kayıtları \(5\)/)).toBeVisible();
}

async function waitForPayrollStatus(page: Page, periodId: string, status: string): Promise<StoredPayroll> {
  await expect
    .poll(async () => {
      const snapshot = await readStoredSnapshot(page);
      return snapshot?.bordrolar.find(
        (payroll) => payroll.personelId === 'p-1' && payroll.donemId === periodId
      )?.status;
    })
    .toBe(status);

  const snapshot = await readStoredSnapshot(page);
  const payroll = snapshot?.bordrolar.find(
    (item) => item.personelId === 'p-1' && item.donemId === periodId
  );
  expect(payroll).toBeDefined();
  return payroll!;
}

async function loadSampleDataset(page: Page): Promise<void> {
  const initialSnapshot = await readStoredSnapshot(page);
  expect(initialSnapshot?.bordrolar ?? []).toHaveLength(0);

  const wasmResponse = page.waitForResponse(
    (response) => /\.wasm(?:\?|$)/.test(response.url()),
    { timeout: 30_000 }
  );
  const dialogHandled = page.waitForEvent('dialog').then(async (dialog) => {
    expect(dialog.type()).toBe('confirm');
    await dialog.accept();
  });

  await Promise.all([
    page.getByTitle('Örnek Veriyi Yeniden Yükle').click(),
    dialogHandled,
  ]);

  const response = await wasmResponse;
  expect(response.status()).toBe(200);
  expect(response.headers()['content-type']).toContain('application/wasm');
  await expect(page.getByText(/4\/D Personel Kayıtları \(5\)/)).toBeVisible();
  await installCalculableFixture(page);
}

async function openPayrollScreen(page: Page): Promise<string> {
  await page.getByRole('button', { name: '3. Bordro Hesaplama' }).click();
  const screen = page.getByTestId('payroll-screen');
  await expect(screen).toBeVisible();
  await expect(screen).toHaveAttribute('data-payroll-engine-kind', 'wasm');
  return (await screen.getAttribute('data-period-id'))!;
}

async function calculateP1(page: Page, periodId: string): Promise<StoredPayroll> {
  await page.getByTestId('calculate-payroll-p-1').click();
  await expect(page.getByText(/Ahmet Yılmaz bordrosu başarıyla hesaplandı\./)).toBeVisible();
  return waitForPayrollStatus(page, periodId, 'CALCULATED');
}

async function selectPeriod(page: Page, periodId: string): Promise<void> {
  await page.getByTestId('active-period-selector').selectOption(periodId);
  await expect(page.getByTestId('payroll-screen')).toHaveAttribute('data-period-id', periodId);
}

test('browser WASM calculation persists in IndexedDB and survives reload', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('4/D Personel Kayıtları (0)')).toBeVisible();
  await loadSampleDataset(page);

  const periodId = await openPayrollScreen(page);
  const payroll = await calculateP1(page, periodId);
  expect(payroll).toMatchObject({
    personelId: 'p-1',
    donemId: periodId,
    status: 'CALCULATED',
  });
  expect(typeof payroll.gelirToplam).toBe('string');
  expect(typeof payroll.kesintiToplam).toBe('string');
  expect(typeof payroll.netOdeme).toBe('string');

  await page.reload();
  await expect(page.getByTestId('payroll-screen')).toBeVisible();
  const reloaded = await waitForPayrollStatus(page, periodId, 'CALCULATED');
  expect(reloaded).toEqual(payroll);
});

test('browser finalization uses WASM, persists FINALIZED, and rejects a finalized mutation', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('4/D Personel Kayıtları (0)')).toBeVisible();
  await loadSampleDataset(page);

  const periodId = await openPayrollScreen(page);
  await calculateP1(page, periodId);

  const payrollRow = page.locator('tbody tr').filter({ hasText: 'Ahmet Yılmaz' }).first();
  await payrollRow.getByRole('button', { name: 'Kesinleştir' }).click();
  await expect(page.getByRole('heading', { name: 'Bordroyu Kesinleştir' })).toBeVisible();
  await expect(page.getByText('Kesinleştirmeye hazır')).toBeVisible();
  await page.getByRole('button', { name: 'Kesinleştir ve Kilitle' }).click();
  await expect(payrollRow).toContainText('Kesinleştirildi');

  const finalized = await waitForPayrollStatus(page, periodId, 'FINALIZED');
  expect(finalized.personelId).toBe('p-1');
  expect(finalized.donemId).toBe(periodId);

  await page.reload();
  await expect(page.getByTestId('payroll-screen')).toBeVisible();
  await waitForPayrollStatus(page, periodId, 'FINALIZED');

  await page.getByRole('button', { name: '2. Puantaj Cetveli' }).click();
  await expect(page.getByText(/Puantaj Özeti/)).toBeVisible();
  let dialogMessage: string | undefined;
  page.on('dialog', async (dialog) => {
    dialogMessage = dialog.message();
    await dialog.accept();
  });
  await page.locator('[data-testid^="attendance-day-"]').first().click();
  await page.waitForTimeout(1_000);
  expect(dialogMessage).toContain('Kesinleştirilmiş');

  const afterRejectedMutation = await waitForPayrollStatus(page, periodId, 'FINALIZED');
  expect(afterRejectedMutation.status).toBe('FINALIZED');
});

test('browser source mutation marks downstream calculated payrolls STALE and persists the state', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('4/D Personel Kayıtları (0)')).toBeVisible();
  await loadSampleDataset(page);

  const activePeriodId = await openPayrollScreen(page);
  const sampleYear = activePeriodId.slice(0, 4);
  const previousPeriodId = `${sampleYear}-06`;
  const currentPeriodId = `${sampleYear}-07`;

  await selectPeriod(page, previousPeriodId);
  await calculateP1(page, previousPeriodId);
  await selectPeriod(page, currentPeriodId);
  await calculateP1(page, currentPeriodId);

  await selectPeriod(page, previousPeriodId);
  await page.getByRole('button', { name: '2. Puantaj Cetveli' }).click();
  await expect(page.getByText(/Puantaj Özeti/)).toBeVisible();
  await page.locator('[data-testid^="attendance-day-"]').first().click();

  await expect
    .poll(async () => {
      const snapshot = await readStoredSnapshot(page);
      return (snapshot?.bordrolar ?? [])
        .filter((item) => item.personelId === 'p-1')
        .filter((item) => [previousPeriodId, currentPeriodId].includes(item.donemId))
        .sort((a, b) => a.donemId.localeCompare(b.donemId))
        .map((item) => item.status);
    })
    .toEqual(['STALE', 'STALE']);

  await page.reload();
  const persisted = await readStoredSnapshot(page);
  expect(
    persisted?.bordrolar
      .filter((item) => item.personelId === 'p-1')
      .filter((item) => [previousPeriodId, currentPeriodId].includes(item.donemId))
      .sort((a, b) => a.donemId.localeCompare(b.donemId))
      .map((item) => item.status)
  ).toEqual(['STALE', 'STALE']);
});
