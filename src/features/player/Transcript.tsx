/** Transcript sidebar (T): every Japanese dialogue line with its time, the current one highlighted and scrolled into view, click to seek, search, gold dot on mined lines. */
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { activeIndex, lastStartedIndex } from '../../lib/cues';
import { fmtTime } from '../../lib/format';
import { useApp } from '../../app/store';
import { IconClose, IconSearch } from './icons';
import { currentDialogue, seekTo, toggleTranscript } from './session';
import './transcript.css';

const USER_SCROLL_GRACE_MS = 3000;

interface RowData {
  id: number;
  index: number;
  startMs: number;
  jp: string;
  en: string;
}

const Row = memo(function Row({ r, offsetMs, active, mined }: { r: RowData; offsetMs: number; active: boolean; mined: boolean }) {
  return (
    <button
      className={`t-row${active ? ' active' : ''}`}
      data-index={r.index}
      aria-current={active ? 'true' : undefined}
      onClick={() => seekTo((r.startMs + offsetMs) / 1000, true)}
    >
      <span className="t-time">{fmtTime((r.startMs + offsetMs) / 1000)}</span>
      <span className="t-text">
        <span className="t-jp" lang="ja">{r.jp}{mined && <i className="t-mined" role="img" aria-label="Already mined" title="Already added to Anki" />}</span>
        {r.en && <span className="t-en">{r.en}</span>}
      </span>
    </button>
  );
});

export default function Transcript() {
  const jpCues = useApp((s) => s.jpCues);
  const enCues = useApp((s) => s.enCues);
  const align = useApp((s) => s.align);
  const offsetMs = useApp((s) => s.offsetMs);
  const mined = useApp((s) => s.minedCueIds);
  const [query, setQuery] = useState('');
  const list = useRef<HTMLDivElement>(null);
  const lastUserScroll = useRef(0);

  const rows = useMemo<RowData[]>(() => {
    const byId = new Map(enCues.map((c) => [c.id, c]));
    return currentDialogue().map((c, index) => ({
      id: c.id,
      index,
      startMs: c.startMs,
      jp: c.text.replace(/\n/g, ' '),
      en: (align[c.id] ?? []).map((id) => byId.get(id)?.text ?? '').filter(Boolean).join(' ').replace(/\n/g, ' '),
    }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jpCues, enCues, align]);

  // The line showing now, or the last one that started (so the highlight does not vanish between lines).
  const current = useApp((s) => {
    const d = currentDialogue();
    const t = s.time * 1000 - s.offsetMs;
    const i = activeIndex(d, t);
    return i >= 0 ? i : lastStartedIndex(d, t);
  });

  const q = query.trim().toLowerCase();
  const shown = q ? rows.filter((r) => r.jp.toLowerCase().includes(q) || r.en.toLowerCase().includes(q)) : rows;

  useEffect(() => {
    if (current < 0 || Date.now() - lastUserScroll.current < USER_SCROLL_GRACE_MS) return;
    const el = list.current?.querySelector<HTMLElement>(`[data-index="${current}"]`);
    el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [current, q]);

  const touch = () => (lastUserScroll.current = Date.now());

  return (
    <aside className="p-panel t-panel" aria-label="Transcript" data-keep-ui onClick={(e) => e.stopPropagation()}>
      <header>
        <h2>Transcript</h2>
        <span className="t-count">{q ? `${shown.length} of ${rows.length}` : `${rows.length} lines`}</span>
        <span className="p-spacer" />
        <button className="p-iconbtn" aria-label="Close transcript (T)" title="Close (T)" onClick={toggleTranscript}><IconClose /></button>
      </header>
      <label className="t-search">
        <IconSearch />
        <input
          type="search" placeholder="Search Japanese or English" value={query} spellCheck={false} aria-label="Search transcript"
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              if (query) setQuery('');
              else (e.target as HTMLInputElement).blur();
            }
          }}
        />
      </label>
      <div className="t-list" ref={list} onWheel={touch} onPointerDown={touch} onTouchMove={touch} onKeyDown={touch}>
        {shown.map((r) => <Row key={r.id} r={r} offsetMs={offsetMs} active={r.index === current} mined={mined.includes(r.id)} />)}
        {shown.length === 0 && <p className="t-empty">{rows.length === 0 ? 'No Japanese subtitles loaded for this video.' : `No line matches "${query.trim()}".`}</p>}
      </div>
    </aside>
  );
}
