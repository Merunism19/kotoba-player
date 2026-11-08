/**
 * Mining logic: the cached Anki connection state, building a MineRequest from the open popup, drafting a card
 * (to the preview modal, or straight to Anki with Shift), marking cues as mined and the "Added to Anki" toast.
 */
import { useEffect, useState } from 'react';
import { backend, type AnkiStatus, type DictEntry, type LookupResult, type MineRequest } from '../../lib/backend';
import { errMsg } from '../../lib/format';
import { player } from '../../lib/player';
import { get, set } from '../../app/store';
import { closePopup, toast } from './session';

const STATUS_TTL_MS = 10_000;
let cached: { at: number; status: AnkiStatus } | null = null;
let inflight: Promise<AnkiStatus> | null = null;

/** Anki connection state, cached for 10 s (never rejects). */
export function getAnkiStatus(force = false): Promise<AnkiStatus> {
  if (!force && cached && Date.now() - cached.at < STATUS_TTL_MS) return Promise.resolve(cached.status);
  inflight ??= backend
    .ankiStatus()
    .catch((e): AnkiStatus => ({ connected: false, decks: [], error: errMsg(e) }))
    .then((status) => {
      cached = { at: Date.now(), status };
      inflight = null;
      return status;
    });
  return inflight;
}

/** The Anki state for a component (null while the first check runs). */
export function useAnkiStatus(): AnkiStatus | null {
  const [status, setStatus] = useState<AnkiStatus | null>(cached && Date.now() - cached.at < STATUS_TTL_MS ? cached.status : null);
  useEffect(() => {
    let alive = true;
    void getAnkiStatus().then((s) => alive && setStatus(s));
    return () => {
      alive = false;
    };
  }, []);
  return status;
}

/** The request for the word shown in the popup, or null when no file/subtitle track is open. */
export function buildRequest(result: LookupResult, entry: DictEntry, senses: number[] = []): MineRequest | null {
  const { media, popup, time, offsetMs, audioId } = get();
  if (!media || !popup || media.jpSubId == null) return null;
  return {
    path: media.path,
    jpSubId: media.jpSubId,
    enSubId: media.enSubId,
    cueId: popup.cueId,
    start: result.start,
    len: result.len,
    entry,
    senses,
    frameMs: Math.round(time * 1000),
    offsetMs,
    audioId: audioId ?? undefined,
  };
}

/** Remember that a Japanese cue was mined (gold tick on the seek bar, dot on the line). */
export function markMined(cueId: number): void {
  set((s) => (s.minedCueIds.includes(cueId) ? s : { minedCueIds: [...s.minedCueIds, cueId] }));
}

/** The success toast with a Show-in-Anki action. */
export function toastAdded(noteId: number): void {
  toast('Added to Anki', 'ok', {
    label: 'Show',
    run: () => void backend.showInAnki([noteId]).catch((e) => toast(errMsg(e))),
  });
}

/**
 * Draft the card for the popup's word. `instant` (Shift) adds it with the defaults; otherwise the mining preview opens.
 * Errors become toasts and the button returns to normal.
 */
export async function startMining(result: LookupResult, entry: DictEntry, instant: boolean): Promise<void> {
  if (get().drafting) return;
  const req = buildRequest(result, entry);
  if (!req) {
    toast('Open a video with Japanese subtitles to add cards.');
    return;
  }
  const resume = get().popup?.resume ?? false;
  set({ drafting: true });
  try {
    const draft = await backend.draftCard(req);
    if (instant) {
      const id = await backend.addCard(draft.draftId, {});
      markMined(req.cueId);
      closePopup();
      toastAdded(id);
    } else {
      set({ popup: null, mining: { req, draft, resume } });
      void player.setPause(true);
    }
  } catch (e) {
    toast(errMsg(e));
  } finally {
    set({ drafting: false });
  }
}

/** Close the mining preview; playback resumes if it was playing before the popup opened. */
export function closeMining(): void {
  const m = get().mining;
  if (!m) return;
  set({ mining: null });
  if (m.resume) void player.setPause(false);
}
