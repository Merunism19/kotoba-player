/** Playback session logic: opening files, position saving, line navigation, offset, track choices, toasts; glue between store, backend and mpv. */
import { backend } from '../../lib/backend';
import type { Cue, PreparedMedia, SyncSummary, WordUnit } from '../../lib/backend';
import { player } from '../../lib/player';
import { activeIndex, lastStartedIndex, nextIndex } from '../../lib/cues';
import { basename, errMsg, fmtOffset, isVideoPath } from '../../lib/format';
import { isFullscreen, onFileDrop, pickVideo, setFullscreen } from '../../lib/platform';
import { EN_MODES, get, set, useApp, type Toast } from '../../app/store';
import { loadSettings, updateSettings } from '../settings/settings';

let toastId = 1;

/** Show a glass toast bottom-right; errors stay 6 s, info 2 s, anything with an action button 6 s. */
export function toast(message: string, kind: 'error' | 'info' | 'ok' = 'error', action?: Toast['action']): void {
  const id = toastId++;
  set((s) => ({ toasts: [...s.toasts.slice(-3), { id, message, kind, action }] }));
  setTimeout(() => dismissToast(id), kind === 'error' || action ? 6000 : 2000);
}

export function dismissToast(id: number): void {
  set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
}

/** Reveal the controls (mouse move, key press, offset change). */
export function wake(): void {
  set((s) => ({ activity: s.activity + 1 }));
}

export async function refreshRecents(): Promise<void> {
  try {
    set({ recents: await backend.recentFiles(), recentsLoaded: true });
  } catch (e) {
    console.warn('recentFiles failed', e);
    set({ recentsLoaded: true });
  }
}

/** Save the current position (every 5 s while playing, on pause, on leaving). */
export async function savePositionNow(): Promise<void> {
  const { media, time, duration } = get();
  if (!media || !duration) return;
  try {
    await backend.savePosition(media.path, Math.round(time * 1000), Math.round(duration * 1000));
  } catch (e) {
    console.warn('savePosition failed', e);
  }
}

async function loadCueData(path: string, jp: number | undefined, en: number | undefined) {
  const [jpCues, enCues] = await Promise.all([
    jp != null ? backend.getCues(path, jp) : Promise.resolve([] as Cue[]),
    en != null ? backend.getCues(path, en) : Promise.resolve([] as Cue[]),
  ]);
  const align: Record<number, number[]> =
    jp != null && en != null ? await backend.alignCues(path, jp, en).catch(() => ({})) : {};
  return { jpCues, enCues, align };
}

/** Bumped by every open and by going home: a slower open that finishes later is discarded (review 2026-11-08). */
let openToken = 0;

/** Open a video: prepare, fetch cues, load in mpv (resuming), switch to the player screen. */
export async function openFile(path: string): Promise<void> {
  if (get().opening) return;
  if (!isVideoPath(path)) {
    toast(`Not a video file: ${basename(path)}`);
    return;
  }
  const token = ++openToken;
  set({ opening: true });
  try {
    if (get().screen === 'player') await savePositionNow();
    const media = await backend.prepareMedia(path);
    const data = await loadCueData(path, media.jpSubId, media.enSubId);
    if (token !== openToken) return; // the user went home (or opened something else) meanwhile
    set({
      media,
      ...data,
      audioId: undefined,
      offsetMs: media.offsetMs,
      time: (media.resumeMs ?? 0) / 1000,
      duration: (media.durationMs ?? 0) / 1000,
      popup: null,
      mining: null,
      minedCueIds: media.minedCueIds ?? [],
      hoverPaused: false,
      screen: 'player',
    });
    wake();
    await player.loadFile(path, { startSec: (media.resumeMs ?? 0) / 1000, signsAssPath: media.signsAssPath });
    await player.setPause(false);
    // mpv sends no event when pause was already off (e.g. after going home, which only stops), so
    // set the state we just applied — otherwise the Play button showed "paused" and did nothing.
    set({ pause: false, hoverPaused: false });
    void autoSyncToAudio(media);
  } catch (e) {
    if (token !== openToken) return;
    toast(errMsg(e));
    await goHome(false);
  } finally {
    if (token === openToken) set({ opening: false });
  }
}

const SUB_EXT = new Set(['ass', 'ssa', 'srt', 'vtt']);
const isSubtitlePath = (path: string) => SUB_EXT.has(basename(path).split('.').pop()?.toLowerCase() ?? '');

/** Files dropped on the window: a subtitle file goes onto the video being played, anything else is opened as a video. */
export async function handleDrop(paths: string[]): Promise<void> {
  const first = paths[0];
  if (!first) return;
  if (isSubtitlePath(first)) {
    if (get().screen === 'player' && get().media) await addSubtitleFile(first);
    else toast('Open a video first, then drop the subtitle file on it.', 'info');
  } else await openFile(first);
}

export async function goHome(save = true): Promise<void> {
  // Cancels an open still in flight (its result is discarded) and lets the next open start.
  openToken++;
  if (save) await savePositionNow();
  set({ screen: 'home', popup: null, mining: null, hoverPaused: false, media: null, pause: true, opening: false });
  try {
    await player.stop();
  } catch (e) {
    console.warn('stop failed', e);
  }
  await refreshRecents();
}

export async function openDialog(): Promise<void> {
  const path = await pickVideo();
  if (path) await openFile(path);
}

/** Boot: start mpv, wire events, drag-and-drop, periodic saves, then open the startup file. */
export async function startApp(): Promise<() => void> {
  const unsubs: (() => void)[] = [];
  await Promise.all([refreshRecents(), loadSettings()]);
  try {
    unsubs.push(
      await player.start({
        onProp: ({ name, value }) => {
          switch (name) {
            case 'time-pos':
              // Ignore the old file's clock while opening, and stale positions that were in flight when
              // we seeked (they made rapid D, D skip only one line) — review 2026-11-08.
              if (get().opening || performance.now() < seekGuardUntil) break;
              set({ time: value as number });
              break;
            case 'duration': set({ duration: value as number }); break;
            case 'pause':
              // Playing again by any route (Space, button, mpv) ends a hover-pause.
              set(value ? { pause: true } : { pause: false, hoverPaused: false });
              break;
            case 'volume': set({ volume: value as number }); break;
            case 'speed': set({ speed: value as number }); break;
          }
        },
        onError: (message) => {
          toast(message);
          void goHome(false);
        },
      }),
    );
  } catch (e) {
    toast(`Could not start the video engine: ${errMsg(e)}`);
  }

  unsubs.push(await onFileDrop((paths) => void handleDrop(paths), (over) => set({ dragging: over })));

  // Save on pause, and every 5 s while playing.
  let lastPause = get().pause;
  unsubs.push(
    useApp.subscribe((s) => {
      if (s.pause !== lastPause) {
        lastPause = s.pause;
        if (s.pause && s.screen === 'player') void savePositionNow();
      }
    }),
  );
  const timer = setInterval(() => {
    const s = get();
    if (s.screen === 'player' && !s.pause) void savePositionNow();
  }, 5000);
  const onUnload = () => void savePositionNow();
  window.addEventListener('beforeunload', onUnload);
  unsubs.push(() => {
    clearInterval(timer);
    window.removeEventListener('beforeunload', onUnload);
  });

  const files = await backend.startupFiles().catch(() => [] as string[]);
  if (files[0]) await openFile(files[0]);
  return () => unsubs.forEach((u) => u());
}

// ---- playback controls -------------------------------------------------------------------------

export function togglePause(): void {
  // A manual pause/play is the user's decision: a later hover-leave must not override it.
  set({ hoverPaused: false });
  void player.setPause(!get().pause);
}

/** time-pos events before this moment are ignored (they were in flight when we seeked). */
let seekGuardUntil = 0;

export function seekTo(sec: number, exact = false): void {
  const d = get().duration;
  const t = Math.max(0, d ? Math.min(sec, d) : sec);
  seekGuardUntil = performance.now() + 150;
  set({ time: t });
  void player.seek(t, 'absolute', exact);
}

export function seekBy(delta: number): void {
  const d = get().duration;
  seekGuardUntil = performance.now() + 150;
  set((s) => ({ time: Math.max(0, d ? Math.min(s.time + delta, d) : s.time + delta) }));
  void player.seek(delta, 'relative');
}

export function setVolume(v: number): void {
  const vol = Math.max(0, Math.min(100, Math.round(v)));
  set({ volume: vol });
  void player.setVolume(vol);
}

export function setSpeed(sp: number): void {
  set({ speed: sp });
  void player.setSpeed(sp);
}

export async function toggleFullscreen(): Promise<void> {
  const on = !(await isFullscreen());
  await setFullscreen(on);
  set({ fullscreen: on });
}

/** Leave fullscreen if we are in it; true when something changed. */
export async function exitFullscreen(): Promise<boolean> {
  if (!(await isFullscreen())) return false;
  await setFullscreen(false);
  set({ fullscreen: false });
  return true;
}

// ---- subtitle lines ----------------------------------------------------------------------------

let dlgSrc: Cue[] | null = null;
let dlg: Cue[] = [];

/** The Japanese dialogue cues (the ones the WebView draws), memoised per cue array. */
export function currentDialogue(): Cue[] {
  const src = get().jpCues;
  if (src !== dlgSrc) {
    dlgSrc = src;
    dlg = src.filter((c) => c.kind === 'dialogue');
  }
  return dlg;
}

/** Playback time in subtitle time (ms): subtitles are late by `offsetMs`, so shift the clock back. */
const subTimeMs = () => get().time * 1000 - get().offsetMs;
// +1 ms: float rounding of an exact seek could otherwise land a hair before the line starts.
const mediaSec = (cue: Cue) => (cue.startMs + get().offsetMs + 1) / 1000;

export function seekLine(dir: -1 | 1): void {
  const cues = currentDialogue();
  const t = subTimeMs();
  let i = dir < 0 ? lastStartedIndex(cues, t - 500) : nextIndex(cues, t + 50);
  // At the edges: A before the first line goes to the first line; D after the last says so.
  if (i < 0 && dir < 0 && cues.length) i = 0;
  if (i < 0) {
    if (dir > 0 && cues.length) toast('That was the last line.', 'info');
    return;
  }
  seekTo(mediaSec(cues[i]), true);
}

/** Jump back to the start of the line being shown (or the one that just ended). */
export function replayLine(): void {
  const cues = currentDialogue();
  const t = subTimeMs();
  let i = activeIndex(cues, t);
  if (i < 0) i = lastStartedIndex(cues, t);
  if (i >= 0) {
    seekTo(mediaSec(cues[i]), true);
    if (get().pause) void player.setPause(false);
  }
}

export function setOffset(ms: number): void {
  const { media } = get();
  const next = Math.round(ms);
  set((s) => ({ offsetMs: next, offsetFlash: s.offsetFlash + 1 }));
  wake();
  if (media) void backend.saveFileState(media.path, { offsetMs: next }).catch((e) => console.warn(e));
}

export const nudgeOffset = (deltaMs: number) => setOffset(get().offsetMs + deltaMs);

/** Shift+Z: make the current line (or the next one, between lines) start exactly now. */
export function syncLineNow(): void {
  const cues = currentDialogue();
  const t = subTimeMs();
  let i = activeIndex(cues, t);
  if (i < 0) i = nextIndex(cues, t);
  if (i >= 0) setOffset(get().time * 1000 - cues[i].startMs);
}

const EN_LABEL = { blur: 'blurred until hover', always: 'always shown', hidden: 'hidden' };

export function cycleEnMode(): void {
  const next = EN_MODES[(EN_MODES.indexOf(get().enMode) + 1) % EN_MODES.length];
  updateSettings({ enMode: next });
  set({ enMode: next }); // also when settings never loaded
  toast(`English line: ${EN_LABEL[next]}`, 'info');
}

// ---- tracks ------------------------------------------------------------------------------------

export async function chooseSub(role: 'jp' | 'en', id: number | null): Promise<void> {
  const { media } = get();
  if (!media) return;
  const jp = role === 'jp' ? id ?? undefined : media.jpSubId;
  const en = role === 'en' ? id ?? undefined : media.enSubId;
  try {
    const data = await loadCueData(media.path, jp, en);
    set({ ...data, media: { ...media, jpSubId: jp, enSubId: en } });
    await backend.saveFileState(media.path, role === 'jp' ? { jpSubId: id } : { enSubId: id });
  } catch (e) {
    toast(errMsg(e));
  }
}

export async function chooseAudio(id: number | null): Promise<void> {
  set({ audioId: id });
  try {
    await player.setAudio(id);
  } catch (e) {
    toast(errMsg(e));
  }
}

/** Reload the cue lists of the chosen tracks after a sync or a new subtitle file. */
async function reloadCues(media: PreparedMedia): Promise<void> {
  set({ ...(await loadCueData(media.path, media.jpSubId, media.enSubId)) });
}

/** A subtitle file was dropped on the player: add it to this video, show its lines, say which language it became. */
export async function addSubtitleFile(file: string): Promise<void> {
  const before = get().media;
  if (!before) return;
  try {
    const media = await backend.addSubtitleFile(before.path, file);
    if (get().media?.path !== before.path) return; // the user moved on meanwhile
    set({ media });
    await reloadCues(media);
    // mpv draws the signs of the JP track: when that changed, reload at the current position.
    if (media.signsAssPath !== before.signsAssPath) {
      const { time, pause } = get();
      await player.loadFile(media.path, { startSec: time, signsAssPath: media.signsAssPath });
      if (pause) await player.setPause(true);
    }
    const role = media.jpSubId !== before.jpSubId ? 'Japanese' : media.enSubId !== before.enSubId ? 'English' : '';
    toast(`Added ${role ? role + ' ' : ''}subtitles: ${basename(file)}`, 'ok');
    if (role) void autoSyncToAudio(media);
  } catch (e) {
    toast(errMsg(e));
  }
}

/** "moved 2.88 s earlier", plus the spread when the track was shifted by different amounts in different places. */
function describeSync(r: SyncSummary, to = ''): string {
  const en = r.followChanged ? '; English lined up with it' : '';
  if (r.changed === 0 || Math.abs(r.medianShiftMs) < 5) {
    if (r.followChanged) return 'English lined up with the Japanese';
    return to ? `Already in sync with the ${to}` : 'Already in sync';
  }
  const s = (Math.abs(r.medianShiftMs) / 1000).toFixed(2);
  let msg = `Synced${to ? ` to the ${to}` : ''}: moved ${s} s ${r.medianShiftMs < 0 ? 'earlier' : 'later'}`;
  if (r.minShiftMs !== r.maxShiftMs) msg += ` (in parts, from ${fmtOffset(r.minShiftMs)} to ${fmtOffset(r.maxShiftMs)})`;
  msg += en;
  return msg;
}

/** Track picker: retime the Japanese subtitles against the English track; the toast offers Undo. */
export async function syncToEnglish(): Promise<void> {
  const { media, syncing } = get();
  if (!media || syncing || media.jpSubId == null || media.enSubId == null) return;
  const jp = media.jpSubId;
  set({ syncing: true });
  try {
    const summary = await backend.syncSubtitles(media.path, jp, media.enSubId);
    // The signs file is rewritten with the new timing; afterSync has mpv re-read it.
    await afterSync(media, [jp], summary, 'English track');
  } catch (e) {
    toast(`Could not sync: ${errMsg(e)}`);
  } finally {
    set({ syncing: false });
  }
}

/** mpv's id of the audio track being heard (the chosen one, else the default, else the first). */
function audioTrackId(media: PreparedMedia): number | undefined {
  const chosen = get().audioId;
  if (typeof chosen === 'number') return chosen;
  const audio = media.tracks.filter((t) => t.kind === 'audio');
  return (audio.find((t) => t.isDefault) ?? audio[0])?.id;
}

/** After a sync: show the new timing (and mpv the moved signs), and say what happened, with Undo (of `undo`'s tracks). */
async function afterSync(media: PreparedMedia, undo: number[], summary: SyncSummary, to: string, toVoices = false): Promise<void> {
  // Synced to the voices, the lines now start with them: a manual delay (offset) left on top put them
  // off again (owner's report 2026-11-10), so it is cleared, and the message counts it in what moved.
  const offset = get().offsetMs;
  const cleared = toVoices && offset !== 0;
  if (cleared) setOffset(0);
  await reloadCues(media);
  if (media.signsAssPath) await player.reloadSubtitles().catch(() => {});
  const shown: SyncSummary = cleared
    ? {
        ...summary,
        medianShiftMs: summary.medianShiftMs - offset,
        minShiftMs: summary.minShiftMs - offset,
        maxShiftMs: summary.maxShiftMs - offset,
        changed: summary.changed || get().jpCues.length || 1,
      }
    : summary;
  const moved = shown.changed || shown.followChanged;
  const undoIt = () => void undoSync(media.path, undo, cleared ? offset : undefined);
  toast(describeSync(shown, to), 'ok', moved ? { label: 'Undo', run: undoIt } : undefined);
}

/** The tracks a sync changed, for its Undo. */
const changedTracks = (s: SyncSummary, jp: number | undefined, en: number | undefined): number[] =>
  [s.changed ? jp : undefined, s.followChanged ? en : undefined].filter((x): x is number => x != null);

/** Track picker: retime the Japanese subtitles to the voices in the audio being heard. */
export async function syncToAudio(): Promise<void> {
  const { media, syncing } = get();
  if (!media || syncing || media.jpSubId == null) return;
  const jp = media.jpSubId;
  set({ syncing: true });
  try {
    const en = media.enSubId ?? undefined;
    const summary = await backend.syncToAudio(media.path, jp, audioTrackId(media), en);
    if (get().media?.path !== media.path) return;
    await afterSync(media, changedTracks(summary, jp, en), summary, 'voices', true);
  } catch (e) {
    toast(`Could not sync: ${errMsg(e)}`);
  } finally {
    set({ syncing: false });
  }
}

/**
 * Subtitle FILES (next to the video, or dropped in) often come from another release and are off,
 * so they are synced automatically, once per file (an Undo sticks): a Japanese file to the voices
 * (the English then follows it), an English file to the Japanese track. Embedded tracks match
 * their video and are left alone. Quiet when there was nothing to fix.
 */
async function autoSyncToAudio(media: PreparedMedia): Promise<void> {
  const isFile = (id: number | null | undefined) => media.tracks.some((t) => t.kind === 'sub' && t.id === id && t.external);
  const jp = media.jpSubId ?? undefined;
  const en = media.enSubId ?? undefined;
  const jpFile = isFile(jp);
  if (get().syncing || jp == null || !(jpFile || isFile(en))) return;
  if (get().settingsView?.settings.autoSyncAudio === false) return;
  set({ syncing: true });
  try {
    const summary = await backend.autoSyncToAudio(media.path, jpFile ? jp : undefined, audioTrackId(media), en, jp);
    const now = get().media;
    if (!summary || !(summary.changed || summary.followChanged) || now?.path !== media.path || now.jpSubId !== media.jpSubId) return;
    await afterSync(now, changedTracks(summary, jp, en), summary, 'voices', true);
  } catch (e) {
    console.warn('auto sync to audio failed', e); // e.g. no audio track: the manual buttons still work
  } finally {
    set({ syncing: false });
  }
}

/** Undo a sync: the tracks' original timing, and the manual delay it cleared (`offsetMs`). */
async function undoSync(path: string, tracks: number[], offsetMs?: number): Promise<void> {
  const media = get().media;
  if (!media || media.path !== path) return;
  try {
    for (const id of tracks) await backend.resetSync(path, id);
    if (offsetMs != null) setOffset(offsetMs);
    await reloadCues(media);
    if (media.signsAssPath) await player.reloadSubtitles().catch(() => {});
    toast('Sync undone', 'info');
  } catch (e) {
    toast(errMsg(e));
  }
}

export function toggleTranscript(): void {
  set((s) => ({ transcriptOpen: !s.transcriptOpen }));
}

// ---- word popup --------------------------------------------------------------------------------

/** Pause while the pointer is over the subtitle line (setting stub, on by default). */
export function hoverLine(over: boolean): void {
  const s = get();
  if (!s.pauseOnHover || s.popup || s.mining) return;
  if (over && !s.pause) {
    set({ hoverPaused: true });
    void player.setPause(true);
  } else if (!over && s.hoverPaused) {
    set({ hoverPaused: false });
    void player.setPause(false);
  }
}

export function openPopup(p: { unit: WordUnit; line: string; cueId: number; anchor: { x: number; top: number; bottom: number; width: number } }): void {
  const s = get();
  // Clicking another word while a popup is open keeps the first popup's decision (playback was
  // already paused by that popup, so `!s.pause` would wrongly say "don't resume") — review 2026-11-08.
  const resume = s.popup ? s.popup.resume : !s.pause || s.hoverPaused;
  set({ popup: { ...p, resume }, hoverPaused: false });
  if (!s.pause) void player.setPause(true);
}

export function closePopup(): void {
  const { popup } = get();
  if (!popup) return;
  set({ popup: null });
  if (popup.resume) void player.setPause(false);
}
