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

// ---- dictionary popup ---------------------------------------------------------------------------

/** Click a word and wait until the popup shows real content (not the skeleton). */
async function openWordPopup(page: Page, word: string, at: number, query = '') {
  await openPlayer(page, query, at);
  await page.locator('.unit.word', { hasText: word }).first().click();
  await expect(page.locator('.p-popup')).toBeVisible();
}
const settled = async (page: Page) => {
  await expect(page.locator('.p-skel')).toHaveCount(0);
  await page.waitForTimeout(450);
};

/** The popup box must not overlap the clicked word and must stay inside the window. */
async function expectClearOfWord(page: Page, word: string) {
  const pop = await page.locator('.p-popup').boundingBox();
  const w = await page.locator('.unit.word.active', { hasText: word }).boundingBox();
  const vp = page.viewportSize()!;
  expect(pop && w).toBeTruthy();
  expect(pop!.x).toBeGreaterThanOrEqual(0);
  expect(pop!.y).toBeGreaterThanOrEqual(0);
  expect(pop!.x + pop!.width).toBeLessThanOrEqual(vp.width);
  expect(pop!.y + pop!.height).toBeLessThanOrEqual(vp.height);
  const overlap = pop!.y < w!.y + w!.height && pop!.y + pop!.height > w!.y;
  expect(overlap).toBe(false);
}

test('popup-verb-deinflected', async ({ page }) => {
  await openWordPopup(page, '諦めない', 1.5);
  await settled(page);
  await expect(page.locator('.p-pitch-name')).toContainText('nakadaka');
  await expect(page.locator('.p-tag')).toHaveText(['negative']);
  await expect(page.locator('.p-badge.freq')).toContainText('#2,184');
  await shot(page, 'popup-verb-deinflected');
});

test('popup-chain', async ({ page }) => {
  await openWordPopup(page, '食べさせられなかった', 5.5);
  await settled(page);
  await expect(page.locator('.p-tag')).toHaveText(['causative', 'passive', 'negative', 'past']);
  await shot(page, 'popup-chain');
});

test('popup-heiban', async ({ page }) => {
  await openWordPopup(page, '夕焼け', 13.5);
  await settled(page);
  await expect(page.locator('.p-pitch-name')).toContainText('heiban');
  await shot(page, 'popup-heiban');
});

test('popup-two-accents', async ({ page }) => {
  await openWordPopup(page, '一緒', 9.5);
  await settled(page);
  await expect(page.locator('.p-pitch-row')).toHaveCount(2);
  await shot(page, 'popup-two-accents');
});

test('popup-no-pitch', async ({ page }) => {
  await openWordPopup(page, 'こんなに', 13.5);
  await settled(page);
  await expect(page.locator('.p-pitch')).toHaveCount(0);
  await expect(page.locator('.p-audio')).toBeDisabled();
  await shot(page, 'popup-no-pitch');
});

test('popup-multi-results', async ({ page }) => {
  await openWordPopup(page, '諦めない', 1.5);
  await settled(page);
  await expect(page.locator('.p-tab')).toHaveText(['諦めない', '諦']);
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('.p-reading')).toHaveText('あきらめ');
  await expect(page.locator('.p-inflect')).toHaveCount(0);
  await page.waitForTimeout(200);
  await shot(page, 'popup-multi-results');
  await page.keyboard.press('ArrowLeft');
  await expect(page.locator('.p-tag')).toHaveText(['negative']);
});

test('popup-multi-entries', async ({ page }) => {
  await openWordPopup(page, '会おう', 26.5);
  await settled(page);
  await expect(page.locator('.p-pager')).toContainText('1 / 2');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.p-pager')).toContainText('2 / 2');
  await expect(page.locator('.p-headword')).toContainText('遭');
  await shot(page, 'popup-multi-entries');
});

test('popup-not-found', async ({ page }) => {
  await openWordPopup(page, 'だね', 5.5);
  await settled(page);
  await expect(page.locator('.p-empty-title')).toContainText('No dictionary entry');
  await shot(page, 'popup-not-found');
});

test('popup-loading', async ({ page }) => {
  await openPlayer(page, '&slowlookup=1', 1.5);
  await page.locator('.unit.word', { hasText: '諦めない' }).click();
  // Nothing visible for the first 150 ms (no flash), then the skeleton.
  await expect(page.locator('.p-popup')).toHaveCount(0);
  await expect(page.locator('.p-skel')).toBeVisible();
  await page.waitForTimeout(500);
  await shot(page, 'popup-loading');
});

test('popup-small-window', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 500 });
  await openWordPopup(page, '諦めない', 1.5);
  await settled(page);
  await shot(page, 'popup-small-window');
  await expectClearOfWord(page, '諦めない');
});

test('popup-1080p', async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await openWordPopup(page, '食べさせられなかった', 5.5);
  await settled(page);
  await shot(page, 'popup-1080p');
  await expectClearOfWord(page, '食べさせられなかった');
});

test('popup-bottom-flip', async ({ page }) => {
  await openPlayer(page, '', 13.5);
  // Move the subtitle line to the top of the window so the popup has no room above.
  await page.addStyleTag({ content: '.p-subs { top: 90px !important; bottom: auto !important; }' });
  await page.locator('.unit.word', { hasText: '綺麗' }).click();
  await expect(page.locator('.p-popup.below')).toBeVisible();
  await settled(page);
  await shot(page, 'popup-bottom-flip');
  await expectClearOfWord(page, '綺麗');
});

test('behaviour: R plays audio, clicking another word replaces the popup, Esc closes', async ({ page }) => {
  await page.addInitScript(() => {
    (window as any).__played = 0;
    HTMLMediaElement.prototype.play = function () {
      (window as any).__played++;
      return Promise.resolve();
    };
  });
  await openWordPopup(page, '諦めない', 1.5);
  await settled(page);
  await expect(page.locator('.p-audio')).toBeEnabled();
  await page.keyboard.press('r');
  await expect.poll(() => page.evaluate(() => (window as any).__played)).toBe(1);
  await page.locator('.unit.word', { hasText: '決めた' }).click();
  await expect(page.locator('.p-popup')).toHaveCount(1);
  await expect(page.locator('.p-reading')).toHaveText('きめる');
  await page.keyboard.press('Escape');
  await expect(page.locator('.p-popup')).toHaveCount(0);
});

test('behaviour: lookup rejection shows an error toast and no popup', async ({ page }) => {
  await openPlayer(page, '', 26.5);
  await page.locator('.unit.word', { hasText: 'ありがとう' }).click();
  await expect(page.locator('.toast.error')).toContainText('lookup failed');
  await expect(page.locator('.p-popup')).toHaveCount(0);
});

test('behaviour: +N more expands the senses', async ({ page }) => {
  await openWordPopup(page, '知らなかった', 13.5);
  await settled(page);
  await expect(page.locator('.p-senses li')).toHaveCount(3);
  await page.getByRole('button', { name: '+2 more' }).click();
  await expect(page.locator('.p-senses li')).toHaveCount(5);
});
