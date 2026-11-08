/** Renders the Anki templates in Chromium the way Anki does (field substitution, sections, furigana filter, sound buttons) into artifacts/shots/card-*.png. */
import { test, expect, type Page } from '@playwright/test';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const ROOT = process.cwd();
const OUT = 'artifacts/shots';
const WORK = resolve(ROOT, 'artifacts/card');
const FIXTURE = resolve(ROOT, 'tests/fixtures/card/screenshot.png');
const read = (p: string) => readFileSync(resolve(ROOT, p), 'utf8');

type Note = Record<string, string>;

const PITCH_SVG =
  '<svg viewBox="0 0 150 34" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">' +
  '<polyline points="12,26 36,8 60,8 84,8 108,26 132,26"/></g><g fill="currentColor"><circle cx="12" cy="26" r="3.5"/><circle cx="36" cy="8" r="3.5"/><circle cx="60" cy="8" r="3.5"/><circle cx="84" cy="8" r="3.5"/><circle cx="108" cy="26" r="3.5"/></g>' +
  '<circle cx="132" cy="26" r="3.5" fill="none" stroke="currentColor" stroke-width="2"/></svg><span style="font-size:12px;opacity:.7;margin-left:8px;vertical-align:middle;font-family:system-ui">[4]</span>';

const AKIRAMERU: Note = {
  Word: '諦める',
  WordFurigana: '諦[あきら]める',
  WordReading: 'あきらめる',
  PitchPattern: PITCH_SVG,
  PitchClass: 'nakadaka',
  Meaning: '<ol><li>to give up; to abandon (hope, plans)</li><li>to resign oneself (to)</li></ol>',
  Sentence: 'もう<b>諦めない</b>って決めたんだ。',
  SentenceFurigana: 'もう<b> 諦[あきら]めない</b>って 決[き]めたんだ。',
  SentenceTranslation: "I decided I'm not giving up anymore.",
  Screenshot: '<img src="kotoba_shot.png">',
  SentenceAudio: '[sound:kotoba_s.mp3]',
  WordAudio: '[sound:kotoba_w.mp3]',
  Source: 'Sunset Journey · S01E07 · 09:52',
};

const CAMERA: Note = {
  Word: 'カメラ',
  WordFurigana: '',
  WordReading: 'カメラ',
  PitchPattern: '',
  PitchClass: 'heiban',
  Meaning: '<ol><li>camera</li></ol>',
  Sentence: '先週の日曜日に友達と一緒に買ったばかりの新しい<b>カメラ</b>を、うっかり電車の中に置き忘れてしまったことに今さら気がついた。',
  SentenceFurigana: '',
  SentenceTranslation: 'I only just realised that I had absent-mindedly left behind on the train the new camera I bought with a friend last Sunday.',
  Screenshot: '',
  SentenceAudio: '[sound:kotoba_s.mp3]',
  WordAudio: '[sound:kotoba_w.mp3]',
  Source: 'Sunset Journey · S01E03 · 14:02',
};

const PLAY = '<a class="replay-button soundLink" href="#"><svg class="playImage" viewBox="0 0 64 64"><circle cx="32" cy="32" r="29"/><path d="M56.502,32.301l-37.502,20.101l0.329,-40.804l37.173,20.703Z" transform="scale(.5) translate(32 32)"/></svg></a>';

/** Anki's furigana filter. */
const furigana = (s: string) => s.replace(/ ?([^ >]+?)\[(.+?)\]/g, '<ruby><rb>$1</rb><rt>$2</rt></ruby>');

/** Renders exactly the template syntax we use: {{#F}}..{{/F}}, {{^F}}..{{/F}}, {{furigana:F}}, {{F}}, {{FrontSide}}. */
function render(tpl: string, note: Note, front = ''): string {
  let out = tpl;
  const section = /\{\{([#^])([A-Za-z]+)\}\}((?:(?!\{\{[#^]).)*?)\{\{\/\2\}\}/s;
  for (let m = section.exec(out); m; m = section.exec(out)) {
    const filled = (note[m[2]] ?? '').trim() !== '';
    out = out.slice(0, m.index) + ((m[1] === '#') === filled ? m[3] : '') + out.slice(m.index + m[0].length);
  }
  out = out.replace(/\{\{FrontSide\}\}/g, front);
  out = out.replace(/\{\{furigana:([A-Za-z]+)\}\}/g, (_, f) => furigana(note[f] ?? ''));
  out = out.replace(/\{\{([A-Za-z]+)\}\}/g, (_, f) => note[f] ?? '');
  out = out.replace(/\[sound:[^\]]+\]/g, PLAY);
  if (/\{\{|\}\}/.test(out)) throw new Error('unrendered template syntax: ' + out.match(/\{\{[^}]*\}\}/)?.[0]);
  return out;
}

function pageHtml(body: string, night: boolean): string {
  const cls = night ? 'card nightMode night_mode' : 'card';
  return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${read('anki/style.css')}</style></head><body class="${cls}"><div class="card">${body}</div></body></html>`;
}

async function show(page: Page, tpl: 'front' | 'back', note: Note, night: boolean, name: string) {
  mkdirSync(WORK, { recursive: true });
  copyFileSync(FIXTURE, resolve(WORK, 'kotoba_shot.png'));
  const front = render(read('anki/front.html'), note);
  const html = tpl === 'front' ? front : render(read('anki/back.html'), note, front);
  const file = resolve(WORK, `${name}.html`);
  writeFileSync(file, pageHtml(html, night));
  await page.goto(pathToFileURL(file).href);
  await page.waitForLoadState('load');
  await page.mouse.move(2, 2);
  await page.waitForTimeout(150);
}

const shot = (page: Page, name: string) => page.screenshot({ path: `${OUT}/${name}.png`, fullPage: true });

test.beforeAll(async ({ browser }) => {
  // Generate the screenshot fixture once from the mockup's .scene art.
  if (existsSync(FIXTURE)) return;
  mkdirSync(resolve(ROOT, 'tests/fixtures/card'), { recursive: true });
  const mock = read('docs/design/mockup.html');
  const css = (mock.match(/\.scene\{[^}]*\}[\s\S]*?\.scene \.grain\{[^}]*\}/) ?? [''])[0];
  const html = `<style>*{margin:0}${css}</style><div class="scene" style="width:640px;height:360px"><div class="sun"></div><div class="m1"></div><div class="m2"></div><div class="figure"></div><div class="m3"></div><div class="grain"></div></div>`;
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  await page.setContent(html);
  await page.screenshot({ path: FIXTURE });
  await page.close();
});

test('front dark', async ({ page }) => {
  await show(page, 'front', AKIRAMERU, true, 'front-dark');
  await expect(page.locator('.kt .word ruby')).toHaveCount(1);
  await expect(page.locator('.kt .shot img')).toBeVisible();
  await shot(page, 'card-front-dark');
});

test('back dark', async ({ page }) => {
  await show(page, 'back', AKIRAMERU, true, 'back-dark');
  await expect(page.locator('.kt .trans')).toContainText('giving up');
  await expect(page.locator('.kt .replay-button')).toHaveCount(2);
  await shot(page, 'card-back-dark');
});

test('front light', async ({ page }) => {
  await show(page, 'front', AKIRAMERU, false, 'front-light');
  await shot(page, 'card-front-light');
});

test('back light', async ({ page }) => {
  await show(page, 'back', AKIRAMERU, false, 'back-light');
  await shot(page, 'card-back-light');
});

test('word does not jump between front and back', async ({ page }) => {
  await show(page, 'front', AKIRAMERU, true, 'jump-f');
  const a = await page.locator('.kt .word').boundingBox();
  await show(page, 'back', AKIRAMERU, true, 'jump-b');
  const b = await page.locator('.kt .word').boundingBox();
  expect(Math.abs(a!.y - b!.y)).toBeLessThan(1);
  expect(Math.abs(a!.x - b!.x)).toBeLessThan(1);
});

test('hover shows reading', async ({ page }) => {
  await show(page, 'back', AKIRAMERU, false, 'back-hover');
  const ruby = page.locator('.kt .sent ruby').nth(1); // 決
  await expect(ruby.locator('rt')).toHaveCSS('opacity', '0');
  await ruby.hover();
  await page.waitForTimeout(300);
  await expect(ruby.locator('rt')).toHaveCSS('opacity', '1');
  await shot(page, 'card-back-hover');
});

test('tap toggles reading (touch)', async ({ page }) => {
  await show(page, 'front', AKIRAMERU, true, 'tap');
  const ruby = page.locator('.kt .word ruby');
  await page.mouse.move(2, 2);
  await ruby.dispatchEvent('click');
  await expect(ruby).toHaveClass(/on/);
});

test('mobile back dark', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await show(page, 'back', AKIRAMERU, true, 'mobile');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(page, 'card-mobile-back-dark');
});

test('katakana back light, empty fields', async ({ page }) => {
  await show(page, 'back', CAMERA, false, 'katakana');
  await expect(page.locator('.kt .shot')).toHaveCount(0);
  await expect(page.locator('.kt .pitch')).toHaveCount(0);
  await shot(page, 'card-katakana-back-light');
});
