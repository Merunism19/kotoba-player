/** Small formatting helpers: clock times, "time left", subtitle offsets, file names, language labels. */
export function fmtTime(sec: number | null | undefined): string {
  if (sec == null || !isFinite(sec) || sec < 0) return '--:--';
  const s = Math.floor(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** "12:34 left", or "Watched" when (almost) finished. */
export function fmtLeft(positionMs: number, durationMs: number): string {
  if (!durationMs) return '';
  const left = durationMs - positionMs;
  if (left < durationMs * 0.03) return 'Watched';
  return `${fmtTime(left / 1000)} left`;
}

/** Offset badge text: "+0.30 s". */
export function fmtOffset(ms: number): string {
  const sign = ms < 0 ? '\u2212' : '+';
  return `${sign}${(Math.abs(ms) / 1000).toFixed(2)} s`;
}

export function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

const VIDEO_EXT = new Set(['mkv', 'mp4', 'webm', 'avi', 'mov', 'm4v', 'ts', 'm2ts', 'wmv', 'flv']);
export const VIDEO_EXTENSIONS = [...VIDEO_EXT];

export function isVideoPath(path: string): boolean {
  const ext = basename(path).split('.').pop()?.toLowerCase() ?? '';
  return VIDEO_EXT.has(ext);
}

const LANGS: Record<string, string> = { jpn: '日本語', ja: '日本語', jp: '日本語', eng: 'English', en: 'English' };

/** "日本語", "English" or the raw tag. */
export function langLabel(lang?: string): string {
  if (!lang) return '';
  return LANGS[lang.toLowerCase()] ?? lang;
}

export function errMsg(e: unknown): string {
  if (e instanceof Error) return e.message;
  return typeof e === 'string' ? e : JSON.stringify(e);
}
