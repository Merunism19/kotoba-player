/** The only module that talks to mpv (tauri-plugin-libmpv-api): init once, observe properties, load/seek/pause/volume/speed/audio; swappable for the mock in screenshots. */
import { invoke } from '@tauri-apps/api/core';
import { getProperty, listenEvents, observeProperties } from 'tauri-plugin-libmpv-api';

// mpv is started and driven only through the app's guarded Rust commands (src-tauri/src/mpv_guard.rs):
// fixed hardened options and an allowlist of commands/properties. The plugin's own init/command/
// set_property are not permitted to the window (security review 2026-11-08); reads and events are.
const init = (): Promise<void> => invoke('mpv_start');
const command = (name: string, args: (string | number | boolean)[] = []): Promise<void> =>
  invoke('mpv_command', { name, args });
const setProperty = (name: string, value: string | number | boolean): Promise<void> =>
  invoke('mpv_set', { name, value });

/** Property changes the UI cares about (time in seconds, volume 0-100). */
export interface PropChange {
  name: 'pause' | 'time-pos' | 'duration' | 'volume' | 'speed';
  value: number | boolean;
}

export interface PlayerHandlers {
  onProp(p: PropChange): void;
  /** The file stopped because of an error (not a normal end or a user stop). */
  onError(message: string): void;
}

export interface LoadOptions {
  startSec: number;
  /** ASS file with only the sign/song lines; mpv draws it, everything else is drawn by the WebView. */
  signsAssPath?: string;
}

export interface PlayerApi {
  /** Init mpv (once) and start delivering events. Returns an unsubscribe function. */
  start(h: PlayerHandlers): Promise<() => void>;
  /** Resolves when the file is loaded and the track selection is applied; rejects on error/timeout. */
  loadFile(path: string, o: LoadOptions): Promise<void>;
  stop(): Promise<void>;
  setPause(p: boolean): Promise<void>;
  /** `exact` = frame-accurate (for line navigation); otherwise mpv's default (fast) seek. */
  seek(sec: number, mode: 'absolute' | 'relative', exact?: boolean): Promise<void>;
  setVolume(v: number): Promise<void>;
  setSpeed(s: number): Promise<void>;
  setAudio(id: number | null): Promise<void>;
}

// Format tuples as the plugin wants them; 'none' marks values that are null before a file is loaded.
const OBSERVED = [
  ['pause', 'flag'],
  ['time-pos', 'double', 'none'],
  ['duration', 'double', 'none'],
  ['volume', 'double'],
  ['speed', 'double'],
] as const;

interface MpvTrack {
  type: string;
  id: number;
  'external-filename'?: string;
}

const norm = (p: string) => p.replace(/[\\]/g, '/').toLowerCase();

/** mpv option values may contain commas/colons: `%<byte length>%value` escapes them. */
const escapeOpt = (v: string) => `%${new TextEncoder().encode(v).length}%${v}`;

let mpvInit: Promise<void> | null = null;

function createMpvPlayer(): PlayerApi {
  const handlers = new Set<PlayerHandlers>();
  let pending: { resolve(): void; reject(e: Error): void } | null = null;
  let signsPath: string | undefined;
  let eventsStarted: Promise<void> | null = null;

  async function applyTrackSelection() {
    // Track ids must be strings (a JS number reaches mpv as a double and is refused). track-list is read
    // as a JSON string: the plugin's 'node' format crashes the app (segfault).
    let sid = 'no';
    if (signsPath) {
      try {
        const raw = await getProperty('track-list', 'string');
        const tracks = JSON.parse(raw ?? '[]') as MpvTrack[];
        const want = norm(signsPath);
        const t = tracks.find((x) => x.type === 'sub' && x['external-filename'] && norm(x['external-filename']) === want);
        if (t) sid = String(t.id);
      } catch (e) {
        console.warn('signs track lookup failed', e);
      }
    }
    await setProperty('sid', sid);
    await setProperty('secondary-sid', 'no');
  }

  function startEvents(): Promise<void> {
    eventsStarted ??= (async () => {
      // Once per page: React dev mode mounts effects twice, and a second init/load raced the first.
      // Options and observed properties are fixed in Rust (mpv_guard.rs); OBSERVED below must match.
      mpvInit ??= init();
      await mpvInit;
      await observeProperties(OBSERVED, ({ name, data }) => {
        if (data == null) return;
        for (const h of handlers) h.onProp({ name, value: data });
      });
      await listenEvents((e) => {
        if (e.event === 'file-loaded' && pending) {
          const p = pending;
          pending = null;
          applyTrackSelection().then(p.resolve, p.reject);
        } else if (e.event === 'end-file' && e.reason === 'error') {
          const msg = 'mpv could not play this file (unsupported codec or damaged file).';
          if (pending) {
            const p = pending;
            pending = null;
            p.reject(new Error(msg));
          } else {
            for (const h of handlers) h.onError(msg);
          }
        }
      });
    })();
    return eventsStarted;
  }

  return {
    async start(h) {
      handlers.add(h);
      await startEvents();
      return () => handlers.delete(h);
    },
    async loadFile(path, o) {
      await startEvents();
      signsPath = o.signsAssPath;
      const opts = ['sid=no', 'secondary-sid=no', 'pause=no'];
      if (o.startSec > 0.5) opts.push(`start=${o.startSec.toFixed(3)}`);
      if (o.signsAssPath) opts.push(`sub-files-append=${escapeOpt(o.signsAssPath)}`);
      // A newer load supersedes one still in flight: settle the old promise instead of leaving it hanging.
      pending?.reject(new Error('Superseded by another file.'));
      const loaded = new Promise<void>((resolve, reject) => {
        pending = { resolve, reject };
        setTimeout(() => {
          if (pending && pending.reject === reject) {
            pending = null;
            reject(new Error('Timed out opening the file.'));
          }
        }, 20000);
      });
      // loadfile <file> <flags> <index> <options>: index -1 = append position (mpv >= 0.38).
      await command('loadfile', [path, 'replace', '-1', opts.join(',')]);
      await loaded;
    },
    stop: () => command('stop'),
    setPause: (p) => setProperty('pause', p),
    seek: (sec, mode, exact) => command('seek', [sec, exact ? `${mode}+exact` : mode]),
    setVolume: (v) => setProperty('volume', v),
    setSpeed: (s) => setProperty('speed', s),
    setAudio: (id) => setProperty('aid', id == null ? 'no' : String(id)),
  };
}

let current: PlayerApi | null = null;

/** The player in use (mpv in the app, a fake in screenshot tests). */
export const player: PlayerApi = new Proxy({} as PlayerApi, {
  get: (_t, key: keyof PlayerApi) => (current ??= createMpvPlayer())[key],
});

/** Replace the player (screenshot tests only). */
export function setPlayer(p: PlayerApi): void {
  current = p;
}
