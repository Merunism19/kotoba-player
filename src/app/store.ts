/** Global UI state (zustand): current screen, prepared media, cues, playback props, popup, toasts. */
import { create } from 'zustand';
import type { CardDraft, Cue, MineRequest, PreparedMedia, RecentItem, WordUnit } from '../lib/backend';

export type EnMode = 'blur' | 'always' | 'hidden';
export const EN_MODES: EnMode[] = ['blur', 'always', 'hidden'];

export interface Toast {
  id: number;
  message: string;
  kind: 'error' | 'info' | 'ok';
  /** Optional button on the toast (e.g. "Show" after a card was added). */
  action?: { label: string; run: () => void };
}

/** The open mining preview: the request, its first draft, and whether to resume playback on close. */
export interface MiningState {
  req: MineRequest;
  draft: CardDraft;
  resume: boolean;
}

/** The dictionary popup, anchored to a clicked word. */
export interface PopupState {
  unit: WordUnit;
  /** The subtitle line the word was clicked in (lookup input together with `unit.start`). */
  line: string;
  /** Id of the Japanese cue the line belongs to (for mining). */
  cueId: number;
  /** Viewport position of the clicked word: center x, top y, bottom y, width. */
  anchor: { x: number; top: number; bottom: number; width: number };
  /** Resume playback when the popup closes. */
  resume: boolean;
}

export interface AppState {
  screen: 'home' | 'player';
  opening: boolean;
  dragging: boolean;
  recents: RecentItem[];
  recentsLoaded: boolean;

  media: PreparedMedia | null;
  jpCues: Cue[];
  enCues: Cue[];
  align: Record<number, number[]>;
  /** Audio track chosen in the picker (null = off); undefined = the file's default. */
  audioId: number | null | undefined;

  /** Seconds, straight from mpv. */
  time: number;
  duration: number;
  pause: boolean;
  volume: number;
  speed: number;
  fullscreen: boolean;

  offsetMs: number;
  /** Bumps every time the offset changes, to replay the badge flash. */
  offsetFlash: number;
  enMode: EnMode;
  pauseOnHover: boolean;
  /** Playback was paused by hovering the subtitle line (so leaving resumes it). */
  hoverPaused: boolean;
  popup: PopupState | null;
  /** Mining preview modal, when open. */
  mining: MiningState | null;
  /** A card is being drafted/added (spinner on the popup's Add to Anki button). */
  drafting: boolean;
  /** Japanese cue ids mined from this file (gold marks on the seek bar and subtitle line). */
  minedCueIds: number[];

  /** Bumps on any activity that should reveal the controls. */
  activity: number;
  toasts: Toast[];
}

export const useApp = create<AppState>(() => ({
  screen: 'home',
  opening: false,
  dragging: false,
  recents: [],
  recentsLoaded: false,

  media: null,
  jpCues: [],
  enCues: [],
  align: {},
  audioId: undefined,

  time: 0,
  duration: 0,
  pause: true,
  volume: 100,
  speed: 1,
  fullscreen: false,

  offsetMs: 0,
  offsetFlash: 0,
  enMode: 'blur',
  pauseOnHover: true,
  hoverPaused: false,
  popup: null,
  mining: null,
  drafting: false,
  minedCueIds: [],

  activity: 0,
  toasts: [],
}));

export const set = useApp.setState;
export const get = useApp.getState;
