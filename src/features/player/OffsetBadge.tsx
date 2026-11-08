/** Subtitle offset badge ("+0.30 s"), shown when the offset is not zero and flashing on every change. */
import { fmtOffset } from '../../lib/format';
import { useApp } from '../../app/store';
import { IconClock } from './icons';

export default function OffsetBadge() {
  const offsetMs = useApp((s) => s.offsetMs);
  const flash = useApp((s) => s.offsetFlash);
  if (offsetMs === 0) return null;
  return (
    <span key={flash} className={`p-chip sync${flash > 0 ? ' flash' : ''}`} title="Subtitle offset (Z / X, Shift+Z to sync this line)">
      <IconClock />
      {fmtOffset(offsetMs)}
    </span>
  );
}
