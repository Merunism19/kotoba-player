/** Placeholder dictionary popup (Phase 1): the clicked unit with its reading as ruby, anchored above the word with an arrow. */
import { useApp } from '../../app/store';
import { closePopup } from './session';
import { IconClose } from './icons';

const MARGIN = 12;

export default function WordPopup() {
  const popup = useApp((s) => s.popup);
  if (!popup) return null;
  const { unit, anchor } = popup;

  const width = Math.min(360, window.innerWidth - MARGIN * 2);
  const left = Math.max(MARGIN, Math.min(anchor.x - width / 2, window.innerWidth - width - MARGIN));
  const bottom = window.innerHeight - anchor.top + 16;
  const hasReading = !!unit.reading && unit.reading !== unit.text;

  return (
    <div
      className="p-popup"
      role="dialog"
      aria-label={`Word: ${unit.text}`}
      style={{ left, width, bottom, ['--arrow-x' as string]: `${anchor.x - left}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="p-popup-body" style={{ maxHeight: anchor.top - 60 }}>
      <button className="p-iconbtn p-popup-close" aria-label="Close" onClick={closePopup}><IconClose /></button>
      <div className="p-headword" lang="ja">
        {hasReading ? <ruby>{unit.text}<rt>{unit.reading}</rt></ruby> : unit.text}
      </div>
      {unit.base && unit.base !== unit.text && (
        <div className="p-base">Dictionary form <b lang="ja">{unit.base}</b></div>
      )}
      <p className="p-note">Dictionary arrives in Phase 2</p>
      <div className="p-kbd"><kbd>Esc</kbd> close</div>
      </div>
    </div>
  );
}
