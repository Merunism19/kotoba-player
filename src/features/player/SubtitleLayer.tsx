/** Subtitle layer: every Japanese dialogue line showing now (overlapping speakers stack) as hoverable/clickable word units, plus the aligned English. */
import { Fragment, useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { Cue, WordUnit } from '../../lib/backend';
import { activeIndex, activeIndices, lastStartedIndex } from '../../lib/cues';
import { getUnits, peekUnits, toSegments } from '../../lib/words';
import { get, useApp, type AppState } from '../../app/store';
import { closePopup, currentDialogue, hoverLine, openPopup } from './session';

/** English text for a Japanese cue: aligned EN cues joined, else the EN dialogue cue showing at that time. */
function englishFor(s: AppState, cue: Cue): string {
  if (s.enCues.length === 0) return '';
  const ids = s.align[cue.id];
  if (ids && ids.length) {
    // enCues are sorted with id = index (the parser's contract), so ids index directly.
    return ids
      .map((id) => (s.enCues[id]?.id === id ? s.enCues[id] : s.enCues.find((c) => c.id === id))?.text ?? '')
      .filter(Boolean)
      .join(' ');
  }
  const t = s.time * 1000 - s.offsetMs;
  const en = s.enCues.filter((c) => c.kind === 'dialogue');
  const i = activeIndex(en, t);
  return i >= 0 ? en[i].text : '';
}

/** The dialogue cues showing now. Compared shallowly, so the layer re-renders when lines change, not on every time tick. */
function useCurrentCues(): Cue[] {
  return useApp(
    useShallow((s) => {
      const d = currentDialogue();
      return activeIndices(d, s.time * 1000 - s.offsetMs).map((i) => d[i]);
    }),
  );
}

/** Units for a line; re-renders when the tokenizer answers. */
function useUnits(text: string): WordUnit[] {
  const [, bump] = useState(0);
  useEffect(() => {
    let alive = true;
    void getUnits(text).then(() => alive && bump((n) => n + 1));
    return () => {
      alive = false;
    };
  }, [text]);
  return peekUnits(text) ?? [];
}

function Lines({ text }: { text: string }) {
  const parts = text.split('\n');
  return <>{parts.map((p, i) => <Fragment key={i}>{i > 0 && <br />}{p}</Fragment>)}</>;
}

/** One Japanese line: word units with hover/click, and a gold dot when it was mined. */
function CueLine({ cue }: { cue: Cue }) {
  const units = useUnits(cue.text);
  const popupUnit = useApp((s) => (s.popup?.cueId === cue.id ? s.popup.unit : undefined));
  const mined = useApp((s) => s.minedCueIds.includes(cue.id));
  const segs = toSegments(cue.text, units);
  return (
    <div className="p-jp" lang="ja">
      {segs.map((seg, i) =>
        seg.unit?.isWord ? (
          <span
            key={i}
            className={`unit word${popupUnit === seg.unit ? ' active' : ''}`}
            // Keyboard: Tab to a word, Enter looks it up (Space stays play/pause).
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key !== 'Enter') return;
              e.preventDefault();
              e.currentTarget.click();
            }}
            onClick={(e) => {
              e.stopPropagation();
              const r = e.currentTarget.getBoundingClientRect();
              if (popupUnit === seg.unit) closePopup();
              else openPopup({ unit: seg.unit!, line: cue.text, cueId: cue.id, anchor: { x: r.left + r.width / 2, top: r.top, bottom: r.bottom, width: r.width } });
            }}
          >
            {seg.text}
          </span>
        ) : (
          <span key={i} className="unit"><Lines text={seg.text} /></span>
        ),
      )}
      {mined && <span className="p-mined-dot" role="img" aria-label="Already mined" title="Already added to Anki" />}
    </div>
  );
}

export default function SubtitleLayer() {
  const cues = useCurrentCues();
  const en = useApp(useShallow((s) => cues.map((c) => englishFor(s, c)).filter(Boolean)));
  const enMode = useApp((s) => s.enMode);
  const subScale = useApp((s) => s.subScale);
  const last = cues[cues.length - 1];

  // Prefetch the tokenization of the next two lines.
  useEffect(() => {
    const d = currentDialogue();
    const s = get();
    const t = s.time * 1000 - s.offsetMs;
    const from = last ? d.indexOf(last) + 1 : lastStartedIndex(d, t) + 1;
    for (const c of d.slice(Math.max(0, from), from + 2)) void getUnits(c.text);
  }, [last]);

  const style = { ['--sub-scale' as string]: subScale };
  if (cues.length === 0) return <div className="p-subs" style={style} />;

  return (
    <div className="p-subs" style={style}>
      <div className="p-jp-lines" onMouseEnter={() => hoverLine(true)} onMouseLeave={() => hoverLine(false)}>
        {cues.map((c) => (
          <CueLine key={c.id} cue={c} />
        ))}
      </div>
      {en.length > 0 && enMode !== 'hidden' && (
        <div className={`p-en ${enMode}`} title={enMode === 'blur' ? 'Hover to reveal' : undefined}>
          {en.map((line, i) => (
            <Fragment key={i}>{i > 0 && <br />}{line}</Fragment>
          ))}
        </div>
      )}
    </div>
  );
}
