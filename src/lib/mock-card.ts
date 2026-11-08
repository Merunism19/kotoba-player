/**
 * Mock card drafts for screenshots: builds the Kotoba Mining fields the way the Rust side will (Anki furigana syntax with a
 * space before each kanji group, `<b>` around the target word, an inline currentColor pitch SVG, `<ol>` meanings), plus a
 * generated screenshot and two short beeps as preview media.
 */
import type { CardDraft, DictEntry, MineRequest, Pitch, WordUnit } from './backend';
import { fmtTime } from './format';

const isKanji = (c: string) => /[一-鿿々]/.test(c);
const toHira = (s: string) => s.replace(/[ァ-ヶ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60));
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

interface Seg {
  text: string;
  ruby?: string;
  /** Code-point range in the line. */
  s: number;
  e: number;
}

/** Splits a word and its reading into [kana][kanji+ruby][kana] pieces, e.g. 諦めない/あきらめない -> 諦[あきら] + めない. */
function splitRuby(text: string, reading: string | undefined): { text: string; ruby?: string }[] {
  const t = [...text];
  const r = [...(reading ?? '')];
  if (!t.some(isKanji) || r.length === 0) return [{ text }];
  let p = 0;
  while (p < t.length && !isKanji(t[p]) && toHira(t[p]) === r[p]) p++;
  let q = 0;
  while (q < t.length - p && !isKanji(t[t.length - 1 - q]) && toHira(t[t.length - 1 - q]) === r[r.length - 1 - q]) q++;
  const out: { text: string; ruby?: string }[] = [];
  if (p) out.push({ text: t.slice(0, p).join('') });
  out.push({ text: t.slice(p, t.length - q).join(''), ruby: r.slice(p, r.length - q).join('') });
  if (q) out.push({ text: t.slice(t.length - q).join('') });
  return out;
}

/** The sentence with furigana segments and `<b>` around [start, start+len). */
function sentenceFields(units: WordUnit[], start: number, len: number): { plain: string; furigana: string } {
  const segs: Seg[] = [];
  for (const u of units) {
    let at = u.start;
    for (const piece of u.isWord ? splitRuby(u.text, u.reading) : [{ text: u.text } as { text: string; ruby?: string }]) {
      const n = [...piece.text].length;
      segs.push({ ...piece, s: at, e: at + n });
      at += n;
    }
  }
  const end = start + len;
  const first = segs.findIndex((g) => g.e > start);
  let last = -1;
  segs.forEach((g, i) => g.s < end && (last = i));
  let plain = '';
  let fur = '';
  segs.forEach((g, i) => {
    if (i === first) {
      plain += '<b>';
      fur += '<b>';
    }
    plain += esc(g.text);
    fur += g.ruby ? `${fur ? ' ' : ''}${esc(g.text)}[${g.ruby}]` : esc(g.text);
    if (i === last) {
      plain += '</b>';
      fur += '</b>';
    }
  });
  return { plain, furigana: fur };
}

/** 諦める -> 諦[あきら]める */
function wordFurigana(entry: DictEntry): string {
  let out = '';
  for (const f of entry.furigana) out += f.ruby ? `${out ? ' ' : ''}${f.text}[${f.ruby}]` : f.text;
  return entry.furigana.some((f) => f.ruby) ? out : '';
}

/** Inline pitch graph (currentColor) with the downstep number, like the fixture in the card tests. */
function pitchSvg(p: Pitch): string {
  const n = p.morae.length;
  const high = (k: number): boolean => {
    if (p.downstep === 0) return k > 1;
    if (p.downstep === 1) return k === 1;
    return k > 1 && k <= p.downstep;
  };
  const x = (k: number) => 12 + 24 * (k - 1);
  const y = (h: boolean) => (h ? 8 : 26);
  const pts = Array.from({ length: n }, (_v, i) => `${x(i + 1)},${y(high(i + 1))}`).join(' ');
  const dots = Array.from({ length: n }, (_v, i) => `<circle cx="${x(i + 1)}" cy="${y(high(i + 1))}" r="3.5"/>`).join('');
  const particle = p.downstep === 0;
  const w = 24 * n + 30;
  return (
    `<svg viewBox="0 0 ${w} 34" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">` +
    `<polyline points="${pts} ${x(n + 1)},${y(particle)}"/></g><g fill="currentColor">${dots}</g>` +
    `<circle cx="${x(n + 1)}" cy="${y(particle)}" r="3.5" fill="none" stroke="currentColor" stroke-width="2"/></svg>` +
    `<span style="font-size:12px;opacity:.7;margin-left:8px;vertical-align:middle;font-family:system-ui">[${p.downstep}]</span>`
  );
}

function meaning(entry: DictEntry, keep: number[]): string {
  const pick = entry.senses.filter((_s, i) => keep.length === 0 || keep.includes(i));
  return `<ol>${pick.map((s) => `<li>${esc(s.glosses.join('; '))}${s.notes.length ? ` (${esc(s.notes.join('; '))})` : ''}</li>`).join('')}</ol>`;
}

let shot: string | undefined;
/** A 640x360 sunset drawn on a canvas (stands in for the video frame). */
function screenshotUrl(): string {
  if (shot) return shot;
  const c = document.createElement('canvas');
  c.width = 640;
  c.height = 360;
  const g = c.getContext('2d')!;
  const sky = g.createLinearGradient(0, 0, 0, 360);
  sky.addColorStop(0, '#2b2d6e');
  sky.addColorStop(0.55, '#d6618f');
  sky.addColorStop(1, '#ffb36b');
  g.fillStyle = sky;
  g.fillRect(0, 0, 640, 360);
  const sun = g.createRadialGradient(430, 225, 6, 430, 225, 120);
  sun.addColorStop(0, '#fff2c4');
  sun.addColorStop(0.25, 'rgba(255,222,150,.9)');
  sun.addColorStop(1, 'rgba(255,170,110,0)');
  g.fillStyle = sun;
  g.fillRect(0, 0, 640, 360);
  const ridge = (base: number, amp: number, color: string, seed: number) => {
    g.fillStyle = color;
    g.beginPath();
    g.moveTo(0, 360);
    for (let x = 0; x <= 640; x += 16) g.lineTo(x, base + Math.sin(x / 70 + seed) * amp + Math.sin(x / 23 + seed * 2) * amp * 0.3);
    g.lineTo(640, 360);
    g.fill();
  };
  ridge(250, 22, '#7b4a7e', 1);
  ridge(285, 18, '#4a2f5c', 3);
  ridge(322, 12, '#241a3a', 5);
  g.fillStyle = '#150f26';
  g.beginPath();
  g.arc(168, 262, 15, 0, Math.PI * 2);
  g.fill();
  g.fillRect(155, 274, 26, 70);
  shot = c.toDataURL('image/jpeg', 0.86);
  return shot;
}

/** A short quiet sine beep as a WAV data URL. */
function beep(hz: number, sec: number): string {
  const rate = 8000;
  const n = Math.floor(rate * sec);
  const bytes = new Uint8Array(44 + n);
  const dv = new DataView(bytes.buffer);
  const w = (o: number, s: string) => [...s].forEach((ch, i) => dv.setUint8(o + i, ch.charCodeAt(0)));
  w(0, 'RIFF');
  dv.setUint32(4, 36 + n, true);
  w(8, 'WAVEfmt ');
  dv.setUint32(16, 16, true);
  dv.setUint16(20, 1, true);
  dv.setUint16(22, 1, true);
  dv.setUint32(24, rate, true);
  dv.setUint32(28, rate, true);
  dv.setUint16(32, 1, true);
  dv.setUint16(34, 8, true);
  w(36, 'data');
  dv.setUint32(40, n, true);
  for (let i = 0; i < n; i++) bytes[44 + i] = 128 + Math.round(Math.sin((2 * Math.PI * hz * i) / rate) * 14 * Math.min(1, (n - i) / 800));
  let bin = '';
  bytes.forEach((b) => (bin += String.fromCharCode(b)));
  return `data:audio/wav;base64,${btoa(bin)}`;
}

export interface MockDraftInput {
  req: MineRequest;
  units: WordUnit[];
  /** Plain English translation ('' = none). */
  translation: string;
  title: string;
  id: string;
  duplicates: number[];
  deck: string;
  decks: string[];
}

/** A realistic CardDraft for the clicked word. */
export function mockDraft(i: MockDraftInput): CardDraft {
  const { req } = i;
  const sent = sentenceFields(i.units, req.start, req.len);
  const first = req.entry.pitch[0];
  return {
    draftId: i.id,
    fields: {
      Word: req.entry.term,
      WordFurigana: wordFurigana(req.entry),
      WordReading: req.entry.reading,
      PitchPattern: first ? pitchSvg(first) : '',
      PitchClass: first?.pattern ?? '',
      Meaning: meaning(req.entry, req.senses),
      Sentence: sent.plain,
      SentenceFurigana: sent.furigana,
      SentenceTranslation: esc(i.translation),
      Screenshot: '<img src="kotoba_mock.webp">',
      SentenceAudio: '[sound:kotoba_mock_s.mp3]',
      WordAudio: '[sound:kotoba_mock_w.mp3]',
      Source: `${i.title} · ${fmtTime(req.frameMs / 1000)}`,
    },
    screenshotUrl: screenshotUrl(),
    sentenceAudioUrl: beep(330, 0.5),
    wordAudioUrl: beep(494, 0.3),
    translationSource: i.translation ? 'subtitles' : 'none',
    duplicates: i.duplicates,
    deck: i.deck,
    decks: i.decks,
  };
}
