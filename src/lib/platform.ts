/** Desktop-shell helpers (file dialog, drag-and-drop, fullscreen) with harmless fallbacks in the browser/mock. */
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { isMock } from './env';
import { VIDEO_EXTENSIONS } from './format';

/** Show the system "open video" dialog; resolves to the chosen path or null. */
export async function pickVideo(): Promise<string | null> {
  if (isMock) return null;
  const path = await open({ multiple: false, filters: [{ name: 'Video', extensions: VIDEO_EXTENSIONS }] });
  return typeof path === 'string' ? path : null;
}

/** Ask for a Yomitan dictionary .zip. */
export async function pickDictionaryZip(): Promise<string | null> {
  if (isMock) return null;
  const path = await open({ multiple: false, filters: [{ name: 'Yomitan dictionary', extensions: ['zip'] }] });
  return typeof path === 'string' ? path : null;
}

/** Listen for files dropped on the window; `hover` reports whether a drag is over the window. */
export async function onFileDrop(onDrop: (paths: string[]) => void, hover: (over: boolean) => void): Promise<() => void> {
  if (isMock) return () => {};
  return getCurrentWebview().onDragDropEvent((e) => {
    const t = e.payload.type;
    if (t === 'enter' || t === 'over') hover(true);
    else if (t === 'leave') hover(false);
    else if (t === 'drop') {
      hover(false);
      onDrop(e.payload.paths);
    }
  });
}

export async function isFullscreen(): Promise<boolean> {
  if (isMock) return false;
  return getCurrentWindow().isFullscreen();
}

export async function setFullscreen(on: boolean): Promise<void> {
  if (isMock) return;
  await getCurrentWindow().setFullscreen(on);
}
