/**
 * The contract between the React UI and the Rust core (Tauri commands). Phase 1.
 *
 * Every call into Rust goes through `backend` below, so the UI can be rendered without the
 * desktop app by swapping in a mock (`installMockBackend` in src/lib/mock-backend.ts, used by
 * the screenshot tests). Playback control itself (play, pause, seek, mpv properties) goes
 * straight to mpv through `src/lib/player.ts`, not through here.
 *
 * Rust side: `src-tauri/src/commands.rs` implements one command per method with the same name in
 * snake_case. Arguments are camelCase in JS (Tauri converts). Errors reject with a user-readable string.
 */
import { invoke } from '@tauri-apps/api/core';

/** A stream in the media file, or an external subtitle file found next to it. */
export interface Track {
  /** mpv's track id for this kind (1-based, the same numbers mpv uses for vid/aid/sid). */
  id: number;
  kind: 'video' | 'audio' | 'sub';
  /** ffprobe codec name, e.g. "h264", "hevc", "flac", "ass", "subrip". */
  codec: string;
  /** ISO 639 code as tagged in the file ("jpn", "eng", "ja"…), if any. */
  lang?: string;
  title?: string;
  isDefault?: boolean;
  /** True for a subtitle file next to the video (e.g. "Ep01.ja.ass") rather than inside it. */
  external?: boolean;
  /** For external tracks: the file path (mpv needs it to add the track). */
  path?: string;
}

/** One subtitle line, already cleaned of ASS override tags. */
export interface Cue {
  /** Index in the track's cue list (stable for a given file + track + parser version). */
  id: number;
  startMs: number;
  endMs: number;
  /** Plain text; line breaks are "\n". */
  text: string;
  /** Dialogue is drawn by the WebView (clickable); signs/songs are drawn by mpv. */
  kind: 'dialogue' | 'sign' | 'song';
}

/** A word-sized piece of a subtitle line for hover/click (several tokens joined, e.g. 知らなかった). */
export interface WordUnit {
  /** Offset in the line, in Unicode code points (use `[...text]`, not `.length`). */
  start: number;
  /** Length in code points. */
  len: number;
  text: string;
  /** Hiragana reading of the whole unit, when the tokenizer knows it. */
  reading?: string;
  /** Dictionary form of the head word (e.g. 知る for 知らなかった), when known. */
  base?: string;
  /** False for punctuation, spaces and symbols: rendered but not hoverable. */
  isWord: boolean;
}

/** Everything the UI needs to start playing a file; returned by `prepareMedia`. */
export interface PreparedMedia {
  path: string;
  /** Display title, e.g. "Sunset Journey – 07" (from the file name). */
  title: string;
  durationMs?: number;
  tracks: Track[];
  /** The Japanese subtitle track chosen (remembered per file, else by language tag), if any. */
  jpSubId?: number;
  /** The English subtitle track chosen, if any. */
  enSubId?: number;
  /**
   * An ASS file containing only the sign/song lines of the Japanese track, for mpv to draw
   * (`sub-files-append` on loadfile, then select it as `sid`). Absent when there are none.
   */
  signsAssPath?: string;
  /** Where to resume (last position), if the file was watched before and not finished. */
  resumeMs?: number;
  /** Subtitle offset remembered for this file (positive = subtitles later). */
  offsetMs: number;
  /** Japanese cue ids already mined from this file (gold marks on the seek bar). */
  minedCueIds: number[];
}

/** Anki connection state for the popup button and the mining dialog. */
export interface AnkiStatus {
  connected: boolean;
  version?: number;
  decks: string[];
  /** Why it isn't connected (Anki closed, AnkiConnect missing…), in plain words. */
  error?: string;
}

/** What the user is mining: the clicked word in the current Japanese line. */
export interface MineRequest {
  path: string;
  jpSubId: number;
  enSubId?: number;
  /** Japanese dialogue cue the word was clicked in. */
  cueId: number;
  /** The word's place in the cue text (code points), highlighted in the sentence. */
  start: number;
  len: number;
  /** The dictionary entry shown in the popup, and which senses to keep (indexes; empty = all). */
  entry: DictEntry;
  senses: number[];
  /** Playback position for the screenshot, in media time (ms). */
  frameMs: number;
  /** Subtitle offset in effect (ms), so the sentence audio is cut where the line is spoken. */
  offsetMs: number;
  /** mpv audio track id in use (undefined = the first audio track). */
  audioId?: number;
}

/** A card ready to send: its field values (HTML, as stored in Anki) and media previews. */
export interface CardDraft {
  /** Pass to addCard. Drafts live in memory until the app closes. */
  draftId: string;
  /** Field name → value, exactly as they will be stored (see anki/fields.json). */
  fields: Record<string, string>;
  /** Preview URLs (data:) for the screenshot and audio. */
  screenshotUrl?: string;
  sentenceAudioUrl?: string;
  wordAudioUrl?: string;
  /** Where the sentence translation came from ('none' = empty, the user can type one). */
  translationSource: 'subtitles' | 'deepl' | 'ollama' | 'none';
  /** Existing notes of this note type with the same word (any deck). */
  duplicates: number[];
  /** Deck the card goes to (remembered), and the user's decks. */
  deck: string;
  decks: string[];
}

/** A file in the "Continue watching" row. */
export interface RecentItem {
  path: string;
  title: string;
  positionMs: number;
  durationMs: number;
  /** ISO-8601 timestamp. */
  lastOpened: string;
  /** Frame image for the card, as a URL the WebView can load (convertFileSrc), if one exists. */
  thumbUrl?: string;
}

/** Pitch accent of one reading (Tokyo dialect, from Kanjium). */
export interface Pitch {
  /** Mora after which the pitch drops; 0 = no drop (heiban). */
  downstep: number;
  pattern: 'heiban' | 'atamadaka' | 'nakadaka' | 'odaka';
  /** The reading split into morae (きゃ, っ, ー count as their own mora where they are one). */
  morae: string[];
}

/** One meaning of an entry. */
export interface Sense {
  glosses: string[];
  /** Part-of-speech labels in plain English, e.g. "Ichidan verb", "transitive", "noun". */
  pos: string[];
  /** Usage notes: "usually written using kana alone", "slang", "colloquial"… */
  notes: string[];
}

/** A dictionary entry as shown in the popup (one written form + reading). */
export interface DictEntry {
  /** JMdict sequence number. */
  id: number;
  /** Headword as written (kanji form, or kana when the word has none / is usually kana). */
  term: string;
  /** Reading in kana (hiragana; katakana kept for katakana words). */
  reading: string;
  /** Furigana segments, e.g. 諦める → [{text:'諦', ruby:'あきら'}, {text:'める'}]. */
  furigana: { text: string; ruby?: string }[];
  senses: Sense[];
  /** One entry per known accent (some words have two). Empty when unknown. */
  pitch: Pitch[];
  /** JMdict "common" (news1/ichi1/spec1/gai1 priority). */
  common: boolean;
  /** Approximate frequency rank (1 = most frequent) from JMdict's nfXX tags; absent when unknown. */
  freqRank?: number;
  /** Other written forms, e.g. あきらめる for 諦める. */
  otherForms: string[];
}

/** Result of looking up the text at a position; several per click (longest match first). */
export interface LookupResult {
  /** Offset (code points) and length of the matched text in the line. */
  start: number;
  len: number;
  /** The text as it appeared, e.g. 諦めない. */
  matched: string;
  /** Dictionary form it was reduced to, when it was conjugated, e.g. 諦める. */
  deinflected?: string;
  /** Conjugation steps, outermost last, in plain words: ["negative"], ["causative", "passive", "negative", "past"]. */
  inflections: string[];
  /** Matching entries, best first (exact reading/term match, common, frequency). */
  entries: DictEntry[];
}

/** Pronunciation audio for a word. */
export interface WordAudio {
  /** Playable URL (data: URL with WAV/MP3). */
  url: string;
  /** Where it came from, for the tooltip/credit ("VOICEVOX: 四国めたん" must be credited). */
  source: 'voicevox' | 'windows' | 'pack';
  credit?: string;
}

/** Everything the user can set (stored by Rust in settings.json; never in browser storage). */
export interface Settings {
  ankiDeck: string;
  /** Extra tags on every mined card (no spaces). */
  ankiTags: string[];
  /** Use DeepL for lines without English subtitles (only when a key is stored). */
  useDeepl: boolean;
  /** Use a local Ollama model when DeepL isn't available. */
  useOllama: boolean;
  /** Preferred Ollama model (null = the first installed one). */
  ollamaModel: string | null;
  enMode: 'blur' | 'always' | 'hidden';
  pauseOnHover: boolean;
  /** Subtitle size multiplier, 0.7–1.6. */
  subScale: number;
  /** Z/X step in ms. */
  offsetStepMs: number;
  accent: 'sakura' | 'twilight' | 'matcha' | 'sunset';
}

/** Settings plus facts about services and secrets (the DeepL key itself never reaches the UI). */
export interface SettingsView {
  settings: Settings;
  hasDeeplKey: boolean;
  /** Installed Ollama models (empty when Ollama isn't running). */
  ollamaModels: string[];
  /** Name of the installed Japanese Windows voice, if any. */
  japaneseVoice: string | null;
  /** VOICEVOX is running on this PC. */
  voicevox: boolean;
  /** Dictionary attribution text for the About section (required by the licences). */
  attribution: string | null;
}

/** Result of an automatic subtitle sync. */
export interface SyncSummary {
  /** Median shift applied (ms; positive = later). */
  medianShiftMs: number;
  /** Min and max differ when the track was split (e.g. around an ad break). */
  minShiftMs: number;
  maxShiftMs: number;
  /** Lines whose timing changed. */
  changed: number;
}

/** Result of importing a Yomitan dictionary. */
export interface ImportSummary {
  title: string;
  revision: string;
  kind: 'Terms' | 'Frequency' | 'Both' | 'Pitch';
  terms: number;
  frequencies: number;
  pitches: number;
}

/** An installed (imported) dictionary. */
export interface UserDictionary {
  title: string;
  kind: ImportSummary['kind'];
  rows: number;
}

export interface Backend {
  /** Probe the file, find tracks (incl. external subs next to it), restore per-file state. */
  prepareMedia(path: string): Promise<PreparedMedia>;
  /** Parsed cues of a subtitle track of the prepared file (all kinds, sorted by start). */
  getCues(path: string, subId: number): Promise<Cue[]>;
  /** For each Japanese dialogue cue id, the English cue ids that overlap it (may be several or none). */
  alignCues(path: string, jpSubId: number, enSubId: number): Promise<Record<number, number[]>>;
  /** Split one line into word units. */
  tokenize(text: string): Promise<WordUnit[]>;
  /** Remember the user's choices for this file. Pass null to clear a subtitle role. */
  saveFileState(path: string, state: { offsetMs?: number; jpSubId?: number | null; enSubId?: number | null }): Promise<void>;
  /** Remember the playback position (call every ~5 s while playing, and on pause/close). */
  savePosition(path: string, positionMs: number, durationMs: number): Promise<void>;
  /** Most recent first, at most 20. */
  recentFiles(): Promise<RecentItem[]>;
  /** Paths passed on the command line ("Open with" from Explorer). */
  startupFiles(): Promise<string[]>;
  /** Dictionary lookup at code-point `offset` of `text` (a subtitle line): longest match first. Empty when nothing matches. */
  lookupAt(text: string, offset: number): Promise<LookupResult[]>;
  /** Pronunciation of a word, or null when no source is available (no VOICEVOX running, no Japanese Windows voice). */
  wordAudio(term: string, reading: string): Promise<WordAudio | null>;
  /** Is Anki reachable (never rejects; errors are in the status). */
  ankiStatus(): Promise<AnkiStatus>;
  /** Make the card's media (screenshot, sentence audio, word audio) and fields; also installs/updates the note type. */
  draftCard(req: MineRequest): Promise<CardDraft>;
  /** Send a draft to Anki (with the user's edits); returns the new note id and marks the cue as mined. */
  addCard(draftId: string, edits: { sentenceTranslation?: string; deck?: string }): Promise<number>;
  /** Open these notes in Anki's browser. */
  showInAnki(noteIds: number[]): Promise<void>;
  /** Add a subtitle file (dropped on the player) to this video; it takes the JP or EN role by its language. */
  addSubtitleFile(path: string, file: string): Promise<PreparedMedia>;
  /** Retime subtitle track `subId` against `refSubId` (e.g. the release's English track); remembered per file. Reload its cues after. */
  syncSubtitles(path: string, subId: number, refSubId: number): Promise<SyncSummary>;
  /** Undo the auto-sync of a track. Reload its cues after. */
  resetSync(path: string, subId: number): Promise<void>;
  getSettings(): Promise<SettingsView>;
  /** Save settings; resolves with what was stored (values are clamped). */
  saveSettings(settings: Settings): Promise<Settings>;
  /** Store a DeepL key (checked with DeepL first; resolves "used / limit" text) or remove it (null). */
  setDeeplKey(key: string | null): Promise<string | null>;
  /** Import a Yomitan dictionary .zip (terms, frequency list or pitch); replaces one with the same title. */
  importDictionary(path: string): Promise<ImportSummary>;
  userDictionaries(): Promise<UserDictionary[]>;
  removeDictionary(title: string): Promise<void>;
}

const tauriBackend: Backend = {
  prepareMedia: (path) => invoke('prepare_media', { path }),
  getCues: (path, subId) => invoke('get_cues', { path, subId }),
  alignCues: (path, jpSubId, enSubId) => invoke('align_cues', { path, jpSubId, enSubId }),
  tokenize: (text) => invoke('tokenize', { text }),
  saveFileState: (path, state) => invoke('save_file_state', { path, state }),
  savePosition: (path, positionMs, durationMs) => invoke('save_position', { path, positionMs, durationMs }),
  recentFiles: () => invoke('recent_files'),
  startupFiles: () => invoke('startup_files'),
  lookupAt: (text, offset) => invoke('lookup_at', { text, offset }),
  wordAudio: (term, reading) => invoke('word_audio', { term, reading }),
  ankiStatus: () => invoke('anki_status'),
  draftCard: (req) => invoke('draft_card', { req }),
  addCard: (draftId, edits) => invoke('add_card', { draftId, edits }),
  showInAnki: (noteIds) => invoke('show_in_anki', { noteIds }),
  addSubtitleFile: (path, file) => invoke('add_subtitle_file', { path, file }),
  syncSubtitles: (path, subId, refSubId) => invoke('sync_subtitles', { path, subId, refSubId }),
  resetSync: (path, subId) => invoke('reset_sync', { path, subId }),
  getSettings: () => invoke('get_settings'),
  saveSettings: (settings) => invoke('save_settings', { settings }),
  setDeeplKey: (key) => invoke('set_deepl_key', { key }),
  importDictionary: (path) => invoke('import_dictionary', { path }),
  userDictionaries: () => invoke('user_dictionaries'),
  removeDictionary: (title) => invoke('remove_dictionary', { title }),
};

let current: Backend = tauriBackend;

/** The backend in use (Tauri in the app, a mock in screenshot tests). */
export const backend: Backend = new Proxy({} as Backend, {
  get: (_t, key: keyof Backend) => current[key],
});

/** Replace the backend (screenshot tests and storybook-like previews only). */
export function setBackend(b: Backend): void {
  current = b;
}
