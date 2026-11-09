/** Home screen: full-window opaque page with logo, drop zone, "Open video" and the Continue watching row. */
import type { RecentItem } from '../../lib/backend';
import { fmtLeft } from '../../lib/format';
import { useApp } from '../../app/store';
import { openDialog, openFile } from '../player/session';
import { IconClock, IconGear, IconPlay, IconUpload } from '../player/icons';
import { openSettings } from '../settings/settings';
import './home.css';

function RecentCard({ item, index }: { item: RecentItem; index: number }) {
  const pct = item.durationMs ? Math.min(100, (item.positionMs / item.durationMs) * 100) : 0;
  return (
    <button className="card" onClick={() => void openFile(item.path)} title={item.path}>
      <div className={`thumb tone-${index % 3}`} style={item.thumbUrl ? { backgroundImage: `url("${item.thumbUrl}")` } : undefined}>
        {!item.thumbUrl && <span className="glyph" aria-hidden="true">{[...item.title][0]}</span>}
        <span className="resume"><IconPlay /></span>
        <span className="bar"><i style={{ width: `${pct}%` }} /></span>
      </div>
      <div className="meta">
        <span className="name">{item.title}</span>
        <span className="left"><IconClock />{fmtLeft(item.positionMs, item.durationMs)}</span>
      </div>
    </button>
  );
}

export default function Home() {
  const recents = useApp((s) => s.recents);
  const loaded = useApp((s) => s.recentsLoaded);
  const dragging = useApp((s) => s.dragging);
  const has = recents.length > 0;

  return (
    <main className={`home${has ? ' has-recents' : ''}${dragging ? ' dragging' : ''}`}>
      <button className="p-iconbtn home-gear" aria-label="Settings (Ctrl+,)" title="Settings (Ctrl+,)" onClick={openSettings}><IconGear /></button>
      <div className="home-inner">
        <header className="hero">
          <div className="logo" aria-hidden="true">言葉</div>
          <h1>Kotoba Player</h1>
          <p className="tag">Watch anime. Click any word. Keep it in Anki.</p>
        </header>

        <section className="drop" aria-label="Open a video">
          <span className="drop-icon"><IconUpload /></span>
          <div className="drop-text">
            <strong>{dragging ? 'Release to start watching' : 'Drop an episode anywhere in this window'}</strong>
            <span>MKV, MP4, WebM and more. Subtitles next to the file are found automatically.</span>
          </div>
          <button className="btn primary" onClick={() => void openDialog()}>Open video</button>
        </section>

        {loaded && has && (
          <section className="recents" aria-label="Continue watching">
            <h2>Continue watching</h2>
            <div className="row">
              {recents.slice(0, 6).map((r, i) => <RecentCard key={r.path} item={r} index={i} />)}
            </div>
          </section>
        )}
        {loaded && !has && <p className="empty">Episodes you watch will show up here so you can pick up where you left off.</p>}
      </div>
    </main>
  );
}
