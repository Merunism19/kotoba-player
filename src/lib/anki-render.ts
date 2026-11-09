/**
 * Renders the "Kotoba Mining" Anki templates the way Anki does, so the mining preview and the card
 * screenshot tests share one renderer: field substitution, `{{#F}}`/`{{^F}}` sections, the
 * `{{furigana:F}}` filter (Anki's exact regex), `[sound:x]` as a play button and `<img src="kotoba_x.webp">`
 * pointing at a preview image. Pure functions, no imports: also loaded by the Playwright specs.
 */

/** Field name -> field value (HTML, as stored in Anki). */
export type Note = Record<string, string>;

export interface RenderOptions {
  /** Preview audio URL per audio field, e.g. `{ SentenceAudio: 'data:…' }`; used by the play buttons. */
  audio?: Record<string, string | undefined>;
  /** Replaces the src of `<img src="kotoba_…">` (the draft's screenshot preview). */
  screenshotUrl?: string;
}

const PLAY_SVG =
  '<svg class="playImage" viewBox="0 0 64 64"><circle cx="32" cy="32" r="29"/><path d="M56.502,32.301l-37.502,20.101l0.329,-40.804l37.173,20.703Z" transform="scale(.5) translate(32 32)"/></svg>';

/** Anki's furigana filter. */
export const furigana = (s: string): string => s.replace(/ ?([^ >]+?)\[(.+?)\]/g, '<ruby><rb>$1</rb><rt>$2</rt></ruby>');

const playButton = (url?: string): string =>
  `<a class="replay-button soundLink" href="#"${url ? ` data-src="${url.replace(/"/g, '&quot;')}"` : ''}>${PLAY_SVG}</a>`;

function decorate(field: string, value: string, o: RenderOptions): string {
  let v = value.replace(/\[sound:[^\]]+\]/g, () => playButton(o.audio?.[field]));
  if (o.screenshotUrl) v = v.replace(/(<img\b[^>]*?\bsrc=")kotoba_[^"]*(")/g, (_m, a: string, b: string) => a + o.screenshotUrl + b);
  return v;
}

/** Renders exactly the template syntax we use: {{#F}}..{{/F}}, {{^F}}..{{/F}}, {{furigana:F}}, {{F}}, {{FrontSide}}. */
export function renderTemplate(tpl: string, note: Note, front = '', opts: RenderOptions = {}): string {
  let out = tpl;
  const section = /\{\{([#^])([A-Za-z]+)\}\}((?:(?!\{\{[#^]).)*?)\{\{\/\2\}\}/s;
  for (let m = section.exec(out); m; m = section.exec(out)) {
    const filled = (note[m[2]] ?? '').trim() !== '';
    out = out.slice(0, m.index) + ((m[1] === '#') === filled ? m[3] : '') + out.slice(m.index + m[0].length);
  }
  out = out.replace(/\{\{FrontSide\}\}/g, () => front);
  out = out.replace(/\{\{furigana:([A-Za-z]+)\}\}/g, (_, f: string) => decorate(f, furigana(note[f] ?? ''), opts));
  out = out.replace(/\{\{([A-Za-z]+)\}\}/g, (_, f: string) => decorate(f, note[f] ?? '', opts));
  if (/\{\{|\}\}/.test(out)) throw new Error('unrendered template syntax: ' + out.match(/\{\{[^}]*\}\}/)?.[0]);
  return out;
}

/** Front and back of a card from its templates. */
export function renderCard(templates: { front: string; back: string }, note: Note, opts: RenderOptions = {}): { front: string; back: string } {
  const front = renderTemplate(templates.front, note, '', opts);
  return { front, back: renderTemplate(templates.back, note, front, opts) };
}

/** Plain text -> HTML for a field: escapes `& < > "` and turns line breaks into `<br>`. */
export function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/\r?\n/g, '<br>');
}

/** Field HTML -> plain text (for editing): `<br>` becomes a line break, other tags are dropped, entities decoded. */
export function htmlToText(html: string): string {
  return html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;/g, "'")
    .replace(/&amp;/g, '&');
}

/** A full HTML page around rendered card HTML, with the card CSS; `night` turns on Anki's night mode classes. */
export function cardDocument(body: string, css: string, night: boolean): string {
  const cls = night ? 'card nightMode night_mode' : 'card';
  // color-scheme makes the iframe's own scrollbars dark in night mode (they were bright white).
  const scheme = night ? 'dark' : 'light';
  return `<!doctype html><html style="color-scheme:${scheme}"><head><meta charset="utf-8"><meta name="color-scheme" content="${scheme}"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${css}</style></head><body class="${cls}"><div class="card" id="kt-root">${body}</div></body></html>`;
}
