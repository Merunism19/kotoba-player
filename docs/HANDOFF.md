# Handoff — Kotoba Player

## What it is
A Windows desktop video player for anime (MKV and everything mpv plays) with clickable Japanese
subtitles: click a word → dictionary popup (English meaning, kana reading, pitch accent, audio)
→ one click makes an Anki card (word, sentence, screenshot, audio; translations on the back,
hover furigana). Full spec: `docs/DESIGN.md`. Visual target: `docs/design/mockup.html`.

## Owner and their rules
- Owner works on Windows 11; wants **everything pretty** — design quality is a requirement, not polish.
- Standing rule (2026-11-08): **read `AGENT-WORKFLOW.md` in full before designing or building**,
  and follow it (orchestrator + helpers, briefs, review, ship script, self-improvement).
- Hard rules: `docs/DESIGN.md` section 8 (privacy, never write next to videos, Anki safety,
  licences, no content-sourcing features, secrets in the OS keyring).

## Build / test / ship
Not set up yet (Phase 0). The mockup is served by `.claude/launch.json` ("design-mockup",
python http.server on 127.0.0.1:5178 → `/mockup.html`). Planned: `pnpm tauri dev`, `node scripts/ci.mjs` (local CI, everything CI
runs), `node scripts/ship.mjs` (ends with `SHIP OK: …`). Details will live in `docs/WORKFLOW.md`.

## Machine facts (checked 2026-11-08)
- Installed: git 2.55, Node 26.6, pnpm 11.21, Python 3.12, Anki (profile "User 1").
- **Missing:** Rust/cargo, mpv/libmpv, ffmpeg — Phase 0 installs/bundles them.
- AnkiConnect add-on (2055492159) **not** in `%APPDATA%\Anki2\addons21` — owner installs it (Phase 3).
- The folder is **not a git repo** yet.

## Gotchas
- Browser `<video>` can't play Hi10P H.264 reliably → that's why the engine is libmpv.
- `tauri-plugin-libmpv` needs a transparent window *and* a transparent page background.
- Dictionary data is CC BY-SA 4.0 (JMdict/EDRDG, Kanjium): attribution required.

## Current state
Design draft written (DESIGN.md + mockup), waiting for owner review. No code yet.

## Next steps
1. Owner reviews the design and answers DESIGN.md §12 (none block Phase 0).
2. Phase 0: install Rust, git init, scaffold, scripts + guards, **libmpv overlay spike**.
3. Phase 1 briefs H1 (subtitles crate) and H2 (player UI) in parallel worktrees.

## Status log (keep ≤ 8 lines; older lines → docs/HISTORY.md)
- 2026-11-08: Read AGENT-WORKFLOW.md; wrote DESIGN.md, mockup, project docs (HANDOFF/WORKFLOW/BRIEF/test plan).
