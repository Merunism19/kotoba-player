/** Hook: fetches (and caches) the pronunciation of the shown entry and plays it on demand; exposes loading/none/ready state for the audio button. */
import { useCallback, useEffect, useRef, useState } from 'react';
import { backend, type DictEntry, type WordAudio } from '../../lib/backend';

export interface AudioControl {
  status: 'loading' | 'ready' | 'none';
  audio: WordAudio | null;
  playing: boolean;
  play(): void;
}

const cache = new Map<string, WordAudio | null>();

export function useWordAudio(entry: DictEntry | undefined): AudioControl {
  const key = entry ? `${entry.term}|${entry.reading}` : '';
  const [res, setRes] = useState<{ key: string; audio: WordAudio | null } | null>(null);
  const [playing, setPlaying] = useState(false);
  const el = useRef<HTMLAudioElement | null>(null);

  const stop = useCallback(() => {
    el.current?.pause();
    el.current = null;
    setPlaying(false);
  }, []);

  useEffect(() => {
    stop();
    if (!entry) return;
    if (cache.has(key)) {
      setRes({ key, audio: cache.get(key) ?? null });
      return;
    }
    let alive = true;
    backend
      .wordAudio(entry.term, entry.reading)
      .catch(() => null)
      .then((a) => {
        cache.set(key, a);
        if (alive) setRes({ key, audio: a });
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  useEffect(() => stop, [stop]);

  const current = res && res.key === key ? res : null;
  const audio = current?.audio ?? null;
  const play = useCallback(() => {
    if (!audio) return;
    el.current?.pause();
    const a = new Audio(audio.url);
    el.current = a;
    a.onended = () => setPlaying(false);
    setPlaying(true);
    a.play().catch(() => setPlaying(false));
  }, [audio]);

  return { status: !current ? 'loading' : audio ? 'ready' : 'none', audio, playing, play };
}
