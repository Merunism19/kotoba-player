/** Player hotkeys (docs/DESIGN.md section 7): one keydown listener mapping keys to actions. */
export interface HotkeyActions {
  togglePause(): void;
  seekBy(sec: number): void;
  prevLine(): void;
  nextLine(): void;
  replayLine(): void;
  offset(deltaMs: number): void;
  syncLineNow(): void;
  fullscreen(): void;
  cycleEn(): void;
  transcript(): void;
  escape(): void;
  volume(delta: number): void;
  /** Any key press (to reveal the controls). */
  activity(): void;
}

/** Install the listener; returns the remover. Ignored while typing in a field. */
export function installHotkeys(a: HotkeyActions): () => void {
  const onKey = (e: KeyboardEvent) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === 'INPUT' && (t as HTMLInputElement).type !== 'range' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    let handled = true;
    switch (e.code) {
      case 'Space': a.togglePause(); break;
      case 'ArrowLeft': a.seekBy(-5); break;
      case 'ArrowRight': a.seekBy(5); break;
      case 'ArrowUp': a.volume(5); break;
      case 'ArrowDown': a.volume(-5); break;
      case 'KeyA': a.prevLine(); break;
      case 'KeyD': a.nextLine(); break;
      case 'KeyS': a.replayLine(); break;
      case 'KeyZ': if (e.shiftKey) a.syncLineNow(); else a.offset(-100); break;
      case 'KeyX': a.offset(100); break;
      case 'KeyF': a.fullscreen(); break;
      case 'KeyE': a.cycleEn(); break;
      case 'KeyT': a.transcript(); break; // reserved: transcript sidebar arrives in Phase 4
      case 'Escape': a.escape(); break;
      default: handled = false;
    }
    if (handled) {
      e.preventDefault();
      a.activity();
    }
  };
  // Space must not also "click" a focused button when released.
  const onUp = (e: KeyboardEvent) => {
    const t = e.target as HTMLElement | null;
    if (e.code === 'Space' && t?.tagName !== 'INPUT') e.preventDefault();
  };
  window.addEventListener('keydown', onKey);
  window.addEventListener('keyup', onUp);
  return () => {
    window.removeEventListener('keydown', onKey);
    window.removeEventListener('keyup', onUp);
  };
}
