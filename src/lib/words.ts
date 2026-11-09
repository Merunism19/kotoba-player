/** Cached, de-duplicated tokenization of subtitle lines into word units, plus helpers to split a line into render segments. */
import { backend, type WordUnit } from './backend';

const cache = new Map<string, WordUnit[]>();
const inflight = new Map<string, Promise<WordUnit[]>>();

/** Units for a line from the cache, if already tokenized. */
export function peekUnits(text: string): WordUnit[] | undefined {
  return cache.get(text);
}

/** Tokenize a line (cached by text). On failure resolves to no units, so the line renders as plain text. */
export function getUnits(text: string): Promise<WordUnit[]> {
  const hit = cache.get(text);
  if (hit) return Promise.resolve(hit);
  let p = inflight.get(text);
  if (!p) {
    p = backend
      .tokenize(text)
      .catch((e) => {
        console.warn('tokenize failed', e);
        return [] as WordUnit[];
      })
      .then((u) => {
        cache.set(text, u);
        inflight.delete(text);
        if (cache.size > 400) cache.delete(cache.keys().next().value as string);
        return u;
      });
    inflight.set(text, p);
  }
  return p;
}

/** One rendered piece of a line: a word unit or plain text between units. */
export interface Segment {
  text: string;
  unit?: WordUnit;
}

/** Cut `text` into segments following `units` (code-point offsets); uncovered text becomes plain segments. */
export function toSegments(text: string, units: WordUnit[]): Segment[] {
  const chars = [...text];
  const out: Segment[] = [];
  let pos = 0;
  for (const u of [...units].sort((a, b) => a.start - b.start)) {
    if (u.start < pos || u.start + u.len > chars.length) continue;
    if (u.start > pos) out.push({ text: chars.slice(pos, u.start).join('') });
    out.push({ text: chars.slice(u.start, u.start + u.len).join(''), unit: u });
    pos = u.start + u.len;
  }
  if (pos < chars.length) out.push({ text: chars.slice(pos).join('') });
  return out;
}
