/** Seek bar: played fill, a tick per dialogue cue, a taller gold tick per mined line, knob, drag-to-seek, keyboard, and a hover time tooltip. */
import { memo, useRef, useState } from 'react';
import { fmtTime } from '../../lib/format';
import { useApp } from '../../app/store';
import { currentDialogue, seekBy, seekTo } from './session';

/**
 * The cue and mined ticks (hundreds of elements). Memoised on what changes them, so the bar's
 * 24-48 time updates per second only move the fill and knob (review 2026-11-08: the whole tick list
 * was re-rendered on every time-pos).
 */
const Ticks = memo(function Ticks({ duration, offsetMs, mined }: { duration: number; offsetMs: number; mined: number[] }) {
  const jpCues = useApp((s) => s.jpCues);
  if (duration <= 0) return null;
  const pos = (startMs: number) => `${((startMs + offsetMs) / 1000 / duration) * 100}%`;
  const minedSet = new Set(mined);
  return (
    <>
      {currentDialogue().map((c) => <i key={c.id} className="p-seek-tick" style={{ left: pos(c.startMs) }} />)}
      {jpCues.filter((c) => minedSet.has(c.id)).map((c) => <i key={`m${c.id}`} className="p-seek-mined" style={{ left: pos(c.startMs) }} />)}
    </>
  );
});

export default function SeekBar() {
  const time = useApp((s) => s.time);
  const duration = useApp((s) => s.duration);
  const offsetMs = useApp((s) => s.offsetMs);
  const mined = useApp((s) => s.minedCueIds);
  const ref = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const [hover, setHover] = useState<{ x: number; t: number } | null>(null);

  const frac = duration > 0 ? Math.min(1, time / duration) : 0;

  const posOf = (e: React.PointerEvent) => {
    const r = ref.current!.getBoundingClientRect();
    const x = Math.max(0, Math.min(r.width, e.clientX - r.left));
    return { x, t: (x / r.width) * duration };
  };
  const endDrag = () => {
    dragging.current = false;
  };

  return (
    <div
      ref={ref}
      className="p-seek"
      role="slider"
      tabIndex={0}
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(time)}
      aria-valuetext={`${fmtTime(time)} of ${fmtTime(duration)}`}
      onKeyDown={(e) => {
        // The global hotkeys already seek with ←/→; Home/End/Page keys only make sense here.
        if (e.key === 'Home') seekTo(0);
        else if (e.key === 'End' && duration) seekTo(duration - 1);
        else if (e.key === 'PageUp') seekBy(30);
        else if (e.key === 'PageDown') seekBy(-30);
        else return;
        e.preventDefault();
      }}
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
        endDrag();
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      // A cancelled pointer or lost capture must not leave the bar "dragging" (later hovers would seek).
      onPointerCancel={endDrag}
      onLostPointerCapture={endDrag}
      onPointerLeave={() => setHover(null)}
    >
      <div className="p-seek-track">
        <div className="p-seek-fill" style={{ width: `${frac * 100}%` }} />
        <Ticks duration={duration} offsetMs={offsetMs} mined={mined} />
        <div className="p-seek-knob" style={{ left: `${frac * 100}%` }} />
      </div>
      {hover && <div className="p-seek-tip" style={{ left: hover.x }}>{fmtTime(hover.t)}</div>}
    </div>
  );
}
