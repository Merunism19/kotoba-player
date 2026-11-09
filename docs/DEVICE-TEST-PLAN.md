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

## Per release (fill in as features land)
- [ ] Open a Hi10P H.264 MKV, an HEVC 10-bit MKV and an MP4; each plays, seeks, fullscreens.
- [ ] Embedded JP subs appear in time; Z/X offset moves them; English line shows under it.
- [ ] Click a word → popup with meaning, reading, pitch; 🔊 speaks it.
- [ ] Add to Anki → card appears in the chosen deck with screenshot, sentence audio, word audio;
      hover shows furigana on desktop; tap shows it on the phone.
