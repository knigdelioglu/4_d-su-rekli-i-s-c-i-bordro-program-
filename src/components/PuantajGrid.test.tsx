import { describe, expect, test } from 'bun:test';
import * as bunTest from 'bun:test';
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';
import { createServer, type ViteDevServer } from 'vite';

const { afterAll, afterEach, beforeAll } = bunTest as unknown as {
  afterAll(callback: () => void | Promise<void>, timeout?: number): void;
  afterEach(callback: () => void | Promise<void>, timeout?: number): void;
  beforeAll(callback: () => void | Promise<void>, timeout?: number): void;
};

let browser: Browser;
let pageUnderTest: Page | null = null;
let bundle = '';
let vite: ViteDevServer;
let viteOrigin = '';
let previousDisableHmr: string | undefined;

beforeAll(async () => {
  const result = await build({
    entryPoints: ['src/components/PuantajGridTestHarness.tsx'], bundle: true,
    format: 'iife', platform: 'browser', target: 'chrome120', write: false,
    jsx: 'automatic', logLevel: 'silent', define: { 'process.env.NODE_ENV': '"production"' },
  });
  bundle = result.outputFiles[0]?.text ?? '';
  if (!bundle) throw new Error('The PuantajGrid test harness did not bundle.');
  previousDisableHmr = process.env.DISABLE_HMR;
  process.env.DISABLE_HMR = 'true';
  vite = await createServer({
    configFile: 'vite.config.ts', appType: 'custom', logLevel: 'error',
    server: { host: '127.0.0.1', port: 0, strictPort: false, hmr: false },
  });
  await vite.listen();
  const address = vite.httpServer?.address();
  if (!address || typeof address === 'string') throw new Error('The PuantajGrid test server did not bind a TCP port.');
  viteOrigin = `http://127.0.0.1:${address.port}`;
  browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH });
}, 30_000);

afterEach(async () => {
  await pageUnderTest?.close();
  pageUnderTest = null;
}, 30_000);

afterAll(async () => {
  await browser?.close();
  await vite?.close();
  if (previousDisableHmr === undefined) delete process.env.DISABLE_HMR;
  else process.env.DISABLE_HMR = previousDisableHmr;
}, 30_000);

async function openGrid(): Promise<Page> {
  pageUnderTest = await browser.newPage();
  pageUnderTest.setDefaultTimeout(5_000);
  await pageUnderTest.setContent('<!doctype html><html lang="tr"><body><div id="root"></div></body></html>');
  await pageUnderTest.addStyleTag({ url: `${viteOrigin}/src/index.css` });
  await pageUnderTest.addScriptTag({ content: bundle });
  await pageUnderTest.getByTestId('quick-attendance-entry').waitFor({ state: 'visible' });
  return pageUnderTest;
}

describe('rendered PuantajGrid bulk confirmation', () => {
  test('previews multi-person sickness days, preserves R, and keeps failed atomic save open for retry', async () => {
    const page = await openGrid();
    await page.getByTestId('attendance-code-T').click();
    await page.getByTestId('open-quick-attendance-calendar').click();
    for (const date of ['2027-01-16', '2027-01-17', '2027-01-19']) {
      await page.getByTestId(`quick-attendance-day-${date}`).click();
    }
    await page.getByTestId('apply-quick-attendance-to-all').click();

    await page.getByTestId('bulk-apply-confirmation').waitFor({ state: 'visible' });
    expect(await page.getByTestId('bulk-apply-preview-row-person-a-2027-01-16').innerText()).toContain('R → T');
    expect(await page.getByTestId('bulk-apply-preview-row-person-a-2027-01-16').innerText()).toContain('R korundu');
    expect(await page.getByTestId('bulk-apply-preview-row-person-a-2027-01-17').innerText()).toContain('R korundu');
    expect(await page.getByTestId('bulk-apply-preview-row-person-b-2027-01-16').innerText()).toContain('R korundu');
    expect(await page.getByTestId('bulk-apply-preview-row-person-b-2027-01-19').innerText()).toContain('Uygulanacak');

    const before = await page.evaluate(() => window.__puantajBulkTest.snapshot());
    await page.getByTestId('bulk-apply-confirm').click();
    await page.getByTestId('bulk-apply-confirmation')
      .getByText(/Toplu puantaj kaydedilemedi: sentetik yazma hatası/)
      .waitFor({ state: 'visible' });
    expect(await page.getByTestId('bulk-apply-confirmation').count()).toBe(1);
    expect(await page.evaluate(() => window.__puantajBulkTest.snapshot())).toEqual(before);
    expect(await page.evaluate(() => window.__puantajBulkTest.attempts())).toBe(1);

    await page.getByTestId('bulk-apply-confirm').click();
    await page.getByTestId('bulk-apply-confirmation').waitFor({ state: 'detached' });
    const after = await page.evaluate(() => window.__puantajBulkTest.snapshot());
    expect(after.find((item) => item.personelId === 'person-a')?.gunler['2027-01-16']).toBe('R');
    expect(after.find((item) => item.personelId === 'person-a')?.gunler['2027-01-17']).toBe('R');
    expect(after.find((item) => item.personelId === 'person-b')?.gunler['2027-01-16']).toBe('R');
    expect(after.find((item) => item.personelId === 'person-b')?.gunler['2027-01-19']).toBe('T');
    expect(after.find((item) => item.personelId === 'person-a')?.gunler['2027-01-19']).toBe('T');
    expect(await page.evaluate(() => window.__puantajBulkTest.attempts())).toBe(2);
  });

  test('blocks all writes when a selected sickness-report date has a non-R code', async () => {
    const page = await openGrid();
    await page.getByTestId('attendance-code-Ç').click();
    await page.getByTestId('open-quick-attendance-calendar').click();
    await page.getByTestId('quick-attendance-day-2027-01-20').click();
    await page.getByTestId('apply-quick-attendance-to-all').click();

    const mismatchRow = page.getByTestId('bulk-apply-preview-row-person-b-2027-01-20');
    await mismatchRow.waitFor({ state: 'visible' });
    expect(await mismatchRow.innerText()).toContain('Ç → Ç');
    expect(await mismatchRow.innerText()).toContain('Rapor / kod uyuşmuyor');
    expect(await page.getByTestId('bulk-apply-confirm').isDisabled()).toBe(true);
    expect(await page.getByText(/Hiçbir personel kaydedilmez/).count()).toBe(1);
    expect(await page.evaluate(() => window.__puantajBulkTest.attempts())).toBe(0);
  });
});
