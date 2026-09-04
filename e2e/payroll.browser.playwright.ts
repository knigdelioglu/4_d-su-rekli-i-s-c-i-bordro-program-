import { expect, test, type Page } from '@playwright/test';

const runtimeIssues = new WeakMap<Page, string[]>();

function installBrowserOutboundGuard(page: Page): void {
  const issues: string[] = [];
  runtimeIssues.set(page, issues);

  page.on('request', (request) => {
    const url = new URL(request.url());
    const isSameOrigin = url.origin === 'http://127.0.0.1:4173';
    const isStaticPath =
      url.pathname === '/' ||
      url.pathname === '/index.html' ||
      url.pathname.startsWith('/assets/') ||
      /^\/favicon(?:\.ico)?$/.test(url.pathname);
    const isAllowed =
      isSameOrigin &&
      ['GET', 'HEAD'].includes(request.method()) &&
      isStaticPath;

    if (!isAllowed) {
      issues.push(`${request.method()} ${request.url()} [${request.resourceType()}]`);
    }
  });
  page.on('websocket', (socket) => {
    issues.push(`WebSocket ${socket.url()}`);
  });
  page.on('console', (message) => {
    if (
      message.type() === 'error' &&
      /CSP|content security|wasm|instantiate|blocked (?:script|worker)|network/i.test(
        message.text()
      )
    ) {
      issues.push(`console: ${message.text()}`);
    }
  });
  page.on('pageerror', (error) => {
    if (/CSP|wasm|instantiate|network/i.test(error.message)) {
      issues.push(`pageerror: ${error.message}`);
    }
  });
}

async function installWasmRequestCapture(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const requestKey = '__payrollWasmRequests';
    const windowWithCapture = window as Window & { [requestKey]?: string[] };
    windowWithCapture[requestKey] = [];
    const originalStringify = JSON.stringify;
    JSON.stringify = function (value, replacer, space) {
      const serialized = originalStringify.call(JSON, value, replacer, space);
      if (
        value &&
        typeof value === 'object' &&
        !Array.isArray(value) &&
        'personnelId' in value &&
        'dataset' in value
      ) {
        windowWithCapture[requestKey]?.push(serialized);
      }
      return serialized;
    };
  });
}

test.beforeEach(async ({ page }) => {
  installBrowserOutboundGuard(page);
});

test.afterEach(async ({ page }) => {
  expect(runtimeIssues.get(page) ?? []).toEqual([]);
});

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
  taxOpenings?: Array<{
    id: string;
    personnelId: string;
    year: number;
    gvCumulativeOpening: string;
    effectiveFromPeriodId: string;
  }>;
  donemler?: StoredPeriod[];
  annualPayrollParameters?: StoredAnnualPayrollParameters[];
};

const databaseName = '4d-bordro-programi';
const objectStoreName = 'snapshots';

async function readStoredPayload(page: Page): Promise<string | null> {
  return page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName }) =>
      new Promise<string | null>((resolve, reject) => {
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
              resolve(typeof payload === 'string' ? payload : null);
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

async function readStoredSnapshot(page: Page): Promise<StoredSnapshot | null> {
  const payload = await readStoredPayload(page);
  return payload ? (JSON.parse(payload) as StoredSnapshot) : null;
}

async function writeStoredPayload(page: Page, payload: string): Promise<void> {
  await page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName, payload }) =>
      new Promise<void>((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const transaction = database.transaction(storeName, 'readwrite');
          const request = transaction.objectStore(storeName).put(payload, 'current');
          request.onerror = () => reject(request.error ?? new Error('Snapshot yazılamadı.'));
          transaction.oncomplete = () => {
            database.close();
            resolve();
          };
          transaction.onerror = () =>
            reject(transaction.error ?? new Error('Snapshot transaction başarısız.'));
        };
      }),
    { databaseName, objectStoreName, payload }
  );
}

async function deleteStoredPayload(page: Page): Promise<void> {
  await page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName }) =>
      new Promise<void>((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const transaction = database.transaction(storeName, 'readwrite');
          const request = transaction.objectStore(storeName).delete('current');
          request.onerror = () => reject(request.error ?? new Error('Snapshot silinemedi.'));
          transaction.oncomplete = () => {
            database.close();
            resolve();
          };
          transaction.onerror = () =>
            reject(transaction.error ?? new Error('Snapshot delete transaction başarısız.'));
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

async function seedExactTaxOpening(page: Page, periodId: string, value: string): Promise<void> {
  await page.evaluate(
    ({ databaseName: dbName, objectStoreName: storeName, periodId: seededPeriodId, value }) =>
      new Promise<void>((resolve, reject) => {
        const openRequest = indexedDB.open(dbName, 1);
        openRequest.onerror = () => reject(openRequest.error ?? new Error('IndexedDB açılamadı.'));
        openRequest.onsuccess = () => {
          const database = openRequest.result;
          const transaction = database.transaction(storeName, 'readwrite');
          const objectStore = transaction.objectStore(storeName);
          const readRequest = objectStore.get('current');
          readRequest.onerror = () => reject(readRequest.error ?? new Error('Snapshot okunamadı.'));
          readRequest.onsuccess = () => {
            try {
              const snapshot = JSON.parse(readRequest.result as string) as StoredSnapshot;
              snapshot.taxOpenings = [
                {
                  id: `exact-${seededPeriodId}`,
                  personnelId: 'p-1',
                  year: Number(seededPeriodId.slice(0, 4)),
                  gvCumulativeOpening: value,
                  effectiveFromPeriodId: seededPeriodId,
                },
              ];
              const writeRequest = objectStore.put(JSON.stringify(snapshot), 'current');
              writeRequest.onerror = () =>
                reject(writeRequest.error ?? new Error('Exact tax opening yazılamadı.'));
            } catch (error) {
              reject(error);
            }
          };
          transaction.oncomplete = () => {
            database.close();
            resolve();
          };
          transaction.onerror = () =>
            reject(transaction.error ?? new Error('Exact tax opening transaction başarısız.'));
        };
      }),
    { databaseName, objectStoreName, periodId, value }
  );
}

async function readCapturedWasmRequests(page: Page): Promise<Array<Record<string, unknown>>> {
  return page.evaluate(() => {
    const requestKey = '__payrollWasmRequests';
    const values = (window as Window & { [requestKey]?: string[] })[requestKey] ?? [];
    return values.flatMap((value) => {
      try {
        return [JSON.parse(value) as Record<string, unknown>];
      } catch {
        return [];
      }
    });
  });
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

async function seedCalculatedSnapshot(page: Page): Promise<string> {
  await page.goto('/');
  await expect(page.getByText('4/D Personel Kayıtları (0)')).toBeVisible();
  await loadSampleDataset(page);
  const periodId = await openPayrollScreen(page);
  await calculateP1(page, periodId);
  const payload = await readStoredPayload(page);
  expect(payload).not.toBeNull();
  return payload!;
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

test('browser rejects a corrupt authoritative IndexedDB snapshot without autosaving over it', async ({ page }) => {
  const validPayload = await seedCalculatedSnapshot(page);
  const snapshot = JSON.parse(validPayload) as Record<string, unknown>;
  const payrolls = snapshot.bordrolar as Array<Record<string, unknown>>;
  payrolls[0].netOdeme = 64179.78;
  const corruptSnapshot = JSON.stringify(snapshot);
  await writeStoredPayload(page, corruptSnapshot);

  await page.reload();
  const storageError = page.getByTestId('storage-error');
  await expect(storageError).toBeVisible();
  await expect(storageError).toContainText('Decimal string');
  await expect(storageError).toContainText('mevcut snapshot değiştirilmedi');
  await expect(page.getByTestId('data-loading-state')).not.toBeVisible();
  await expect(page.getByText('4/D Personel Kayıtları (0)')).not.toBeVisible();
  await expect.poll(() => readStoredPayload(page)).toBe(corruptSnapshot);
});

test('browser rejects a schema-corrupt authoritative snapshot without autosaving over it', async ({ page }) => {
  const validPayload = await seedCalculatedSnapshot(page);
  const snapshot = JSON.parse(validPayload) as Record<string, unknown>;
  const payrolls = snapshot.bordrolar as Array<Record<string, unknown>>;
  payrolls[0].status = 'DONE';
  const corruptSnapshot = JSON.stringify(snapshot);
  await writeStoredPayload(page, corruptSnapshot);

  await page.reload();
  const storageError = page.getByTestId('storage-error');
  await expect(storageError).toBeVisible();
  await expect(storageError).toContainText('status');
  await expect(storageError).toContainText('DONE');
  await expect(storageError).toContainText('mevcut snapshot değiştirilmedi');
  await expect(page.getByTestId('payroll-screen')).not.toBeVisible();
  await expect(page.getByText('4/D Personel Kayıtları (5)')).not.toBeVisible();
  await expect.poll(() => readStoredPayload(page)).toBe(corruptSnapshot);
});

test('browser migrates a numeric legacy localStorage backup into exact IndexedDB storage', async ({ page }) => {
  const currentPayload = await seedCalculatedSnapshot(page);
  const legacySnapshot = JSON.parse(currentPayload) as Record<string, unknown>;
  legacySnapshot.backupVersion = 1;
  const legacyPayroll = (legacySnapshot.bordrolar as Array<Record<string, unknown>>)[0];
  legacyPayroll.netOdeme = 64179.78;
  delete legacyPayroll.status;
  const legacyPayload = JSON.stringify(legacySnapshot);
  await page.evaluate(
    ({ storageKey, payload }) => localStorage.setItem(storageKey, payload),
    { storageKey: '4d_bordro_programi_mvp_v2', payload: legacyPayload }
  );
  await deleteStoredPayload(page);

  await page.reload();
  await expect(page.getByTestId('payroll-screen')).toBeVisible();
  await expect
    .poll(async () => (await readStoredSnapshot(page))?.bordrolar?.[0]?.netOdeme)
    .toBe('64179.78');
});

test('browser exact Decimal survives WASM result, IndexedDB reload, and the next WASM request', async ({ page }) => {
  await installWasmRequestCapture(page);
  await page.goto('/');
  await expect(page.getByText('4/D Personel Kayıtları (0)')).toBeVisible();
  await loadSampleDataset(page);

  const periodId = await openPayrollScreen(page);
  const exactFixtures = ['0.123456789012345678901', '123456789012345678.91'];

  for (const exactValue of exactFixtures) {
    await seedExactTaxOpening(page, periodId, exactValue);
    await page.reload();
    await expect(page.getByTestId('payroll-screen')).toBeVisible();

    const reloaded = await readStoredSnapshot(page);
    expect(reloaded?.taxOpenings?.[0]?.gvCumulativeOpening).toBe(exactValue);

    await openPayrollScreen(page);
    await expect(page.getByRole('columnheader', { name: 'Tediye (Manuel)' })).toHaveCount(0);
    await expect(
      page
        .locator('tbody tr')
        .filter({ hasText: 'Ahmet Yılmaz' })
        .first()
        .locator('input[inputmode="decimal"]')
    ).toHaveCount(0);
    await expect(page.getByTestId('normal-payment-date')).toBeVisible();
    const activePeriod = reloaded?.donemler?.find((period) => period.id === periodId);
    expect(activePeriod).toBeDefined();
    const explicitPaymentDate = `${activePeriod!.taxYear}-${String(activePeriod!.taxMonth).padStart(2, '0')}-13`;
    const normalPaymentDate = page.getByTestId('normal-payment-date');
    await normalPaymentDate.fill(explicitPaymentDate);
    expect(await normalPaymentDate.inputValue()).toBe(explicitPaymentDate);

    // The explicit NORMAL payment date and cross-period tax opening are sent to
    // the next WASM request as their original strings. Core may round the
    // official payroll display to two places; that is separate from boundary
    // fidelity.
    await calculateP1(page, periodId);
    const requests = await readCapturedWasmRequests(page);
    const calculationRequest = requests.find(
      (request) =>
        request.personnelId === 'p-1' &&
        request.periodId === periodId &&
        (request.accrual as { accrualType?: unknown; paymentDate?: unknown } | undefined)
          ?.accrualType === 'NORMAL' &&
        (request.accrual as { paymentDate?: unknown } | undefined)?.paymentDate ===
          explicitPaymentDate
    );
    expect(calculationRequest).toBeDefined();
    expect(
      (calculationRequest?.dataset as { taxOpenings?: Array<{ gvCumulativeOpening?: unknown }> })
        ?.taxOpenings?.[0]?.gvCumulativeOpening
    ).toBe(exactValue);
  }
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
