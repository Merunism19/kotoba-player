/** Playback session logic: opening files, position saving, line navigation, offset, track choices, toasts; glue between store, backend and mpv. */
import { backend } from '../../lib/backend';
import type { Cue, WordUnit } from '../../lib/backend';
import { player } from '../../lib/player';
import { activeIndex, lastStartedIndex, nextIndex } from '../../lib/cues';
import { basename, errMsg, isVideoPath } from '../../lib/format';
import { isFullscreen, onFileDrop, pickVideo, setFullscreen } from '../../lib/platform';
import { EN_MODES, get, set, useApp, type Toast } from '../../app/store';

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

/** Open a video: prepare, fetch cues, load in mpv (resuming), switch to the player screen. */
export async function openFile(path: string): Promise<void> {
  if (get().opening) return;
  if (!isVideoPath(path)) {
    toast(`Not a video file: ${basename(path)}`);
    return;
  }
  set({ opening: true });
  try {
    if (get().screen === 'player') await savePositionNow();
    const media = await backend.prepareMedia(path);
    const data = await loadCueData(path, media.jpSubId, media.enSubId);
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
  } catch (e) {
    toast(errMsg(e));
    await goHome(false);
  } finally {
    set({ opening: false });
  }
}

export async function goHome(save = true): Promise<void> {
  if (save) await savePositionNow();
  set({ screen: 'home', popup: null, mining: null, hoverPaused: false, media: null, pause: true });
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
  await refreshRecents();
  try {
    unsubs.push(
      await player.start({
        onProp: ({ name, value }) => {
          switch (name) {
            case 'time-pos': set({ time: value as number }); break;
            case 'duration': set({ duration: value as number }); break;
            case 'pause': set({ pause: value as boolean }); break;
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

  unsubs.push(await onFileDrop((paths) => paths[0] && void openFile(paths[0]), (over) => set({ dragging: over })));

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
  void player.setPause(!get().pause);
}

export function seekTo(sec: number, exact = false): void {
  const d = get().duration;
  const t = Math.max(0, d ? Math.min(sec, d) : sec);
  set({ time: t });
  void player.seek(t, 'absolute', exact);
}

export function seekBy(delta: number): void {
  const d = get().duration;
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
const mediaSec = (cue: Cue) => (cue.startMs + get().offsetMs) / 1000;

export function seekLine(dir: -1 | 1): void {
  const cues = currentDialogue();
  const t = subTimeMs();
  const i = dir < 0 ? lastStartedIndex(cues, t - 500) : nextIndex(cues, t + 50);
  if (i >= 0) seekTo(mediaSec(cues[i]), true);
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
  set({ enMode: next });
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
  set({ popup: { ...p, resume: !s.pause || s.hoverPaused }, hoverPaused: false });
  if (!s.pause) void player.setPause(true);
}

export function closePopup(): void {
  const { popup } = get();
  if (!popup) return;
  set({ popup: null });
  if (popup.resume) void player.setPause(false);
}
