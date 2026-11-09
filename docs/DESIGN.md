# Design: Anime video player with click-to-look-up dictionary and Anki mining

Working name: **Kotoba Player** (rename any time — one string in `tauri.conf.json` + this file).

Status: design approved for build? **No — draft, waiting for owner review.** Visual mockup:
[docs/design/mockup.html](design/mockup.html) (open it in a browser).

---

## 1. What the owner asked for (their words, condensed)

1. A video player that opens any video file, mainly anime (`.mkv` and everything else), with
   **subtitles in sync**.
2. **Everything pretty.**
3. Click a word in the subtitle → a pretty **dictionary popup** with: English translation, a
   button to **hear it**, its **hiragana/katakana** reading below it, and its **pitch accent**.
4. A button in the popup **sends it to Anki** as a card:
   - **Front:** the word; the example sentence it came from; a screenshot of the anime below.
   - **Back:** the Japanese word and Japanese sentence stay; below each, in a pretty way, the
     English translation of the word and of the sentence; the hiragana/katakana reading too.
   - **Hovering a Japanese word** (on the card) shows its kana reading.

Everything below serves those points. Section 11 lists the extras I added and why.

## 2. Key technical decisions (with reasons)

| Decision | Choice | Why / what was ruled out |
|---|---|---|
| App shell | **Tauri v2** (Rust core + WebView2 UI) on Windows 11 | Small, fast, native file access; the UI is HTML/CSS, so it can look as good as we want. Electron would also work but is heavier and has no advantage here. |
| Video engine | **libmpv** embedded through `tauri-plugin-libmpv` (mpv draws into the window, a **transparent WebView** draws our UI on top) | Anime releases use codecs a browser `<video>` can't play reliably: H.264 **Hi10P** (8-bit-only hardware), AC3/DTS audio, MKV with font attachments and styled ASS subs. mpv plays all of it, seeks instantly, has hardware decoding, and can take screenshots. The plugin's docs say Windows is the fully tested platform. **Fallback** if the spike (Phase 0) fails: `tauri-plugin-mpv` (mpv.exe child process over JSON IPC, same overlay idea). **Spike passed 2026-11-08** (plugin 0.3.2, wrapper 0.1.1, libmpv 2026-10-30): Hi10P H.264 (software decode), HEVC 10-bit (d3d11va) and H.264 MP4 all play with 0 dropped frames, 24–48 `time-pos` events/s reach the WebView, the overlay is clickable, fullscreen and clean shutdown work. |
| Subtitle display | **We draw the Japanese dialogue line ourselves** as HTML (each word clickable); mpv still renders signs/songs from ASS if the user wants | The user has to be able to click words, so the line must be DOM text, not pixels. |
| Subtitle source of truth | Rust parses the whole track (ASS/SSA/SRT/VTT; embedded tracks extracted with **ffmpeg**) into a cue list; the overlay follows mpv's `time-pos` + a per-file **offset** | Owning the cue list gives: offset sync, "previous/next line", transcript sidebar, sentence boundaries for mining, and lining up the English track with the Japanese one. |
| Sync | Manual offset hotkeys (±100 ms), "this line starts now" sync, and **automatic sync** of an external subtitle file to the audio (`alass-core`, GPL-3) | Japanese subs for anime often come from a different release and are offset; auto-sync fixes it in seconds. |
| Word lookup | **Owner, 2026-11-08: a click must select the whole word** (several kanji/kana together, e.g. 夕焼け, 綺麗, 知らなかった), never a single character. Yomitan-style **scan from the clicked character**: try the longest substring first, **deinflect** it (our own conjugation rule table), look it up, rank by match length and frequency | Clicking anywhere in 食べさせられなかった finds 食べる and shows "causative · passive · negative · past". A plain tokenizer can split wrongly; the scan is what learners expect. |
| Tokenizer | **Lindera** (MIT) + IPADIC/UniDic for word underlines, sentence furigana and readings | Needed for hover-furigana of the *whole sentence* on the card, and to show word boundaries on hover in the player. |
| Dictionary data | **JMdict (English)** — EDRDG licence, CC BY-SA 4.0 — compiled into **SQLite** by a build tool; **Kanjium** pitch accents (CC BY-SA 4.0); later: import any **Yomitan-format** dictionary zip | Free, accurate, offline. Attribution goes on the About screen. |
| Word audio ("hear it") | **Decided 2026-11-08 (owner: "you decide").** Chain, first that works: (1) **VOICEVOX** engine if it is running locally (`127.0.0.1:50021`, auto-detected; free, natural Japanese voices with correct pitch accent; its terms require crediting the voice — shown in the popup tooltip and About), (2) **Windows Japanese voice** (WinRT `SpeechSynthesizer`, offline, the always-available default), (3) a local audio pack folder if the user ever adds one | Everything stays on the machine. Windows voice needs the Japanese speech pack (device test plan); VOICEVOX is an optional free upgrade the owner can install any time. |
| Sentence audio | ffmpeg cuts the line's audio (cue start − 250 ms … end + 250 ms) to `.mp3` | Goes on the card, so the user hears the real anime voice. |
| Screenshot | mpv `screenshot-to-file` with `video` mode (no subtitles burned in), resized to 640 px wide, `.webp` | Clean frame for the card front. |
| Sentence translation | **Decided by owner 2026-11-08:** (1) the **English subtitle track** (embedded or external), lined up by time overlap — always first; (2) if there is none: **DeepL API Free** (free account, 500k characters/month, owner's key in the OS keyring); (3) if no key / quota used up: a local **Ollama** model if one is running (owner has Ollama installed; free, offline); (4) the editable field in the mining preview always lets the owner fix it | Free in every case. Only the one sentence being mined is sent to DeepL, and only when a key is set. |
| Anki | **AnkiConnect** on `127.0.0.1:8765` (API v6). The app creates **its own note type** ("Kotoba Mining") with our templates and updates it when the template version changes | One-click cards with media; never touches the user's other note types. |
| UI stack | React 19 + TypeScript + Vite, CSS variables for design tokens, Framer Motion for the popup springs, Zustand for state | Common, well-known, helpers write it reliably. |
| Package manager | pnpm (installed) | — |
| Licence of the app | **GPL-3.0** (libmpv and alass are GPL) | Fine for personal use. Publishing is the owner's call. |

## 3. Architecture

```
┌──────────────────────── Tauri window (frameless, transparent) ───────────────────────┐
│  WebView2 (React UI, transparent background)                                          │
│   ├─ Player chrome: top bar, controls, seek bar with subtitle-cue ticks               │
│   ├─ SubtitleLayer: JP line as <span> per token, EN line (blur until hover)           │
│   ├─ DictionaryPopup / MiningPreview / TranscriptSidebar / Settings / Library         │
│   └─ talks to Rust through typed `invoke` commands + events (ts-rs generated types)   │
│  ─────────────────────────────────────────────────────────────────────────────────── │
│  libmpv video surface (drawn into the same HWND, under the WebView)                   │
└───────────────────────────────────────────────────────────────────────────────────────┘
Rust core (src-tauri)
  player/   mpv bridge: load, play/pause, seek, tracks, observe time-pos/pause/duration, screenshot
  media/    ffmpeg/ffprobe sidecar: probe tracks, extract subtitle track, cut audio clip
  subs/     parse (ASS/SRT/VTT) → Cue{start,end,text,style}, sign/song filter, align EN↔JP, offset, auto-sync
  jp/       tokenize (Lindera), furigana (anki "漢字[かんじ]" format), deinflect, kana utils
  dict/     SQLite (rusqlite, bundled) queries: lookup(term, reading), pitch, frequency, Yomitan import
  audio/    word audio chain (local pack → TTS)
  anki/     AnkiConnect client: version, decks, ensure note type, canAddNotes, storeMediaFile, addNote, guiBrowse
  library/  recent files, per-file offset/track choices, watch position (JSON in app data dir, one registry)
  settings/ one registry for every stored setting (section 8 guard: "data stores are registered")
crates/ (separate Rust crates so tests and fuzzing are fast, no Tauri needed)
  subtitles/  parser + aligner + fuzz target
  jpdict/     deinflector + lookup + furigana
tools/build-dict/   JMdict XML + Kanjium accents → dict.sqlite (shipped with the installer)
anki/               note type: front.html, back.html, style.css, version.txt
```

Data flow for a lookup: click on character *i* of the current cue → `lookup_at(cue_id, i)` →
Rust scans substrings starting at *i* (longest first, max 16 chars), deinflects, queries SQLite,
returns `LookupResult[]` (term, reading, furigana, pitch downsteps, senses, inflection chain, match
length) → popup renders; matched characters get highlighted in the subtitle line. The player
**pauses automatically** while the popup is open and resumes when it closes (setting).

Mining flow: "Add to Anki" → Rust in parallel: screenshot at the current frame, cut sentence
audio, get word audio, build furigana for word and sentence, take the aligned English line →
**mining preview** (the card as it will look, editable sentence translation, pick which sense) →
"Send" → `storeMediaFile` ×3 + `addNote`. **Shift+click** on "Add to Anki" skips the preview.
Duplicate check first (`canAddNotes`): the button shows "Already in Anki" with a link that opens
it in the Anki browser.

## 4. Screens

1. **Library / home** — **fills the whole window** (owner, 2026-11-08: no small card on black;
   it is an opaque page covering mpv's idle surface), big drop zone ("Drop an episode"), "Continue watching" row with
   thumbnails and progress bars, recent folders. Opening a folder lists episodes in order.
2. **Player** — full-bleed video; controls fade out after 2 s of no mouse movement.
   - Top bar: show title, episode, track picker (audio / JP subs / EN subs), sync badge
     ("+0.3 s"), settings.
   - Bottom: seek bar with small ticks for subtitle cues and a gold tick for every mined line;
     play, ±5 s, previous/next line, replay line, volume, speed, fullscreen.
   - **Subtitle layer**: JP line large (Noto Sans JP 600, soft shadow, no box); hovering shows a
     faint underline under each word (from the tokenizer); hover can pause playback (setting).
     The EN line sits below, smaller and **blurred until hovered** (setting: always / blurred / hidden).
   - **Transcript sidebar** (T): all lines, current one highlighted, click to jump, mined lines
     marked.
3. **Dictionary popup** — glass card anchored above the clicked word, arrow pointing at it.
   - Header: the word in large type with **furigana above it**, the kana reading again below it
     (as the owner asked), 🔊 button, frequency badge, JLPT/common badge.
   - **Pitch accent**: the kana drawn with the classic high/low line (SVG), coloured by pattern
     (平板 heiban blue, 頭高 atamadaka red, 中高 nakadaka orange, 尾高 odaka green), with the
     pattern name and downstep number.
   - Inflection chip when deinflected: `食べさせられなかった → 食べる · causative · passive · negative · past`.
   - Senses: numbered English meanings with part-of-speech pills; extra senses collapse.
   - Other matches (shorter matches, other readings) as small tabs.
   - Footer: **Add to Anki** (primary, sakura pink), "Copy", status ("Added ✓", "Already in Anki").
   - Keyboard: Esc closes, ←/→ moves through matches, Enter = Add to Anki, R = play audio.
4. **Mining preview** — a modal showing the real card front and back side by side, editable
   sentence translation, sense checkboxes, deck name, tags; "Send to Anki".
5. **Settings** — Anki (connection test, deck, note type status, tags), Subtitles (font size,
   EN display mode, sign/song filter, auto-pause), Audio (sources order, TTS voice, test),
   Sync (offset step, auto-sync), Appearance (accent colour, theme), Hotkeys, About (licences,
   dictionary attributions).

## 5. Visual design system

Mood: **"midnight cinema"** — the video is the star; UI is dark glass that floats over it.

| Token | Value | Use |
|---|---|---|
| `--bg` | `#0B0D12` | app background, letterboxing |
| `--glass` | `rgba(18, 20, 28, 0.72)` + `backdrop-filter: blur(24px) saturate(140%)` | popup, bars, panels |
| `--hairline` | `rgba(255,255,255,0.08)` 1 px | panel borders, dividers |
| `--text` / `--text-2` / `--text-3` | `#F2F3F7` / `#B4B8C5` / `#7A7F8E` | text levels |
| `--accent` | `#FF7AA2` (sakura) | primary buttons, mined ticks, focus rings |
| `--accent-2` | `#8A9BFF` (twilight indigo) | links, secondary highlights |
| `--gold` | `#FFD27A` | "mined" marks, frequency badge |
| pitch | heiban `#5BA8FF`, atamadaka `#FF6B6B`, nakadaka `#FFA94D`, odaka `#4FD6A8` | pitch graph + reading colour |
| radius | 20 px panels, 12 px buttons, 999 px pills | |
| shadow | `0 20px 60px rgba(0,0,0,.45)` | floating panels |

**Glass over video can't blur (spike finding).** mpv draws the video in its own surface under the
WebView, so CSS `backdrop-filter` only sees the page, never the video: panels over video look
tinted, not frosted. Over video use **tinted glass**: `rgba(14,16,22,0.82)` (popup 0.9) + hairline
border + shadow, no blur. Blur stays only where the page itself is underneath (home, settings,
panels over panels). The mockup was updated to show it this way.

Type: **Inter** for UI, **Noto Sans JP** (subtitles, dictionary) and **Zen Maru Gothic** for the
big headword. Fonts are bundled (OFL), never loaded from the web.

App icon (owner request, 2026-11-08): a speech bubble (kotoba = "words") in the sakura →
twilight-indigo gradient with 言 (Zen Maru Gothic 700) cut out of it, on a dark ink rounded
square. Source: `scripts/icon/icon.html`; `node scripts/icon/make-icon.mjs [--preview]` renders
`src-tauri/icons/source.png` + `src/assets/logo.png` (the home-screen logo), then
`pnpm tauri icon src-tauri/icons/source.png` makes the platform set (delete `android/`, `ios/`).
The installer always creates a desktop shortcut (`src-tauri/windows/hooks.nsh`), except on
updater runs and `/NS`.

Motion: popup springs in from 96 % scale + 6 px rise, 160 ms; controls fade 200 ms; the "Added"
state morphs the button into a check. Respect `prefers-reduced-motion`.

Accessibility: every control reachable by keyboard, visible focus ring, contrast ≥ 4.5:1 for text
on glass (glass opacity is chosen so this holds over a white frame — a headless screenshot test
renders the popup over a white and a busy frame).

## 6. The Anki card

Note type **"Kotoba Mining"**, fields (in order):

| Field | Example | Notes |
|---|---|---|
| `Word` | 諦める | the dictionary form |
| `WordFurigana` | 諦[あきら]める | Anki furigana syntax |
| `WordReading` | あきらめる | plain kana (katakana kept for katakana words) |
| `PitchPattern` | `4` + generated SVG/HTML graph | |
| `Meaning` | `<ol><li>to give up; to abandon</li>…</ol>` | the senses picked in the preview |
| `Sentence` | もう<b>諦めない</b>って決めたんだ | target word bolded as it appeared |
| `SentenceFurigana` | もう<b>諦[あきら]めない</b>って 決[き]めたんだ | from the tokenizer |
| `SentenceTranslation` | I decided I won't give up anymore. | from the EN track or typed |
| `Screenshot` | `<img src="kotoba_….webp">` | |
| `SentenceAudio` | `[sound:kotoba_….mp3]` | |
| `WordAudio` | `[sound:kotoba_….mp3]` | |
| `Source` | Frieren · S01E07 · 12:34 | |

**Front**: the word, large, centred (hover → furigana appears above it); the sentence below it
with the target word highlighted in sakura pink (hover any word → its furigana); the screenshot
below with rounded corners and a soft shadow.

**Back**: the front stays exactly where it was (no jump). Under the word: the kana reading,
pitch graph and the English meanings. Under the sentence: the English translation in a lighter,
italic style with a thin sakura rule on the left. Audio buttons for the word and the sentence; the
source in small text at the bottom. The kana reading is printed on the back and also appears on
hover on both sides.

Hover-furigana works with Anki's built-in `{{furigana:…}}` filter plus CSS
(`ruby rt { opacity: 0 } ruby:hover rt { opacity: 1 }`); on phones (AnkiDroid / AnkiMobile)
tapping a word toggles it (a 10-line script in the template). Dark and light themes both handled
(`.nightMode`). The templates live in `anki/` with a version number; the app updates the note
type's templates and CSS when that number goes up.

## 7. Hotkeys (defaults, all rebindable)

Space play/pause · ←/→ 5 s · A / D previous / next line · S replay line · Z / X subtitle offset
−/+100 ms · Shift+Z "sync this line now" · T transcript · F fullscreen · Esc close popup ·
Enter add to Anki (popup open) · R play word audio · E toggle English line.

## 8. Hard rules (non-negotiable; copied into `docs/agents/BRIEF.md`)

1. **Privacy:** nothing about the user's files, subtitles or lookups leaves the machine, except
   (a) local services on `127.0.0.1` (AnkiConnect, VOICEVOX, Ollama) and (b) the DeepL call for
   the one sentence being mined, only when there is no English track and the owner has entered
   their own key. No analytics, no auto-update pings without a setting.
2. **Never write next to the user's videos.** Extracted subtitles, clips and screenshots go to the
   app cache dir; media for Anki goes through `storeMediaFile`.
3. **Anki safety:** only create/update the app's own note type; never delete or edit existing
   notes, decks or other note types. Check duplicates before adding.
4. **Licences:** dependencies only from the allow-list (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
   LGPL, GPL-2.0+/GPL-3.0, OFL for fonts, CC BY-SA 4.0 for dictionary data). Attribution for
   JMdict/EDRDG and Kanjium on the About screen and in `THIRD-PARTY.md`.
5. **No content-sourcing features** (no torrent/streaming-site search or downloading). The app
   plays files the user already has. Audio packs are user-supplied.
6. Secrets (DeepL key) stored with the OS credential store (`keyring` crate), never in plain
   settings files or logs.

## 9. Build plan — phases, each one ships something usable

Each phase is a set of briefs (section 4 of AGENT-WORKFLOW). **H** = helper brief,
**O** = orchestrator does it. Parallel helpers each get their own git worktree.

**Status (2026-11-08): all five phases built.** Phase 0 merged (PR #1); Phases 1–3 are open PRs #2–#4
(stacked, gated on local CI); Phases 4–5 are on `phase4-polish` → v1.0.0. Deviations from the plan
below, decided on the way:
- Word audio has no "local audio pack" source yet (VOICEVOX → Windows voice only).
- Auto-sync uses the release's English subtitles as the timing reference (no audio-based sync);
  the track's signs are rewritten with the same shifts and mpv reloads them.
- ffmpeg ships as the LGPL **shared** build (~100 MB smaller); UniDic stays embedded (~194 MB in
  the exe; the installer compresses to ~137 MB).
- Security hardening after review: mpv is driven only through an allowlisting Rust layer
  (`mpv_guard.rs`), strict CSP, sandboxed card preview, ffmpeg limited to local-file inputs.

### Phase 0 — Foundation and the risky spike (O, ~1 session)
- Install Rust (rustup, MSVC toolchain), add mpv dev build (`libmpv-2.dll`) and ffmpeg/ffprobe
  binaries under `src-tauri/bin/` (scripted download with pinned versions + SHA-256 checks).
- `git init`, scaffold Tauri v2 + React + TS + Vite, `docs/CODEMAP.md` generator, `scripts/ci.mjs`
  (local CI), `scripts/ship.mjs` (resumable, ends with `SHIP OK`), guards: UTF-8/no-CR, licence
  allow-list (`cargo-deny` + `license-checker`).
- **Spike:** a transparent Tauri window with libmpv underneath playing (1) a Hi10P H.264 MKV with
  ASS, (2) an HEVC 10-bit MKV, (3) an MP4; a React overlay button on top; `time-pos` events at
  ≥10 Hz into the WebView. **Exit criterion:** all three play smoothly, overlay is clickable,
  fullscreen works, CPU use reasonable. If not → switch to the JSON-IPC fallback and note why.

### Phase 1 — Player MVP (ship v0.1)
- H1 `crates/subtitles`: ASS/SSA/SRT/VTT parser (tags stripped, `\N` handled, karaoke tags
  removed), sign/song filter heuristics, EN↔JP alignment by overlap, fuzz tests, fixtures from
  real releases.
- O: mpv bridge commands/events, ffprobe track listing, subtitle extraction. Signs/songs: mpv's
  `sub-text` merges every line on screen (the spike showed a sign and a dialogue line joined), so
  we split them ourselves and hand mpv a **signs-only ASS** (`sub-add`) to render, while dialogue
  stays in the WebView.
- H-tok (moved up from Phase 2 at the owner's request): Lindera tokenizer behind a
  `tokenize(line)` command; the subtitle line renders **one hoverable span per word** (whole-word
  hover highlight, click hands the word's start to the Phase 2 lookup).
- H2 Player UI: controls, seek bar with cue ticks, track picker, auto-hide, hotkeys, offset badge,
  library/home with recent files + resume position, drag-and-drop, headless screenshot tests.

### Phase 2 — Dictionary popup (ship v0.2)
- H3 `tools/build-dict`: JMdict + Kanjium → `dict.sqlite` (indexes on expression and reading,
  frequency from JMdict priority tags), with a size/time report.
- H4 `crates/jpdict`: deinflection table + tests (≥150 conjugation cases from real lines),
  lookup-at-position scan, furigana generation, kana utilities.
- H5 Popup UI: the design in section 4.3 incl. the SVG pitch graph, inflection chip, senses,
  keyboard navigation; word-boundary hover underline; auto-pause.
- O: word audio chain (local pack → Windows TTS).

### Phase 3 — Anki mining (ship v0.3)
- H6 `anki/` templates + CSS (front/back, hover furigana, phone tap, dark/light) with a
  Playwright screenshot test of both sides.
- O: AnkiConnect client + "ensure note type" + duplicate check (security-relevant: localhost only).
- H7 Media: screenshot, sentence audio clip, mining preview modal, Shift+click instant mine,
  gold "mined" ticks on the seek bar.

### Phase 4 — Sync and polish (ship v0.4)
- External subtitle loading (same-name files, drag a `.ass/.srt` onto the player), auto-sync with
  `alass-core`, "sync this line now", transcript sidebar, settings screen, accent colour, Yomitan
  dictionary import (with zip fuzzing), optional DeepL.

### Phase 5 — Packaging (ship v1.0)
- Tauri NSIS installer with mpv, ffmpeg, dictionary and fonts bundled; About/licences screen;
  release through the ship script; review loop per risk area (section 5 of the workflow):
  file handling, Anki writes, subtitle parsing, crash paths.

## 10. Testing

- Rust unit tests per crate; **fuzz tests** for subtitle parsing and dictionary import.
- **Real-output fixtures**: ffprobe JSON from real anime files, AnkiConnect responses,
  real ASS files from several fansub groups (dialogue/sign/song styles).
- Vitest for UI logic; **Playwright headless screenshots** of every screen and of the Anki card
  (front/back, dark/light) so the orchestrator sees the UI without the owner.
- Owner device test plan: `docs/DEVICE-TEST-PLAN.md`.

## 11. Extras I added (cheap, and learners expect them)

- Auto-pause while the popup is open / while hovering subtitles.
- Previous/next/replay line (A/S/D) — the core "study" loop.
- English line blurred until hover (so the user reads Japanese first).
- Gold marks on the seek bar and transcript for already-mined lines; "Already in Anki" status.
- Shift+click instant mining.
- Resume where you left off, per-file offset and track memory.

## 12. Owner decisions and open items

Decided 2026-11-08: private GitHub repo (`Merunism19/kotoba-player`); name stays **Kotoba
Player**; translation = English track → DeepL Free → local Ollama → manual; word audio chosen by
the orchestrator (section 2: VOICEVOX if running → Windows voice).

Still open (owner actions, needed only from Phase 2/3):
1. Install **AnkiConnect** (add-on code `2055492159`) before Phase 3.
2. Install the Windows **Japanese speech pack**; optionally install VOICEVOX for nicer audio.
3. Create a free **DeepL API Free** key when you want machine translation for files without English subs.
