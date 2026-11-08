/** Settings logic: load once at start, apply to the store and the accent tokens, save changes debounced (400 ms) through the backend. */
import { backend } from '../../lib/backend';
import type { Settings } from '../../lib/backend';
import { errMsg } from '../../lib/format';
import { get, set } from '../../app/store';
import { toast } from '../player/session';

const SAVE_DELAY_MS = 400;
let pending = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** Push the values every screen reads into the store, and the accent palette onto <html>. */
export function applySettings(s: Settings): void {
  set({ enMode: s.enMode, pauseOnHover: s.pauseOnHover, subScale: s.subScale, offsetStepMs: s.offsetStepMs });
  const root = document.documentElement;
  if (s.accent === 'sakura') delete root.dataset.accent;
  else root.dataset.accent = s.accent;
}

/** Read the settings once at app start; the defaults in the store stay if the backend can't answer. */
export async function loadSettings(): Promise<void> {
  try {
    const view = await backend.getSettings();
    set({ settingsView: view });
    applySettings(view.settings);
  } catch (e) {
    console.warn('getSettings failed', e);
  }
}

async function flush(): Promise<void> {
  timer = undefined;
  pending = false;
  const view = get().settingsView;
  if (!view) return;
  try {
    const stored = await backend.saveSettings(view.settings);
    if (pending) return; // newer edits are on their way; they will be reconciled by the next save
    set((s) => (s.settingsView ? { settingsView: { ...s.settingsView, settings: stored } } : {}));
    applySettings(stored);
  } catch (e) {
    toast(`Could not save settings: ${errMsg(e)}`);
  }
}

/** Change settings: applied at once, saved 400 ms after the last change, then replaced by the clamped stored values. */
export function updateSettings(patch: Partial<Settings>): void {
  const view = get().settingsView;
  if (!view) return;
  const next = { ...view.settings, ...patch };
  set({ settingsView: { ...view, settings: next } });
  applySettings(next);
  pending = true;
  clearTimeout(timer);
  timer = setTimeout(() => void flush(), SAVE_DELAY_MS);
}

/** Store (or remove, with null) the DeepL key; resolves the usage text DeepL answered. The key is never kept in the UI. */
export async function saveDeeplKey(key: string | null): Promise<string | null> {
  const text = await backend.setDeeplKey(key);
  set((s) => (s.settingsView ? { settingsView: { ...s.settingsView, hasDeeplKey: key !== null } } : {}));
  return text;
}

export function openSettings(): void {
  set({ settingsOpen: true });
}
export function closeSettings(): void {
  set({ settingsOpen: false });
}
