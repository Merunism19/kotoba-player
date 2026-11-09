/** App shell: boots the session once, routes home <-> player, hosts toasts, and draws the mock video frame in screenshot mode. */
import { useEffect } from 'react';
import Home from '../features/home/Home';
import PlayerScreen from '../features/player/PlayerScreen';
import Toasts from '../features/player/Toasts';
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

  return (
    <>
      {isMock && screen === 'player' && <MockScene bright={mockParams.has('bright')} />}
      {screen === 'home' ? <Home /> : <PlayerScreen />}
      {dragging && screen === 'player' && <div className="drop-veil">Drop to play</div>}
      <Toasts />
    </>
  );
}
