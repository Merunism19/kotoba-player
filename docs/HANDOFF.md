# Handoff — Kotoba Player

## What it is
A Windows desktop video player for anime (MKV and everything mpv plays) with clickable Japanese
subtitles: click a word → dictionary popup (English meaning, kana reading, pitch accent, audio)
→ one click makes an Anki card (word, sentence, screenshot, audio; translations on the back,
hover furigana). Full spec: `docs/DESIGN.md`. Visual target: `docs/design/mockup.html`.
Repo: `Merunism19/kotoba-player` (private GitHub).

## Owner and their rules
- Owner works on Windows 11; wants **everything pretty** — design quality is a requirement, not polish.
- Standing rule (2026-11-08): **read `AGENT-WORKFLOW.md` in full before designing or building**,
  and follow it (orchestrator + helpers, briefs, review, ship script, self-improvement).
- Clicking a subtitle must pick the **whole word** (several kanji together), never one character.
- Home screen fills the whole window (no small card on black).
- Translation: English sub track first, then DeepL Free, then local Ollama. Word audio: VOICEVOX
  if running, else Windows Japanese voice. (DESIGN §2, §12.)
- Hard rules: `docs/DESIGN.md` section 8 (privacy, never write next to videos, Anki safety,
  licences, no content-sourcing features, secrets in the OS keyring).

## Build / test / ship
- First time: `pnpm install`, `node scripts/fetch-deps.mjs` (pinned libmpv, wrapper, ffmpeg →
  `src-tauri/lib/`, SHA-256 checked). Rust is at `%USERPROFILE%\.cargo\bin` (add to PATH in new shells).
- Run: `pnpm tauri dev`. Local CI: `node scripts/ci.mjs` (ends `CI OK`). Ship: `node scripts/ship.mjs`
  (ends `SHIP OK`). Details: `docs/WORKFLOW.md`.
- See the running app without the owner: `pnpm dev` + `cargo build -p kotoba-player`, then
  `bash scripts/dev/probe.sh <video> <tag>` → `artifacts/probe/` (window + fullscreen PNGs, mpv frame, state).
- Test videos: `node scripts/make-samples.mjs <full ffmpeg with x264/x265>` → `samples/` (gitignored).
  On this PC that ffmpeg is the winget Gyan build under `%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*`.
- The design mockup is served by `.claude/launch.json` ("design-mockup", 127.0.0.1:5178 → `/mockup.html`).

## Machine facts (checked 2026-11-08)
- git 2.55, Node 26.6, pnpm 11.21, Python 3.12, Rust 1.99.1 (rustup, MSVC; VS Build Tools 2022), gh (logged in as Merunism19), Anki.
- The `ffmpeg` on PATH is a **2013 build** (Python Scripts folder) — never use PATH ffmpeg; the app bundles its own.
- AnkiConnect add-on (2055492159) **not** installed yet — owner installs it before Phase 3.
- **Hosted GitHub Actions can't run** (2026-11-08): jobs are refused for the account's billing
  (failed payment / spending limit). Owner chose to gate on local CI: ship with
  `node scripts/ship.mjs --local-gate` until they say billing is fixed.

## Gotchas
- `tauri-plugin-libmpv`: transparent window **and** transparent page; DLLs in `src-tauri/lib/`
  (copied next to the exe by the build).
- **Never read an mpv property with format `'node'`** through the plugin: it segfaults the app.
  Read node properties (e.g. `track-list`) as `'string'` — mpv returns JSON.
- **Pass integer properties as strings** (`setProperty('secondary-sid', '2')`): a JS number arrives as a double and mpv refuses it.
- mpv's `sub-text` joins every line on screen (sign + dialogue) — we parse tracks ourselves (Phase 1).
- CSS `backdrop-filter` can't blur the video (separate surface): tinted glass over video (DESIGN §5).
- React dev mode runs effects twice: initialise mpv once per page (module-level promise).
- WebView2 browser processes outlive the app for a few seconds and get reused by the next launch
  (new debug port ignored) — the probe waits for them; do the same in any launch automation.
- ffmpeg/ffprobe are static builds of ~130 MB each — revisit (shared build) before the installer (Phase 5).
- No Japanese speech voice is installed on this PC (only en-US); VOICEVOX and Ollama aren't running.
- `jptext` build downloads UniDic (~140 MB) from lindera.dev on a cold build (pinned MD5).
- Dictionary data is CC BY-SA 4.0 (JMdict/EDRDG, Kanjium): attribution required.

## Current state
v0.2.0 — Phase 1 done (player MVP): full-window home with "Continue watching" (thumbnails), player
with auto-hiding controls, seek-bar cue ticks, track picker, offset hotkeys (Z/X, Shift+Z), line
navigation (A/S/D), resume + per-file memory (`library.json` via `stores.rs`), error toasts.
Subtitles are parsed in Rust (`crates/subtitles`); dialogue is drawn by the WebView as whole-word
units (`crates/jptext`, Lindera + UniDic); signs/songs are drawn by mpv from a signs-only ASS.
Checked in the real app with `scripts/dev/probe.sh` (Hi10P sample: words, EN alignment, signs track).

## Next steps
1. Phase 2 (DESIGN §9): dictionary build tool (JMdict + Kanjium → SQLite), `crates/jpdict`
   (deinflection, lookup at position, pitch), popup UI, word audio (VOICEVOX → Windows voice).
2. Known gaps: audio-track choice isn't remembered (contract has no field); white subtitle text could
   use a stronger outline over very bright frames; UniDic embeds ~194 MB into the binary (revisit in Phase 5).

## Status log (keep ≤ 8 lines; older lines → docs/HISTORY.md)
- 2026-11-08: Read AGENT-WORKFLOW.md; wrote DESIGN.md, mockup, project docs (HANDOFF/WORKFLOW/BRIEF/test plan).
- 2026-11-08: Phase 0: repo + private GitHub, Rust installed, scaffold, helper built CI/ship/guards, spike passed.
- 2026-11-08: Phase 1: contract-first, 3 helpers in parallel worktrees (subtitles, tokenizer, UI) + Rust core; real-app probe passed.
