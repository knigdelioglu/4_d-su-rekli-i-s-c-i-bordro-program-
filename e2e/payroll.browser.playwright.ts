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
  gvDetay?: {
    oncekiKumulatifGvMatrahi?: string;
  };
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
  kurumDegerleriMap?: Record<string, {
    gunlukAsgariUcret?: string;
    sgkIsciOraniYuzde?: string;
    issizlikIsciOraniYuzde?: string;
    pekTavanKatsayisi?: string;
    gunlukYemekIstisnasiSGK?: string;
    gunlukYemekIstisnasiGV?: string;
    damgaVergisiOraniBinde?: string;
  }>;
  sickLeaveRecords?: Array<{
    id: string;
    personnelId: string;
    startDate: string;
    endDate: string;
  }>;
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
              const stored = request.result as unknown;
              const storedRecord =
                stored && typeof stored === 'object' && !Array.isArray(stored)
                  ? (stored as { payload?: unknown })
                  : null;
              const payload =
                typeof stored === 'string'
                  ? stored
                  : typeof storedRecord?.payload === 'string'
                    ? storedRecord.payload
                    : null;
              database.close();
              resolve(payload);
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
              const stored = readRequest.result as unknown;
              const storedRecord =
                stored && typeof stored === 'object' && !Array.isArray(stored)
                  ? (stored as { payload?: unknown; revision?: unknown })
                  : null;
              const payload =
                typeof stored === 'string'
                  ? stored
                  : typeof storedRecord?.payload === 'string'
                    ? storedRecord.payload
                    : null;
              const revision =
                typeof storedRecord?.revision === 'number' &&
                Number.isInteger(storedRecord.revision) &&
                storedRecord.revision >= 0
                  ? storedRecord.revision
                  : 0;
              if (payload === null) throw new Error('Fixture snapshot payload bulunamadı.');
              const snapshot = JSON.parse(payload) as StoredSnapshot;
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
              const activeYear = Math.max(...snapshot.donemler.map((period) => period.yil));
              const annual = snapshot.annualPayrollParameters.find(
                (parameters) => parameters.year === activeYear
              );
              if (!annual) throw new Error('Örnek fixture yıllık vergi parametresi içermiyor.');
              annual.sigortaGvYillikBrutAsgariUcretTavani = '396360';

              const writeRequest = objectStore.put(
                { payload: JSON.stringify(snapshot), revision },
                'current'
              );
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
              const stored = readRequest.result as unknown;
              const storedRecord =
                stored && typeof stored === 'object' && !Array.isArray(stored)
                  ? (stored as { payload?: unknown; revision?: unknown })
                  : null;
              const payload =
                typeof stored === 'string'
                  ? stored
                  : typeof storedRecord?.payload === 'string'
                    ? storedRecord.payload
                    : null;
              const revision =
                typeof storedRecord?.revision === 'number' &&
                Number.isInteger(storedRecord.revision) &&
                storedRecord.revision >= 0
                  ? storedRecord.revision
                  : 0;
              if (payload === null) throw new Error('Snapshot payload bulunamadı.');
              const snapshot = JSON.parse(payload) as StoredSnapshot;
              // Changing an authoritative tax opening changes the inputs for
              // any calculated payroll. Keep this boundary fixture replayable
              // before exercising the next exact opening value.
              snapshot.bordrolar = [];
              snapshot.taxOpenings = [
                {
                  id: `exact-${seededPeriodId}`,
                  personnelId: 'p-1',
                  year: Number(seededPeriodId.slice(0, 4)),
                  gvCumulativeOpening: value,
                  effectiveFromPeriodId: seededPeriodId,
                },
              ];
              const writeRequest = objectStore.put(
                { payload: JSON.stringify(snapshot), revision },
                'current'
              );
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
  await page.getByTitle('Örnek Veriyi Yeniden Yükle').click();
  await page.getByRole('dialog').getByRole('button', { name: 'Evet, Örnek Verileri Yükle' }).click();

  const response = await wasmResponse;
  expect(response.status()).toBe(200);
  expect(response.headers()['content-type']).toContain('application/wasm');
  await page.getByTestId('nav-personel').click();
  await expect(page.getByText(/4\/D Personel Kayıtları \(5\)/)).toBeVisible();
  await installCalculableFixture(page);
}

async function openPayrollScreen(page: Page): Promise<string> {
  await page.getByTestId('nav-bordro').click();
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
  await expect(page.getByTestId('period-summary')).toBeVisible();
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

test('desktop primary navigation lives in the sidebar and period parameters open as a page', async ({ page }) => {
  await page.goto('/');

  await expect(page.getByTestId('nav-ozet')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('period-summary')).toBeVisible();
  for (const tab of ['personel', 'puantaj', 'bordro', 'banka', 'sgk-kontrol', 'kesintiler', 'parametrelar']) {
    await expect(page.getByTestId(`nav-${tab}`)).toBeVisible();
  }
  await expect(page.locator('header').getByRole('button', { name: '3. Bordro Hesaplama' })).toHaveCount(0);

  await page.getByTestId('nav-puantaj').click();
  await expect(page.getByTestId('nav-puantaj')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('nav-personel')).not.toHaveAttribute('aria-current', 'page');

  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('nav-parametrelar')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('period-settings-page')).toBeVisible();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
});

test('SGK prim kontrolü tüm personeli ve manuel mutabakat farkını gösterir', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  await page.getByTestId('nav-sgk-kontrol').click();
  const screen = page.getByTestId('sgk-prim-kontrolu-screen');
  await expect(screen).toBeVisible();
  await expect(page.getByTestId('nav-sgk-kontrol')).toHaveAttribute('aria-current', 'page');
  await expect(screen.getByRole('columnheader').allTextContents()).resolves.toEqual([
    'S.No',
    'T.C. Kimlik No',
    'SGK Sicil No',
    'Ad Soyad',
    'Durum',
    'SGK İşveren —',
    'İşveren İşsizlik —',
    'SGK İşçi —',
    'İşçi İşsizlik —',
    'Retro kaynak PEK farkı',
    'PEK Alt Sınır İşveren Tamamlama',
    'Toplam',
  ]);
  await expect(screen.locator('tbody tr')).toHaveCount(5);
  await expect(screen.getByText('Bordro hesaplanmadı')).toHaveCount(5);

  await screen.getByTestId('sgk-reported-total-input').fill('1.000,00');
  await expect(screen.getByText(/Mutabakat tamamlanamaz/)).toBeVisible();
  await expect(screen.getByText('Uyumlu')).toHaveCount(0);
});

test('first opening uses the period summary and a valid saved tab survives reload', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('nav-ozet')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('period-summary')).toBeVisible();

  await page.getByTestId('nav-puantaj').click();
  await page.reload();
  await expect(page.getByTestId('nav-puantaj')).toHaveAttribute('aria-current', 'page');
});

test('normal payroll table keeps core columns and moves secondary details to the payslip', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);
  const screen = page.getByTestId('payroll-screen');
  await openPayrollScreen(page);

  await expect
    .poll(async () =>
      (await screen.getByRole('columnheader').allTextContents()).map((header) => header.trim())
    )
    .toEqual(['Personel', 'Puantaj', 'Brüt', 'Kesinti', 'Net', 'Durum', 'İşlemler']);
  await expect(screen.getByRole('columnheader', { name: 'İş Primi Grubu' })).toHaveCount(0);
  await expect(screen.getByRole('columnheader', { name: 'Önceki Küm. GV' })).toHaveCount(0);
  await expect(screen.getByRole('columnheader', { name: 'Normal Ödeme/Tahakkuk' })).toHaveCount(0);
  await expect(screen).not.toContainText(/Sıra\s+\d+|Yeni NORMAL|CALCULATED|FINALIZED|STALE|DRAFT|SUPPLEMENTAL/);
});

test('stale payment in the accrual timeline asks for confirmation before deletion', async ({ page }) => {
  const payload = await seedCalculatedSnapshot(page);
  const snapshot = JSON.parse(payload) as StoredSnapshot;
  const payroll = snapshot.bordrolar.find((item) => item.personelId === 'p-1');
  expect(payroll).toBeDefined();
  payroll!.status = 'STALE';
  await writeStoredPayload(page, JSON.stringify(snapshot));

  await page.reload();
  await openPayrollScreen(page);
  await page.getByTestId('timeline-toggle-p-1').click();
  const timeline = page.getByTestId('accrual-timeline-p-1');
  await expect(timeline.getByRole('button', { name: 'Tahakkuku Sil' })).toBeVisible();
  await timeline.getByRole('button', { name: 'Tahakkuku Sil' }).click();
  await expect(timeline.getByRole('button', { name: 'Silmeyi Onayla' })).toBeVisible();
  await expect(timeline.getByRole('button', { name: 'Vazgeç' })).toBeVisible();

  await timeline.getByRole('button', { name: 'Silmeyi Onayla' }).click();
  await expect(page.getByText('Tahakkuk başarıyla silindi.')).toBeVisible();
  const persisted = await readStoredSnapshot(page);
  expect(persisted?.bordrolar.some((item) => item.personelId === 'p-1')).toBe(false);
});

test('missing supplementary rows reveal the actual work period, gross and stale status of saved accruals', async ({ page }) => {
  await seedCalculatedSnapshot(page);
  const sourceId = (await page.getByTestId('payroll-screen').getAttribute('data-period-id'))!;
  await page.getByTestId('nav-bordro-tediye').click();
  await page.getByTestId('timeline-toggle-p-1').click();
  for (const [type, gross, label] of [
    ['TEDIYE', '24500', 'Tediye'],
    ['TIS_IKRAMIYE', '18000', 'TİS İkramiyesi'],
    ['SUPPLEMENTAL', '12000', 'Ek Ödeme'],
  ]) {
    await page.getByTestId('add-accrual-p-1').click();
    await page.getByRole('combobox', { name: 'Tür', exact: true }).selectOption(type);
    await page.getByRole('textbox', { name: 'Brüt tutar', exact: true }).fill(gross);
    await page.getByRole('button', { name: 'Hesapla ve Kaydet' }).click();
    await expect(page.getByText(`Ahmet Yılmaz için ${label} tahakkuku hesaplandı.`)).toBeVisible();
  }
  const saved = JSON.parse((await readStoredPayload(page))!) as {
    donemler: Array<{ id: string; donemAdi: string }>;
    bordrolar: Array<{ personelId: string; accrualType: string; status: string }>;
  };
  const sourceLabel = saved.donemler.find((p) => p.id === sourceId)!.donemAdi;
  const otherId = saved.donemler.find((p) => p.id !== sourceId)!.id;
  // A stale predecessor also makes its later same-month events stale.
  for (const payroll of saved.bordrolar) {
    if (payroll.personelId === 'p-1' && payroll.accrualType !== 'NORMAL') payroll.status = 'STALE';
  }
  await writeStoredPayload(page, JSON.stringify(saved));
  await page.reload();
  await expect(page.getByTestId('payroll-screen')).toBeVisible();
  if (await page.getByTestId('nav-bordro').getAttribute('aria-expanded') !== 'true') {
    await page.getByTestId('nav-bordro').click();
  }
  await selectPeriod(page, otherId);
  for (const [view, gross] of [['tediye', '24.500'], ['tis', '18.000'], ['ek-odeme', '12.000']]) {
    await page.getByTestId(`nav-bordro-${view}`).click();
    const discovery = page.getByTestId('other-period-accruals-p-1');
    await discovery.locator('summary').click();
    await expect(discovery).toContainText(sourceLabel);
    await expect(discovery).toContainText(gross);
    if (view === 'tediye') await expect(discovery).toContainText('Yeniden Hesaplanmalı');
    await expect(page.getByTestId('payroll-row-p-1')).toContainText('Tahakkuk Eklenmedi');
  }
});

test('normal payroll screen keeps summary cards compact and desktop table rows visible without excessive scrolling', async ({ page }) => {
  // Use Tauri desktop application default window size (1280x800)
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/');
  await loadSampleDataset(page);
  const screen = page.getByTestId('payroll-screen');
  await openPayrollScreen(page);

  const summaryGrid = screen.getByTestId('payroll-summary-cards');
  await expect(summaryGrid).toBeVisible();

  // Calculate payrolls so cards show real monetary totals and rows have full actions and calculations
  const calculateAllButton = page.getByRole('button', { name: /Tüm Hesaplanabilir Bordroları Hesapla/ });
  await calculateAllButton.click();
  await expect(page.getByText(/personelin bordrosu başarıyla güncellendi/i)).toBeVisible();

  const monetaryCardConfigs = [
    { cardId: 'kpi-card-gross', expectedTitle: 'Vergi ve SGK Öncesi' },
    { cardId: 'kpi-card-deductions', expectedTitle: 'SGK + Vergi + Kesinti' },
    { cardId: 'kpi-card-net', expectedTitle: 'Banka Ele Geçen' },
    { cardId: 'kpi-card-employer-cost', expectedTitle: 'Kurum SGK + İşsizlik' },
  ];

  for (const { cardId, expectedTitle } of monetaryCardConfigs) {
    const card = screen.getByTestId(cardId);
    await expect(card).toBeVisible();
    const box = await card.boundingBox();
    expect(box).not.toBeNull();
    // Compact summary cards must not push the table down (>100px); they must remain tight (~60-70px).
    expect(box!.height).toBeLessThanOrEqual(75);

    // Amount and currency must form a single unbreakable run without wrapping TL to a newline
    const amountEl = card.locator('.font-mono');
    await expect(amountEl).toBeVisible();
    await expect(amountEl).toHaveText(/TL$/);
    const whiteSpace = await amountEl.evaluate((el) => window.getComputedStyle(el).whiteSpace);
    expect(whiteSpace).toBe('nowrap');

    // Amount text must not span multiple lines
    const amountBox = await amountEl.boundingBox();
    expect(amountBox).not.toBeNull();
    expect(amountBox!.height).toBeLessThanOrEqual(28);

    // Supporting text must be readable and contain descriptive title attribute to avoid silent truncation
    const supportingText = card.locator(`[title="${expectedTitle}"]`);
    await expect(supportingText).toBeVisible();
  }

  // Personnel count card assertions
  const personnelCard = screen.getByTestId('kpi-card-personnel');
  await expect(personnelCard).toBeVisible();
  const personnelBox = await personnelCard.boundingBox();
  expect(personnelBox).not.toBeNull();
  expect(personnelBox!.height).toBeLessThanOrEqual(75);
  await expect(personnelCard.locator('.whitespace-nowrap')).toHaveText(/\d+\s+Kişi/);

  // Ensure table header and the first 3 personnel data rows are visible in desktop viewport without scrolling
  const table = screen.getByRole('table');
  await expect(table).toBeVisible();

  // Verify table does not overflow horizontally in the 1280px desktop viewport
  const tableContainer = table.locator('..');
  const overflowState = await tableContainer.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
    scrollLeft: el.scrollLeft,
  }));
  const colWidths = await table.locator('thead th').evaluateAll((ths) =>
    ths.map((th) => ({
      name: th.textContent?.trim(),
      offsetWidth: (th as HTMLElement).offsetWidth,
      clientWidth: (th as HTMLElement).clientWidth,
    }))
  );
  console.log('[COLUMNS]', JSON.stringify(colWidths, null, 2));
  console.log(`[TABLE CONTAINER] scrollWidth=${overflowState.scrollWidth}, clientWidth=${overflowState.clientWidth}`);
  expect(overflowState.scrollWidth).toBeLessThanOrEqual(overflowState.clientWidth);

  const rows = screen.getByRole('row');
  // 1 header row + 5 personnel rows in sample dataset
  await expect(rows).toHaveCount(6);

  for (let r = 0; r < 6; r++) {
    const box = await rows.nth(r).boundingBox();
    console.log(`[MEASURED ROW ${r}] y=${box?.y}, height=${box?.height}, bottom=${(box?.y ?? 0) + (box?.height ?? 0)}`);
  }

  // Measure that the first 3 personnel data rows are fully visible within the 800px desktop window
  for (const rowIndex of [1, 2, 3]) {
    const row = rows.nth(rowIndex);
    await expect(row).toBeVisible();
    const box = await row.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.y).toBeGreaterThan(0);
    expect(box!.y).toBeLessThan(800);
    expect(box!.y + box!.height).toBeLessThanOrEqual(800);

    // Assert that every action button in the row is within the 1280px viewport
    const actionButtons = row.locator('button');
    const buttonCount = await actionButtons.count();
    expect(buttonCount).toBeGreaterThan(0);
    for (let b = 0; b < buttonCount; b++) {
      const button = actionButtons.nth(b);
      const bBox = await button.boundingBox();
      expect(bBox).not.toBeNull();
      const rightEdge = bBox!.x + bBox!.width;
      const ariaLabel = (await button.getAttribute('aria-label')) || (await button.getAttribute('title')) || (await button.textContent());
      console.log(`[ROW ${rowIndex} BUTTON ${b}] "${ariaLabel?.trim()}" x=${bBox!.x}, width=${bBox!.width}, right=${rightEdge}`);
      expect(bBox!.x).toBeGreaterThanOrEqual(0);
      expect(rightEdge).toBeLessThanOrEqual(1280);
    }
  }

  // Verify the accessible 'Diğer işlemler' menu opens and renders its actions within viewport
  const firstRow = rows.nth(1);
  const moreActionsBtn = firstRow.getByRole('button', { name: /Diğer işlemler/i });
  await expect(moreActionsBtn).toBeVisible();
  await expect(moreActionsBtn).toHaveAttribute('aria-expanded', 'false');
  await moreActionsBtn.click();
  await expect(moreActionsBtn).toHaveAttribute('aria-expanded', 'true');

  const menu = page.getByRole('menu', { name: /Diğer işlemler menüsü/i });
  await expect(menu).toBeVisible();
  const menuBox = await menu.boundingBox();
  expect(menuBox).not.toBeNull();
  expect(menuBox!.x).toBeGreaterThanOrEqual(0);
  expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(1280);

  const viewPayslipItem = menu.getByRole('menuitem', { name: /Bordro Gör/i });
  await expect(viewPayslipItem).toBeVisible();
  const finalizeItem = menu.getByRole('menuitem', { name: /Kesinleştir/i });
  await expect(finalizeItem).toBeVisible();

  // Press Escape to dismiss menu cleanly
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await expect(moreActionsBtn).toHaveAttribute('aria-expanded', 'false');
});

test('period settings child navigation renders sections and persists after reload', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await expect(page.getByTestId('nav-parametre-gelir')).toHaveAttribute('aria-current', 'page');
  for (const childTestId of [
    'nav-parametre-gelir',
    'nav-parametre-kesinti',
    'nav-parametre-gv',
    'nav-parametre-tediye-tis',
    'nav-parametre-rapor',
    'nav-parametre-donemler',
    'nav-parametre-yeni-donem',
  ]) {
    await expect(page.getByTestId(childTestId)).toBeVisible();
  }

  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('nav-parametre-gelir')).not.toBeVisible();
  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('nav-parametre-gelir')).toHaveAttribute('aria-current', 'page');

  await page.getByTestId('nav-parametre-gv').click();
  await expect(page.getByTestId('period-settings-gv')).toBeVisible();
  await expect(page.getByTestId('nav-parametre-gv')).toHaveAttribute('aria-current', 'page');

  await page.getByTestId('nav-parametre-tediye-tis').click();
  await expect(page.getByTestId('period-settings-tediye-tis')).toBeVisible();

  await page.getByTestId('nav-parametre-donemler').click();
  await expect(page.getByTestId('period-settings-donemler')).toBeVisible();

  await page.getByTestId('nav-parametre-gv').click();
  await page.reload();
  await expect(page.getByTestId('nav-parametrelar')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('period-settings-page')).toBeVisible();
  await expect(page.getByTestId('period-settings-gv')).toBeVisible();
  await expect(page.getByTestId('nav-parametre-gv')).toHaveAttribute('aria-current', 'page');
});

test('period settings expose new period creation without an active period', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('period-summary')).toBeVisible();

  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await page.getByTestId('period-settings-gelir').getByRole('button', { name: 'Yeni Dönem Aç' }).click();

  await expect(page.getByTestId('period-settings-yeni-donem')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Bordro Dönemi ve Kurum Değerleri' })).toHaveCount(0);

  await page.getByTestId('period-settings-yeni-donem').getByRole('button', { name: 'Dönemi Oluştur ve Geç' }).click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  const periodId = await page.getByTestId('active-period-selector').inputValue();
  expect(periodId).not.toBe('');
  const snapshot = await readStoredSnapshot(page);
  expect(snapshot?.annualPayrollParameters?.find((item) => item.year === 2026)).toMatchObject({
    year: 2026,
    sigortaGvYillikBrutAsgariUcretTavani: '396360',
  });
  expect(snapshot?.kurumDegerleriMap?.[periodId]).toMatchObject({
    gunlukAsgariUcret: '1101',
    sgkIsciOraniYuzde: '14',
    issizlikIsciOraniYuzde: '1',
    pekTavanKatsayisi: '9',
    gunlukYemekIstisnasiSGK: '300',
    gunlukYemekIstisnasiGV: '300',
    damgaVergisiOraniBinde: '7.59',
  });
});

test('selecting existing period in new period form switches safely without recreating or overwriting parameters', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  // Navigate to Period Settings -> New Period form
  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await page.getByTestId('nav-parametre-yeni-donem').click();
  await expect(page.getByTestId('period-settings-yeni-donem')).toBeVisible();

  // In sample dataset, 2026-01 already exists
  const form = page.getByTestId('period-settings-yeni-donem');
  await form.locator('select').nth(0).selectOption('2026');
  await form.locator('select').nth(1).selectOption('1');

  // Verify warning indicates existing period is protected rather than rewritten
  const banner = page.getByTestId('period-already-exists-banner');
  await expect(banner).toBeVisible();
  await expect(banner).toContainText('2026-01');
  await expect(banner).toContainText('doğrudan bu döneme geçebilirsiniz');

  // Verify action button shows safe switch text instead of create
  const submitBtn = page.getByTestId('submit-period-action');
  await expect(submitBtn).toBeVisible();
  await expect(submitBtn).toContainText('Mevcut Döneme Geç');

  // Click button and verify it safely switches to the period without errors
  await submitBtn.click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await expect(page.getByTestId('active-period-selector')).toHaveValue('2026-01');
});

test('creates 2026-12 period with 2027-01 tax transition and shows clear feedback', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  // Navigate to Period Settings -> New Period form
  await page.getByTestId('nav-parametrelar').click();
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await page.getByTestId('nav-parametre-yeni-donem').click();
  await expect(page.getByTestId('period-settings-yeni-donem')).toBeVisible();

  const form = page.getByTestId('period-settings-yeni-donem');
  // Select 2026 and month 12
  await form.locator('select').nth(0).selectOption('2026');
  await form.locator('select').nth(1).selectOption('12');

  // Verify tax defaults automatically set to 2027 January
  await expect(form.locator('select').nth(2)).toHaveValue('2027');
  await expect(form.locator('select').nth(3)).toHaveValue('1');

  // Verify preview shows 2026-12 range and 2027-01 tax payment month
  await expect(form).toContainText('2026-12-15 → 2027-01-14');
  await expect(form).toContainText('Ödeme/Tahakkuk Ayı: Ocak 2027');

  // Period does not exist yet
  await expect(page.getByTestId('period-already-exists-banner')).toHaveCount(0);
  const submitBtn = page.getByTestId('submit-period-action');
  await expect(submitBtn).toBeVisible();
  await expect(submitBtn).toContainText('Dönemi Oluştur ve Geç');

  // Submit form
  await submitBtn.click();

  // Verify explicit success feedback and automatic transition to Ücretler
  await expect(page.getByTestId('period-settings-success-banner')).toBeVisible();
  await expect(page.getByTestId('period-settings-success-banner')).toContainText('2026-12');
  await expect(page.getByTestId('period-settings-gelir')).toBeVisible();
  await expect(page.getByTestId('active-period-selector')).toHaveValue('2026-12');

  // Verify 2026-12 appears in period list
  await page.getByTestId('nav-parametre-donemler').click();
  await expect(page.getByTestId('period-settings-donemler')).toBeVisible();
  await expect(page.getByTestId('period-row-2026-12')).toBeVisible();

  // Re-open New Period form and select 2026-12 again
  await page.getByTestId('nav-parametre-yeni-donem').click();
  await expect(page.getByTestId('period-settings-yeni-donem')).toBeVisible();
  await form.locator('select').nth(0).selectOption('2026');
  await form.locator('select').nth(1).selectOption('12');

  // Now existing preview warning must appear
  await expect(page.getByTestId('period-already-exists-banner')).toBeVisible();
  await expect(page.getByTestId('period-already-exists-banner')).toContainText('2026-12');
  await expect(submitBtn).toContainText('Mevcut Döneme Geç');
});

test('active period selector shows explicit year and distinguishes same month across different years', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  // In sample dataset, 2026-01 already exists
  const selector = page.getByTestId('active-period-selector');
  await expect(selector).toBeVisible();

  // Create 2027-01 (Ocak 2027) via Period Settings -> New Period
  await page.getByTestId('nav-parametrelar').click();
  await page.getByTestId('nav-parametre-yeni-donem').click();
  const form = page.getByTestId('period-settings-yeni-donem');
  await form.locator('select').nth(0).selectOption('2027');
  await form.locator('select').nth(1).selectOption('1');
  await page.getByTestId('submit-period-action').click();

  await expect(page.getByTestId('period-settings-success-banner')).toBeVisible();
  await expect(selector).toHaveValue('2027-01');

  // Verify both 2026-01 and 2027-01 options exist with distinct, year-qualified labels
  const option2026 = selector.locator('option[value="2026-01"]');
  const option2027 = selector.locator('option[value="2027-01"]');
  await expect(option2026).toBeAttached();
  await expect(option2027).toBeAttached();

  await expect(option2026).toHaveText('Ocak 2026 · 15 Ocak - 14 Şubat');
  await expect(option2027).toHaveText('Ocak 2027 · 15 Ocak - 14 Şubat');

  // Verify selecting 2026 by label explicitly activates 2026-01 (not 2027-01)
  await selector.selectOption({ label: 'Ocak 2026 · 15 Ocak - 14 Şubat' });
  await expect(selector).toHaveValue('2026-01');

  // Verify selecting 2027 by label explicitly activates 2027-01 (not 2026-01)
  await selector.selectOption({ label: 'Ocak 2027 · 15 Ocak - 14 Şubat' });
  await expect(selector).toHaveValue('2027-01');
});

test('mobile navigation opens as a drawer without reducing the content area', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.goto('/');

  await expect(page.getByTestId('sidebar-toggle')).toBeVisible();
  await expect(page.getByTestId('nav-personel')).not.toBeVisible();
  await page.getByTestId('sidebar-toggle').click();
  await expect(page.getByTestId('nav-personel')).toBeVisible();
  await page.getByTestId('nav-kesintiler').click();
  await expect(page.getByTestId('nav-kesintiler')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('nav-kesinti-sendika')).toBeVisible();
  await page.getByTestId('nav-kesinti-icra').click();
  await expect(page.getByTestId('nav-kesinti-icra')).not.toBeVisible();
});

test('deduction child navigation switches content and persists the selected type after reload', async ({ page }) => {
  await page.goto('/');
  await loadSampleDataset(page);

  await page.getByTestId('nav-kesintiler').click();
  await expect(page.getByTestId('nav-kesintiler')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByTestId('nav-kesinti-sendika')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('deduction-screen')).toHaveAttribute('data-deduction-type', 'sendika');
  await expect(page.getByTestId('deduction-screen').locator('h2')).toContainText(
    'Sendika Aidatı Listesi'
  );
  await expect(page.getByRole('button', { name: /1\. Sendika Aidatı Listesi/ })).toHaveCount(0);

  await page.getByTestId('nav-kesinti-icra').click();
  await expect(page.getByTestId('deduction-screen')).toHaveAttribute('data-deduction-type', 'icra');
  await expect(page.getByTestId('nav-kesinti-icra')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('deduction-screen').locator('h2')).toContainText(
    'İcra Kesintisi Listesi'
  );

  await page.getByTestId('nav-kesinti-bes').click();
  await expect(page.getByTestId('deduction-screen')).toHaveAttribute('data-deduction-type', 'bes');
  await expect(page.getByTestId('deduction-screen').locator('h2')).toContainText(
    'BES Kesintisi Listesi'
  );

  await page.getByTestId('nav-kesinti-icra').click();
  await page.reload();
  await expect(page.getByTestId('deduction-screen')).toHaveAttribute('data-deduction-type', 'icra');
  await expect(page.getByTestId('nav-kesinti-icra')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('deduction-screen').locator('h2')).toContainText(
    'İcra Kesintisi Listesi'
  );
});

test('top bar keeps the active period and backup export/import actions', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('active-period-selector')).toBeVisible();

  const downloadPromise = page.waitForEvent('download');
  await page.getByTitle('Yedek İndir (JSON)').click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/^4D_Bordro_Yedek_bos_\d{4}-\d{2}-\d{2}\.json$/);

  const backupPath = await download.path();
  if (!backupPath) throw new Error('Yedek indirme yolu alınamadı.');

  const dialogTypes: string[] = [];
  const dialogMessages: string[] = [];
  page.on('dialog', async (dialog) => {
    dialogTypes.push(dialog.type());
    dialogMessages.push(dialog.message());
    await dialog.accept();
  });

  await page.locator('input[type="file"]').setInputFiles(backupPath);
  await expect.poll(() => dialogTypes.length).toBe(2);
  expect(dialogTypes).toEqual(['confirm', 'alert']);
  expect(dialogMessages[1]).toBe('Yedek başarıyla yüklendi!');
});

test('browser WASM calculation persists in IndexedDB and survives reload', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('period-summary')).toBeVisible();
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

test('browser tax opening save persists the active period as an explicit canonical pair', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('period-summary')).toBeVisible();
  await loadSampleDataset(page);

  const periodId = await openPayrollScreen(page);
  await page.getByTitle('Sisteme ilk defa girildiğinde veya yıl ortasında önceki kümülatif vergi matrahlarını elle girmek için tıklayın').click();
  const ahmetRow = page.getByRole('row').filter({ hasText: 'Ahmet Yılmaz' });
  await ahmetRow.locator('input[inputmode="decimal"]').nth(0).fill('185000');
  await page.getByRole('button', { name: 'Kaydet ve Bordroları Yeniden Hesapla' }).click();

  await expect.poll(async () => (await readStoredSnapshot(page))?.taxOpenings?.find(
    (opening) => opening.personnelId === 'p-1'
  )).toMatchObject({
    year: 2026,
    gvCumulativeOpening: '185000',
    effectiveFromPeriodId: periodId,
  });
  await expect.poll(async () => {
    const snapshot = await readStoredSnapshot(page);
    return snapshot?.bordrolar.find(
      (payroll) => payroll.personelId === 'p-1' && payroll.donemId === periodId
    )?.gvDetay?.oncekiKumulatifGvMatrahi;
  }).toBe('185000');
  await page.reload();
  const reloaded = await readStoredSnapshot(page);
  expect(reloaded?.taxOpenings?.find((opening) => opening.personnelId === 'p-1')).toMatchObject({
    gvCumulativeOpening: '185000',
    effectiveFromPeriodId: periodId,
  });
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
  await expect(storageError).toContainText('Tarayıcıdaki bordro verisi okunamadı.');
  await expect(storageError).toContainText('Mevcut veriler değiştirilmedi.');
  await expect(storageError).not.toContainText(/snapshot|indexeddb|decimal/i);
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
  await expect(storageError).toContainText('Tarayıcıdaki bordro verisi okunamadı.');
  await expect(storageError).toContainText('Mevcut veriler değiştirilmedi.');
  await expect(storageError).not.toContainText(/status|done|snapshot|indexeddb|decimal/i);
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
  (legacyPayroll.kesintiler as Record<string, unknown>).digerKesinti = '7297.96';
  legacyPayroll.kesintiToplam = '31984.88';
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
  await expect(page.getByTestId('period-summary')).toBeVisible();
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
  await expect(page.getByTestId('period-summary')).toBeVisible();
  await loadSampleDataset(page);

  const periodId = await openPayrollScreen(page);
  await calculateP1(page, periodId);

  const payrollRow = page.locator('tbody tr').filter({ hasText: 'Ahmet Yılmaz' }).first();
  await payrollRow.getByRole('button', { name: /Diğer işlemler/i }).click();
  const finalizeMenuItem = page.getByRole('menuitem', { name: /Kesinleştir/i });
  await expect(finalizeMenuItem).toBeVisible();
  await finalizeMenuItem.click();
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

  await page.getByTestId('nav-puantaj').click();
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
  await expect(page.getByTestId('period-summary')).toBeVisible();
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
  await page.getByTestId('nav-puantaj').click();
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

test('saving sick leave automatically marks puantaj as R across 15-14 boundary with conflict confirmation and preserves manual edits on delete', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('period-summary')).toBeVisible();
  await loadSampleDataset(page);

  // Navigate to Period Settings -> Raporlar
  await page.getByTestId('nav-parametrelar').click();
  await page.getByTestId('nav-parametre-rapor').click();
  await expect(page.getByTestId('period-settings-rapor')).toBeVisible();

  // In sample dataset:
  // p-1 is Ahmet Yılmaz
  // Periods: 2026-06 (2026-06-15..2026-07-14) and 2026-07 (2026-07-15..2026-08-14)
  const form = page.getByTestId('period-settings-rapor').locator('form');
  await form.locator('input[type="date"]').nth(0).fill('2026-07-13');
  await form.locator('input[type="date"]').nth(1).fill('2026-07-16');

  // Click Save
  await form.getByRole('button', { name: /Rapor Olayını Kaydet/i }).click();

  // 1. Conflict modal must appear because 2026-07-13..2026-07-16 already have 'Ç' codes
  const conflictModal = page.getByTestId('sick-leave-conflict-modal');
  await expect(conflictModal).toBeVisible();
  await expect(conflictModal).toContainText('Puantaj Kod Çakışması Onayı');
  await expect(conflictModal).toContainText('2026-07-13');
  await expect(conflictModal).toContainText('2026-07-14');
  await expect(conflictModal).toContainText('2026-07-15');
  await expect(conflictModal).toContainText('2026-07-16');

  // 2. Click "İptal": neither record should change!
  await page.getByTestId('sick-leave-conflict-cancel').click();
  await expect(conflictModal).not.toBeVisible();

  // Verify sick leave was NOT saved
  const snapshotAfterCancel = await readStoredSnapshot(page);
  expect(
    (snapshotAfterCancel?.sickLeaveRecords ?? []).some((r) => r.startDate === '2026-07-13')
  ).toBe(false);
  // Verify puantaj was NOT changed
  const p1AttendanceJunAfterCancel = snapshotAfterCancel?.puantajlar?.find(
    (a) => a.personelId === 'p-1' && a.donemId === '2026-06'
  );
  expect(p1AttendanceJunAfterCancel?.gunler['2026-07-13']).toBe('Ç');

  // 3. Submit again and click "Uygula"
  await form.getByRole('button', { name: /Rapor Olayını Kaydet/i }).click();
  await expect(conflictModal).toBeVisible();
  await page.getByTestId('sick-leave-conflict-apply').click();
  await expect(conflictModal).not.toBeVisible();

  // Verify sick leave record is now listed in the table
  const table = page.getByTestId('period-settings-rapor').locator('table');
  await expect(table).toContainText('2026-07-13');
  await expect(table).toContainText('2026-07-16');

  // Verify puantaj in both 2026-06 and 2026-07 automatically became 'R'
  await expect
    .poll(async () => {
      const snap = await readStoredSnapshot(page);
      const jun = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-06');
      const jul = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-07');
      return {
        jul13: jun?.gunler['2026-07-13'],
        jul14: jun?.gunler['2026-07-14'],
        jul15: jul?.gunler['2026-07-15'],
        jul16: jul?.gunler['2026-07-16'],
      };
    })
    .toEqual({
      jul13: 'R',
      jul14: 'R',
      jul15: 'R',
      jul16: 'R',
    });

  // 4. Test edit: Edit sick leave to end on 2026-07-15 (trimming 2026-07-16)
  const initialRow = table.locator('tr', { hasText: '2026-07-13' });
  const editBtn = initialRow.getByRole('button', { name: /Rapor Olayını Düzenle/i });
  const deleteBtn = initialRow.getByRole('button', { name: /Rapor Olayını Sil/i });
  await expect(editBtn).toBeVisible();
  await expect(editBtn).toContainText('Düzenle');
  await expect(deleteBtn).toBeVisible();
  await expect(deleteBtn).toContainText('Sil');

  await editBtn.click();
  await expect(form.getByRole('button', { name: /Rapor Olayını Güncelle/i })).toBeVisible();
  await form.locator('input[type="date"]').nth(1).fill('2026-07-15');
  await form.getByRole('button', { name: /Rapor Olayını Güncelle/i }).click();
  await expect(table).toContainText('2026-07-15');

  // Verify 2026-07-16 was trimmed and restored to default ('Ç'), while 13..15 remain 'R'
  await expect
    .poll(async () => {
      const snap = await readStoredSnapshot(page);
      const jun = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-06');
      const jul = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-07');
      return {
        jul13: jun?.gunler['2026-07-13'],
        jul14: jun?.gunler['2026-07-14'],
        jul15: jul?.gunler['2026-07-15'],
        jul16: jul?.gunler['2026-07-16'],
      };
    })
    .toEqual({
      jul13: 'R',
      jul14: 'R',
      jul15: 'R',
      jul16: 'Ç',
    });

  // 4. A same-day duplicate of an existing report is rejected visibly and atomically.
  const beforeDuplicate = await readStoredSnapshot(page);
  await form.locator('input[type="date"]').nth(0).fill('2026-07-13');
  await form.locator('input[type="date"]').nth(1).fill('2026-07-13');
  await form.getByRole('button', { name: /Rapor Olayını Kaydet/i }).click();
  await expect(conflictModal).toBeVisible();
  await page.getByTestId('sick-leave-conflict-apply').click();
  const sickLeaveError = page.getByTestId('sick-leave-error-banner');
  await expect(sickLeaveError).toBeVisible();
  await expect(sickLeaveError).toContainText('Rapor tarihleri çakışıyor');
  await expect(conflictModal).toBeVisible();
  const afterDuplicate = await readStoredSnapshot(page);
  expect(afterDuplicate?.sickLeaveRecords).toEqual(beforeDuplicate?.sickLeaveRecords);
  expect(afterDuplicate?.puantajlar).toEqual(beforeDuplicate?.puantajlar);
  await page.getByTestId('sick-leave-conflict-cancel').click();

  // 5. Test delete preserves manual modification:
  // User navigates to puantaj and manually changes 2026-07-15 from 'R' to 'Ç'
  await page.getByTestId('nav-puantaj').click();
  await page.getByTestId('active-period-selector').selectOption('2026-07');
  await page.getByTestId('attendance-code-Ç').click();
  const jul15Cell = page.getByTestId('attendance-day-2026-07-15');
  await expect(jul15Cell).toBeVisible();
  await jul15Cell.click();

  // Verify manual change in snapshot: 2026-07-15 is now 'Ç'
  await expect
    .poll(async () => {
      const snap = await readStoredSnapshot(page);
      const jul = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-07');
      return jul?.gunler['2026-07-15'];
    })
    .toBe('Ç');

  // Now go back to Raporlar and delete the sick leave
  await page.getByTestId('nav-parametrelar').click();
  await page.getByTestId('nav-parametre-rapor').click();
  await expect(page.getByTestId('period-settings-rapor')).toBeVisible();

  // Find delete button on the row for 2026-07-13
  const row = table.locator('tr', { hasText: '2026-07-13' });
  await row.getByRole('button', { name: /Rapor Olayını Sil/i }).click();

  // Verify sick leave record is deleted
  await expect(table.locator('tr', { hasText: '2026-07-13' })).toHaveCount(0);

  // Verify dates still 'R' (2026-07-13, 2026-07-14, 2026-07-16) were restored to default ('Ç'),
  // while manually modified date (2026-07-15) remains 'Ç' and was not overwritten!
  await expect
    .poll(async () => {
      const snap = await readStoredSnapshot(page);
      const jun = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-06');
      const jul = snap?.puantajlar?.find((a) => a.personelId === 'p-1' && a.donemId === '2026-07');
      return {
        jul13: jun?.gunler['2026-07-13'],
        jul14: jun?.gunler['2026-07-14'],
        jul15: jul?.gunler['2026-07-15'],
        jul16: jul?.gunler['2026-07-16'],
      };
    })
    .toEqual({
      jul13: 'Ç',
      jul14: 'Ç',
      jul15: 'Ç',
      jul16: 'Ç',
    });
});
