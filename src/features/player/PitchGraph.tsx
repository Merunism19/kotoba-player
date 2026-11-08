/** Pitch accent block of the dictionary popup: SVG graph (one dot per mora + hollow dot for the next particle), pattern name and drop hint. */
import type { Pitch } from '../../lib/backend';

const NAMES: Record<Pitch['pattern'], { kanji: string; romaji: string }> = {
  heiban: { kanji: '平板', romaji: 'heiban' },
  atamadaka: { kanji: '頭高', romaji: 'atamadaka' },
  nakadaka: { kanji: '中高', romaji: 'nakadaka' },
  odaka: { kanji: '尾高', romaji: 'odaka' },
};

/** CSS colour (token) for a pattern. */
export const patternColor = (p: Pitch['pattern']) => `var(--pitch-${p})`;

/** Is mora `i` (1-based) high? `i = n + 1` is the particle that follows the word. */
function isHigh(i: number, downstep: number): boolean {
  if (downstep === 0) return i >= 2;
  if (downstep === 1) return i === 1;
  return i >= 2 && i <= downstep;
}

/** Short hint under the pattern name. */
function hint(p: Pitch): string {
  if (p.downstep === 0) return 'stays high, no drop';
  const m = p.morae[p.downstep - 1] ?? '';
  return p.pattern === 'odaka' ? `drops after ${m}, on the particle` : `drops after ${m}`;
}

function Graph({ pitch, compact }: { pitch: Pitch; compact: boolean }) {
  const n = pitch.morae.length;
  const step = Math.min(compact ? 26 : 30, (compact ? 150 : 200) / Math.max(n, 1));
  const pad = 14;
  const hi = compact ? 7 : 8;
  const lo = compact ? 20 : 26;
  const w = pad * 2 + step * n;
  const h = compact ? 40 : 54;
  const x = (i: number) => pad + step * (i - 1);
  const y = (i: number) => (isHigh(i, pitch.downstep) ? hi : lo);
  const color = patternColor(pitch.pattern);
  const pts = Array.from({ length: n }, (_, k) => `${x(k + 1)},${y(k + 1)}`).join(' ');
  const desc = pitch.morae.map((m, k) => `${m} ${isHigh(k + 1, pitch.downstep) ? 'high' : 'low'}`).join(', ');
  return (
    <svg className="p-pitch-svg" width={w} height={h} viewBox={`0 0 ${w} ${h}`} role="img" aria-label={`Pitch: ${desc}`}>
      <g fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points={pts} />
        <polyline points={`${x(n)},${y(n)} ${x(n + 1)},${y(n + 1)}`} strokeDasharray="3 4" opacity="0.6" />
      </g>
      <g fill={color}>
        {pitch.morae.map((_, k) => <circle key={k} cx={x(k + 1)} cy={y(k + 1)} r="4" />)}
      </g>
      <circle cx={x(n + 1)} cy={y(n + 1)} r="4" fill="none" stroke={color} strokeWidth="2" opacity="0.6" />
      <g fontFamily="var(--font-jp)" fontSize={compact ? 12 : 13} fill="var(--text-2)" textAnchor="middle">
        {pitch.morae.map((m, k) => <text key={k} x={x(k + 1)} y={h - 5}>{m}</text>)}
      </g>
    </svg>
  );
}

/** One or more accents; renders nothing when there is no pitch data. */
export default function PitchBlock({ pitch }: { pitch: Pitch[] }) {
  if (pitch.length === 0) return null;
  const compact = pitch.length > 1;
  return (
    <div className={`p-pitch${compact ? ' multi' : ''}`}>
      {pitch.map((p, i) => (
        <div className="p-pitch-row" key={i} style={{ ['--pc' as string]: patternColor(p.pattern) }}>
          <Graph pitch={p} compact={compact} />
          <div className="p-pitch-name">
            <b lang="ja">{NAMES[p.pattern].kanji} <span>{NAMES[p.pattern].romaji}</span> [{p.downstep}]</b>
            {hint(p)}
          </div>
        </div>
      ))}
    </div>
  );
}
