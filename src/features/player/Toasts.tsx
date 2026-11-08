/** Toast stack (bottom-right glass cards, auto-dismissed by session.toast). */
import { useApp } from '../../app/store';
import { dismissToast } from './session';
import { IconAlert, IconClose } from './icons';

export default function Toasts() {
  const toasts = useApp((s) => s.toasts);
  const screen = useApp((s) => s.screen);
  return (
    <div className={`toasts${screen === 'player' ? ' above-controls' : ''}`} role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          {t.kind === 'error' && <span className="toast-icon"><IconAlert /></span>}
          <span className="toast-msg">{t.message}</span>
          <button className="p-iconbtn toast-x" aria-label="Dismiss" onClick={() => dismissToast(t.id)}><IconClose /></button>
        </div>
      ))}
    </div>
  );
}
