/** Mock dictionary + word audio for screenshots: hand-made entries (pitch, deinflection, multi-result/entry cases) and a generated 0.3 s beep. */
import type { DictEntry, LookupResult, Pitch, WordAudio } from './backend';
import { mockParams } from './env';

/** Build a Pitch from the reading's morae and the downstep (pattern derived like the real data). */
function pitch(morae: string[], downstep: number): Pitch {
  const n = morae.length;
  const pattern: Pitch['pattern'] = downstep === 0 ? 'heiban' : downstep === 1 ? 'atamadaka' : downstep === n ? 'odaka' : 'nakadaka';
  return { downstep, pattern, morae };
}

const entry = (e: Partial<DictEntry> & Pick<DictEntry, 'id' | 'term' | 'reading' | 'senses'>): DictEntry => ({
  furigana: [{ text: e.term }],
  pitch: [],
  common: false,
  otherForms: [],
  ...e,
});

const ICHIDAN_T = ['Ichidan verb', 'transitive'];
const GODAN_U = ['Godan verb with ‘u’ ending', 'intransitive'];

const AKIRAMERU = entry({
  id: 1_198_850,
  term: '諦める',
  reading: 'あきらめる',
  furigana: [{ text: '諦', ruby: 'あきら' }, { text: 'める' }],
  senses: [
    { glosses: ['to give up', 'to abandon (hope, plans)'], pos: ICHIDAN_T, notes: [] },
    { glosses: ['to resign oneself (to)', 'to accept (fate)'], pos: ICHIDAN_T, notes: [] },
  ],
  pitch: [pitch(['あ', 'き', 'ら', 'め', 'る'], 4)],
  common: true,
  freqRank: 2184,
  otherForms: ['あきらめる', '明らめる'],
});
const AKIRAME = entry({
  id: 1_198_840,
  term: '諦め',
  reading: 'あきらめ',
  furigana: [{ text: '諦', ruby: 'あきら' }, { text: 'め' }],
  senses: [{ glosses: ['resignation', 'acceptance', 'giving up'], pos: ['Noun'], notes: [] }],
  pitch: [pitch(['あ', 'き', 'ら', 'め'], 0)],
  common: true,
  freqRank: 9120,
});
const TABERU = entry({
  id: 1_358_280,
  term: '食べる',
  reading: 'たべる',
  furigana: [{ text: '食', ruby: 'た' }, { text: 'べる' }],
  senses: [
    { glosses: ['to eat'], pos: ICHIDAN_T, notes: [] },
    { glosses: ['to live on (e.g. a salary)', 'to live off'], pos: ICHIDAN_T, notes: [] },
  ],
  pitch: [pitch(['た', 'べ', 'る'], 2)],
  common: true,
  freqRank: 815,
});
const YUUYAKE = entry({
  id: 1_588_990,
  term: '夕焼け',
  reading: 'ゆうやけ',
  furigana: [{ text: '夕焼', ruby: 'ゆうや' }, { text: 'け' }],
  senses: [{ glosses: ['sunset glow', 'evening glow', 'red sky at sunset'], pos: ['Noun'], notes: [] }],
  pitch: [pitch(['ゆ', 'う', 'や', 'け'], 0)],
  common: true,
  freqRank: 11_480,
  otherForms: ['夕やけ', 'ゆうやけ'],
});
const KIREI = entry({
  id: 1_011_850,
  term: '綺麗',
  reading: 'きれい',
  furigana: [{ text: '綺', ruby: 'き' }, { text: '麗', ruby: 'れい' }],
  senses: [
    { glosses: ['pretty', 'lovely', 'beautiful', 'fair'], pos: ['Na-adjective'], notes: ['usually written using kana alone'] },
    { glosses: ['clean', 'pure', 'clear', 'tidy'], pos: ['Na-adjective'], notes: [] },
  ],
  pitch: [pitch(['き', 'れ', 'い'], 1)],
  common: true,
  freqRank: 1960,
  otherForms: ['奇麗', 'キレイ'],
});
const SHIRU = entry({
  id: 1_315_720,
  term: '知る',
  reading: 'しる',
  furigana: [{ text: '知', ruby: 'し' }, { text: 'る' }],
  senses: [
    { glosses: ['to know', 'to be aware of', 'to be acquainted with'], pos: ['Godan verb with ‘ru’ ending', 'transitive'], notes: [] },
    { glosses: ['to understand', 'to comprehend'], pos: ['Godan verb with ‘ru’ ending', 'transitive'], notes: [] },
    { glosses: ['to remember', 'to be able to recall'], pos: ['Godan verb with ‘ru’ ending', 'transitive'], notes: [] },
    { glosses: ['to experience', 'to go through'], pos: ['Godan verb with ‘ru’ ending', 'transitive'], notes: [] },
    { glosses: ['to be concerned with', 'to care about'], pos: ['Godan verb with ‘ru’ ending', 'transitive'], notes: ['usually in the negative'] },
  ],
  pitch: [pitch(['し', 'る'], 0)],
  common: true,
  freqRank: 224,
});
const ISSHO = entry({
  id: 1_156_550,
  term: '一緒',
  reading: 'いっしょ',
  furigana: [{ text: '一', ruby: 'いっ' }, { text: '緒', ruby: 'しょ' }],
  senses: [
    { glosses: ['together', 'at the same time'], pos: ['Noun', 'Adverb taking the ‘to’ particle'], notes: [] },
    { glosses: ['the same', 'identical'], pos: ['Noun'], notes: [] },
  ],
  pitch: [pitch(['い', 'っ', 'しょ'], 3), pitch(['い', 'っ', 'しょ'], 0)],
  common: true,
  freqRank: 1320,
  otherForms: ['一処'],
});
const KONNANI = entry({
  id: 1_004_960,
  term: 'こんなに',
  reading: 'こんなに',
  senses: [{ glosses: ['so', 'like this', 'to this extent'], pos: ['Adverb'], notes: [] }],
  common: true,
  freqRank: 1050,
});
const AU = entry({
  id: 1_198_180,
  term: '会う',
  reading: 'あう',
  furigana: [{ text: '会', ruby: 'あ' }, { text: 'う' }],
  senses: [{ glosses: ['to meet', 'to encounter', 'to see'], pos: GODAN_U, notes: [] }],
  pitch: [pitch(['あ', 'う'], 1)],
  common: true,
  freqRank: 374,
});
const AU2 = entry({
  id: 1_198_190,
  term: '遭う',
  reading: 'あう',
  furigana: [{ text: '遭', ruby: 'あ' }, { text: 'う' }],
  senses: [{ glosses: ['to meet with (an accident, misfortune)', 'to experience'], pos: GODAN_U, notes: ['esp. 遭う'] }],
  pitch: [pitch(['あ', 'う'], 1)],
  common: false,
});
const KIMERU = entry({
  id: 1_270_340,
  term: '決める',
  reading: 'きめる',
  furigana: [{ text: '決', ruby: 'き' }, { text: 'める' }],
  senses: [
    { glosses: ['to decide', 'to choose', 'to determine'], pos: ICHIDAN_T, notes: [] },
    { glosses: ['to set', 'to arrange'], pos: ICHIDAN_T, notes: [] },
  ],
  pitch: [pitch(['き', 'め', 'る'], 0)],
  common: true,
  freqRank: 330,
});

interface Spec {
  surface: string;
  deinflected?: string;
  inflections?: string[];
  entries: DictEntry[];
}
const LOOKUPS: Spec[] = [
  { surface: '諦めない', deinflected: '諦める', inflections: ['negative'], entries: [AKIRAMERU] },
  { surface: '諦', entries: [AKIRAME] },
  { surface: '食べさせられなかった', deinflected: '食べる', inflections: ['causative', 'passive', 'negative', 'past'], entries: [TABERU] },
  { surface: '夕焼け', entries: [YUUYAKE] },
  { surface: '綺麗', entries: [KIREI] },
  { surface: '知らなかった', deinflected: '知る', inflections: ['negative', 'past'], entries: [SHIRU] },
  { surface: '一緒', entries: [ISSHO] },
  { surface: 'こんなに', entries: [KONNANI] },
  { surface: '会おう', deinflected: '会う', inflections: ['volitional'], entries: [AU, AU2] },
  { surface: '決めた', deinflected: '決める', inflections: ['past'], entries: [KIMERU] },
];

/** `?slowlookup=1` makes lookups take 4 s (loading skeleton shot). */
const lookupDelay = mockParams.has('slowlookup') ? 4000 : 0;

/** Longest match first, like the real `lookup_at`. A line starting with ありがとう rejects (error toast test). */
export async function mockLookup(text: string, offset: number): Promise<LookupResult[]> {
  if (lookupDelay) await new Promise((r) => setTimeout(r, lookupDelay));
  if (text.startsWith('ありがとう') && offset === 0) throw new Error('The dictionary file could not be opened.');
  const rest = [...text].slice(offset).join('');
  return LOOKUPS.filter((l) => rest.startsWith(l.surface))
    .sort((a, b) => [...b.surface].length - [...a.surface].length)
    .map((l) => ({
      start: offset,
      len: [...l.surface].length,
      matched: l.surface,
      deinflected: l.deinflected,
      inflections: l.inflections ?? [],
      entries: l.entries,
    }));
}

/** A 0.3 s 440 Hz sine as a WAV data URL (16-bit mono, 22.05 kHz, faded at both ends). */
function beepUrl(): string {
  const rate = 22_050;
  const n = Math.round(rate * 0.3);
  const buf = new Uint8Array(44 + n * 2);
  const v = new DataView(buf.buffer);
  const str = (o: number, t: string) => [...t].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, 'RIFF'); v.setUint32(4, 36 + n * 2, true); str(8, 'WAVE'); str(12, 'fmt ');
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, rate, true); v.setUint32(28, rate * 2, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  str(36, 'data'); v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++) {
    const fade = Math.min(1, i / 400, (n - i) / 400);
    v.setInt16(44 + i * 2, Math.round(Math.sin((2 * Math.PI * 440 * i) / rate) * 9000 * fade), true);
  }
  let bin = '';
  buf.forEach((b) => (bin += String.fromCharCode(b)));
  return `data:audio/wav;base64,${btoa(bin)}`;
}
let beep: string | null = null;

/** Always available as a Windows voice, except for こんなに (no-audio case) or with `?noaudio=1`. */
export async function mockWordAudio(term: string): Promise<WordAudio | null> {
  if (mockParams.has('noaudio') || term === 'こんなに') return null;
  return { url: (beep ??= beepUrl()), source: 'windows' };
}
