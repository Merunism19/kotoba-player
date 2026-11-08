/** Full-bleed CSS "anime frame" standing in for the mpv video surface in screenshots (mock mode only; art from docs/design/mockup.html). */
import './mock-scene.css';

const STARS: [number, number][] = [[12, 10], [30, 6], [48, 14], [71, 8], [86, 16], [20, 22], [58, 4], [94, 7]];

export default function MockScene({ bright = false }: { bright?: boolean }) {
  return (
    <div className={`mock-scene${bright ? ' bright' : ''}`} aria-hidden="true">
      {STARS.map(([l, t]) => <i key={l} className="star" style={{ left: `${l}%`, top: `${t}%` }} />)}
      <div className="sun" />
      <div className="m1" />
      <div className="m2" />
      <div className="figure" />
      <div className="m3" />
      <div className="grain" />
    </div>
  );
}
