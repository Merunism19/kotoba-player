/** Binary searches over a start-sorted cue list: active cue at a time, previous and next cue. */
import type { Cue } from './backend';

/** Index of the first cue whose (shifted) start is after `tMs`. */
function upperBound(cues: Cue[], tMs: number): number {
  let lo = 0;
  let hi = cues.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (cues[mid].startMs <= tMs) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The cue showing at `tMs` (start <= t < end), or -1. With overlaps, the latest-starting one wins. */
export function activeIndex(cues: Cue[], tMs: number): number {
  const ub = upperBound(cues, tMs);
  for (let i = ub - 1; i >= 0 && i >= ub - 4; i--) {
    if (tMs < cues[i].endMs) return i;
  }
  return -1;
}

/** Index of the last cue that started at or before `tMs`, or -1. */
export function lastStartedIndex(cues: Cue[], tMs: number): number {
  return upperBound(cues, tMs) - 1;
}

/** Index of the first cue starting after `tMs`, or -1. */
export function nextIndex(cues: Cue[], tMs: number): number {
  const ub = upperBound(cues, tMs);
  return ub < cues.length ? ub : -1;
}
