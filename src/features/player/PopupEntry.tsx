/** One dictionary entry inside the popup: headword with furigana, reading, audio button, badges, pitch, inflection chips, senses, other forms. */
import { useState } from 'react';
import type { DictEntry, LookupResult, Sense } from '../../lib/backend';
import PitchBlock, { patternColor } from './PitchGraph';
import { IconSpeaker } from './icons';
import type { AudioControl } from './useWordAudio';

const SHOWN_SENSES = 3;

/** "Ichidan verb" -> "ichidan"; "Godan verb with 'u' ending" -> "godan"; others lowercased as is. */
function shortPos(pos: string): string {
  const l = pos.toLowerCase();
  const m = /^(ichidan|godan|suru)\b/.exec(l);
  return m ? m[1] : l;
}

function audioTitle(a: AudioControl): string {
  if (a.status === 'none') return 'Install a Japanese voice (Windows Settings → Speech) or run VOICEVOX';
  if (a.status === 'loading') return 'Loading pronunciation…';
  const src = a.audio?.credit ?? (a.audio?.source === 'windows' ? 'Windows voice' : a.audio?.source === 'pack' ? 'Audio pack' : '');
  return `Listen (R)${src ? ` · ${src}` : ''}`;
}

function Senses({ senses }: { senses: Sense[] }) {
  const [all, setAll] = useState(false);
  const shown = all ? senses : senses.slice(0, SHOWN_SENSES);
  const more = senses.length - shown.length;
  return (
    <>
      <ol className="p-senses">
        {shown.map((s, i) => {
          const pos = s.pos.join(' · ');
          const prev = i > 0 ? senses[i - 1].pos.join(' · ') : null;
          return (
            <li key={i}>
              <div>
                <span lang="en">{s.glosses.join('; ')}</span>
                {s.notes.length > 0 && <span className="p-note-i"> ({s.notes.join('; ')})</span>}
                {pos && pos !== prev && <span className="p-pos">{pos}</span>}
              </div>
            </li>
          );
        })}
      </ol>
      {more > 0 && (
        <button className="p-more" onClick={() => setAll(true)}>+{more} more</button>
      )}
    </>
  );
}

export default function PopupEntry({ entry, result, audio }: { entry: DictEntry; result: LookupResult; audio: AudioControl }) {
  const first = entry.pitch[0];
  const color = first ? patternColor(first.pattern) : 'var(--text-2)';
  const pos = entry.senses[0]?.pos ?? [];
  const posBadge = pos.slice(0, 2).map(shortPos).join(' · ');
  const inflected = !!result.deinflected && result.inflections.length > 0;

  return (
    <>
      <div className="p-head">
        <div className="p-head-text" style={{ ['--pc' as string]: color }}>
          <div className="p-headword" lang="ja">
            {entry.furigana.length > 0
              ? entry.furigana.map((f, i) => (f.ruby ? <ruby key={i}>{f.text}<rt>{f.ruby}</rt></ruby> : <span key={i}>{f.text}</span>))
              : entry.term}
          </div>
          <div className="p-reading" lang="ja">{entry.reading}</div>
        </div>
        <span className="p-audio-wrap" title={audioTitle(audio)}>
          <button
            className={`p-audio${audio.playing ? ' playing' : ''}`}
            aria-label="Play pronunciation"
            disabled={audio.status !== 'ready'}
            onClick={audio.play}
          >
            {audio.status === 'loading' ? <span className="p-spin" /> : <IconSpeaker />}
          </button>
        </span>
      </div>

      <div className="p-badges">
        {entry.common && <span className="p-badge common">common</span>}
        {entry.freqRank != null && <span className="p-badge freq">{'★'} freq #{entry.freqRank.toLocaleString('en-US')}</span>}
        {posBadge && <span className="p-badge">{posBadge}</span>}
      </div>

      <PitchBlock pitch={entry.pitch} />

      {inflected && (
        <div className="p-inflect">
          <span className="p-from" lang="ja">{result.matched}</span>
          <span aria-hidden="true">{'→'}</span>
          <span className="p-from" lang="ja">{result.deinflected}</span>
          {result.inflections.map((t, i) => <span className="p-tag" key={i}>{t}</span>)}
        </div>
      )}

      <Senses senses={entry.senses} />

      {entry.otherForms.length > 0 && (
        <div className="p-forms">Other forms <span lang="ja">{entry.otherForms.join('、')}</span></div>
      )}
    </>
  );
}
