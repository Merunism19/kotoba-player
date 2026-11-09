/** Toast stack (bottom-right glass cards, auto-dismissed by session.toast; error / info / success with an optional action button). */
import { useApp } from '../../app/store';
import { dismissToast } from './session';
import { IconAlert, IconCheck, IconClose } from './icons';

export default function Toasts() {
  const toasts = useApp((s) => s.toasts);
  const screen = useApp((s) => s.screen);
  return (
    <div className={`toasts${screen === 'player' ? ' above-controls' : ''}`} role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          {t.kind === 'error' && <span className="toast-icon"><IconAlert /></span>}
          {t.kind === 'ok' && <span className="toast-icon ok"><IconCheck /></span>}
          <span className="toast-msg">{t.message}</span>
          {t.action && (
            <button className="toast-action" onClick={() => { t.action!.run(); dismissToast(t.id); }}>{t.action.label}</button>
          )}
          <button className="p-iconbtn toast-x" aria-label="Dismiss" onClick={() => dismissToast(t.id)}><IconClose /></button>
        </div>
      ))}
    </div>
  );
}
