/** Fake player for screenshots: a 30 s clock that advances while unpaused and answers the same API as mpv. */
import { setPlayer, type PlayerApi, type PlayerHandlers } from './player';

export const MOCK_DURATION = 30;

/** Test hooks on `window.__kp` (mock mode only). */
export interface MockHooks {
  setTime(sec: number): void;
  setPaused(p: boolean): void;
  getTime(): number;
  isPaused(): boolean;
}

export function installMockPlayer(): MockHooks {
  const handlers = new Set<PlayerHandlers>();
  let time = 0;
  let paused = true;
  const emit = (name: Parameters<PlayerHandlers['onProp']>[0]['name'], value: number | boolean) =>
    handlers.forEach((h) => h.onProp({ name, value }));

  setInterval(() => {
    if (paused) return;
    time = Math.min(MOCK_DURATION, time + 0.1);
    emit('time-pos', time);
    if (time >= MOCK_DURATION) {
      paused = true;
      emit('pause', true);
    }
  }, 100);

  const api: PlayerApi = {
    async start(h) {
      handlers.add(h);
      return () => handlers.delete(h);
    },
    async loadFile(_path, o) {
      time = o.startSec;
      emit('duration', MOCK_DURATION);
      emit('time-pos', time);
    },
    async stop() {
      paused = true;
      emit('pause', true);
    },
    async setPause(p) {
      paused = p;
      emit('pause', p);
    },
    async seek(sec, mode) {
      time = Math.max(0, Math.min(MOCK_DURATION, mode === 'absolute' ? sec : time + sec));
      emit('time-pos', time);
    },
    async setVolume(v) {
      emit('volume', v);
    },
    async setSpeed(s) {
      emit('speed', s);
    },
    async setAudio() {},
    async reloadSubtitles() {},
  };
  setPlayer(api);

  return {
    setTime(sec) {
      time = sec;
      emit('time-pos', time);
    },
    setPaused(p) {
      paused = p;
      emit('pause', p);
    },
    getTime: () => time,
    isPaused: () => paused,
  };
}
