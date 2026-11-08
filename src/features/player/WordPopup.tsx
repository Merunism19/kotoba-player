/**
 * Dictionary popup: looks up the clicked word (`backend.lookupAt(line, unit.start)`), shows result tabs / entry pager,
 * handles loading, nothing-found and errors, keyboard (R listen, arrows switch, Esc close) and placement
 * (above the word with an arrow, flips below when there is no room, clamped inside the window).
 */
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { backend, type LookupResult } from '../../lib/backend';
import { errMsg } from '../../lib/format';
import { useApp, type PopupState } from '../../app/store';
import { closePopup, toast } from './session';
import PopupEntry from './PopupEntry';
import { IconChevron, IconPlus } from './icons';
import { useWordAudio } from './useWordAudio';
import './popup.css';

const MARGIN = 10;
const GAP = 14;
const MAX_W = 420;
const SKELETON_DELAY_MS = 150;

type Lookup =
  | { phase: 'wait' | 'loading' | 'empty' }
  | { phase: 'ready'; results: LookupResult[] };

/** Runs the lookup; stays invisible for the first 150 ms so fast answers never flash a skeleton. */
function useLookup(line: string, offset: number): Lookup {
  const [state, setState] = useState<Lookup>({ phase: 'wait' });
  useEffect(() => {
    let alive = true;
    const t = window.setTimeout(() => alive && setState((s) => (s.phase === 'wait' ? { phase: 'loading' } : s)), SKELETON_DELAY_MS);
    backend
      .lookupAt(line, offset)
      .then((all) => {
        if (!alive) return;
        const results = all.filter((r) => r.entries.length > 0);
        setState(results.length > 0 ? { phase: 'ready', results } : { phase: 'empty' });
      })
      .catch((e) => {
        if (!alive) return;
        toast(`Dictionary lookup failed: ${errMsg(e)}`);
        closePopup();
      });
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [line, offset]);
  return state;
}

function useViewport() {
  const [v, setV] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const on = () => setV({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', on);
    return () => window.removeEventListener('resize', on);
  }, []);
  return v;
}

/** The positioned glass card: measures its content and picks above/below, clamps horizontally, caps the height. */
function Frame({ anchor, label, children, footer }: { anchor: PopupState['anchor']; label: string; children: ReactNode; footer: ReactNode }) {
  const { w: vw, h: vh } = useViewport();
  const bodyRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const footRef = useRef<HTMLDivElement>(null);
  const [natural, setNatural] = useState(0);

  useLayoutEffect(() => {
    const measure = () => {
      const b = bodyRef.current;
      const f = footRef.current;
      if (b && f) setNatural(Math.ceil(b.scrollHeight + f.offsetHeight + 2));
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (contentRef.current) ro.observe(contentRef.current);
    if (footRef.current) ro.observe(footRef.current);
    return () => ro.disconnect();
  }, []);

  const width = Math.min(MAX_W, vw - MARGIN * 2);
  const left = Math.max(MARGIN, Math.min(anchor.x - width / 2, vw - width - MARGIN));
  const arrowX = Math.max(26, Math.min(anchor.x - left, width - 26));
  const above = anchor.top - GAP - MARGIN;
  const below = vh - anchor.bottom - GAP - MARGIN;
  const side: 'above' | 'below' = natural === 0 || natural <= above ? 'above' : natural <= below ? 'below' : above >= below ? 'above' : 'below';
  const maxH = Math.max(120, side === 'above' ? above : below);
  const pos = side === 'above' ? { bottom: vh - anchor.top + GAP } : { top: anchor.bottom + GAP };

  return (
    <div
      className={`p-popup ${side}`}
      role="dialog"
      aria-label={label}
      style={{ left, width, maxHeight: maxH, ...pos, ['--arrow-x' as string]: `${arrowX}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="p-popup-body" ref={bodyRef}>
        <div ref={contentRef}>{children}</div>
      </div>
      <div className="p-popup-foot" ref={footRef}>{footer}</div>
    </div>
  );
}

function Skeleton() {
  return (
    <div className="p-skel" aria-busy="true" aria-label="Looking up">
      <div className="b" style={{ width: '46%', height: 40 }} />
      <div className="b" style={{ width: '30%', height: 14, marginTop: 10 }} />
      <div className="row"><div className="b" style={{ width: 62, height: 20 }} /><div className="b" style={{ width: 96, height: 20 }} /></div>
      <div className="b" style={{ width: '100%', height: 64, marginTop: 14, borderRadius: 14 }} />
      <div className="b" style={{ width: '90%', height: 14, marginTop: 16 }} />
      <div className="b" style={{ width: '70%', height: 14, marginTop: 10 }} />
    </div>
  );
}

function Footer({ term, empty }: { term?: string; empty?: boolean }) {
  const copy = () => {
    if (!term) return;
    navigator.clipboard?.writeText(term).then(
      () => toast(`Copied ${term}`, 'info'),
      () => toast('Could not copy to the clipboard'),
    );
  };
  return (
    <>
      {!empty && (
        <>
          <span className="p-tip" title="Anki arrives in Phase 3">
            <button className="p-btn primary" disabled><IconPlus />Add to Anki</button>
          </span>
          <button className="p-btn" onClick={copy}>Copy</button>
        </>
      )}
      <span className="p-kbd">
        {!empty && <><kbd>Enter</kbd> add {'·'} <kbd>R</kbd> listen {'·'} </>}<kbd>Esc</kbd>
      </span>
    </>
  );
}

function PopupInner({ popup }: { popup: PopupState }) {
  const { unit, line, anchor } = popup;
  const lookup = useLookup(line, unit.start);
  const [ri, setRi] = useState(0);
  const [ei, setEi] = useState(0);

  const results = lookup.phase === 'ready' ? lookup.results : [];
  const result = results[ri];
  const entry = result?.entries[ei];
  const audio = useWordAudio(entry);

  const sel = useRef({ results, ri, ei, audio });
  sel.current = { results, ri, ei, audio };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const s = sel.current;
      const swallow = () => {
        e.preventDefault();
        e.stopImmediatePropagation();
      };
      switch (e.code) {
        case 'KeyR':
          swallow();
          s.audio.play();
          break;
        case 'ArrowLeft':
        case 'ArrowRight': {
          swallow();
          const n = s.results.length;
          if (n > 1) {
            setRi((s.ri + (e.code === 'ArrowRight' ? 1 : n - 1)) % n);
            setEi(0);
          }
          break;
        }
        case 'ArrowUp':
        case 'ArrowDown': {
          swallow();
          const n = s.results[s.ri]?.entries.length ?? 0;
          if (n > 1) setEi((s.ei + (e.code === 'ArrowDown' ? 1 : n - 1)) % n);
          break;
        }
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  if (lookup.phase === 'wait') return null;

  const nEntries = result?.entries.length ?? 0;
  const nav = results.length > 1 || nEntries > 1;

  return (
    <Frame
      anchor={anchor}
      label={`Dictionary: ${unit.text}`}
      footer={<Footer term={entry?.term} empty={lookup.phase !== 'ready'} />}
    >
      {lookup.phase === 'loading' && <Skeleton />}
      {lookup.phase === 'empty' && (
        <div className="p-empty">
          <div className="p-empty-title">No dictionary entry for</div>
          <div className="p-empty-word" lang="ja">{unit.text}</div>
          {(unit.reading && unit.reading !== unit.text) || (unit.base && unit.base !== unit.text) ? (
            <div className="p-empty-meta">
              {unit.reading && unit.reading !== unit.text && <span>Reading <b lang="ja">{unit.reading}</b></span>}
              {unit.base && unit.base !== unit.text && <span>Dictionary form <b lang="ja">{unit.base}</b></span>}
            </div>
          ) : null}
        </div>
      )}
      {lookup.phase === 'ready' && result && entry && (
        <>
          {nav && (
            <div className="p-nav">
              <div className="p-tabs" role="tablist">
                {results.length > 1 &&
                  results.map((r, i) => (
                    <button key={i} role="tab" aria-selected={i === ri} className={`p-tab${i === ri ? ' on' : ''}`} lang="ja" onClick={() => { setRi(i); setEi(0); }}>
                      {r.matched}
                    </button>
                  ))}
              </div>
              {nEntries > 1 && (
                <div className="p-pager" aria-label="Entries">
                  <button aria-label="Previous entry" onClick={() => setEi((ei + nEntries - 1) % nEntries)}><IconChevron dir="up" /></button>
                  <span>{ei + 1} / {nEntries}</span>
                  <button aria-label="Next entry" onClick={() => setEi((ei + 1) % nEntries)}><IconChevron dir="down" /></button>
                </div>
              )}
            </div>
          )}
          <PopupEntry key={`${ri}:${entry.id}:${entry.term}:${entry.reading}`} entry={entry} result={result} audio={audio} />
        </>
      )}
    </Frame>
  );
}

export default function WordPopup() {
  const popup = useApp((s) => s.popup);
  if (!popup) return null;
  return <PopupInner key={`${popup.line}:${popup.unit.start}`} popup={popup} />;
}
