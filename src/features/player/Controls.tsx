/** Bottom control bar: seek bar, play/pause, line navigation, time, speed menu, volume, fullscreen. */
import { useEffect, useState } from 'react';
import { fmtTime } from '../../lib/format';
import { useApp } from '../../app/store';
import SeekBar from './SeekBar';
import { replayLine, seekLine, setSpeed, setVolume, toggleFullscreen, togglePause } from './session';
import { IconFullscreen, IconNext, IconPause, IconPlay, IconPrev, IconReplay, IconVolume, IconMute } from './icons';

const SPEEDS = [0.75, 0.9, 1, 1.25];
const fmtSpeed = (s: number) => `${Number(s.toFixed(2))}×`;

function SpeedMenu() {
  const speed = useApp((s) => s.speed);
  const [open, setOpen] = useState(false);
  // Esc closes the menu (and only the menu: the global Esc skips fullscreen exit while a menu is open).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open]);
  return (
    <div className="p-speed">
      <button className="p-chip button" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)} title="Playback speed">
        {fmtSpeed(speed)}
      </button>
      {open && (
        <>
          <div className="p-scrim" onClick={() => setOpen(false)} />
          <div className="p-panel p-menu" role="menu" data-keep-ui>
            {SPEEDS.map((s) => (
              <button
                key={s}
                role="menuitemradio"
                aria-checked={s === speed}
                className={`p-opt${s === speed ? ' on' : ''}`}
                onClick={() => { setSpeed(s); setOpen(false); }}
              >
                {fmtSpeed(s)}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

export default function Controls() {
  const pause = useApp((s) => s.pause);
  const time = useApp((s) => s.time);
  const duration = useApp((s) => s.duration);
  const volume = useApp((s) => s.volume);
  const [lastVolume, setLastVolume] = useState(100);

  return (
    <div className="p-bottom p-panel">
      <SeekBar />
      <div className="p-row">
        <button className="p-iconbtn" aria-label="Previous line (A)" title="Previous line (A)" onClick={() => seekLine(-1)}><IconPrev /></button>
        <button className="p-iconbtn p-play" aria-label={pause ? 'Play (Space)' : 'Pause (Space)'} onClick={togglePause}>
          {pause ? <IconPlay /> : <IconPause />}
        </button>
        <button className="p-iconbtn" aria-label="Next line (D)" title="Next line (D)" onClick={() => seekLine(1)}><IconNext /></button>
        <button className="p-iconbtn" aria-label="Replay line (S)" title="Replay line (S)" onClick={replayLine}><IconReplay /></button>
        <span className="p-time">{fmtTime(time)} / {fmtTime(duration)}</span>
        <span className="p-spacer" />
        <SpeedMenu />
        <div className="p-vol">
          <button
            className="p-iconbtn"
            aria-label={volume === 0 ? 'Unmute' : 'Mute'}
            onClick={() => {
              if (volume > 0) { setLastVolume(volume); setVolume(0); } else setVolume(lastVolume || 100);
            }}
          >
            {volume === 0 ? <IconMute /> : <IconVolume />}
          </button>
          <input
            type="range" min={0} max={100} value={Math.round(volume)} aria-label="Volume"
            style={{ ['--v' as string]: `${volume}%` }}
            onChange={(e) => setVolume(Number(e.target.value))}
          />
        </div>
        <button className="p-iconbtn" aria-label="Fullscreen (F)" title="Fullscreen (F)" onClick={() => void toggleFullscreen()}><IconFullscreen /></button>
      </div>
    </div>
  );
}
