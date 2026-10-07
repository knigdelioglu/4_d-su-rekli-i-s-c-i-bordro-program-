import { describe, expect, test } from 'bun:test';
import * as bunTest from 'bun:test';
import { chromium, type Browser, type Page } from '@playwright/test';
import { build } from 'esbuild';

const { afterAll, afterEach, beforeAll } = bunTest as unknown as {
  afterAll(callback: () => void | Promise<void>, timeout?: number): void;
  afterEach(callback: () => void | Promise<void>, timeout?: number): void;
  beforeAll(callback: () => void | Promise<void>, timeout?: number): void;
};

let browser: Browser;
let pageUnderTest: Page | null = null;
let harnessBundle = '';

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
  browser = await chromium.launch({ headless: true });
}, 30_000);

afterEach(async () => {
  await pageUnderTest?.close();
  pageUnderTest = null;
}, 30_000);

afterAll(async () => {
  await browser?.close();
}, 30_000);

async function openPolicyPage(): Promise<Page> {
  pageUnderTest = await browser.newPage();
  await pageUnderTest.setContent('<!doctype html><html lang="tr"><body><div id="root"></div></body></html>');
  await pageUnderTest.addScriptTag({ content: harnessBundle });
  await pageUnderTest.getByRole('heading', { name: 'Ücret Politikası / Ücret Geçmişi' }).waitFor({ state: 'visible' });
  return pageUnderTest;
}

async function events(page: Page): Promise<Array<{ type: string; value?: unknown }>> {
  return page.evaluate(() => window.__compensationPolicyTest.events);
}

async function selectBaselinePolicyFlags(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await page.locator(`input[name="policy-boundary-scope"][value="${scope}"]`).check();
  await page.locator(`input[name="policy-road-rounding"][value="${roadMode}"]`).check();
}

async function prepareValidInitialBaseline(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await page.getByRole('button', { name: 'Eski ayar arşivinden taslak hazırla' }).click();
  await selectBaselinePolicyFlags(page, scope, roadMode);
}

async function selectExistingChangeFlags(page: Page, scope = 'FULL_EARNING', roadMode = 'PERIOD_TOTAL'): Promise<void> {
  await page.locator(`input[name="policy-boundary-scope"][value="${scope}"]`).check();
  await page.locator(`input[name="policy-road-rounding"][value="${roadMode}"]`).check();
}

async function requestPreview(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Değişikliği ve etkileri önizle' }).click();
  await page.getByRole('heading', { name: 'Önizleme etkileri' }).waitFor({ state: 'visible' });
}

async function prepareExistingDateCorrection(page: Page): Promise<void> {
  await page.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
  await page.locator('#policy-operation').selectOption('SAME_DATE_CORRECTION');
  await page.locator('#policy-effective-date').selectOption('2026-09-15');
  await selectExistingChangeFlags(page);
}

describe('rendered CompensationPolicyPage interactions with frozen props', () => {
  test('loads and retries typed quarantine errors from the rendered page', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.setLoadIdle());
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('quarantine'));
    await page.getByRole('button', { name: 'Ücret geçmişini yükle' }).click();
    await page.getByText('Kapsam içindeki bir ücret politikası okunamıyor.').waitFor({ state: 'visible' });
    const errorText = await page.locator('[role="alert"]').last().innerText();
    expect(errorText.includes('2026-09-15')).toBe(true);
    expect(errorText.includes('yol yuvarlama biçimi eksik')).toBe(true);
    const recorded = await events(page);
    expect(recorded.filter((entry) => entry.type === 'load').length).toBe(1);
  });

  test('prefills a missing baseline, confirms the exact date/state, resets consent on date change, and supports cancel', async () => {
    const page = await openPolicyPage();
    await page.getByRole('button', { name: 'Eski ayar arşivinden taslak hazırla' }).click();

    expect(await page.locator('#policy-base-wage').inputValue()).toBe('9007199254740993,12');
    expect(await page.locator('#policy-meal').inputValue()).toBe('50,00');
    expect(await page.locator('#policy-extra-payment-presence').inputValue()).toBe('VALUE');
    expect(await page.locator('#policy-extra-payment').inputValue()).toBe('0');
    expect(await page.locator('#policy-other-income-presence').inputValue()).toBe('ABSENT');
    expect(await page.locator('#policy-other-income').count()).toBe(0);
    expect(await page.locator('#policy-group-id-1').inputValue()).toBe('B');
    expect(await page.locator('#policy-group-rate-1').inputValue()).toBe('0');
    expect(await page.getByRole('checkbox', { name: 'Etkin' }).nth(1).isChecked()).toBe(false);

    await selectBaselinePolicyFlags(page);
    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'confirm-baseline').length).toBe(1);

    await page.locator('#policy-effective-date').fill('2026-09-16');
    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).waitFor({ state: 'visible' });
    await page.getByRole('button', { name: 'Değişikliği ve etkileri önizle' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    expect((await page.locator('[role="alert"]').last().innerText()).includes('ayrıca onaylayın')).toBe(true);

    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).click();
    await page.getByRole('button', { name: 'Başlangıç onayını iptal et' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'cancel-baseline').length).toBe(1);
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
  });

  test('applies an exact percentage helper, keeps the source state intact, and previews a full ROAD_ONLY baseline', async () => {
    const page = await openPolicyPage();
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

    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).click();
    await requestPreview(page);
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
    await page.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(1);
    const saveEvent = (await events(page)).filter((entry) => entry.type === 'save').at(-1);
    const saveRequest = saveEvent?.value as { initialBaselineConfirmation?: { confirmed: boolean; effectiveFrom: string } };
    expect(saveRequest.initialBaselineConfirmation).toEqual({ confirmed: true, effectiveFrom: '2026-09-15' });
    await page.getByText('Kayıt tamamlandı; başarıyı göstermeden önce yetkili ücret geçmişi yeniden yükleniyor.').waitFor({ state: 'visible' });
    expect(await page.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').count()).toBe(0);
    await page.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').waitFor({ state: 'visible' });
  });

  test('shows eligible retro effects in preview and reports stale batch and payment IDs after save', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).click();
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('retro'));

    await requestPreview(page);
    const previewText = await page.locator('body').innerText();
    expect(previewText.includes('Etkilenen retro kayıtları')).toBe(true);
    expect(previewText.includes('Batch retro-batch-1')).toBe(true);
    expect(previewText.includes('Kaynak dönemler: 2026-08 · bağlı ödeme olayı: 1')).toBe(true);
    expect(previewText.includes('Politika kaydıyla birlikte STALE yapılacak')).toBe(true);

    await page.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).click();
    await page.getByText('STALE yapılan retro batch’ler: retro-batch-1.').waitFor({ state: 'visible' });
    await page.getByText('STALE yapılan bağlı ödeme olayları: retro-batch-1.').waitFor({ state: 'visible' });
  });

  test('names immutable retro blockers returned by preview', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.getByRole('button', { name: 'Bu tarihten itibaren geçerli başlangıç ücret politikası olarak onayla' }).click();
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('retroblocker'));
    await page.getByRole('button', { name: 'Değişikliği ve etkileri önizle' }).click();

    await page.getByText('Kesinleşmiş veya settlement uygulanmış retro kayıt değişikliği engelliyor.').waitFor({ state: 'visible' });
    const blockerText = await page.locator('[role="alert"]').last().innerText();
    expect(blockerText.includes('retro-final-batch')).toBe(true);
    expect(blockerText.includes('batch kesinleştirilmiş')).toBe(true);
    expect(blockerText.includes('settlement uygulanmış')).toBe(true);
  });

  test('keeps invalid full-state drafts out of preview/save and focuses the rendered error summary', async () => {
    const page = await openPolicyPage();
    await prepareValidInitialBaseline(page);
    await page.locator('#policy-base-wage').fill('');
    await page.getByRole('button', { name: 'Değişikliği ve etkileri önizle' }).click();

    await page.getByRole('heading', { name: 'Önizleme için formu tamamlayın' }).waitFor({ state: 'visible' });
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(0);
    await page.waitForFunction(() => document.activeElement?.getAttribute('tabindex') === '-1');
    expect(await page.evaluate(() => document.activeElement?.getAttribute('tabindex'))).toBe('-1');
    expect(await page.locator('#policy-base-wage').getAttribute('aria-invalid')).toBe('true');
  });

  test('targets the scoped same-date leaf and disables an outdated preview after a draft edit', async () => {
    const page = await openPolicyPage();
    await prepareExistingDateCorrection(page);

    await page.locator('#policy-effective-date').selectOption('');
    expect(await page.locator('#policy-effective-date').getAttribute('aria-invalid')).toBe('true');
    expect(await page.locator('#policy-effective-date').getAttribute('aria-describedby')).toBe('policy-correction-target-error');
    await page.getByRole('button', { name: 'Değişikliği ve etkileri önizle' }).click();
    expect((await events(page)).filter((entry) => entry.type === 'preview').length).toBe(0);
    await page.locator('#policy-effective-date').selectOption('2026-09-15');

    await requestPreview(page);
    const firstOperation = (await events(page)).filter((entry) => entry.type === 'preview').at(-1)?.value as { kind: string; supersedesVersionId?: string };
    expect(firstOperation.kind).toBe('SAME_DATE_CORRECTION');
    expect(firstOperation.supersedesVersionId).toBe('leaf-2');
    expect(await page.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).isDisabled()).toBe(false);

    await page.locator('#policy-base-wage').fill('9007199254740994.12');
    await page.getByText('Bu önizleme başka bir taslak, tarih veya kapsam içindi.').waitFor({ state: 'visible' });
    expect(await page.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).isDisabled()).toBe(true);
  });

  test('blocks preview results containing FINALIZED payrolls and prevents duplicate save clicks', async () => {
    const page = await openPolicyPage();
    await page.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(page);
    await page.evaluate(() => window.__compensationPolicyTest.setScenario('finalized'));
    await requestPreview(page);
    await page.getByText('Kesinleştirilmiş bordro değişikliği engelliyor').waitFor({ state: 'visible' });
    expect(await page.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).isDisabled()).toBe(true);
    expect((await events(page)).filter((entry) => entry.type === 'save').length).toBe(0);

    await page.close();
    pageUnderTest = null;
    const secondPage = await openPolicyPage();
    await secondPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(secondPage);
    await secondPage.evaluate(() => window.__compensationPolicyTest.setScenario('slow'));
    await requestPreview(secondPage);
    await secondPage.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).evaluate((button) => {
      (button as HTMLButtonElement).click();
      (button as HTMLButtonElement).click();
    });
    await secondPage.getByText('Ücret politikası ve etkilenen bordro bağımlılıkları kaydediliyor.').waitFor({ state: 'visible' });
    expect((await events(secondPage)).filter((entry) => entry.type === 'save').length).toBe(1);
    await secondPage.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').waitFor({ state: 'visible' });
  });

  test('surfaces conflict, persistence failure, persisted reload failure, and only reports success after authoritative reload', async () => {
    const conflictPage = await openPolicyPage();
    await conflictPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(conflictPage);
    await conflictPage.evaluate(() => window.__compensationPolicyTest.setScenario('conflict'));
    await requestPreview(conflictPage);
    await conflictPage.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).click();
    await conflictPage.getByText('Timeline önizlemeden sonra değişti.').waitFor({ state: 'visible' });
    await conflictPage.getByText('Güncel hedef leaf: newer-leaf.').waitFor({ state: 'visible' });
    await conflictPage.getByRole('button', { name: 'Yetkili durumu yeniden yükle' }).click();
    await conflictPage.getByRole('heading', { name: 'Bu hizmet dönemi kapsamındaki ücret geçmişi' }).waitFor({ state: 'visible' });
    expect((await events(conflictPage)).filter((entry) => entry.type === 'reload-authoritative').length).toBe(1);
    await conflictPage.close();
    pageUnderTest = null;

    const persistencePage = await openPolicyPage();
    await persistencePage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(persistencePage);
    await persistencePage.evaluate(() => window.__compensationPolicyTest.setScenario('persistence'));
    await requestPreview(persistencePage);
    await persistencePage.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).click();
    await persistencePage.getByText('Ücret politikası işlemi tamamlanamadı.').waitFor({ state: 'visible' });
    expect(await persistencePage.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').count()).toBe(0);
    await persistencePage.close();
    pageUnderTest = null;

    const reloadPage = await openPolicyPage();
    await reloadPage.evaluate(() => window.__compensationPolicyTest.useExistingTimeline());
    await selectExistingChangeFlags(reloadPage);
    await reloadPage.evaluate(() => window.__compensationPolicyTest.setScenario('reloadfailure'));
    await requestPreview(reloadPage);
    await reloadPage.getByRole('button', { name: 'Önizlenen ücret politikasını kaydet' }).click();
    await reloadPage.getByText('Politika kaydedildi; yetkili görünüm yeniden yüklenemedi.').waitFor({ state: 'visible' });
    expect(await reloadPage.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').count()).toBe(0);
    await reloadPage.getByRole('button', { name: 'Yetkili ücret geçmişini yeniden yükle' }).click();
    await reloadPage.getByText('Ücret politikası kaydedildi ve yetkili geçmiş yeniden yüklendi.').waitFor({ state: 'visible' });
    expect((await events(reloadPage)).filter((entry) => entry.type === 'reload-authoritative').length).toBe(1);
  });
});
