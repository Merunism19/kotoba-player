# Rules for every helper (read before your brief)

Your brief says **what** to build. This file says **how**. Follow both.

## Hard rules (never break; ask the orchestrator instead)
1. Nothing about the user's files, subtitles or lookups leaves the machine. The only network
   targets allowed: AnkiConnect at `127.0.0.1`, and the DeepL module when the brief says so.
2. Never write next to the user's video files. Use the app cache dir.
3. Anki: only the app's own note type; never delete/edit existing notes, decks or note types.
4. New dependencies only under the licence allow-list in `docs/DESIGN.md` §8; name each new one
   and its licence in your reply.
5. No content-sourcing features (no torrent/stream search or downloads).
6. Secrets only through the `keyring` crate; never logged.

## How to work
- Read `docs/CODEMAP.md` first to find files; read only what the brief points to plus what you need.
- Edit files with the file-edit tool, not scripted replacements (Windows scripts turn LF into CRLF).
  All files UTF-8, LF line endings.
- Design: use tokens from `src/design/tokens.css`; match `docs/design/mockup.html`. Pretty is a requirement.
- Don't commit, push, switch branches or touch git config.
- Run the verify command **after your last edit**: `node scripts/ci.mjs --quick`
  (or the narrower command your brief names). If you edit after verifying, verify again.
- Stop every background process (dev servers, watchers) and wait for background commands before replying.
- Write the list of files you changed to `.ship/changes.txt` (one path per line).
- Claim only what you actually ran. "Should work" is not a result.

## Reply format (6–12 lines)
1. What was built (one or two lines).
2. Files changed — and "changes.txt matches" after checking it against `git status`.
3. Exactly what ran and its result (command → pass/fail counts).
4. New dependencies + licences (or "none").
5. What needs the owner's real device (or "nothing").
