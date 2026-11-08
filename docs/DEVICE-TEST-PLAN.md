# Device test plan (owner's PC)

One ordered list of checks that need the owner's machine. Every PR adds its checks here.

## Before Phase 3
- [ ] Install AnkiConnect in Anki (Tools → Add-ons → Get Add-ons → code `2055492159`), restart Anki.
- [ ] Install a Japanese speech voice: Windows Settings → Time & language → Speech → Add voices → Japanese.

## v0.1.1 (Phase 0 spike) — optional, already checked by the probe on this PC
- [ ] `pnpm tauri dev`, open one of your real anime MKVs: plays smoothly, JP line clickable, English line under it (if the file has one).
- [ ] Try a file with styled signs: signs are not shown yet (expected until Phase 1).

## v0.2.0 (Phase 1 player)
- [ ] Open a real episode (MKV with JP + EN subs): JP line shows, hovering highlights whole words, EN line blurred until hover.
- [ ] Styled signs/songs show (drawn by mpv); dialogue is not doubled.
- [ ] Z/X shifts subtitles, badge shows the offset; reopening the file keeps the offset and resumes where you stopped.
- [ ] A file with only an external `Name.ja.srt` next to it picks that up.

## v0.3.0 (Phase 2 dictionary)
- [ ] Click words in a real episode: the popup finds whole words (conjugated verbs too) with meanings, reading and pitch.
- [ ] Install the Japanese speech voice (Windows Settings → Time & language → Speech → Add voices → Japanese) and press 🔊 / R.
- [ ] Optional: install and start VOICEVOX — 🔊 then uses it (credit shown in the tooltip).

## v0.4.0 (Phase 3 Anki mining)
- [ ] Anki open with AnkiConnect: click a word → Add to Anki → the preview matches what lands in Anki (desktop, dark + light).
- [ ] The card plays the line's audio; the screenshot is the right frame; hover shows readings; on the phone a tap does.
- [ ] Adding the same word again shows "Already in Anki"; Shift+click adds without the preview.

## v1.0.0 (Phases 4–5: polish + installer)
- [ ] Install `Kotoba Player_1.0.0_x64-setup.exe` (from the GitHub release or `target/release/bundle/nsis/`); it opens a real episode from Explorer ("Open with").
- [ ] Settings (gear / Ctrl+,): change accent, subtitle size, English mode — they stick after a restart.
- [ ] Optional DeepL: paste your free key in Settings → it shows "used / limit"; mine a line from a file with no English subs → the preview says "machine translation (DeepL)".
- [ ] Drop a Japanese `.srt` from another release onto the player → it becomes the JP track; Track picker → "Sync to English" fixes its timing (Undo works).
- [ ] T opens the transcript; clicking a line jumps there.
- [ ] Optional: import a Yomitan frequency dictionary (Settings → Dictionaries) → the popup shows "★ freq #…".

## Per release (fill in as features land)
- [ ] Open a Hi10P H.264 MKV, an HEVC 10-bit MKV and an MP4; each plays, seeks, fullscreens.
- [ ] Embedded JP subs appear in time; Z/X offset moves them; English line shows under it.
- [ ] Click a word → popup with meaning, reading, pitch; 🔊 speaks it.
- [ ] Add to Anki → card appears in the chosen deck with screenshot, sentence audio, word audio;
      hover shows furigana on desktop; tap shows it on the phone.
