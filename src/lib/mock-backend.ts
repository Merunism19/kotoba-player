/**
 * Fake backend for screenshots: deterministic "Sunset Journey - 07" with the 7 sample lines, English, hand-made word units, 3 recents,
 * a mock dictionary (mock-dict.ts) and Anki mining (cards from mock-card.ts). Params: anki=off (Anki unreachable), dup=1 (the word is
 * already in Anki), notrans=1 (no English line), trsrc=deepl|ollama (translation source of drafts), mined=1,3 (cue ids already mined), adderr=1 (addCard fails),
 * deepl=1 (a DeepL key is stored), unsynced=1 (Japanese lines are 2.88 s late until synced), models=0 (Ollama not running). Calls are recorded in window.__mock.
 */
import { setBackend, type Backend, type MineRequest, type Settings, type Cue, type PreparedMedia, type RecentItem, type Track, type WordUnit } from './backend';
import { mockParams } from './env';
import { basename } from './format';
import { mockLookup, mockWordAudio } from './mock-dict';
import { mockDraft } from './mock-card';

const LINES: [number, number, string, string][] = [
  [1.0, 4.0, 'もう諦めないって決めたんだ。', "I decided I'm not giving up anymore."],
  [4.5, 8.0, '食べさせられなかったのは残念だね。', "It's a shame we couldn't make them eat it."],
  [9.0, 12.0, '明日、一緒に行こうよ。', "Let's go together tomorrow."],
  [12.5, 16.0, '夕焼けがこんなに綺麗だなんて知らなかった。', "I didn't know sunsets could be this beautiful."],
  [17.0, 20.5, '約束だよ。絶対に忘れないでね。', "It's a promise. Don't you ever forget."],
  [21.0, 25.0, 'カメラを持ってきたから、写真を撮ろう！', "I brought a camera, so let's take pictures!"],
  [26.0, 29.5, 'ありがとう。また会おうね。', "Thank you. Let's meet again."],
];

/** Pieces per line: [text, reading?, base?]; reading "-" marks a non-word (punctuation). */
const PIECES: Record<string, [string, string?, string?][]> = {
  'もう諦めないって決めたんだ。': [['もう', 'もう'], ['諦めない', 'あきらめない', '諦める'], ['って', 'って'], ['決めた', 'きめた', '決める'], ['んだ', 'んだ'], ['。', '-']],
  '食べさせられなかったのは残念だね。': [['食べさせられなかった', 'たべさせられなかった', '食べる'], ['のは', 'のは'], ['残念', 'ざんねん'], ['だね', 'だね'], ['。', '-']],
  '明日、一緒に行こうよ。': [['明日', 'あした'], ['、', '-'], ['一緒', 'いっしょ'], ['に', 'に'], ['行こう', 'いこう', '行く'], ['よ', 'よ'], ['。', '-']],
  '夕焼けがこんなに綺麗だなんて知らなかった。': [['夕焼け', 'ゆうやけ'], ['が', 'が'], ['こんなに', 'こんなに'], ['綺麗', 'きれい'], ['だなんて', 'だなんて'], ['知らなかった', 'しらなかった', '知る'], ['。', '-']],
  '約束だよ。絶対に忘れないでね。': [['約束', 'やくそく'], ['だよ', 'だよ'], ['。', '-'], ['絶対', 'ぜったい'], ['に', 'に'], ['忘れないで', 'わすれないで', '忘れる'], ['ね', 'ね'], ['。', '-']],
  'カメラを持ってきたから、写真を撮ろう！': [['カメラ', 'かめら'], ['を', 'を'], ['持ってきた', 'もってきた', '持ってくる'], ['から', 'から'], ['、', '-'], ['写真', 'しゃしん'], ['を', 'を'], ['撮ろう', 'とろう', '撮る'], ['！', '-']],
  'ありがとう。また会おうね。': [['ありがとう', 'ありがとう'], ['。', '-'], ['また', 'また'], ['会おう', 'あおう', '会う'], ['ね', 'ね'], ['。', '-']],
};

const tracks: Track[] = [
  { id: 1, kind: 'video', codec: 'h264' },
  { id: 1, kind: 'audio', codec: 'flac', lang: 'jpn', title: 'Japanese 2.0', isDefault: true },
  { id: 2, kind: 'audio', codec: 'aac', lang: 'eng', title: 'English 2.0 (dub)' },
  { id: 1, kind: 'sub', codec: 'ass', lang: 'jpn', title: 'Japanese (ASS)', isDefault: true },
  { id: 2, kind: 'sub', codec: 'subrip', lang: 'eng', title: 'English' },
  { id: 3, kind: 'sub', codec: 'subrip', lang: 'jpn', title: 'Japanese (SRT)', external: true, path: 'Sunset Journey - 07.ja.srt' },
];

const baseJpCues: Cue[] = [
  { id: 0, startMs: 500, endMs: 6000, text: '第七話「夕焼けの旅」', kind: 'sign' },
  ...LINES.map(([a, b, jp], i): Cue => ({ id: i + 1, startMs: a * 1000, endMs: b * 1000, text: jp, kind: 'dialogue' })),
];
/** Japanese lines as the "release" has them: 2.88 s late when unsynced=1 until syncSubtitles runs. */
const UNSYNCED_MS = mockParams.has('unsynced') ? 2880 : 0;
let lateMs = UNSYNCED_MS;
let jpSubId = 1;
let trackList = [...tracks];
const lateCues = (): Cue[] => baseJpCues.map((c) => (c.kind === 'dialogue' ? { ...c, startMs: c.startMs + lateMs, endMs: c.endMs + lateMs } : c));
const jpCues = baseJpCues; // ids and text (timing comes from lateCues)
const enCues: Cue[] = LINES.map(([a, b, , en], i): Cue => ({ id: i, startMs: a * 1000, endMs: b * 1000, text: en, kind: 'dialogue' }));

const MOCK_TITLE = 'Sunset Journey \u2013 07';
let offsetMs = Number(mockParams.get('offset') ?? 0);
const recentCount = mockParams.has('recents') ? Number(mockParams.get('recents')) : 3;

const ALL_RECENTS: RecentItem[] = [
  { path: 'C:/Anime/Sunset Journey - 07.mkv', title: MOCK_TITLE, positionMs: 1_080_000, durationMs: 1_420_000, lastOpened: '2026-11-08T20:10:00Z' },
  { path: 'C:/Anime/Lantern Festival - 03.mkv', title: 'Lantern Festival \u2013 03', positionMs: 310_000, durationMs: 1_440_000, lastOpened: '2026-11-07T21:30:00Z' },
  { path: 'C:/Anime/Harbor Town Diaries - 11.mkv', title: 'Harbor Town Diaries \u2013 11', positionMs: 1_400_000, durationMs: 1_460_000, lastOpened: '2026-11-04T19:00:00Z' },
];

function unitsFor(text: string): WordUnit[] {
  const pieces = PIECES[text];
  if (!pieces) return [{ start: 0, len: [...text].length, text, isWord: true }];
  let start = 0;
  return pieces.map(([t, reading, base]) => {
    const len = [...t].length;
    const u: WordUnit = { start, len, text: t, isWord: reading !== '-' };
    if (reading && reading !== '-') u.reading = reading;
    if (base) u.base = base;
    start += len;
    return u;
  });
}

const DECKS = ['Kotoba Mining', 'Japanese::Anime'];
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** Mining calls the mock received, for tests (window.__mock). */
const mockLog: {
  drafts: MineRequest[];
  added: { draftId: string; edits: { sentenceTranslation?: string; deck?: string } }[];
  settingsSaved: Settings[];
  deeplKeys: (string | null)[];
  subFiles: string[];
  syncs: { subId: number; refSubId: number }[];
  resets: number[];
} = { drafts: [], added: [], settingsSaved: [], deeplKeys: [], subFiles: [], syncs: [], resets: [] };
(window as unknown as { __mock: typeof mockLog }).__mock = mockLog;

let hasDeeplKey = mockParams.get('deepl') === '1';
let stored: Settings = {
  ankiDeck: 'Kotoba Mining', ankiTags: ['anime'], useDeepl: true, useOllama: true, ollamaModel: null,
  enMode: 'blur', pauseOnHover: true, subScale: Number(mockParams.get('scale') ?? 1), offsetStepMs: 100,
  accent: (mockParams.get('accent') as Settings['accent'] | null) ?? 'sakura',
};

const backend: Backend = {
  async prepareMedia(path) {
    if (/broken/i.test(path)) throw new Error('Could not open "broken.mkv": the file is damaged or uses an unsupported codec.');
    const m: PreparedMedia = {
      path,
      title: MOCK_TITLE,
      durationMs: 30_000,
      tracks: trackList,
      jpSubId,
      enSubId: 2,
      resumeMs: 0,
      offsetMs,
      minedCueIds: (mockParams.get('mined') ?? '').split(',').filter(Boolean).map(Number),
    };
    return m;
  },
  async getCues(_path, subId) {
    return subId === 1 || subId === 4 ? lateCues() : subId === 2 ? enCues : [];
  },
  async alignCues() {
    return Object.fromEntries(LINES.map((_l, i) => [i + 1, [i]]));
  },
  async tokenize(text) {
    return unitsFor(text);
  },
  async saveFileState(_path, state) {
    if (state.offsetMs != null) offsetMs = state.offsetMs;
  },
  async savePosition() {},
  async recentFiles() {
    return ALL_RECENTS.slice(0, recentCount);
  },
  async startupFiles() {
    return mockParams.has('open') ? ['C:/Anime/Sunset Journey - 07.mkv'] : [];
  },
  lookupAt: mockLookup,
  wordAudio: mockWordAudio,
  async ankiStatus() {
    if (mockParams.get('anki') === 'off') return { connected: false, decks: [], error: "Anki isn't reachable" };
    return { connected: true, version: 6, decks: DECKS };
  },
  async draftCard(req) {
    await sleep(120);
    const cue = jpCues.find((c) => c.id === req.cueId);
    if (!cue) throw new Error('mock: unknown cue ' + req.cueId);
    const translation = mockParams.has('notrans') ? '' : (enCues[req.cueId - 1]?.text ?? '');
    mockLog.drafts.push(req);
    return mockDraft({
      req,
      units: unitsFor(cue.text),
      translation,
      source: (mockParams.get('trsrc') as 'deepl' | 'ollama' | null) ?? undefined,
      title: MOCK_TITLE,
      id: `mock-draft-${mockLog.drafts.length}`,
      duplicates: mockParams.has('dup') ? [1700000000000] : [],
      deck: DECKS[0],
      decks: DECKS,
    });
  },
  async addCard(draftId, edits) {
    await sleep(300);
    if (mockParams.has('adderr')) throw new Error('AnkiConnect: cannot create note because it is a duplicate');
    mockLog.added.push({ draftId, edits });
    return 1700000000001 + mockLog.added.length;
  },
  async showInAnki() {},
  async addSubtitleFile(path, file) {
    await sleep(150);
    mockLog.subFiles.push(file);
    const name = basename(file);
    if (!trackList.some((t) => t.id === 4)) {
      trackList = [...trackList, { id: 4, kind: 'sub', codec: 'subrip', lang: 'jpn', title: name === 'other-release.ja.srt' ? name : 'other-release.ja.srt', external: true, path: file }];
    }
    jpSubId = 4;
    return { ...(await backend.prepareMedia(path)), jpSubId: 4 };
  },
  async syncSubtitles(_path, subId, refSubId) {
    await sleep(400);
    mockLog.syncs.push({ subId, refSubId });
    lateMs = 0;
    return { medianShiftMs: -2880, minShiftMs: -2880, maxShiftMs: -2880, changed: 7 };
  },
  async resetSync(_path, subId) {
    mockLog.resets.push(subId);
    lateMs = UNSYNCED_MS;
  },
  async getSettings() {
    return {
      settings: stored,
      hasDeeplKey,
      ollamaModels: mockParams.get('models') === '0' ? [] : ['qwen3:8b'],
      japaneseVoice: mockParams.has('voice') ? 'Microsoft Haruka' : null,
      voicevox: mockParams.has('voicevox'),
      attribution: 'Dictionary data: JMdict/EDICT, © the Electronic Dictionary Research and Development Group, CC BY-SA 4.0. Pitch accent: Kanjium.',
    };
  },
  async saveSettings(s) {
    mockLog.settingsSaved.push(s);
    stored = {
      ...s,
      subScale: Math.min(1.6, Math.max(0.7, s.subScale)),
      offsetStepMs: Math.min(1000, Math.max(10, Math.round(s.offsetStepMs))),
      ankiTags: s.ankiTags.map((t) => t.replace(/\s+/g, '')).filter(Boolean),
    };
    return stored;
  },
  async importDictionary() {
    const d = { title: 'JPDB frequency', revision: '2026-01', kind: 'Frequency' as const, terms: 0, frequencies: 172_000, pitches: 0 };
    mockDicts.push({ title: d.title, kind: d.kind, rows: d.frequencies });
    return d;
  },
  async userDictionaries() {
    return [...mockDicts];
  },
  async removeDictionary(title: string) {
    mockDicts = mockDicts.filter((d) => d.title !== title);
  },
  async setDeeplKey(key) {
    await sleep(350);
    mockLog.deeplKeys.push(key);
    if (key === 'bad') throw new Error('DeepL rejected this key (403). Check that you copied the whole key.');
    hasDeeplKey = key !== null;
    return key === null ? null : '12,345 / 500,000 characters used this month';
  },
};

let mockDicts: { title: string; kind: 'Terms' | 'Frequency' | 'Both' | 'Pitch'; rows: number }[] = mockParams.has('dicts')
  ? [{ title: 'JPDB frequency', kind: 'Frequency', rows: 172_000 }, { title: 'Daijirin (JA-JA)', kind: 'Terms', rows: 230_000 }]
  : [];

export function installMockBackend(): void {
  setBackend(backend);
}
