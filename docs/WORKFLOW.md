# Project workflow — Kotoba Player

The project-specific instance of `AGENT-WORKFLOW.md`. Read that file first; this one only holds
what is specific to this repo.

## Roles here
- Orchestrator: the strongest model; owns the mpv bridge, AnkiConnect client and any
  security/data-writing code, plus review and shipping.
- Helpers: well-scoped pieces (parsers, crates, screens, templates, tests) per `DESIGN.md` §9.
  Parallel helpers each get their own git worktree (`git worktree add ../kp-<brief> -b <branch>`).

## Commands (fill in during Phase 0; keep exact)
| What | Command |
|---|---|
| Dev run | `pnpm tauri dev` |
| Local CI (everything CI runs) | `node scripts/ci.mjs` (`pnpm run ci`); `--quick` skips licences, vite build, clippy (`pnpm run ci:quick`); `--docs-only` runs encoding + codemap check |
| Quiet Rust checks | `cargo clippy -q --workspace -- -D warnings` / `cargo test -q --workspace` |
| UI checks | `pnpm -s lint && pnpm -s typecheck && pnpm -s test` |
| Screenshots | `pnpm -s shots` (Playwright, writes `artifacts/shots/*.png`) |
| Code map | `node scripts/codemap.mjs` → `docs/CODEMAP.md` (`--check` fails if stale) |
| Ship | `node scripts/ship.mjs [--bump patch\|minor\|none] [--release] [--local-gate] [--dry-run]`, text from `.ship/commit-msg.txt`, `.ship/pr-body.md`, `.ship/release-notes.md` (bump → codemap → guards → commit/PR from files → gate → merge → release → `SHIP OK`) |

## Gates (local CI runs all of them)
1. Encoding guard: every text file valid UTF-8, no CR bytes, no stray control chars.
2. Licence guard: `scripts/guards/licenses.mjs` (own script, no cargo-deny): `cargo metadata` + `pnpm licenses` against the allow-list in DESIGN §8.
3. Rust: fmt, clippy (-D warnings), tests, fuzz smoke (time-boxed) for `crates/subtitles` and dictionary import.
4. UI: eslint, `tsc --noEmit`, vitest, Playwright screenshots (popup over white + busy frame; card front/back dark/light).
5. Settings registry guard: no settings/storage file opened outside `settings/registry`.
6. Network guard (`scripts/guards/network.mjs`): grep that no HTTP client targets anything but `127.0.0.1` except the DeepL module.

## Conventions
- Rust 2021, `thiserror` for errors, no `unwrap()` outside tests.
- TS strict; types for Tauri commands generated with `ts-rs` (never hand-written twice).
- Design tokens only from `src/design/tokens.css`; no raw colours in components.
- Commit/PR text in files under `.ship/` (never quoted on the command line).

## Changelog
- 2026-11-08: Created from AGENT-WORKFLOW.md v1.0 during the design round.
- 2026-11-08: Added scripts/ci.mjs, ship.mjs, codemap.mjs, guards (encoding, licenses, network) and `.github/workflows/ci.yml` (PR-only, ubuntu, no Rust).
- 2026-11-08: Hosted Actions blocked by account billing; owner chose local-CI gating (`ship.mjs --local-gate`), which now also notes the gate on an already-open PR.
