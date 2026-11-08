/** App shell: boots the session once, routes home <-> player, hosts toasts, and draws the mock video frame in screenshot mode. */
import { useEffect } from 'react';
import Home from '../features/home/Home';
import PlayerScreen from '../features/player/PlayerScreen';
import Toasts from '../features/player/Toasts';
import SettingsPanel from '../features/settings/SettingsPanel';
import { closeSettings, openSettings } from '../features/settings/settings';
import { startApp } from '../features/player/session';
import { isMock, mockParams } from '../lib/env';
import MockScene from '../lib/MockScene';
import { useApp } from './store';
import './app.css';

export default function App() {
  const screen = useApp((s) => s.screen);
  const dragging = useApp((s) => s.dragging);

  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    void startApp().then((s) => (cancelled ? s() : (stop = s)));
    return () => {
      cancelled = true;
      stop?.();
    };
  }, []);

  // Ctrl+, opens (or closes) Settings from anywhere, also while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key === ',') {
        e.preventDefault();
        if (useApp.getState().settingsOpen) closeSettings();
        else openSettings();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  return (
    <>
      {isMock && screen === 'player' && <MockScene bright={mockParams.has('bright')} />}
      {screen === 'home' ? <Home /> : <PlayerScreen />}
      {dragging && screen === 'player' && <div className="drop-veil">Drop to play</div>}
      <SettingsPanel />
      <Toasts />
    </>
  );
}
