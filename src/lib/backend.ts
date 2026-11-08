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
