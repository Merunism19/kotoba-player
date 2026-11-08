/** Track picker: chips in the top bar open a panel to choose the audio track and the Japanese / English subtitle tracks (any sub for either role). */
import { useEffect, useState } from 'react';
import type { Track } from '../../lib/backend';
import { langLabel } from '../../lib/format';
import { useApp } from '../../app/store';
import { chooseAudio, chooseSub } from './session';
import { IconCheck, IconHeadphones, IconSubs } from './icons';

const CODEC: Record<string, string> = { subrip: 'SRT', webvtt: 'VTT', ass: 'ASS', ssa: 'SSA' };

function label(t: Track): string {
  const parts = [t.title, langLabel(t.lang), t.external ? 'external' : '', CODEC[t.codec] ?? t.codec.toUpperCase()].filter((p): p is string => !!p);
  const seen = new Set<string>();
  return parts.filter((p) => !seen.has(p.toLowerCase()) && seen.add(p.toLowerCase())).join(' · ');
}

function Option({ on, children, onPick }: { on: boolean; children: React.ReactNode; onPick(): void }) {
  return (
    <button className={`p-opt${on ? ' on' : ''}`} role="menuitemradio" aria-checked={on} onClick={onPick}>
      <span className="p-opt-check">{on && <IconCheck />}</span>
      <span className="p-opt-label">{children}</span>
    </button>
  );
}

export default function TrackPicker() {
  const media = useApp((s) => s.media);
  const audioId = useApp((s) => s.audioId);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open]);

  if (!media) return null;
  const audio = media.tracks.filter((t) => t.kind === 'audio');
  const subs = media.tracks.filter((t) => t.kind === 'sub');
  const curAudio = audioId === undefined ? (audio.find((t) => t.isDefault) ?? audio[0])?.id : audioId;
  const audioTrack = audio.find((t) => t.id === curAudio);
  const jpOn = subs.some((t) => t.id === media.jpSubId);
  const enOn = subs.some((t) => t.id === media.enSubId);
  const subLabel = jpOn && enOn ? 'JP + EN' : jpOn ? 'JP' : enOn ? 'EN' : 'Off';

  return (
    <div className="p-picker-wrap">
      <button className="p-chip button" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <IconHeadphones />{audioTrack ? langLabel(audioTrack.lang) || audioTrack.title || 'Audio' : 'No audio'}
      </button>
      <button className="p-chip button" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <IconSubs />{subLabel}
      </button>
      {open && (
        <>
          <div className="p-scrim" onClick={() => setOpen(false)} />
          <div className="p-panel p-picker" role="menu" data-keep-ui>
            <h3>Audio</h3>
            {audio.map((t) => <Option key={t.id} on={curAudio === t.id} onPick={() => void chooseAudio(t.id)}>{label(t)}</Option>)}
            <Option on={curAudio === null} onPick={() => void chooseAudio(null)}>None</Option>

            <h3>Japanese subtitles</h3>
            {subs.map((t) => <Option key={t.id} on={media.jpSubId === t.id} onPick={() => void chooseSub('jp', t.id)}>{label(t)}</Option>)}
            <Option on={media.jpSubId == null} onPick={() => void chooseSub('jp', null)}>None</Option>

            <h3>English subtitles</h3>
            {subs.map((t) => <Option key={t.id} on={media.enSubId === t.id} onPick={() => void chooseSub('en', t.id)}>{label(t)}</Option>)}
            <Option on={media.enSubId == null} onPick={() => void chooseSub('en', null)}>None</Option>
          </div>
        </>
      )}
    </div>
  );
}
