/** Settings side panel (gear button, Ctrl+,): Anki, translation, subtitles, audio, appearance and about. Blurred glass on the home screen, tinted glass over the video. */
import { useEffect, useRef, useState } from 'react';
import { backend } from '../../lib/backend';
import type { AnkiStatus, Settings, UserDictionary } from '../../lib/backend';
import { pickDictionaryZip } from '../../lib/platform';
import { errMsg } from '../../lib/format';
import { useApp } from '../../app/store';
import { IconCheck, IconClose } from '../player/icons';
import { version } from '../../../package.json';
import { closeSettings, saveDeeplKey, updateSettings } from './settings';
import './settings.css';

const ACCENTS: { id: Settings['accent']; label: string; a: string; b: string }[] = [
  { id: 'sakura', label: 'Sakura', a: '#ff7aa2', b: '#8a9bff' },
  { id: 'twilight', label: 'Twilight', a: '#8a9bff', b: '#ff7aa2' },
  { id: 'matcha', label: 'Matcha', a: '#8fd694', b: '#5ba8ff' },
  { id: 'sunset', label: 'Sunset', a: '#ffa94d', b: '#ff6b6b' },
];
const EN_MODE_OPTIONS: { id: Settings['enMode']; label: string }[] = [
  { id: 'blur', label: 'Blur until hover' },
  { id: 'always', label: 'Always' },
  { id: 'hidden', label: 'Hidden' },
];
const OFFSET_STEPS = [50, 100, 250, 500];

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="s-sec">
      <h3>{title}</h3>
      <div className="s-card">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children?: React.ReactNode }) {
  return (
    <div className="s-row">
      <div className="s-row-text">
        <span className="s-label">{label}</span>
        {hint && <span className="s-hint">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

function Switch({ on, onChange, label, disabled }: { on: boolean; onChange(v: boolean): void; label: string; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} disabled={disabled} className={`s-switch${on ? ' on' : ''}`} onClick={() => onChange(!on)}>
      <i />
    </button>
  );
}

function Segmented<T extends string | number>({ value, options, onChange, label }: { value: T; options: { id: T; label: string }[]; onChange(v: T): void; label: string }) {
  return (
    <div className="s-seg" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.id} role="radio" aria-checked={o.id === value} className={o.id === value ? 'on' : ''} onClick={() => onChange(o.id)}>{o.label}</button>
      ))}
    </div>
  );
}

function AnkiSection({ s }: { s: Settings }) {
  const [status, setStatus] = useState<AnkiStatus | null>(null);
  const [tag, setTag] = useState('');
  useEffect(() => {
    let alive = true;
    void backend.ankiStatus().then((a) => alive && setStatus(a)).catch(() => alive && setStatus({ connected: false, decks: [], error: 'Could not check Anki' }));
    return () => {
      alive = false;
    };
  }, []);
  const addTags = (raw: string) => {
    const add = raw.split(/[\s,]+/).filter(Boolean).filter((t) => !s.ankiTags.includes(t));
    if (add.length) updateSettings({ ankiTags: [...s.ankiTags, ...add] });
    setTag('');
  };
  return (
    <Section title="Anki">
      <div className={`s-status ${status ? (status.connected ? 'ok' : 'err') : ''}`}>
        <i />{status ? (status.connected ? `Connected to Anki · ${status.decks.length} deck${status.decks.length === 1 ? '' : 's'}` : status.error ?? "Anki isn't reachable") : 'Checking Anki…'}
      </div>
      <label className="s-field">
        <span className="s-label">Deck</span>
        <input className="s-input" list="s-decks" value={s.ankiDeck} spellCheck={false} onChange={(e) => updateSettings({ ankiDeck: e.target.value })} />
        <datalist id="s-decks">{(status?.decks ?? []).map((d) => <option key={d} value={d} />)}</datalist>
      </label>
      <div className="s-field">
        <span className="s-label">Extra tags <span className="s-hint inline">added to every card; no spaces</span></span>
        <div className="s-chips" onClick={(e) => (e.currentTarget.querySelector('input') as HTMLInputElement | null)?.focus()}>
          {s.ankiTags.map((t) => (
            <span className="s-chip" key={t}>{t}
              <button aria-label={`Remove tag ${t}`} onClick={() => updateSettings({ ankiTags: s.ankiTags.filter((x) => x !== t) })}><IconClose /></button>
            </span>
          ))}
          <input
            aria-label="Add a tag" placeholder={s.ankiTags.length ? '' : 'e.g. anime'} value={tag} spellCheck={false}
            onChange={(e) => (/[\s,]$/.test(e.target.value) ? addTags(e.target.value) : setTag(e.target.value))}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && tag.trim()) addTags(tag);
              else if (e.key === 'Backspace' && !tag && s.ankiTags.length) updateSettings({ ankiTags: s.ankiTags.slice(0, -1) });
            }}
            onBlur={() => tag.trim() && addTags(tag)}
          />
        </div>
      </div>
    </Section>
  );
}

function TranslationSection({ s, hasKey, models }: { s: Settings; hasKey: boolean; models: string[] }) {
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const run = async (k: string | null) => {
    setBusy(true);
    try {
      const usage = await saveDeeplKey(k);
      setKey('');
      setMsg({ ok: true, text: k === null ? 'DeepL key removed.' : usage ?? 'DeepL key saved.' });
    } catch (e) {
      setMsg({ ok: false, text: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Section title="Translation">
      <p className="s-note">The card's sentence translation comes from the first source that has one: <b>English subtitles → DeepL → Ollama</b>. DeepL and Ollama only see lines you mine, and only if enabled here.</p>
      <div className="s-field">
        <span className="s-label">DeepL API key {hasKey && <em className="s-pill ok">key saved</em>}</span>
        <div className="s-inline">
          <input
            className="s-input" type="password" autoComplete="off" spellCheck={false} aria-label="DeepL API key"
            placeholder={hasKey ? 'Paste a new key to replace it' : 'Paste your DeepL API key'} value={key}
            onChange={(e) => setKey(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && key.trim() && void run(key.trim())}
          />
          <button className="s-btn primary" disabled={!key.trim() || busy} onClick={() => void run(key.trim())}>{busy ? 'Checking…' : 'Save'}</button>
          {hasKey && <button className="s-btn" disabled={busy} onClick={() => void run(null)}>Remove</button>}
        </div>
        {msg && <span className={`s-msg ${msg.ok ? 'ok' : 'err'}`} role="status">{msg.text}</span>}
      </div>
      <Row label="Use DeepL" hint={hasKey ? undefined : 'Save a key first'}>
        <Switch label="Use DeepL" on={s.useDeepl && hasKey} disabled={!hasKey} onChange={(v) => updateSettings({ useDeepl: v })} />
      </Row>
      <Row label="Use Ollama" hint="Local model, nothing leaves this PC">
        <Switch label="Use Ollama" on={s.useOllama} onChange={(v) => updateSettings({ useOllama: v })} />
      </Row>
      <label className="s-field">
        <span className="s-label">Ollama model</span>
        <select className="s-input" disabled={models.length === 0} value={s.ollamaModel ?? ''} onChange={(e) => updateSettings({ ollamaModel: e.target.value || null })}>
          {models.length === 0 ? <option value="">Ollama isn't running</option> : (
            <>
              <option value="">Automatic (first installed)</option>
              {models.map((m) => <option key={m} value={m}>{m}</option>)}
            </>
          )}
        </select>
      </label>
    </Section>
  );
}

function SubtitlesSection({ s }: { s: Settings }) {
  const steps = OFFSET_STEPS.includes(s.offsetStepMs) ? OFFSET_STEPS : [...OFFSET_STEPS, s.offsetStepMs].sort((a, b) => a - b);
  return (
    <Section title="Subtitles">
      <div className="s-field">
        <span className="s-label">English line</span>
        <Segmented label="English line" value={s.enMode} options={EN_MODE_OPTIONS} onChange={(v) => updateSettings({ enMode: v })} />
      </div>
      <div className="s-field">
        <span className="s-label">Size <b className="s-val">×{s.subScale.toFixed(2)}</b></span>
        <input
          type="range" className="s-range" min={0.7} max={1.6} step={0.05} value={s.subScale} aria-label="Subtitle size"
          style={{ ['--v' as string]: `${((s.subScale - 0.7) / 0.9) * 100}%` }}
          onChange={(e) => updateSettings({ subScale: Number(e.target.value) })}
        />
        <div className="s-preview" lang="ja" style={{ fontSize: `${18 * s.subScale}px` }}>もう諦めないって決めたんだ。</div>
      </div>
      <Row label="Pause while hovering a line" hint="So you can read and click words">
        <Switch label="Pause on hover" on={s.pauseOnHover} onChange={(v) => updateSettings({ pauseOnHover: v })} />
      </Row>
      <div className="s-field">
        <span className="s-label">Offset step <span className="s-hint inline">Z / X</span></span>
        <Segmented label="Offset step" value={s.offsetStepMs} options={steps.map((n) => ({ id: n, label: `${n} ms` }))} onChange={(v) => updateSettings({ offsetStepMs: v })} />
      </div>
    </Section>
  );
}

function AudioSection({ voicevox, voice }: { voicevox: boolean; voice: string | null }) {
  const [ok, line, tip] = voicevox
    ? [true, 'VOICEVOX is running', 'Word audio is spoken by VOICEVOX. Keep the engine open while you watch.']
    : voice
      ? [true, `Windows voice: ${voice}`, 'Word audio is spoken by this installed Japanese voice.']
      : [false, 'No word-audio source', 'Run VOICEVOX, or add a Japanese voice in Windows Settings → Time & language → Speech → Add voices.'];
  return (
    <Section title="Audio">
      <div className={`s-status ${ok ? 'ok' : 'err'}`}><i />{line}</div>
      <p className="s-note">{tip}</p>
    </Section>
  );
}

const KIND_LABEL: Record<UserDictionary['kind'], string> = {
  Terms: 'definitions',
  Frequency: 'frequency list',
  Both: 'definitions + frequency',
  Pitch: 'pitch accents',
};

/** Yomitan dictionaries imported by the user: list, import, remove. */
function DictionariesSection() {
  const [dicts, setDicts] = useState<UserDictionary[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const refresh = () => backend.userDictionaries().then(setDicts).catch(() => setDicts([]));
  useEffect(() => {
    void refresh();
  }, []);
  const importOne = async () => {
    const path = await pickDictionaryZip();
    if (!path) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await backend.importDictionary(path);
      const parts = [
        r.terms && `${r.terms.toLocaleString()} entries`,
        r.frequencies && `${r.frequencies.toLocaleString()} frequencies`,
        r.pitches && `${r.pitches.toLocaleString()} pitch accents`,
      ].filter(Boolean);
      setMsg({ ok: true, text: `Imported “${r.title}”: ${parts.join(', ') || 'nothing usable'}.` });
      await refresh();
    } catch (e) {
      setMsg({ ok: false, text: errMsg(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Section title="Dictionaries">
      <p className="s-note">
        JMdict (English) is built in. Add Yomitan-format dictionaries (.zip) for more definitions, pitch accents, or a
        frequency list — that one shows “★ freq #…” in the popup.
      </p>
      {dicts && dicts.length > 0 && (
        <ul className="s-dicts">
          {dicts.map((d) => (
            <li key={d.title}>
              <div>
                <b>{d.title}</b>
                <span className="s-hint">{KIND_LABEL[d.kind]} · {d.rows.toLocaleString()} rows</span>
              </div>
              <button className="p-btn" aria-label={`Remove ${d.title}`} onClick={() => void backend.removeDictionary(d.title).then(refresh).catch((e) => setMsg({ ok: false, text: errMsg(e) }))}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      <button className="p-btn" disabled={busy} onClick={() => void importOne()}>
        {busy ? <span className="p-spin small" /> : null}
        {busy ? 'Importing…' : 'Import dictionary…'}
      </button>
      {msg && <div className={`s-status ${msg.ok ? 'ok' : 'err'}`}><i />{msg.text}</div>}
    </Section>
  );
}

function AppearanceSection({ accent }: { accent: Settings['accent'] }) {
  return (
    <Section title="Appearance">
      <div className="s-field">
        <span className="s-label">Accent colour</span>
        <div className="s-swatches" role="radiogroup" aria-label="Accent colour">
          {ACCENTS.map((a) => (
            <button key={a.id} role="radio" aria-checked={a.id === accent} aria-label={a.label} className={`s-swatch${a.id === accent ? ' on' : ''}`} onClick={() => updateSettings({ accent: a.id })}>
              <i style={{ background: `linear-gradient(135deg, ${a.a}, ${a.b})` }}>{a.id === accent && <IconCheck />}</i>
              <span>{a.label}</span>
            </button>
          ))}
        </div>
      </div>
    </Section>
  );
}

function AboutSection({ attribution }: { attribution: string | null }) {
  return (
    <Section title="About">
      <div className="s-about">
        <span className="s-logo" aria-hidden="true">言葉</span>
        <div>
          <b>Kotoba Player</b> <span className="s-hint inline">v{version}</span>
          <br />
          <span className="s-hint">Free software under the GPL-3.0 licence. Nothing you watch or look up leaves this PC (except DeepL, if you turn it on).</span>
        </div>
      </div>
      {attribution && <p className="s-note">{attribution}</p>}
      <p className="s-note">Dictionary: JMdict (edrdg.org/jmdict) · Pitch accents: Kanjium (github.com/mifunetoshiro/kanjium) · Voice: VOICEVOX (voicevox.hiroshiba.jp)</p>
    </Section>
  );
}

export default function SettingsPanel() {
  const open = useApp((s) => s.settingsOpen);
  const view = useApp((s) => s.settingsView);
  const screen = useApp((s) => s.screen);
  const panel = useRef<HTMLElement>(null);

  useEffect(() => {
    if (open) panel.current?.focus();
  }, [open]);
  // On the player Esc is routed by the hotkeys (it also closes the popup); on home we listen here.
  useEffect(() => {
    if (!open || screen !== 'home') return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && closeSettings();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, screen]);

  if (!open) return null;
  const s = view?.settings;
  return (
    <>
      <div className="s-scrim" onClick={closeSettings} />
      <aside ref={panel} tabIndex={-1} className={`s-panel ${screen === 'player' ? 'on-video' : 'on-page'}`} role="dialog" aria-label="Settings" data-keep-ui data-no-hotkeys
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.stopPropagation();
            closeSettings();
          }
        }}
      >
        <header>
          <h2>Settings</h2>
          <kbd>Ctrl ,</kbd>
          <span className="s-spacer" />
          <button className="p-iconbtn" aria-label="Close settings" onClick={closeSettings}><IconClose /></button>
        </header>
        <div className="s-body">
          {!view || !s ? <p className="s-note">Settings aren't available right now.</p> : (
            <>
              <AnkiSection s={s} />
              <TranslationSection s={s} hasKey={view.hasDeeplKey} models={view.ollamaModels} />
              <SubtitlesSection s={s} />
              <AudioSection voicevox={view.voicevox} voice={view.japaneseVoice} />
              <DictionariesSection />
              <AppearanceSection accent={s.accent} />
              <AboutSection attribution={view.attribution} />
            </>
          )}
        </div>
      </aside>
    </>
  );
}
