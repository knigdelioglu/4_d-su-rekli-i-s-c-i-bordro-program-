import { describe, expect, test } from 'bun:test';
import * as bunTest from 'bun:test';
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import { createServer, type ViteDevServer } from 'vite';
import type { Server as NodeHttpServer } from 'node:http';

const { afterAll, afterEach, beforeAll } = bunTest as unknown as {
  afterAll(callback: () => void | Promise<void>, timeout?: number): void;
  afterEach(callback: () => void | Promise<void>, timeout?: number): void;
  beforeAll(callback: () => void | Promise<void>, timeout?: number): void;
};

let browser: Browser;
let pageUnderTest: Page | null = null;
let harnessBundle = '';
let vite: ViteDevServer;
let viteOrigin = '';
let previousDisableHmr: string | undefined;

beforeAll(async () => {
  const result = await build({
    entryPoints: ['src/components/Settings/CompensationPolicyPageTestHarness.tsx'],
    bundle: true,
    format: 'iife',
    platform: 'browser',
    target: 'chrome120',
    write: false,
    jsx: 'automatic',
    logLevel: 'silent',
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  harnessBundle = result.outputFiles[0]?.text ?? '';
  if (!harnessBundle) throw new Error('The rendered component test harness did not bundle.');
  previousDisableHmr = process.env.DISABLE_HMR;
  process.env.DISABLE_HMR = 'true';
  vite = await createServer({
    configFile: 'vite.config.ts',
    appType: 'custom',
    logLevel: 'error',
    server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false },
  });
  await vite.listen();
  const address = vite.httpServer?.address();
  if (!address || typeof address === 'string') throw new Error('The rendered component test CSS server did not bind a TCP port.');
  viteOrigin = `http://127.0.0.1:${address.port}`;
  browser = await chromium.launch({ headless: true });
}, 30_000);

afterEach(async () => {
  await pageUnderTest?.close();
  pageUnderTest = null;
}, 30_000);

afterAll(async () => {
  await browser?.close();
  (vite?.httpServer as NodeHttpServer | undefined)?.closeAllConnections();
  await vite?.close();
  if (previousDisableHmr === undefined) delete process.env.DISABLE_HMR;
  else process.env.DISABLE_HMR = previousDisableHmr;
}, 30_000);

async function openPolicyPage(): Promise<Page> {
  pageUnderTest = await browser.newPage();
  pageUnderTest.setDefaultTimeout(5_000);
  await pageUnderTest.setContent('<!doctype html><html lang="tr"><body><div id="root"></div></body></html>');
  await pageUnderTest.addStyleTag({ url: `${viteOrigin}/src/index.css` });
  await pageUnderTest.addScriptTag({ content: harnessBundle });
  await pageUnderTest.locator('#compensation-policy-title').waitFor({ state: 'visible' });
  return pageUnderTest;
}

async function events(page: Page): Promise<Array<{ type: string; value?: unknown }>> {
  return page.evaluate(() => window.__compensationPolicyTest.events);
}

async function selectBaselinePolicyFlags(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await page.locator('#policy-calculation-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });
  await page.getByRole('radio', { name: scope === 'ROAD_ONLY' ? 'Yalnız yol tutarı' : 'Tüm ücret kalemleri' }).check();
  await page.getByRole('radio', { name: roadMode === 'PERIOD_TOTAL' ? 'Dönem toplamı bir kez' : 'Her kazanç diliminde ayrı' }).check();
}

async function openAdvancedValues(page: Page): Promise<void> {
  await page.locator('#policy-oneoff-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });
  await page.locator('#policy-legacy-premium-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });
}

async function prepareValidInitialBaseline(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await selectBaselinePolicyFlags(page, scope, roadMode);
  await page.locator('#policy-reason').fill('Eylül 2026 başlangıç ücretleri');
}

async function selectExistingChangeFlags(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await page.locator('#policy-calculation-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });
  await page.getByRole('radio', { name: scope === 'ROAD_ONLY' ? 'Yalnız yol tutarı' : 'Tüm ücret kalemleri' }).check();
  await page.getByRole('radio', { name: roadMode === 'PERIOD_TOTAL' ? 'Dönem toplamı bir kez' : 'Her kazanç diliminde ayrı' }).check();
}

async function requestPreview(page: Page): Promise<void> {
  await page.getByRole('button', { name: /Etkileri önizle|Önizlemeyi yenile/ }).click();
  await page.getByRole('heading', { name: '3. Etkileri inceleyin' }).waitFor({ state: 'visible' });
}

async function prepareExistingDateCorrection(page: Page): Promise<void> {
  await page.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
  await page.locator('#policy-operation').selectOption('SAME_DATE_CORRECTION');
  await page.locator('#policy-effective-date').selectOption('2026-09-15');
  await selectExistingChangeFlags(page);
}

describe('rendered CompensationPolicyPage interactions with frozen props', () => {
  test('shows a quarantine error in user language and retries loading the period', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.setLoadIdle());
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('quarantine'));
    await page.getByRole('button', { name: 'Ücret geçmişini yükle' }).click();
    await page.getByText('Ücret geçmişindeki bir kayıt doğrulanamıyor.').waitFor({ state: 'visible' });
    const errorText = await page.locator('[role="alert"]').last().innerText();
    expect(errorText.includes('2026-09-15')).toBe(true);
    expect(errorText.includes('yol yuvarlama biçimi eksik')).toBe(true);
    const recorded = await events(page);
    expect(recorded.filter((entry) => entry.type === 'load').length).toBe(1);

    await page.evaluate(() => window.__compensationPolicyTest.setScenario('success'));
    await page.getByRole('button', { name: 'Ücret geçmişini yeniden yükle' }).click();
    await page.locator('#policy-base-wage').waitFor({ state: 'visible' });
    expect(await page.locator('#policy-base-wage').inputValue()).toBe('2616,26');
    expect((await events(page)).filter((entry) => entry.type === 'load').length).toBe(2);
  });

  test('loads the available draft immediately, confirms the exact date/state, resets consent on date change, and supports cancel', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.usePrecisionPrefill());

    expect(await page.locator('#policy-base-wage').inputValue()).toBe('9007199254740993,12');
    expect(await page.locator('#policy-meal').inputValue()).toBe('50,00');
    await openAdvancedValues(page);
    expect(await page.getByRole('checkbox', { name: 'Ek ödeme tutarı tanımla' }).isChecked()).toBe(true);
    expect(await page.locator('#policy-extra-payment').inputValue()).toBe('0');
    expect(await page.getByRole('checkbox', { name: 'Varsayılan tutar tanımla' }).isChecked()).toBe(false);
    expect(await page.locator('#policy-other-income').count()).toBe(0);
    await page.locator('details').filter({ has: page.locator('#policy-group-id-1') }).locator('summary').click();
    expect(await page.locator('#policy-group-id-1').inputValue()).toBe('B');
    expect(await page.locator('#policy-group-rate-1').inputValue()).toBe('0');
    expect(await page.getByRole('checkbox', { name: 'Teknik etkin' }).isChecked()).toBe(false);
    expect(await page.locator('#policy-reason').inputValue()).toBe('');
    expect(await page.getByRole('alert').count()).toBe(0);

    await selectBaselinePolicyFlags(page);
    await page.locator('#policy-reason').fill('Başlangıç ücretleri');
    await requestPreview(page);
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).count()).toBe(0);
    await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'confirm-baseline').length).toBe(1);
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).isEnabled()).toBe(true);

    await page.locator('#policy-effective-date').fill('2026-09-16');
    await page.getByText('Taslak önizlemeden sonra değişti. Kaydetmeden önce güncel etkileri yeniden önizleyin.').waitFor({ state: 'visible' });
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).count()).toBe(0);
    await requestPreview(page);

    await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).click();
    await page.getByRole('button', { name: 'Onayı kaldır' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'cancel-baseline').length).toBe(1);
    expect((await events(page)).filter((entry) => entry.type === 'confirm-baseline').length).toBe(2);
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(2);
  });

  test('shows the exact 2026 September institution values as a draft without creating or confirming a policy', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useEylulPrefill());

    expect(await page.locator('#policy-base-wage').inputValue()).toBe('2616,26');
    expect(await page.locator('#policy-meal').inputValue()).toBe('300,75');
    expect(await page.locator('#policy-social').inputValue()).toBe('5089,7');
    expect(await page.locator('#policy-road').inputValue()).toBe('128,93');
    expect(await page.locator('#policy-clothing').inputValue()).toBe('269,7');
    expect(await page.locator('#policy-service-unit').inputValue()).toBe('24,67');
    expect(await page.locator('#policy-global-premium').inputValue()).toBe('0');
    expect(await page.locator('#policy-extra-payment').inputValue()).toBe('0');
    expect(await page.locator('#policy-other-income').inputValue()).toBe('0');
    expect(await page.locator('#policy-group-name-3').inputValue()).toBe('UX Deneme Grubu');
    expect(await page.locator('#policy-group-rate-3').inputValue()).toBe('6');
    expect(await page.locator('input[name="policy-boundary-scope"]:checked').count()).toBe(0);
    expect(await page.locator('input[name="policy-road-rounding"]:checked').count()).toBe(0);
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).count()).toBe(0);
    expect((await events(page)).filter((entry) => entry.type === 'confirm-baseline').length).toBe(0);
    const visibleText = await page.locator('body').innerText();
    expect(visibleText.includes('Bu işlem kayıt oluşturmadı.')).toBe(true);
    for (const internalLabel of ['FULL_EARNING', 'ROAD_ONLY', 'STALE', 'settlement', 'Batch', 'null']) {
      expect(visibleText.includes(internalLabel)).toBe(false);
    }
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(0);

  });

  test('keeps approval and actions in the real app viewport without covering form content at start, middle, and end', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useEylulPrefill());
    const viewports = [
      { label: 'desktop', width: 1366, height: 768 },
      { label: 'narrow', width: 390, height: 844 },
    ] as const;

    for (const viewport of viewports) {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      await page.locator('[data-testid="compensation-policy-scroll-region"]').evaluate((node) => { node.scrollTop = 0; });
      await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
      if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
        const screenshotPath = viewport.label === 'desktop'
          ? '/private/tmp/compensation-policy-ux-initial-desktop.png'
          : '/private/tmp/compensation-policy-ux-initial-narrow.png';
        await page.screenshot({ path: screenshotPath });
      }

      for (const position of ['start', 'middle', 'end'] as const) {
        const metrics = await page.evaluate(async (scrollPosition) => {
          const scrollRegion = document.querySelector<HTMLElement>('[data-testid="compensation-policy-scroll-region"]');
          const footer = document.querySelector<HTMLElement>('[data-testid="compensation-policy-action-bar"]');
          const appMain = document.querySelector<HTMLElement>('[data-testid="application-main-content"]');
          const contentEnd = document.querySelector<HTMLElement>('[data-testid="compensation-policy-content-end"]');
          if (!scrollRegion || !footer || !appMain || !contentEnd) throw new Error('The app viewport test fixture is incomplete.');
          const maxScroll = scrollRegion.scrollHeight - scrollRegion.clientHeight;
          scrollRegion.scrollTop = scrollPosition === 'start' ? 0 : scrollPosition === 'middle' ? maxScroll / 2 : maxScroll;
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
          const rect = (element: Element) => {
            const bounds = element.getBoundingClientRect();
            return { top: bounds.top, bottom: bounds.bottom, left: bounds.left, right: bounds.right, width: bounds.width, height: bounds.height };
          };
          return {
            maxScroll,
            scrollTop: scrollRegion.scrollTop,
            viewportHeight: window.innerHeight,
            documentHeight: document.documentElement.scrollHeight,
            appMain: rect(appMain),
            scrollRegion: rect(scrollRegion),
            footer: rect(footer),
            contentEnd: rect(contentEnd),
            buttons: Array.from(footer.querySelectorAll('button')).map((button) => ({
              ...rect(button),
              display: getComputedStyle(button).display,
              visibility: getComputedStyle(button).visibility,
            })),
          };
        }, position);

        if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
          console.log(`[viewport-rects] ${viewport.label}/${position} ${JSON.stringify({
            viewport: { width: viewport.width, height: viewport.height },
            documentHeight: metrics.documentHeight,
            maxScroll: metrics.maxScroll,
            scrollTop: metrics.scrollTop,
            appMain: metrics.appMain,
            scrollRegion: metrics.scrollRegion,
            footer: metrics.footer,
            buttons: metrics.buttons,
            contentEnd: metrics.contentEnd,
          })}`);
        }

        expect(metrics.maxScroll).toBeGreaterThan(0);
        if (position === 'start') expect(metrics.scrollTop).toBe(0);
        if (position === 'middle') {
          expect(metrics.scrollTop).toBeGreaterThan(0);
          expect(metrics.scrollTop).toBeLessThan(metrics.maxScroll);
        }
        if (position === 'end') expect(metrics.scrollTop).toBe(metrics.maxScroll);
        expect(metrics.documentHeight <= metrics.viewportHeight + 1).toBe(true);
        expect(metrics.scrollRegion.bottom <= metrics.footer.top + 1).toBe(true);
        expect(metrics.footer.top >= metrics.appMain.top).toBe(true);
        expect(metrics.footer.bottom <= metrics.appMain.bottom + 1).toBe(true);
        expect(metrics.footer.top >= 0).toBe(true);
        expect(metrics.footer.bottom <= metrics.viewportHeight).toBe(true);
        expect(metrics.footer.left >= 0).toBe(true);
        expect(metrics.footer.right <= viewport.width).toBe(true);
        expect(metrics.buttons.length).toBeGreaterThan(0);
        for (const button of metrics.buttons) {
          expect(button.width).toBeGreaterThan(0);
          expect(button.height).toBeGreaterThan(0);
          expect(button.display).not.toBe('none');
          expect(button.visibility).toBe('visible');
          expect(button.top >= metrics.footer.top - 1).toBe(true);
          expect(button.bottom <= metrics.footer.bottom + 1).toBe(true);
          expect(button.top >= 0).toBe(true);
          expect(button.bottom <= metrics.viewportHeight).toBe(true);
          expect(button.left >= 0).toBe(true);
          expect(button.right <= viewport.width).toBe(true);
        }
        if (position === 'end') {
          expect(metrics.contentEnd.top >= metrics.scrollRegion.top - 1).toBe(true);
          expect(metrics.contentEnd.bottom <= metrics.scrollRegion.bottom + 1).toBe(true);
          if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1' && viewport.label === 'narrow') {
            await page.locator('[data-testid="compensation-policy-action-bar"]').screenshot({ path: '/private/tmp/compensation-policy-ux-narrow-actions.png' });
          }
        }
      }
    }
  });

  test('keeps the four premium groups editable and readable at laptop and narrow widths', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useEylulPrefill());
    const viewports = [
      { label: 'desktop', width: 1366, height: 768 },
      { label: 'narrow', width: 390, height: 844 },
    ] as const;

    for (const viewport of viewports) {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      const table = page.getByTestId('policy-premium-group-table');
      await table.scrollIntoViewIfNeeded();
      const rows = page.getByTestId('policy-premium-group-row');
      if (viewport.label === 'narrow') await page.locator('#policy-group-rate-0').fill('9');
      expect(await rows.count()).toBe(4);
      expect(await page.locator('#policy-group-name-0').inputValue()).toBe('1. Grup');
      expect(await page.locator('#policy-group-rate-0').inputValue()).toBe('9');
      expect(await page.locator('#policy-group-name-3').inputValue()).toBe('UX Deneme Grubu');
      expect(await page.locator('#policy-group-rate-3').inputValue()).toBe('6');

      await page.locator('#policy-group-rate-0').fill('9,5');
      expect(await page.locator('#policy-group-rate-0').inputValue()).toBe('9,5');
      const layout = await page.evaluate(() => {
        const region = document.querySelector<HTMLElement>('[data-testid="compensation-policy-scroll-region"]');
        const tableElement = document.querySelector<HTMLElement>('[data-testid="policy-premium-group-table"]');
        const rect = (element: Element) => {
          const bounds = element.getBoundingClientRect();
          return { top: bounds.top, bottom: bounds.bottom, left: bounds.left, right: bounds.right };
        };
        if (!region || !tableElement) throw new Error('The group table viewport fixture is incomplete.');
        return {
          region: rect(region),
          table: rect(tableElement),
          scrollWidth: region.scrollWidth,
          clientWidth: region.clientWidth,
        };
      });

      if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
        console.log(`[groups-layout] ${viewport.label} ${JSON.stringify(layout)}`);
      }
      expect(layout.table.top >= layout.region.top - 1).toBe(true);
      expect(layout.table.bottom <= layout.region.bottom + 1).toBe(true);
      expect(layout.scrollWidth <= layout.clientWidth + 1).toBe(true);
      expect(await page.getByRole('button', { name: 'UX Deneme Grubu grubunu kaldır' }).isVisible()).toBe(true);
      expect(await page.locator('details summary').filter({ hasText: 'Kod' }).count()).toBe(4);
      expect((await page.locator('body').innerText()).includes('grup_1791176163314')).toBe(false);

      if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
        const path = viewport.label === 'desktop'
          ? '/private/tmp/compensation-policy-ux-groups-desktop.png'
          : '/private/tmp/compensation-policy-ux-groups-narrow.png';
        await page.screenshot({ path });
      }
    }
  });

  test('reopens and restores a saved ROAD_ONLY and PERIOD_TOTAL policy without asking for new selections', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.usePersistedRoadOnlyPolicy());
    await page.locator('#policy-calculation-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });
    const roadOnly = page.locator('input[name="policy-boundary-scope"][value="ROAD_ONLY"]');
    const periodTotal = page.locator('input[name="policy-road-rounding"][value="PERIOD_TOTAL"]');
    expect(await roadOnly.isChecked()).toBe(true);
    expect(await periodTotal.isChecked()).toBe(true);
    expect(await page.locator('#policy-base-wage').inputValue()).toBe('9007199254740993,12');

    await page.locator('input[name="policy-boundary-scope"][value="FULL_EARNING"]').check();
    await page.locator('input[name="policy-road-rounding"][value="PER_COMPENSATION_SLICE"]').check();
    await page.locator('#policy-base-wage').fill('9007199254740994');
    await page.getByRole('button', { name: 'Etkin ücretleri ve seçimleri taslağa geri al' }).click();
    expect(await roadOnly.isChecked()).toBe(true);
    expect(await periodTotal.isChecked()).toBe(true);
    expect(await page.locator('#policy-base-wage').inputValue()).toBe('9007199254740993,12');

    await requestPreview(page);
    const operation = (await events(page)).filter((entry) => entry.type === 'preview').at(-1)?.value as {
      boundaryScope: string;
      roadRoundingMode: string;
      state: { gunlukTabanUcret: string };
    };
    expect(operation.boundaryScope).toBe('ROAD_ONLY');
    expect(operation.roadRoundingMode).toBe('PERIOD_TOTAL');
    expect(operation.state.gunlukTabanUcret).toBe('9007199254740993.12');
  });

  test('same-date correction copies the selected target version values and ROAD_ONLY/PERIOD_TOTAL flags', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useCorrectionPreferenceTimeline());
    await page.locator('#policy-operation').selectOption('SAME_DATE_CORRECTION');
    await page.locator('#policy-calculation-details').evaluate((node) => { (node as HTMLDetailsElement).open = true; });

    expect(await page.locator('input[name="policy-boundary-scope"][value="ROAD_ONLY"]').isChecked()).toBe(true);
    expect(await page.locator('input[name="policy-road-rounding"][value="PERIOD_TOTAL"]').isChecked()).toBe(true);
    expect(await page.locator('#policy-effective-date').inputValue()).toBe('2026-10-01');
    expect(await page.locator('#policy-base-wage').inputValue()).toBe('3141,59');
    expect(await page.locator('#policy-road').inputValue()).toBe('175,25');

    await requestPreview(page);
    const operation = (await events(page)).filter((entry) => entry.type === 'preview').at(-1)?.value as {
      kind: string;
      supersedesVersionId: string;
      boundaryScope: string;
      roadRoundingMode: string;
      state: { gunlukTabanUcret: string; gunlukVasitaYol: string };
    };
    expect(operation.kind).toBe('SAME_DATE_CORRECTION');
    expect(operation.supersedesVersionId).toBe('preference-target');
    expect(operation.boundaryScope).toBe('ROAD_ONLY');
    expect(operation.roadRoundingMode).toBe('PERIOD_TOTAL');
    expect(operation.state.gunlukTabanUcret).toBe('3141.59');
    expect(operation.state.gunlukVasitaYol).toBe('175.25');
  });

  test('applies an exact percentage helper, keeps the source state intact, and previews a full ROAD_ONLY baseline', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.usePrecisionPrefill());
    await prepareValidInitialBaseline(page, 'ROAD_ONLY', 'PERIOD_TOTAL');
    await page.getByText('Günlük taban ücret ve yemeği yüzdeyle artır', { exact: true }).click();
    await page.locator('#policy-percentage-helper').fill('10');
    await page.getByRole('button', { name: 'Artışı uygula' }).click();

    expect(await page.locator('#policy-base-wage').inputValue()).toBe('9907919180215092,432');
    expect(await page.locator('#policy-meal').inputValue()).toBe('55');
    expect(await page.locator('#policy-clothing').inputValue()).toBe('0');
    expect((await page.getByRole('status').last().innerText()).includes('Diğer ücret alanları değişmedi')).toBe(true);

    const draftEvents = (await events(page)).filter((entry) => entry.type === 'draft');
    const source = draftEvents[0]?.value as { gunlukTabanUcret: string; giyimYardimi: string };
    const changed = draftEvents.at(-1)?.value as { gunlukTabanUcret: string; gunlukYemek: string; giyimYardimi: string };
    expect(source.gunlukTabanUcret).toBe('9007199254740993,12');
    expect(changed.gunlukTabanUcret).toBe('9907919180215092,432');
    expect(changed.gunlukYemek).toBe('55');
    expect(changed.giyimYardimi).toBe('0');

    await requestPreview(page);
    await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).click();
    const previewEvent = (await events(page)).filter((entry) => entry.type === 'preview').at(-1);
    const operation = previewEvent?.value as {
      kind: string;
      boundaryScope: string;
      roadRoundingMode: string;
      state: Record<string, unknown>;
    };
    expect(operation.kind).toBe('INITIAL_BASELINE');
    expect(operation.boundaryScope).toBe('ROAD_ONLY');
    expect(operation.roadRoundingMode).toBe('PERIOD_TOTAL');
    expect('changeIntent' in operation).toBe(false);
    expect(operation.state).toEqual({
      gunlukTabanUcret: '9907919180215092.432',
      gunlukYemek: '55',
      birlestirilmisSosyalYardim: '20',
      gunlukVasitaYol: '30',
      giyimYardimi: '0',
      hizmetZammiBirimi: '5',
      isPrimiYuzde: null,
      isPrimiGruplari: [{ id: 'A', ad: 'Atesli', oran: '65.5', aktif: true }, { id: 'B', ad: 'Teknik', oran: '0', aktif: false }],
      geceCalismaPrimiYuzde: '20',
      geceCalismaTatiliPrimiYuzde: '30',
      ekOdeme: '0',
      digerGelirVarsayilan: null,
    });

    await page.evaluate(() => window.__compensationPolicyTest.setScenario('reloadwait'));
    await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(1);
    const saveEvent = (await events(page)).filter((entry) => entry.type === 'save').at(-1);
    const saveRequest = saveEvent?.value as { initialBaselineConfirmation?: { confirmed: boolean; effectiveFrom: string } };
    expect(saveRequest.initialBaselineConfirmation).toEqual({ confirmed: true, effectiveFrom: '2026-09-15' });
    await page.getByText('Kayıt tamamlandı; ücret bilgileri yeniden yükleniyor…').waitFor({ state: 'visible' });
    expect(await page.getByText('Ücret değişikliği kaydedildi.').count()).toBe(0);
    await page.getByText('Ücret değişikliği kaydedildi.').waitFor({ state: 'visible' });
  });

  test('shows eligible retro effects in preview and reports stale batch and payment IDs after save', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('retro'));

    await requestPreview(page);
    const previewText = await page.locator('body').innerText();
    expect(previewText.includes('Etkilenen geçmiş farkı kayıtları')).toBe(true);
    expect(previewText.includes('Geçmiş farkı kaydı')).toBe(true);
    expect(previewText.includes('Kaynak dönemler: Ağustos 2026 · bağlı ödeme kaydı: 1')).toBe(true);
    expect(previewText.includes('yeniden hesaplanması gerekecek')).toBe(true);

    await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).click();
    await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).click();
    await page.getByText('Yeniden hesaplanması gereken geçmiş farkı kaydı: 1.').waitFor({ state: 'visible' });
    await page.getByText('Yeniden incelenmesi gereken bağlı ödeme kaydı: 1.').waitFor({ state: 'visible' });
  });

  test('names immutable retro blockers returned by preview', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('retroblocker'));
    await page.getByRole('button', { name: /Etkileri önizle|Önizlemeyi yenile/ }).click();

    await page.getByText('Geçmiş farkı kaydı bu değişikliği engelliyor.').waitFor({ state: 'visible' });
    const blockerText = await page.locator('[role="alert"]').last().innerText();
    expect(blockerText.includes('geçmiş farkı kaydı kesinleştirilmiş')).toBe(true);
    expect(blockerText.includes('ödeme tamamlanmış')).toBe(true);
  });

  test('keeps invalid full-state drafts out of preview/save and focuses the rendered error summary', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.locator('#policy-base-wage').fill('');
    await page.getByRole('button', { name: /Etkileri önizle/ }).click();

    await page.getByRole('heading', { name: 'Önizleme için formu tamamlayın' }).waitFor({ state: 'visible' });
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(0);
    await page.waitForFunction(() => document.activeElement?.getAttribute('tabindex') === '-1');
    expect(await page.evaluate(() => document.activeElement?.getAttribute('tabindex'))).toBe('-1');
    expect(await page.locator('#policy-base-wage').getAttribute('aria-invalid')).toBe('true');
  });

  test('opens calculation details and focuses the first missing decision when preview is requested', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useEylulPrefill());
    await page.locator('#policy-reason').fill('Başlangıç ücretleri');

    expect(await page.locator('#policy-calculation-details-summary').innerText()).toContain('Seçim gerekli');
    await page.getByRole('button', { name: /Etkileri önizle/ }).click();

    await page.waitForFunction(() => (document.getElementById('policy-calculation-details') as HTMLDetailsElement | null)?.open === true);
    await page.waitForFunction(() => document.activeElement?.id === 'policy-boundary-scope-full');
    expect(await page.locator('#policy-boundary-scope-error').isVisible()).toBe(true);
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    const focusLayout = await page.evaluate(() => {
      const target = document.getElementById('policy-boundary-scope-full')?.getBoundingClientRect();
      const region = document.querySelector('[data-testid="compensation-policy-scroll-region"]')?.getBoundingClientRect();
      return target && region ? { targetTop: target.top, targetBottom: target.bottom, regionTop: region.top, regionBottom: region.bottom } : null;
    });
    expect(focusLayout).toBeTruthy();
    expect(focusLayout!.targetTop >= focusLayout!.regionTop).toBe(true);
    expect(focusLayout!.targetBottom <= focusLayout!.regionBottom).toBe(true);
    if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
      await page.screenshot({ path: '/private/tmp/compensation-policy-ux-validation.png' });
    }
  });

  test('shows a fresh read-only preview before separate baseline consent and save', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useEylulPrefill());
    await prepareValidInitialBaseline(page);
    await requestPreview(page);
    expect(await page.getByRole('heading', { name: '3. Etkileri inceleyin' }).isVisible()).toBe(true);
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini onayla' }).isVisible()).toBe(true);
    expect(await page.getByRole('button', { name: 'Başlangıç ücretini kaydet' }).count()).toBe(0);
    if (process.env.CAPTURE_COMPENSATION_POLICY_SCREENSHOTS === '1') {
      await page.getByRole('heading', { name: '3. Etkileri inceleyin' }).scrollIntoViewIfNeeded();
      await page.screenshot({ path: '/private/tmp/compensation-policy-ux-preview.png' });
    }
  });

  test('targets the scoped same-date leaf and disables an outdated preview after a draft edit', async () => {
    const page = await openPolicyPage();
    await prepareExistingDateCorrection(page);

    await page.locator('#policy-effective-date').selectOption('');
    expect(await page.locator('#policy-effective-date').getAttribute('aria-invalid')).toBe('false');
    expect(await page.locator('#policy-effective-date').getAttribute('aria-describedby')).toBe(null);
    await page.getByRole('button', { name: /Etkileri önizle/ }).click();
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    expect(await page.locator('#policy-effective-date').getAttribute('aria-invalid')).toBe('true');
    expect(await page.locator('#policy-effective-date').getAttribute('aria-describedby')).toBe('policy-correction-target-error');
    await page.locator('#policy-effective-date').selectOption('2026-09-15');

    await requestPreview(page);
    const firstOperation = (await events(page)).filter((entry) => entry.type === 'preview').at(-1)?.value as { kind: string; supersedesVersionId?: string };
    expect(firstOperation.kind).toBe('SAME_DATE_CORRECTION');
    expect(firstOperation.supersedesVersionId).toBe('leaf-2');
    expect(await page.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).isDisabled()).toBe(false);

    await page.locator('#policy-base-wage').fill('9007199254740994.12');
    await page.getByText('Taslak önizlemeden sonra değişti. Kaydetmeden önce güncel etkileri yeniden önizleyin.').waitFor({ state: 'visible' });
    expect(await page.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).count()).toBe(0);
    expect(await page.getByRole('button', { name: /Etkileri önizle|Önizlemeyi yenile/ }).isVisible()).toBe(true);
  });

  test('blocks preview results containing FINALIZED payrolls and prevents duplicate save clicks', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(page);
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('finalized'));
    await requestPreview(page);
    await page.getByText('Kesinleştirilmiş bordro değişikliği engelliyor').waitFor({ state: 'visible' });
    expect(await page.getByRole('button', { name: /Etkileri önizle|Önizlemeyi yenile/ }).isVisible()).toBe(true);
    expect(await page.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).count()).toBe(0);
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(0);

    await page.close();
    pageUnderTest = null;
    const secondPage = await openPolicyPage();
    await secondPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(secondPage);
    await secondPage.evaluate(() => window.__compensationPolicyTest.setScenario('slow'));
    await requestPreview(secondPage);
    await secondPage.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).evaluate((button) => {
      (button as HTMLButtonElement).click();
      (button as HTMLButtonElement).click();
    });
    await secondPage.getByText('Ücret değişikliği kaydediliyor…').waitFor({ state: 'visible' });
    expect((await events(secondPage)).filter((entry) => entry.type === 'save').length).toBe(1);
    await secondPage.getByText('Ücret değişikliği kaydedildi.').waitFor({ state: 'visible' });
  });

  test('surfaces conflict, persistence failure, persisted reload failure, and only reports success after authoritative reload', async () => {
    const conflictPage = await openPolicyPage();
    await conflictPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(conflictPage);
    await conflictPage.evaluate(() => window.__compensationPolicyTest.setScenario('conflict'));
    await requestPreview(conflictPage);
    await conflictPage.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).click();
    await conflictPage.getByText('Önizlemeden sonra ücret geçmişi değişti.').waitFor({ state: 'visible' });
    await conflictPage.getByText('Aynı tarihte başka bir değişiklik kaydı bulundu.').waitFor({ state: 'visible' });
    await conflictPage.getByRole('button', { name: 'Yetkili durumu yeniden yükle' }).click();
    await conflictPage.getByText(/Bu dönem için geçerli ücret kayıtları \(\d+\)/).waitFor({ state: 'visible' });
    expect((await events(conflictPage)).filter((entry) => entry.type === 'reload-authoritative').length).toBe(1);
    await conflictPage.close();
    pageUnderTest = null;

    const persistencePage = await openPolicyPage();
    await persistencePage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(persistencePage);
    await persistencePage.evaluate(() => window.__compensationPolicyTest.setScenario('persistence'));
    await requestPreview(persistencePage);
    await persistencePage.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).click();
    await persistencePage.getByText('Ücret değişikliği tamamlanamadı.').waitFor({ state: 'visible' });
    expect(await persistencePage.getByText('Ücret değişikliği kaydedildi.').count()).toBe(0);
    await persistencePage.close();
    pageUnderTest = null;

    const reloadPage = await openPolicyPage();
    await reloadPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(reloadPage);
    await reloadPage.evaluate(() => window.__compensationPolicyTest.setScenario('reloadfailure'));
    await requestPreview(reloadPage);
    await reloadPage.getByRole('button', { name: 'Önizlenen değişikliği kaydet' }).click();
    await reloadPage.getByText('Kayıt tamamlandı ancak güncel ücret bilgileri yeniden yüklenemedi.').waitFor({ state: 'visible' });
    expect(await reloadPage.getByText('Ücret değişikliği kaydedildi.').count()).toBe(0);
    await reloadPage.getByRole('button', { name: 'Ücret bilgilerini yeniden yükle' }).click();
    await reloadPage.getByText('Ücret değişikliği kaydedildi.').waitFor({ state: 'visible' });
    expect((await events(reloadPage)).filter((entry) => entry.type === 'reload-authoritative').length).toBe(1);
  });
});
