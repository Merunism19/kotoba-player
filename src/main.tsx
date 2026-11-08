/** Entry point: loads bundled fonts and tokens, installs the mock backend/player in `?mock=1` mode, then renders the app. */
import React from 'react';
import ReactDOM from 'react-dom/client';
import '@fontsource-variable/inter';
import '@fontsource-variable/noto-sans-jp';
import '@fontsource/zen-maru-gothic/500.css';
import '@fontsource/zen-maru-gothic/700.css';
import './design/tokens.css';
import App from './app/App';
import { isMock } from './lib/env';
import * as session from './features/player/session';

async function boot() {
  if (isMock) {
    const [{ installMockBackend }, { installMockPlayer }] = await Promise.all([
      import('./lib/mock-backend'),
      import('./lib/mock-player'),
    ]);
    installMockBackend();
    // Test hooks for the screenshot harness: fake clock controls plus the session API.
    (window as unknown as { __kp: unknown }).__kp = { ...installMockPlayer(), ...session };
  }
  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void boot();
