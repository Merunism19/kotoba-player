/** Screenshot harness: renders every screen in mock mode at 1280x720 (plus a small window) into artifacts/shots/*.png. */
import { test, expect, type Page } from '@playwright/test';

const OUT = 'artifacts/shots';
const shot = (page: Page, name: string) => page.screenshot({ path: `${OUT}/${name}.png` });

/** Open the mock player already playing line 4 ("夕焼け...知らなかった") and wait for tokenization. */
async function openPlayer(page: Page, query = '', at = 13.5) {
  await page.goto(`/?mock=1&open=1${query}`);
  await expect(page.locator('.p-top')).toBeVisible();
  await page.evaluate((t) => {
    const kp = (window as any).__kp;
    kp.setPaused(true);
    kp.setTime(t);
  }, at);
  await expect(page.locator('.unit.word').first()).toBeVisible();
  await page.mouse.move(640, 300);
  await page.waitForTimeout(250);
}

test('home-empty', async ({ page }) => {
  await page.goto('/?mock=1&recents=0');
  await expect(page.locator('.drop')).toBeVisible();
  await page.waitForTimeout(300);
  await shot(page, 'home-empty');
});

test('home-recents', async ({ page }) => {
  await page.goto('/?mock=1');
  await expect(page.locator('.card')).toHaveCount(3);
  await page.waitForTimeout(300);
  await shot(page, 'home-recents');
});

test('player-playing', async ({ page }) => {
  await openPlayer(page);
  await shot(page, 'player-playing');
});

test('player-hover-word', async ({ page }) => {
  await openPlayer(page);
  await page.locator('.unit.word', { hasText: '知らなかった' }).hover();
  await page.waitForTimeout(300);
  await shot(page, 'player-hover-word');
});

test('player-popup', async ({ page }) => {
  await openPlayer(page, '', 1.5);
  await page.locator('.unit.word', { hasText: '諦めない' }).click();
  await expect(page.locator('.p-popup')).toBeVisible();
  await page.waitForTimeout(400);
  await shot(page, 'player-popup');
  // Esc closes it.
  await page.keyboard.press('Escape');
  await expect(page.locator('.p-popup')).toHaveCount(0);
});

test('player-popup-small', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 500 });
  await openPlayer(page, '', 13.5);
  await page.locator('.unit.word', { hasText: '知らなかった' }).click();
  await expect(page.locator('.p-popup')).toBeVisible();
  await page.waitForTimeout(400);
  await shot(page, 'player-popup-800x500');
});

test('player-popup-bright', async ({ page }) => {
  await openPlayer(page, '&bright=1', 1.5);
  await page.locator('.unit.word', { hasText: '諦めない' }).click();
  await page.waitForTimeout(400);
  await shot(page, 'player-popup-bright');
});

test('player-track-picker', async ({ page }) => {
  await openPlayer(page);
  await page.getByRole('button', { name: /JP \+ EN/ }).click();
  await expect(page.locator('.p-picker')).toBeVisible();
  await page.waitForTimeout(300);
  await shot(page, 'player-track-picker');
});

test('player-offset-badge', async ({ page }) => {
  await openPlayer(page);
  for (let i = 0; i < 3; i++) await page.keyboard.press('x');
  await page.waitForTimeout(120);
  await shot(page, 'player-offset-badge');
  expect(await page.locator('.p-chip.sync').innerText()).toContain('+0.30 s');
});

test('player-controls-hidden', async ({ page }) => {
  await openPlayer(page);
  await page.waitForTimeout(2600);
  await expect(page.locator('.p-stage')).toHaveClass(/idle/);
  await shot(page, 'player-controls-hidden');
});

test('error-toast', async ({ page }) => {
  await openPlayer(page);
  await page.evaluate(() => (window as any).__kp.openFile('C:/Anime/broken.mkv'));
  await expect(page.locator('.toast.error')).toBeVisible();
  await page.waitForTimeout(400);
  await shot(page, 'error-toast');
});

test('behaviour: unsupported drop shows a toast; hotkeys move the line', async ({ page }) => {
  await openPlayer(page, '', 5);
  await page.evaluate(() => (window as any).__kp.openFile('C:/Anime/notes.txt'));
  await expect(page.locator('.toast', { hasText: 'Not a video file' })).toBeVisible();
  await page.keyboard.press('d');
  await expect.poll(() => page.evaluate(() => (window as any).__kp.getTime())).toBeCloseTo(9, 0);
  await page.keyboard.press('s');
  await expect.poll(() => page.evaluate(() => (window as any).__kp.getTime())).toBeGreaterThanOrEqual(9);
});
