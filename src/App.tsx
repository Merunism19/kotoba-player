/**
 * Phase 0 spike: mpv video under a transparent WebView with a clickable subtitle line and glass controls.
 * Replaced by the real player in Phase 1; kept small on purpose.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { command, getProperty, init, listenEvents, observeProperties, setProperty } from 'tauri-plugin-libmpv-api';
import './App.css';

const OBSERVED = [
  ['pause', 'flag'],
  ['time-pos', 'double', 'none'],
  ['duration', 'double', 'none'],
  ['filename', 'string', 'none'],
  ['sub-text', 'string', 'none'],
  ['secondary-sub-text', 'string', 'none'],
  ['hwdec-current', 'string', 'none'],
  ['video-codec', 'string', 'none'],
  ['video-params/pixelformat', 'string', 'none'],
] as const;

let mpvInit: Promise<string> | null = null;
let startupLoaded = false;

const fmt = (s: number | null) => {
  if (s == null || !isFinite(s)) return '--:--';
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${String(sec).padStart(2, '0')}`;
};

export default function App() {
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pause, setPause] = useState(true);
  const [time, setTime] = useState<number | null>(null);
  const [duration, setDuration] = useState<number | null>(null);
  const [file, setFile] = useState<string | null>(null);
  const [jp, setJp] = useState('');
  const [en, setEn] = useState('');
  const [clicked, setClicked] = useState<{ i: number; ch: string } | null>(null);
  const [info, setInfo] = useState<Record<string, string>>({});
  const [rate, setRate] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const ticks = useRef(0);

  const load = useCallback(async (path: string) => {
    setClicked(null);
    await command('loadfile', [path]);
    await setProperty('pause', false);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        // Once per page: React dev mode mounts effects twice, and a second init/load raced the first.
        mpvInit ??= init({
          initialOptions: {
            vo: 'gpu-next',
            hwdec: 'auto-safe',
            'keep-open': 'yes',
            'force-window': 'yes',
            // Dialogue is drawn by the WebView (clickable); mpv keeps track selection.
            'sub-visibility': 'no',
            'secondary-sub-visibility': 'no',
            slang: 'jpn,ja,jp',
            alang: 'jpn,ja,jp',
            'osd-level': '0',
          },
          observedProperties: OBSERVED,
        });
        await mpvInit;
        if (cancelled) return;
        unlisten = await observeProperties(OBSERVED, ({ name, data }) => {
          switch (name) {
            case 'pause': setPause(data); break;
            case 'time-pos': setTime(data); ticks.current++; break;
            case 'duration': setDuration(data); break;
            case 'filename': setFile(data); break;
            case 'sub-text': setJp(data ?? ''); break;
            case 'secondary-sub-text': setEn(data ?? ''); break;
            default: setInfo((prev) => ({ ...prev, [name]: String(data ?? '–') }));
          }
        });
        if (cancelled) { unlisten(); return; }
        setReady(true);
        if (!startupLoaded) {
          startupLoaded = true;
          const files = await invoke<string[]>('startup_files');
          if (files[0]) await load(files[0]);
        }
      } catch (e) {
        setError(String(e));
      }
    })();
    const id = setInterval(() => { setRate(ticks.current); ticks.current = 0; }, 1000);
    return () => { cancelled = true; clearInterval(id); unlisten?.(); };
  }, [load]);

  // After a file loads, pick the English subtitle track as the secondary one.
  // Track ids must be passed as strings (a JS number reaches mpv as a double and is refused), and
  // track-list is read as a JSON string: the plugin's 'node' format crashes the app (segfault).
  useEffect(() => {
    const un = listenEvents(async (e) => {
      if (e.event !== 'file-loaded') return;
      try {
        const raw = await getProperty('track-list', 'string');
        const tracks = JSON.parse(raw ?? '[]') as { type: string; id: number; lang?: string }[];
        const eng = tracks.find((t) => t.type === 'sub' && /^en/i.test(t.lang ?? ''));
        await setProperty('secondary-sid', eng ? String(eng.id) : 'no');
      } catch (err) {
        console.warn('secondary track selection failed', err);
      }
    });
    return () => { void un.then((f) => f()); };
  }, []);

  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === 'drop' && e.payload.paths[0]) void load(e.payload.paths[0]);
    });
    return () => { void un.then((f) => f()); };
  }, [load]);

  const openFile = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: 'Video', extensions: ['mkv', 'mp4', 'webm', 'avi', 'mov', 'm4v', 'ts', 'm2ts', 'wmv', 'flv'] }],
    });
    if (typeof path === 'string') await load(path);
  };

  const togglePause = () => setProperty('pause', !pause);
  const seek = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!duration) return;
    const r = e.currentTarget.getBoundingClientRect();
    void command('seek', [((e.clientX - r.left) / r.width) * duration, 'absolute']);
  };
  const fullscreen = async () => {
    const w = getCurrentWindow();
    await w.setFullscreen(!(await w.isFullscreen()));
  };
  const shot = async () => {
    const path = await join(await appCacheDir(), `spike-shot-${Date.now()}.webp`);
    await command('screenshot-to-file', [path, 'video']);
    setToast(`Screenshot → ${path}`);
    setTimeout(() => setToast(null), 3500);
  };
  const clickChar = (i: number, ch: string) => {
    setClicked({ i, ch });
    void setProperty('pause', true);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === 'Space') { e.preventDefault(); void togglePause(); }
      if (e.key === 'ArrowLeft') void command('seek', [-5]);
      if (e.key === 'ArrowRight') void command('seek', [5]);
      if (e.key === 'f') void fullscreen();
      if (e.key === 'Escape') setClicked(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return (
    <main className="stage">
      {!file && (
        <div className="home">
          <div className="home-inner">
            <div className="logo">言葉</div>
            <h1>Kotoba Player</h1>
            <p className="tag">Watch anime. Click any word. Keep it in Anki.</p>
            <div className="drop">
              <svg viewBox="0 0 24 24" width="34" height="34" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M12 15V3M7 8l5-5 5 5" /><path d="M20 15v4a2 2 0 01-2 2H6a2 2 0 01-2-2v-4" /></svg>
              <p>{error ? `mpv failed: ${error}` : ready ? 'Drop an episode anywhere in this window' : 'Starting mpv…'}</p>
              <button className="btn primary" onClick={openFile} disabled={!ready}>Open video</button>
            </div>
          </div>
        </div>
      )}

      {file && (
        <>
          <div className="topbar glass">
            <span className="title">{file}</span>
            <span className="spacer" />
            <span className="chip">{info['video-codec']?.split(' ')[0] ?? '…'}</span>
            <span className="chip">{info['video-params/pixelformat'] ?? '…'}</span>
            <span className="chip">hwdec: {info['hwdec-current'] ?? '…'}</span>
            <span className="chip sync">{rate} time events/s</span>
            <button className="btn" onClick={openFile}>Open</button>
          </div>

          {clicked && (
            <div className="popup glass" role="dialog">
              <div className="headword">{clicked.ch}</div>
              <p>Clicked character #{clicked.i} — the dictionary popup goes here (Phase 2).</p>
              <button className="btn" onClick={() => setClicked(null)}>Close</button>
            </div>
          )}

          <div className="subs">
            {jp && (
              <div className="jp-line">
                {[...jp].map((ch, i) => (
                  <span key={i} className={`tok${clicked?.i === i ? ' active' : ''}`} onClick={() => clickChar(i, ch)}>
                    {ch === '\n' ? <br /> : ch}
                  </span>
                ))}
              </div>
            )}
            {en && <div className="en-line">{en}</div>}
          </div>

          <div className="controls glass">
            <div className="seek" onClick={seek}>
              <div className="played" style={{ width: `${duration && time ? (time / duration) * 100 : 0}%` }} />
            </div>
            <div className="row">
              <button className="btn play" onClick={togglePause}>{pause ? '▶' : '❚❚'}</button>
              <span className="time">{fmt(time)} / {fmt(duration)}</span>
              <span className="spacer" />
              <button className="btn" onClick={shot}>Screenshot</button>
              <button className="btn" onClick={fullscreen}>Fullscreen</button>
            </div>
          </div>
        </>
      )}
      {toast && <div className="toast glass">{toast}</div>}
    </main>
  );
}
