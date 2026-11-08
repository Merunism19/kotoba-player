/**
 * Mining preview modal: the card exactly as Anki will show it (Front and Back rendered by src/lib/anki-render.ts with the real
 * templates and CSS, each in an iframe so the app's CSS cannot leak in), a dark/light preview toggle, the editable sentence
 * translation, sense checkboxes (re-drafts the card), deck choice, duplicate notice and "Send to Anki".
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import { backend, type CardDraft } from '../../lib/backend';
import { cardDocument, escapeHtml, htmlToText, renderCard } from '../../lib/anki-render';
import { CARD_CSS, TEMPLATES } from '../../lib/anki-templates';
import { errMsg } from '../../lib/format';
import { useApp, type MiningState } from '../../app/store';
import { closeMining, markMined, toastAdded } from './mining';
import { toast } from './session';
import { IconAlert, IconClose, IconPlus } from './icons';
import './mining.css';

const REDRAFT_MS = 300;
/** Preview-only tweak: the iframe body is exactly as tall as the card, so the frame can size itself. */
const FRAME_CSS = 'body.card{min-height:0}';

let previewAudio: HTMLAudioElement | null = null;
function playPreview(url: string): void {
  previewAudio?.pause();
  previewAudio = new Audio(url);
  void previewAudio.play().catch(() => {});
}

/** One card side in an isolated iframe; content is patched in place (no reload, no flicker) unless the theme changes. */
function CardFrame({ html, night, label }: { html: string; night: boolean; label: string }) {
  const ref = useRef<HTMLIFrameElement>(null);
  const latest = useRef(html);
  latest.current = html;
  const srcDoc = useMemo(() => cardDocument(latest.current, CARD_CSS + FRAME_CSS, night), [night]);
  const [height, setHeight] = useState(360);
  const obs = useRef<ResizeObserver | null>(null);

  const attach = () => {
    const doc = ref.current?.contentDocument;
    if (!doc?.body) return;
    obs.current?.disconnect();
    const RO = (doc.defaultView as unknown as { ResizeObserver: typeof ResizeObserver }).ResizeObserver; // the frame's own, so loop warnings stay in the frame
    obs.current = new RO(() => requestAnimationFrame(() => setHeight(Math.ceil(doc.body.getBoundingClientRect().height))));
    obs.current.observe(doc.body);
    doc.addEventListener('click', (e) => {
      const a = (e.target as Element | null)?.closest?.('a.replay-button');
      if (!a) return;
      e.preventDefault();
      const src = a.getAttribute('data-src');
      if (src) playPreview(src);
    });
  };

  useEffect(() => {
    const root = ref.current?.contentDocument?.getElementById('kt-root');
    if (root && root.innerHTML !== html) root.innerHTML = html;
  }, [html]);
  useEffect(() => () => obs.current?.disconnect(), []);

  return <iframe ref={ref} className="m-frame" title={label} srcDoc={srcDoc} style={{ height }} onLoad={attach} />;
}

const SOURCE_LABEL: Record<CardDraft['translationSource'], string> = {
  subtitles: 'from English subtitles',
  deepl: 'machine translation (DeepL)',
  ollama: 'machine translation (Ollama, local)',
  none: 'none — type one',
};

function Modal({ m }: { m: MiningState }) {
  const { req } = m;
  const senses = req.entry.senses;
  const [draft, setDraft] = useState<CardDraft>(m.draft);
  const [translation, setTranslation] = useState(() => htmlToText(m.draft.fields.SentenceTranslation ?? ''));
  const [deck, setDeck] = useState(m.draft.deck);
  const [picked, setPicked] = useState<boolean[]>(() => {
    const keep = req.senses.length ? new Set(req.senses) : null;
    return senses.map((_s, i) => !keep || keep.has(i));
  });
  const initialPicked = useRef(picked);
  const [night, setNight] = useState(true);
  const [pending, setPending] = useState(false);
  const [sending, setSending] = useState(false);
  const seq = useRef(0);
  const sendRef = useRef<() => void>(() => {});

  // Sense checkboxes re-draft the card (debounced); the typed translation is kept.
  useEffect(() => {
    if (picked === initialPicked.current) return; // nothing changed yet (also keeps StrictMode's double effect from re-drafting)
    const mine = ++seq.current;
    setPending(true);
    const t = window.setTimeout(() => {
      const idx = picked.flatMap((on, i) => (on ? [i] : []));
      backend
        .draftCard({ ...req, senses: idx.length === picked.length ? [] : idx })
        .then((d) => {
          if (mine !== seq.current) return;
          setDraft(d);
          setPending(false);
        })
        .catch((e) => {
          if (mine !== seq.current) return;
          setPending(false);
          toast(errMsg(e));
        });
    }, REDRAFT_MS);
    return () => window.clearTimeout(t);
  }, [picked, req]);

  const cards = useMemo(
    () =>
      renderCard(
        TEMPLATES,
        { ...draft.fields, SentenceTranslation: escapeHtml(translation.trim()) },
        { audio: { SentenceAudio: draft.sentenceAudioUrl, WordAudio: draft.wordAudioUrl }, screenshotUrl: draft.screenshotUrl },
      ),
    [draft, translation],
  );

  const send = async () => {
    if (sending || pending) return;
    setSending(true);
    try {
      const id = await backend.addCard(draft.draftId, { sentenceTranslation: translation.trim(), deck });
      markMined(req.cueId);
      closeMining();
      toastAdded(id);
    } catch (e) {
      toast(errMsg(e));
      setSending(false);
    }
  };
  sendRef.current = () => void send();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === 'Escape') {
        e.preventDefault();
        closeMining();
      } else if (e.code === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        sendRef.current();
      }
      e.stopPropagation(); // the player's hotkeys must not react while the dialog is open
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  const toggle = (i: number) =>
    setPicked((p) => {
      const next = p.map((v, j) => (j === i ? !v : v));
      return next.some(Boolean) ? next : p;
    });
  const onlyOne = picked.filter(Boolean).length === 1;
  const dups = draft.duplicates;

  return (
    <div className="m-backdrop" onMouseDown={(e) => e.target === e.currentTarget && closeMining()}>
      <div className="m-modal" role="dialog" aria-modal="true" aria-label="Mining preview">
        <header className="m-head">
          <div className="m-title">
            <h2>Add to Anki</h2>
            <span className="m-word" lang="ja">{req.entry.term}</span>
          </div>
          <div className="m-seg" role="group" aria-label="Preview theme">
            <button aria-pressed={night} className={night ? 'on' : ''} onClick={() => setNight(true)}>Dark</button>
            <button aria-pressed={!night} className={!night ? 'on' : ''} onClick={() => setNight(false)}>Light</button>
          </div>
          <button className="p-iconbtn" aria-label="Close" title="Close (Esc)" onClick={closeMining}><IconClose /></button>
        </header>

        <div className="m-body">
          <div className="m-cards">
            <figure className="m-card">
              <figcaption>Front</figcaption>
              <CardFrame html={cards.front} night={night} label="Card front" />
            </figure>
            <figure className="m-card">
              <figcaption>Back</figcaption>
              <CardFrame html={cards.back} night={night} label="Card back" />
            </figure>
          </div>

          <aside className="m-side">
            <label className="m-field">
              <span className="m-label">Sentence translation <em className={`m-src ${draft.translationSource}`}>{SOURCE_LABEL[draft.translationSource]}</em></span>
              <textarea
                value={translation}
                rows={3}
                spellCheck={false}
                placeholder="Type a translation for this line"
                onChange={(e) => setTranslation(e.target.value)}
              />
              {(draft.translationSource === 'deepl' || draft.translationSource === 'ollama') && (
                <span className="m-src-note">Machine translations can be wrong. Check it before sending.</span>
              )}
            </label>

            <label className="m-field">
              <span className="m-label">Deck</span>
              <select value={deck} onChange={(e) => setDeck(e.target.value)}>
                {(draft.decks.includes(deck) ? draft.decks : [deck, ...draft.decks]).map((d) => <option key={d}>{d}</option>)}
              </select>
            </label>

            {senses.length > 0 && (
              <fieldset className="m-field m-senses">
                <legend className="m-label">Meanings on the card</legend>
                {senses.map((s, i) => (
                  <label key={i} className={`m-sense${picked[i] ? ' on' : ''}`}>
                    <input type="checkbox" checked={picked[i]} disabled={picked[i] && onlyOne} onChange={() => toggle(i)} />
                    <span className="m-box" aria-hidden="true" />
                    <span className="m-sense-text">
                      <span lang="en">{s.glosses.join('; ')}</span>
                      {s.pos.length > 0 && <small>{s.pos.join(' · ')}</small>}
                    </span>
                  </label>
                ))}
              </fieldset>
            )}
          </aside>
        </div>

        <footer className="m-foot">
          {dups.length > 0 && (
            <div className="m-dup" role="status">
              <IconAlert />
              <span>Already in Anki ({dups.length})</span>
              <button onClick={() => void backend.showInAnki(dups).catch((e) => toast(errMsg(e)))}>Show</button>
            </div>
          )}
          <span className="m-spacer" />
          <span className="m-hint"><kbd>Esc</kbd> cancel {'·'} <kbd>Ctrl</kbd>+<kbd>Enter</kbd> send</span>
          <button className="p-btn" onClick={closeMining}>Cancel</button>
          <button className={`p-btn primary${sending ? ' busy' : ''}`} autoFocus disabled={pending} aria-busy={sending} onClick={() => void send()}>
            {sending ? <span className="p-spin small" /> : <IconPlus />}Send to Anki
          </button>
        </footer>
      </div>
    </div>
  );
}

export default function MiningPreview() {
  const m = useApp((s) => s.mining);
  if (!m) return null;
  return <Modal key={m.draft.draftId} m={m} />;
}
