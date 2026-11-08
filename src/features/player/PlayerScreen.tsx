/** Player screen: transparent stage over mpv's video with auto-hiding top/bottom bars, subtitle layer, popup and hotkeys. */
import { useEffect, useRef, useState } from 'react';
import { installHotkeys } from '../../lib/hotkeys';
import { useApp } from '../../app/store';
import Controls from './Controls';
import OffsetBadge from './OffsetBadge';
import SubtitleLayer from './SubtitleLayer';
import TrackPicker from './TrackPicker';
import WordPopup from './WordPopup';
import { IconBack, IconFullscreen } from './icons';
import {
  closePopup, cycleEnMode, exitFullscreen, goHome, nudgeOffset, replayLine, seekBy, seekLine, setVolume, syncLineNow,
  toggleFullscreen, togglePause, wake,
} from './session';
import { get } from '../../app/store';
import './player.css';

const HIDE_MS = 2000;

/** Controls stay up while a menu or the popup is open, or the pointer rests on a bar. */
function shouldKeepUi(root: HTMLElement | null): boolean {
  return !!(get().popup || document.querySelector('[data-keep-ui]') || root?.querySelector('.p-top:hover, .p-bottom:hover'));
}

export default function PlayerScreen() {
  const title = useApp((s) => s.media?.title ?? '');
  const activity = useApp((s) => s.activity);
  const loading = useApp((s) => s.opening);
  const [visible, setVisible] = useState(true);
  const root = useRef<HTMLDivElement>(null);
  const timer = useRef<number | undefined>(undefined);

  const arm = () => {
    setVisible(true);
    window.clearTimeout(timer.current);
    const tick = () => {
      if (shouldKeepUi(root.current)) timer.current = window.setTimeout(tick, HIDE_MS);
      else setVisible(false);
    };
    timer.current = window.setTimeout(tick, HIDE_MS);
  };

  useEffect(() => {
    arm();
    return () => window.clearTimeout(timer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activity]);

  useEffect(
    () =>
      installHotkeys({
        togglePause,
        seekBy,
        prevLine: () => seekLine(-1),
        nextLine: () => seekLine(1),
        replayLine,
        offset: nudgeOffset,
        syncLineNow,
        fullscreen: () => void toggleFullscreen(),
        cycleEn: cycleEnMode,
        transcript: () => {}, // reserved for Phase 4
        escape: () => {
          if (get().popup) closePopup();
          else void exitFullscreen();
        },
        volume: (d) => setVolume(get().volume + d),
        activity: wake,
      }),
    [],
  );

  return (
    <div
      ref={root}
      className={`p-stage${visible ? '' : ' idle'}`}
      onMouseMove={arm}
      onClick={(e) => {
        if (e.target !== e.currentTarget && !(e.target as HTMLElement).classList.contains('p-subs')) return;
        if (get().popup) closePopup();
        else togglePause();
      }}
      onDoubleClick={(e) => e.target === e.currentTarget && void toggleFullscreen()}
    >
      <div className="p-top p-panel">
        <button className="p-iconbtn" aria-label="Back to home" title="Back to home" onClick={() => void goHome()}><IconBack /></button>
        <div className="p-title">{title}</div>
        <span className="p-spacer" />
        <OffsetBadge />
        <TrackPicker />
        <button className="p-iconbtn" aria-label="Fullscreen (F)" title="Fullscreen (F)" onClick={() => void toggleFullscreen()}><IconFullscreen /></button>
      </div>

      {loading && <div className="p-loading">Opening…</div>}
      <SubtitleLayer />
      <WordPopup />
      <Controls />
    </div>
  );
}
