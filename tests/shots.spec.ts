/** Screenshot harness: renders every screen (incl. Anki mining) in mock mode at 1280x720 (plus a small window) into artifacts/shots/*.png. */
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

// ---- Anki mining --------------------------------------------------------------------------------

/** Open the popup for a word and wait until the Add to Anki button has its final state. */
async function openMinePopup(page: Page, query = '', word = '諦めない', at = 1.5) {
  await openWordPopup(page, word, at, query);
  await settled(page);
  await expect(page.locator('.p-btn.primary')).toBeVisible();
}
const addBtn = (page: Page) => page.locator('.p-popup .p-btn.primary');
const mock = (page: Page) => page.evaluate(() => (window as any).__mock as { drafts: any[]; added: any[] });

/** Click Add to Anki, wait for the preview and for both card frames to be drawn. */
async function openPreview(page: Page, query = '', word = '諦めない', at = 1.5) {
  await openMinePopup(page, query, word, at);
  await addBtn(page).click();
  await expect(page.locator('.m-modal')).toBeVisible();
  await expect(page.frameLocator('iframe[title="Card back"]').locator('.kt .trans')).toBeVisible();
  await expect(page.frameLocator('iframe[title="Card front"]').locator('.kt .shot img')).toBeVisible();
  await page.waitForTimeout(450);
}

test('mine-button-offline', async ({ page }) => {
  await openMinePopup(page, '&anki=off');
  await expect(addBtn(page)).toBeDisabled();
  const tip = page.locator('.p-tip.has-tip');
  await expect(tip).toHaveAttribute('data-tip', /AnkiConnect installed \(add-on 2055492159\)\nAnki isn't reachable/);
  await tip.hover();
  await page.waitForTimeout(300);
  await shot(page, 'mine-button-offline');
});

test('mine-preview-dark', async ({ page }) => {
  await openPreview(page);
  await expect(page.locator('.m-frame')).toHaveCount(2);
  await expect(page.frameLocator('iframe[title="Card front"]').locator('.kt .word ruby')).toHaveCount(1);
  await expect(page.locator('.m-src')).toHaveText('from English subtitles');
  await shot(page, 'mine-preview-dark');
});

test('mine-preview-light-theme', async ({ page }) => {
  await openPreview(page);
  await page.getByRole('button', { name: 'Light' }).click();
  await expect(page.frameLocator('iframe[title="Card back"]').locator('body')).not.toHaveClass(/nightMode/);
  await expect(page.frameLocator('iframe[title="Card back"]').locator('.kt .trans')).toBeVisible();
  await page.waitForTimeout(450);
  await shot(page, 'mine-preview-light-theme');
});

test('mine-preview-duplicate', async ({ page }) => {
  await openPreview(page, '&dup=1');
  await expect(page.locator('.m-dup')).toContainText('Already in Anki (1)');
  await shot(page, 'mine-preview-duplicate');
});

test('mine-preview-small', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 650 });
  await openPreview(page);
  const box = await page.locator('.m-modal').boundingBox();
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.y + box!.height).toBeLessThanOrEqual(650);
  await shot(page, 'mine-preview-small');
});

test('mine-added-toast', async ({ page }) => {
  await openPreview(page);
  await page.getByRole('button', { name: 'Send to Anki' }).click();
  await expect(page.locator('.m-modal')).toHaveCount(0);
  await expect(page.locator('.toast.ok')).toContainText('Added to Anki');
  await expect(page.locator('.toast.ok .toast-action')).toHaveText('Show');
  await expect(page.locator('.p-mined-dot')).toHaveCount(1);
  await page.waitForTimeout(350);
  expect((await mock(page)).added).toHaveLength(1);
  await shot(page, 'mine-added-toast');
});

test('seekbar-mined-ticks', async ({ page }) => {
  await openPlayer(page, '&mined=1,3,4,6', 13.5);
  await expect(page.locator('.p-seek-mined')).toHaveCount(4);
  await expect(page.locator('.p-mined-dot')).toHaveCount(1); // line 4 is showing
  await page.locator('.p-seek').hover({ position: { x: 640, y: 11 } });
  await page.waitForTimeout(250);
  await shot(page, 'seekbar-mined-ticks');
});

test('behaviour: Shift+click adds instantly without a modal', async ({ page }) => {
  await openMinePopup(page);
  await addBtn(page).click({ modifiers: ['Shift'] });
  await expect(page.locator('.toast.ok')).toContainText('Added to Anki');
  await expect(page.locator('.m-modal')).toHaveCount(0);
  await expect(page.locator('.p-popup')).toHaveCount(0);
  const m = await mock(page);
  expect(m.added).toHaveLength(1);
  expect(m.added[0].edits).toEqual({});
  expect(m.drafts[0]).toMatchObject({ cueId: 1, start: 2, len: 4, senses: [], jpSubId: 1, enSubId: 2 });
  await expect(page.locator('.p-mined-dot')).toHaveCount(1);
});

test('behaviour: Enter opens the preview, Shift+Enter adds instantly', async ({ page }) => {
  await openMinePopup(page);
  await page.keyboard.press('Enter');
  await expect(page.locator('.m-modal')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('.m-modal')).toHaveCount(0);
  await page.locator('.unit.word', { hasText: '決めた' }).click();
  await expect(page.locator('.p-reading')).toHaveText('きめる');
  await page.keyboard.press('Shift+Enter');
  await expect(page.locator('.toast.ok')).toContainText('Added to Anki');
  expect((await mock(page)).added).toHaveLength(1);
});

test('behaviour: instant add failure shows an error toast and the button recovers', async ({ page }) => {
  await openMinePopup(page, '&adderr=1');
  await addBtn(page).click({ modifiers: ['Shift'] });
  await expect(page.locator('.toast.error')).toContainText('duplicate');
  await expect(addBtn(page)).toBeEnabled();
  await expect(page.locator('.p-popup .p-spin')).toHaveCount(0);
});

test('behaviour: sense checkbox re-drafts the card and keeps the typed translation', async ({ page }) => {
  await openPreview(page);
  const back = page.frameLocator('iframe[title="Card back"]');
  await expect(back.locator('.meaning li')).toHaveCount(2);
  await page.locator('.m-modal textarea').fill('My own translation.');
  await page.locator('.m-sense', { hasText: 'to resign oneself' }).click();
  await expect(back.locator('.meaning li')).toHaveCount(1);
  await expect(back.locator('.meaning li')).toContainText('to give up');
  await expect(back.locator('.trans')).toHaveText('My own translation.');
  const m = await mock(page);
  expect(m.drafts).toHaveLength(2);
  expect(m.drafts[1].senses).toEqual([0]);
  // The last remaining sense cannot be unchecked.
  await expect(page.locator('.m-sense input').first()).toBeDisabled();
});

test('behaviour: editing the translation updates the Back preview and is sent', async ({ page }) => {
  await openPreview(page);
  const back = page.frameLocator('iframe[title="Card back"]');
  await expect(back.locator('.trans')).toContainText("I decided I'm not giving up anymore.");
  await page.locator('.m-modal textarea').fill('Never giving up <again>.');
  await expect(back.locator('.trans')).toHaveText('Never giving up <again>.');
  await page.locator('.m-modal select').selectOption('Japanese::Anime');
  await page.getByRole('button', { name: 'Send to Anki' }).click();
  await expect(page.locator('.toast.ok')).toBeVisible();
  const m = await mock(page);
  expect(m.added[0].edits).toEqual({ sentenceTranslation: 'Never giving up <again>.', deck: 'Japanese::Anime' });
});

test('behaviour: Esc and backdrop click close the preview', async ({ page }) => {
  await openPreview(page);
  await page.keyboard.press('Escape');
  await expect(page.locator('.m-modal')).toHaveCount(0);
  await page.locator('.unit.word', { hasText: '諦めない' }).click();
  await expect(addBtn(page)).toBeVisible();
  await addBtn(page).click();
  await expect(page.locator('.m-modal')).toBeVisible();
  await page.mouse.click(8, 8);
  await expect(page.locator('.m-modal')).toHaveCount(0);
  expect((await mock(page)).added).toHaveLength(0);
});
