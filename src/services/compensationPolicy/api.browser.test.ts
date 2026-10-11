import { describe, expect, test } from 'bun:test';
import * as bunTest from 'bun:test';
import { chromium, type Browser, type BrowserContext, type Page } from '@playwright/test';
import { createServer, type ViteDevServer } from 'vite';

let browser: Browser;
let context: BrowserContext;
let vite: ViteDevServer;
let origin = '';
let previousDisableHmr: string | undefined;
const pages: Page[] = [];
const acceptanceContexts: BrowserContext[] = [];
const { afterAll, afterEach, beforeAll } = bunTest as unknown as {
  afterAll(callback: () => void | Promise<void>, timeout?: number): void;
  afterEach(callback: () => void | Promise<void>, timeout?: number): void;
  beforeAll(callback: () => void | Promise<void>, timeout?: number): void;
};
const testWithTimeout = test as unknown as (
  name: string,
  callback: () => void | Promise<void>,
  timeout?: number
) => void;

beforeAll(async () => {
  previousDisableHmr = process.env.DISABLE_HMR;
  process.env.DISABLE_HMR = 'true';
  vite = await createServer({
    configFile: 'vite.config.ts',
    appType: 'custom',
    logLevel: 'error',
    server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false },
  });
  vite.middlewares.use('/__compensation_policy_api_test__', (_request, response) => {
    response.statusCode = 200;
    response.setHeader('content-type', 'text/html; charset=utf-8');
    response.end('<!doctype html><html lang="tr"><head></head><body></body></html>');
  });
  await vite.listen();
  const address = vite.httpServer?.address();
  if (!address || typeof address === 'string') throw new Error('Vite API test server did not bind a TCP port.');
  origin = `http://127.0.0.1:${address.port}`;
  browser = await chromium.launch({ headless: true });
  context = await browser.newContext();
}, 60_000);

afterEach(async () => {
  await Promise.all(pages.splice(0).map((page) => page.close()));
  await Promise.all(acceptanceContexts.splice(0).map((isolatedContext) => isolatedContext.close()));
}, 30_000);

afterAll(async () => {
  await context?.close();
  await browser?.close();
  await vite?.close();
  if (previousDisableHmr === undefined) delete process.env.DISABLE_HMR;
  else process.env.DISABLE_HMR = previousDisableHmr;
}, 30_000);

async function openPolicyApiTab(): Promise<Page> {
  const page = await context.newPage();
  pages.push(page);
  await page.goto(`${origin}/__compensation_policy_api_test__`);
  await page.addScriptTag({
    url: `${origin}/src/services/compensationPolicy/apiBrowserTestHarness.ts`,
    type: 'module',
  });
  await page.waitForFunction(() => Boolean(window.__compensationPolicyBrowserApiTest));
  return page;
}

async function installAcceptanceApplication(page: Page): Promise<void> {
  await page.addScriptTag({
    url: `${origin}/src/services/compensationPolicy/apiBrowserTestHarness.ts`,
    type: 'module',
  });
  await page.waitForFunction(() => Boolean(window.__compensationAcceptanceBrowserApp));
  await page.evaluate(() => window.__compensationAcceptanceBrowserApp.mount());
  await page.waitForSelector('[data-compensation-acceptance-app="mounted"]', { state: 'attached' });
}

async function openAcceptanceTab(): Promise<Page> {
  const isolatedContext = await browser.newContext();
  acceptanceContexts.push(isolatedContext);
  const page = await isolatedContext.newPage();
  pages.push(page);
  await page.goto(`${origin}/__compensation_policy_api_test__`);
  await installAcceptanceApplication(page);
  return page;
}

async function openMissingBaselineUiTab(): Promise<Page> {
  const page = await context.newPage();
  pages.push(page);
  await page.goto(`${origin}/__compensation_policy_api_test__`);
  await page.addScriptTag({
    url: `${origin}/src/services/compensationPolicy/apiBrowserTestHarness.ts`,
    type: 'module',
  });
  await page.waitForFunction(() => Boolean(window.__compensationAcceptanceBrowserApp));
  await page.evaluate(() => window.__compensationAcceptanceBrowserApp.mountMissingBaselineGate());
  await page.waitForSelector('[data-testid="missing-baseline-ui-app"][data-route="payroll"]');
  return page;
}

describe('browser CompensationPolicyApi IndexedDB integration', () => {
  test('loads the archived September compensation values exactly without writing a policy', async () => {
    const page = await openPolicyApiTab();
    const loaded = await page.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.loadArchivedSeptemberPrefill());
    expect(loaded.result.ok).toBe(true);
    if (loaded.result.ok === false) throw new Error(loaded.result.error.message);
    expect(loaded.result.value.initialBaselineRequired).toBe(true);
    expect(loaded.result.value.timeline.versions).toEqual([]);
    expect(loaded.result.value.nonAuthoritativePrefill.kind).toBe('AVAILABLE');
    if (loaded.result.value.nonAuthoritativePrefill.kind !== 'AVAILABLE') {
      throw new Error('The archived September settings were not returned as a draft.');
    }
    expect(loaded.result.value.nonAuthoritativePrefill.sourcePeriodId).toBe('2026-09');
    expect(loaded.result.value.nonAuthoritativePrefill.state).toMatchObject({
      gunlukTabanUcret: '2616.26',
      gunlukYemek: '300.75',
      birlestirilmisSosyalYardim: '5089.7',
      gunlukVasitaYol: '128.93',
      giyimYardimi: '269.7',
      hizmetZammiBirimi: '24.67',
      isPrimiYuzde: '0',
      geceCalismaPrimiYuzde: '0',
      geceCalismaTatiliPrimiYuzde: '0',
      ekOdeme: '0',
      digerGelirVarsayilan: '0',
      isPrimiGruplari: [
        { id: '1. Grup', ad: '1. Grup', oran: '9', aktif: true },
        { id: '2. Grup', ad: '2. Grup', oran: '8', aktif: true },
        { id: '3. Grup', ad: '3. Grup', oran: '7', aktif: true },
        { id: 'grup_1791176163314', ad: 'UX Deneme Grubu', oran: '6', aktif: true },
      ],
    });
    expect(loaded.beforeRevision).toBe(loaded.afterRevision);
  });

  test('blocks a January rounding-mode change, exposes an invalid legacy timeline, and repairs it through same-date correction', async () => {
    const page = await openPolicyApiTab();
    const result = await page.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.exerciseRoadRoundingRecovery());

    expect(result.rejected).toMatchObject({ code: 'INVALID_POLICY' });
    expect(result.rejected?.message).toContain('2028-01-01');
    expect(result.rejected?.message).toContain('road-rounding-december-baseline');
    expect(result.rejected?.message).toContain('__compensation-policy-preview__');
    expect(result.loadedInvalid).toMatchObject({
      resolution: 'INVALID_ROAD_ROUNDING',
      roadRoundingIssues: [{
        effectiveFrom: '2028-01-01',
        expectedMode: 'PERIOD_TOTAL',
        actualMode: 'PER_COMPENSATION_SLICE',
        expectedVersionId: 'road-rounding-december-baseline',
        actualVersionId: 'road-rounding-invalid-january-policy',
        periodStartInclusive: '2027-12-15',
        periodEndInclusive: '2028-01-14',
      }],
      error: null,
    });
    expect(result.correctionPreview).toBeNull();
    expect(result.saved).toMatchObject({
      version: {
        effectiveFrom: '2028-01-01',
        roadRoundingMode: 'PERIOD_TOTAL',
        supersedesVersionId: 'road-rounding-invalid-january-policy',
      },
    });
    expect(result.reloaded).toMatchObject({
      resolution: 'RESOLVED',
      roadRoundingIssues: [],
      error: null,
    });
    expect(result.reloaded.resolvedRoadRoundingModes.every((mode) => mode === 'PERIOD_TOTAL')).toBe(true);
    expect(result.reloaded.versions.at(-1)).toMatchObject({
      effectiveFrom: '2028-01-01',
      roadRoundingMode: 'PERIOD_TOTAL',
      supersedesVersionId: 'road-rounding-invalid-january-policy',
    });
    expect(result.scope).toMatchObject({
      periodStartInclusive: '2027-12-15',
      periodEndInclusive: '2028-01-14',
    });
  });

  testWithTimeout('previews, saves and reloads the four salary and ten ROAD_ONLY dates through 15–14 periods', async () => {
    const page = await openPolicyApiTab();
    const result = await page.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.exerciseSyntheticPolicySchedule());

    expect(result.salaryValues).toEqual([
      { date: '2027-02-01', value: '1070.80' },
      { date: '2027-09-01', value: '1193.45' },
      { date: '2028-02-01', value: '1268.04' },
      { date: '2028-09-01', value: '1388.50' },
    ]);
    expect(result.roadValues).toEqual([
      { date: '2027-03-15', value: '75.250' },
      { date: '2027-04-15', value: '75.375' },
      { date: '2027-05-15', value: '75.500' },
      { date: '2027-06-15', value: '75.625' },
      { date: '2027-07-15', value: '75.750' },
      { date: '2027-10-15', value: '75.875' },
      { date: '2027-11-15', value: '76.000' },
      { date: '2028-03-15', value: '76.125' },
      { date: '2028-05-15', value: '76.250' },
      { date: '2028-07-15', value: '76.375' },
    ]);
    expect(result).toMatchObject({
      savedVersionCount: 17,
      storedVersionCount: 17,
      resolution: 'RESOLVED',
      finalRoadMode: 'PERIOD_TOTAL',
      finalRoadAmount: '76.375',
      previewSaveRoutes: ['INITIAL_BASELINE', 'NEW_EFFECTIVE_DATE', 'SAME_DATE_CORRECTION'],
    });
  }, 90_000);

  testWithTimeout('single and batch NORMAL UI routes navigate to policy without saving when baseline is missing', async () => {
    const singlePage = await openMissingBaselineUiTab();
    const personId = await singlePage.locator('[data-testid="missing-baseline-ui-app"]').getAttribute('data-person-id');
    if (!personId) throw new Error('Missing-baseline UI fixture has no personnel id.');
    await singlePage.getByTestId(`calculate-payroll-${personId}`).click();
    await singlePage.waitForSelector('[data-testid="compensation-policy-screen"]');
    expect(await singlePage.locator('[data-testid="missing-baseline-ui-app"]').getAttribute('data-route')).toBe('policy');
    expect(await singlePage.locator('[data-testid="missing-baseline-ui-app"]').getAttribute('data-saved-count')).toBe('0');

    const batchPage = await openMissingBaselineUiTab();
    await batchPage.getByRole('button', { name: /Tüm Hesaplanabilir Bordroları Hesapla/ }).click();
    await batchPage.waitForSelector('[data-testid="compensation-policy-screen"]');
    expect(await batchPage.locator('[data-testid="missing-baseline-ui-app"]').getAttribute('data-route')).toBe('policy');
    expect(await batchPage.locator('[data-testid="missing-baseline-ui-app"]').getAttribute('data-saved-count')).toBe('0');
  }, 60_000);

  test('saves explicitly entered policy settings atomically, reloads, and rejects a stale cross-tab baseline', async () => {
    const firstTab = await openPolicyApiTab();
    const prepared = await firstTab.evaluate(() => window.__compensationPolicyBrowserApiTest.prepareBaseline());
    expect(prepared.preview.directCompensationEffects.length).toBeGreaterThan(0);
    expect(prepared.preview.dependencyClosureEffects.length).toBe(1);
    expect(prepared.preview.dependencyClosureEffects[0].action).toBe('CALCULATED_TO_STALE');
    expect(prepared.operation.state.gunlukTabanUcret).toBeTruthy();

    const secondTab = await openPolicyApiTab();
    const secondPrepared = await secondTab.evaluate((periodId) =>
      window.__compensationPolicyBrowserApiTest.previewBaseline(periodId), prepared.scope.periodId);
    expect(secondPrepared.preview.previewToken.policyTimelineVersionIds).toEqual([]);

    const committed = await secondTab.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.saveBaseline(value), secondPrepared);
    expect(committed.ok).toBe(true);

    const staleCommit = await firstTab.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.saveBaseline(value), prepared);
    expect(staleCommit.ok).toBe(false);
    if (staleCommit.ok === false) expect(staleCommit.error.code).toBe('CONFLICT');

    const authoritative = await firstTab.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.readAuthoritativeSnapshot());
    expect(authoritative?.policyVersionCount).toBe(1);
    expect(authoritative?.payrollStatuses).toEqual(['STALE']);
    expect(authoritative?.revision).toBeGreaterThan(1);
  });

  test('includes persisted retro dependencies in the token and stales the batch and linked event atomically', async () => {
    const page = await openPolicyApiTab();
    const prepared = await page.evaluate(() => window.__compensationPolicyBrowserApiTest.prepareRetroBaseline());
    const lifecycle = prepared.lifecycle;
    expect(prepared.preview.retroEffects).toEqual([{
      batchId: lifecycle.batchId,
      sourcePeriodIds: [lifecycle.sourcePeriodId],
      linkedPaymentEventIds: [lifecycle.paymentEventId],
      action: 'STALE_ELIGIBLE',
      blockers: [],
    }]);
    expect(prepared.preview.previewToken.retroDependencyStates.length).toBe(1);
    expect(prepared.preview.previewToken.retroDependencyStates[0].persistedState.allocations[0].id)
      .toBe(lifecycle.allocationIds[0]);
    expect(prepared.preview.previewToken.retroDependencyStates[0].persistedState.paymentEvents[0].id)
      .toBe(lifecycle.paymentEventId);
    expect(prepared.preview.previewToken.retroDependencyStates[0].persistedState.sourcePayrolls
      .find((payroll) => payroll.accrualType === 'NORMAL')?.id)
      .toBe(lifecycle.sourcePayrollId);

    const committed = await page.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.savePreparedOperation(value), prepared);
    if (committed.ok === false) {
      throw new Error(`Retro baseline commit failed: ${committed.error.code}: ${committed.error.message}`);
    }
    expect(committed.ok).toBe(true);
    if (committed.ok) {
      expect(committed.value.invalidatedRetroBatches).toEqual([{
        batchId: lifecycle.batchId,
        fromStatus: 'CALCULATED',
        toStatus: 'STALE',
      }]);
      expect(committed.value.invalidatedRetroPaymentEvents.length).toBe(1);
    }
    const authoritative = await page.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.readAuthoritativeSnapshot());
    expect(authoritative?.retroBatchStatuses).toEqual([{
      id: lifecycle.batchId,
      status: 'STALE',
    }]);
    expect(authoritative?.payrollStatuses).toEqual(['STALE', 'STALE']);
  });

  testWithTimeout('saves work-premium groups as a dated immutable policy correction and stales payroll and retro dependents', async () => {
    const page = await openPolicyApiTab();
    const result = await page.evaluate(() =>
      window.__compensationPolicyBrowserApiTest.exerciseWorkPremiumGroupSave());

    expect(result.result.ok).toBe(true);
    expect(result.versions).toHaveLength(2);
    expect(result.versions[0]).toEqual(result.original);
    expect(result.versions[1]).toMatchObject({
      effectiveFrom: result.original.effectiveFrom,
      supersedesVersionId: result.original.id,
      state: { isPrimiGruplari: [{ id: '1. Grup', ad: 'Grup 1', oran: '11', aktif: true }] },
    });
    expect(result.groups).toEqual([{ id: '1. Grup', ad: 'Grup 1', oran: '11', aktif: true }]);
    expect(result.payrollStatuses).toEqual(['STALE', 'STALE']);
    expect(result.retroStatuses).toEqual(['STALE']);
  }, 60_000);

  test('ROAD_ONLY accepts decimal-equivalent non-road values with different scales', async () => {
    const page = await openPolicyApiTab();
    const baseline = await page.evaluate(() => window.__compensationPolicyBrowserApiTest.prepareBaseline());
    const savedBaseline = await page.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.saveBaseline(value), baseline);
    expect(savedBaseline.ok).toBe(true);

    const roadChange = await page.evaluate((periodId) =>
      window.__compensationPolicyBrowserApiTest.previewRoadOnlyScaleChange(periodId), baseline.scope.periodId);
    expect(roadChange.preview.directCompensationEffects[0].changedFields).toEqual(['gunlukVasitaYol']);
    const savedRoadChange = await page.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.savePreparedOperation(value), roadChange);
    if (savedRoadChange.ok === false) {
      throw new Error(`Browser ROAD_ONLY save failed: ${savedRoadChange.error.code}: ${savedRoadChange.error.message}`);
    }
    expect(savedRoadChange.ok).toBe(true);
  });

  test('aborted IndexedDB put rolls back policy, payroll, retro, and allocation state', async () => {
    const page = await openPolicyApiTab();
    const prepared = await page.evaluate(() => window.__compensationPolicyBrowserApiTest.prepareRetroBaseline());
    const before = await page.evaluate(() => window.__compensationPolicyBrowserApiTest.readRawSnapshotState());
    const aborted = await page.evaluate((value) =>
      window.__compensationPolicyBrowserApiTest.saveOperationWithAbortedPut(value), prepared);
    expect(aborted.didAbort).toBe(true);
    expect(aborted.result.ok).toBe(false);
    const after = await page.evaluate(() => window.__compensationPolicyBrowserApiTest.readRawSnapshotState());
    expect(after).toEqual(before);
  });

  testWithTimeout('runs 12 periods through WASM, restores V6 into sentinel IndexedDB, then reloads and calculates normally and retroactively', async () => {
    const page = await openAcceptanceTab();
    const history = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.runTwelvePeriodSeries()
    );
    expect(history.payrollCount).toBe(12);
    expect(history.periods).toEqual([
      '2026-09', '2026-10', '2026-11', '2026-12',
      '2027-01', '2027-02', '2027-03', '2027-04',
      '2027-05', '2027-06', '2027-07', '2027-08',
    ]);
    expect(history.statuses).toEqual(Array(12).fill('FINALIZED'));
    expect(history.runtimeRaiseCalendarAbsent).toBe(true);
    expect(String(history.annual2027Cap)).toBe('180000');
    expect(String(history.taxOpening2026)).toBe('1234.567');
    expect(history.marchSegments.map(({ startDate, endDate, versionId }) => ({ startDate, endDate, versionId }))).toEqual([
      { startDate: '2027-02-15', endDate: '2027-02-28', versionId: history.versionIds[0] },
      { startDate: '2027-03-01', endDate: '2027-03-14', versionId: history.versionIds[1] },
    ]);
    expect(history.septemberSegments.map(({ startDate, endDate, versionId }) => ({ startDate, endDate, versionId }))).toEqual([
      { startDate: '2027-08-15', endDate: '2027-08-31', versionId: history.versionIds[1] },
      { startDate: '2027-09-01', endDate: '2027-09-14', versionId: history.versionIds[2] },
    ]);

    const sentinel = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.persistSentinelState()
    );
    expect(sentinel.personName).toBe('SENTINEL BEFORE RESTORE');
    expect(sentinel.payrollStatuses).toEqual([]);

    const restored = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.importV6BackupOverSentinel()
    );
    expect(restored.sentinelReplaced).toBe(true);
    expect(restored.hasLoadedBackup).toBe(true);
    expect(restored.payrollStatuses).toEqual(Array(12).fill('FINALIZED'));
    expect(restored.policyVersionIds.length).toBe(3);

    await page.reload();
    await installAcceptanceApplication(page);
    await page.waitForFunction(() => {
      const summary = window.__compensationAcceptanceBrowserApp.read();
      return summary.loadError !== null || (summary.isDataLoaded && summary.payrollStatuses.length === 12);
    }, { timeout: 120_000 });
    const freshPageSummary = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.read()
    );
    expect(freshPageSummary.loadError).toBe(null);
    expect(freshPageSummary.payrollStatuses).toEqual(Array(12).fill('FINALIZED'));
    expect(freshPageSummary.policyVersionIds).toEqual(history.versionIds);

    const postRestore = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.runAfterFreshRestore()
    );
    expect(postRestore.freshEngineKind).toBe('wasm');
    expect(postRestore.normalStatus).toBe('FINALIZED');
    expect(postRestore.normalPolicyVersionIds).toEqual([history.versionIds[2]]);
    expect(postRestore.retroStatus).toBe('CALCULATED');
    expect(postRestore.retroAllocationCount).toBeGreaterThan(0);
    expect(/^[0-9]+(?:\.[0-9]+)?$/.test(postRestore.retroPayable)).toBe(true);
    expect(postRestore.deletedPeriodId).toBe('2028-01');
    expect(postRestore.policyTimelineUnchanged).toBe(true);
  }, 180_000);

  testWithTimeout('restores a V5 backup without policy history, fails closed, then calculates after an API baseline save', async () => {
    const page = await openAcceptanceTab();
    const restored = await page.evaluate(() =>
      window.__compensationAcceptanceBrowserApp.runV5RestoreBaselineGate()
    );
    expect(/ücret politikası|policy/i.test(restored.missingBaselineError)).toBe(true);
    expect(restored.policyVersionCount).toBe(1);
    expect(restored.calculatedStatus).toBe('CALCULATED');
    expect(restored.calculatedVersionIds).toEqual([restored.baselineVersionId]);
  }, 120_000);
});
