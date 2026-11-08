/** Seek bar: played fill, a tick per dialogue cue, a taller gold tick per mined line, knob, drag-to-seek and a hover time tooltip. */
import { useRef, useState } from 'react';
import { fmtTime } from '../../lib/format';
import { useApp } from '../../app/store';
import { currentDialogue, seekTo } from './session';

export default function SeekBar() {
  const time = useApp((s) => s.time);
  const duration = useApp((s) => s.duration);
  const offsetMs = useApp((s) => s.offsetMs);
  const jpCues = useApp((s) => s.jpCues);
  const ref = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const [hover, setHover] = useState<{ x: number; t: number } | null>(null);

  const frac = duration > 0 ? Math.min(1, time / duration) : 0;
  const mined = useApp((s) => s.minedCueIds);
  const minedTicks = duration > 0 ? jpCues.filter((c) => mined.includes(c.id)).map((c) => (c.startMs + offsetMs) / 1000 / duration) : [];
  const ticks = duration > 0 ? currentDialogue().map((c) => (c.startMs + offsetMs) / 1000 / duration) : [];

  const posOf = (e: React.PointerEvent) => {
    const r = ref.current!.getBoundingClientRect();
    const x = Math.max(0, Math.min(r.width, e.clientX - r.left));
    return { x, t: (x / r.width) * duration };
  };

  return (
    <div
      ref={ref}
      className="p-seek"
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(time)}
      onPointerDown={(e) => {
        if (!duration) return;
        dragging.current = true;
        e.currentTarget.setPointerCapture(e.pointerId);
        seekTo(posOf(e).t);
      }}
      onPointerMove={(e) => {
        if (!duration) return;
        const p = posOf(e);
        setHover(p);
        if (dragging.current) seekTo(p.t);
      }}
      onPointerUp={(e) => {
        dragging.current = false;
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onPointerLeave={() => setHover(null)}
    >
      <div className="p-seek-track">
        <div className="p-seek-fill" style={{ width: `${frac * 100}%` }} />
        {ticks.map((f, i) => <i key={i} className="p-seek-tick" style={{ left: `${f * 100}%` }} />)}
        {minedTicks.map((f, i) => <i key={i} className="p-seek-mined" style={{ left: `${f * 100}%` }} />)}
        <div className="p-seek-knob" style={{ left: `${frac * 100}%` }} />
      </div>
      {hover && <div className="p-seek-tip" style={{ left: hover.x }}>{fmtTime(hover.t)}</div>}
    </div>
  );
}
