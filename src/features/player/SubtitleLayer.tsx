/** Subtitle layer: current Japanese dialogue line as hoverable/clickable word units, plus the aligned English line. */
import { Fragment, useEffect, useState } from 'react';
import type { Cue, WordUnit } from '../../lib/backend';
import { activeIndex, lastStartedIndex } from '../../lib/cues';
import { getUnits, peekUnits, toSegments } from '../../lib/words';
import { get, useApp, type AppState } from '../../app/store';
import { closePopup, currentDialogue, hoverLine, openPopup } from './session';

/** English text for a Japanese cue: aligned EN cues joined, else the EN dialogue cue showing at that time. */
function englishFor(s: AppState, cue: Cue | null): string {
  if (!cue || s.enCues.length === 0) return '';
  const ids = s.align[cue.id];
  if (ids && ids.length) {
    const byId = new Map(s.enCues.map((c) => [c.id, c]));
    return ids.map((id) => byId.get(id)?.text ?? '').filter(Boolean).join(' ');
  }
  const t = s.time * 1000 - s.offsetMs;
  const en = s.enCues.filter((c) => c.kind === 'dialogue');
  const i = activeIndex(en, t);
  return i >= 0 ? en[i].text : '';
}

function useCurrentCue(): Cue | null {
  return useApp((s) => {
    const d = currentDialogue();
    const i = activeIndex(d, s.time * 1000 - s.offsetMs);
    return i >= 0 ? d[i] : null;
  });
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

export default function SubtitleLayer() {
  const cue = useCurrentCue();
  const en = useApp((s) => englishFor(s, cue));
  const enMode = useApp((s) => s.enMode);
  const popupUnit = useApp((s) => s.popup?.unit);
  const text = cue?.text ?? '';
  const units = useUnits(text);

  // Prefetch the tokenization of the next two lines.
  useEffect(() => {
    const d = currentDialogue();
    const s = get();
    const t = s.time * 1000 - s.offsetMs;
    const from = cue ? d.indexOf(cue) + 1 : lastStartedIndex(d, t) + 1;
    for (const c of d.slice(Math.max(0, from), from + 2)) void getUnits(c.text);
  }, [cue]);

  if (!cue) return <div className="p-subs" />;

  const segs = toSegments(text, units);
  return (
    <div className="p-subs">
      <div className="p-jp" onMouseEnter={() => hoverLine(true)} onMouseLeave={() => hoverLine(false)} lang="ja">
        {segs.map((seg, i) =>
          seg.unit?.isWord ? (
            <span
              key={i}
              className={`unit word${popupUnit === seg.unit ? ' active' : ''}`}
              onClick={(e) => {
                e.stopPropagation();
                const r = e.currentTarget.getBoundingClientRect();
                if (popupUnit === seg.unit) closePopup();
                else openPopup({ unit: seg.unit!, anchor: { x: r.left + r.width / 2, top: r.top, width: r.width } });
              }}
            >
              {seg.text}
            </span>
          ) : (
            <span key={i} className="unit"><Lines text={seg.text} /></span>
          ),
        )}
      </div>
      {en && enMode !== 'hidden' && (
        <div className={`p-en ${enMode}`} title={enMode === 'blur' ? 'Hover to reveal' : undefined}>
          {en}
        </div>
      )}
    </div>
  );
}
