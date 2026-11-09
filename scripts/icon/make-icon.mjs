// Renders scripts/icon/icon.html to a 1024 px PNG (src-tauri/icons/source.png) and a 256 px copy
// for the UI (src/assets/logo.png). Then run `pnpm tauri icon src-tauri/icons/source.png` to
// regenerate the platform icon set (icon.ico etc.). Usage: node scripts/icon/make-icon.mjs [--preview]
import { chromium } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const SRC = pathToFileURL(resolve(ROOT, 'scripts/icon/icon.html')).href;

/** Open the icon page and wait for the kanji's web font (a fallback font would render otherwise). */
async function open(page) {
  await page.goto(SRC);
  const ok = await page.evaluate(async () => {
    await document.fonts.load('700 360px "Zen Maru Gothic"', '言');
    return document.fonts.check('700 360px "Zen Maru Gothic"', '言');
  });
  if (!ok) throw new Error('Zen Maru Gothic did not load (run pnpm install)');
}

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1024, height: 1024 } });
  await open(page);
  const icon = page.locator('#icon');
  const out = resolve(ROOT, 'src-tauri/icons/source.png');
  await icon.screenshot({ path: out, omitBackground: true });
  console.log('wrote', out);

  // The UI copy: same drawing at 256 px (crisp at the 96 px hero size on 2x screens).
  const small = await browser.newPage({ viewport: { width: 256, height: 256 } });
  await open(small);
  await small.evaluate(() => {
    const s = document.getElementById('icon');
    s.setAttribute('width', '256');
    s.setAttribute('height', '256');
  });
  const ui = resolve(ROOT, 'src/assets/logo.png');
  mkdirSync(dirname(ui), { recursive: true });
  await small.locator('#icon').screenshot({ path: ui, omitBackground: true });
  console.log('wrote', ui);

  if (process.argv.includes('--preview')) {
    // Sizes Windows actually shows: 16 (title bar), 24/32 (taskbar), 48 (desktop), 256 (large).
    const prev = await browser.newPage({ viewport: { width: 760, height: 330 } });
    const sizes = [16, 24, 32, 48, 64, 128, 256];
    // A data URL: an about:blank page can't load file:// images.
    const src = `data:image/png;base64,${readFileSync(out).toString('base64')}`;
    const row = (bg) =>
      `<div style="display:flex;gap:24px;align-items:end;padding:20px;background:${bg}">` +
      sizes.map((s) => `<img src="${src}" width="${s}" height="${s}">`).join('') +
      '</div>';
    await prev.setContent(`<body style="margin:0">${row('#202020')}${row('#f3f3f3')}</body>`);
    await prev.waitForLoadState('networkidle');
    const p = resolve(ROOT, 'artifacts/shots/icon-preview.png');
    await prev.screenshot({ path: p });
    console.log('wrote', p);
  }
} finally {
  await browser.close();
}
