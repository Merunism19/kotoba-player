# Third-party software and data

Kotoba Player bundles or builds on the components below. The About screen shows the same
attributions. Licence texts of the Rust and JavaScript packages are listed by `node scripts/guards/licenses.mjs`
against the allow-list in `docs/DESIGN.md` section 8.

## Dictionary data

### JMdict (English)

This application uses the JMdict/EDICT dictionary files. These files are the property of the
Electronic Dictionary Research and Development Group (EDRDG), and are used in conformance with the
Group's licence: <https://www.edrdg.org/edrdg/licence.html>.

- Source: <https://www.edrdg.org/jmdict/j_jmdict.html>, converted to JSON by
  [jmdict-simplified](https://github.com/scriptin/jmdict-simplified) 3.6.2+20261026175559
  (dictionary date 2026-10-26). SHA-256 of the downloaded file is pinned in `scripts/fetch-deps.mjs`.
- Licence: Creative Commons Attribution-ShareAlike 4.0 (CC BY-SA 4.0), <https://creativecommons.org/licenses/by-sa/4.0/>.
- Use: compiled by `crates/jpdict` (`build-dict`) into `src-tauri/lib/dict/jmdict.sqlite`. Entries are shown unchanged
  except that part-of-speech and usage tags are expanded to plain English. The database is distributed under the
  same CC BY-SA 4.0 terms.

### Kanjium (pitch accents)

- Source: <https://github.com/mifunetoshiro/kanjium>, commit `9ebca4589565c696e7c39e089e2e928ad65f678c`,
  file `data/source_files/raw/accents.txt` (SHA-256 pinned in `scripts/fetch-deps.mjs`).
- Licence: Creative Commons Attribution-ShareAlike 4.0 (CC BY-SA 4.0).
- Use: merged into the same SQLite database; shown as pitch accent patterns in the dictionary popup.

### UniDic

- Source: NINJAL (National Institute for Japanese Language and Linguistics) UniDic, embedded through the
  `lindera` crate (`embed-unidic`) for word segmentation of subtitle lines.
- Licence: UniDic is tri-licensed (GPL-2.0, LGPL-2.1, BSD); this application uses it under the BSD terms.
  Copyright (c) the National Institute for Japanese Language and Linguistics.

## Media components (fetched by `scripts/fetch-deps.mjs`, not stored in the repository)

### libmpv

- mpv, built as `libmpv-2.dll` by [zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild) (LGPL variant, release 2026-10-30).
- Licence: LGPL-2.1-or-later. The DLL is loaded dynamically and can be replaced by the user with another build.
  Source code: <https://github.com/mpv-player/mpv>.
- `libmpv-wrapper` v0.1.1 by [nini22P](https://github.com/nini22P/libmpv-wrapper): a small wrapper library loaded next to libmpv;
  see that repository for its licence.

### Microsoft Visual C++ runtime

- `vcruntime140.dll` from the Visual C++ 2015–2022 Redistributable (copied by `scripts/fetch-deps.mjs` from the
  Visual Studio Build Tools' `VC\Redist\MSVC\<version>\x64\Microsoft.VC143.CRT` folder; installed next to the
  exe). `libmpv-wrapper.dll` needs it, and Windows doesn't include it.
- Redistributed app-local, unmodified, as Microsoft's Visual Studio licence terms allow for its "Distributable
  Code" (see `redist.txt` / <https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files>). It is a
  system library in the sense of the GPL.

### FFmpeg

- FFmpeg n8.1.3 (`ffmpeg.exe`, `ffprobe.exe` and their shared libraries `avcodec`, `avformat`, `avfilter`, `avdevice`,
  `avutil`, `swresample`, `swscale`), LGPL shared build from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds).
- Licence: LGPL-2.1-or-later. The executables are run as separate processes; the DLLs are unmodified and replaceable.
  Source code: <https://ffmpeg.org/>.

## Rust libraries added for the dictionary

| Crate | Licence | Note |
| --- | --- | --- |
| `rusqlite` (feature `bundled`) | MIT | SQLite bindings |
| SQLite (compiled in by `libsqlite3-sys`) | Public domain | <https://sqlite.org/copyright.html> |
| `serde`, `serde_json` | MIT OR Apache-2.0 | JSON |

## Other notable libraries

| Component | Licence | Note |
| --- | --- | --- |
| `alass-core` | GPL-3.0 | automatic subtitle synchronization (the app is GPL-3.0-or-later) |
| `lindera` + UniDic | MIT; UniDic BSD-3-Clause | tokenizer and its dictionary (embedded) |
| `encoding_rs` | MIT OR Apache-2.0 (+ BSD-3-Clause data) | subtitle decoding |
| `ureq` (native-tls) | MIT OR Apache-2.0 | loopback HTTP (AnkiConnect, VOICEVOX, Ollama) and DeepL |
| `keyring` | MIT OR Apache-2.0 | DeepL key in Windows Credential Manager |
| `windows` | MIT OR Apache-2.0 | Windows speech (WinRT) |
| `tauri`, `tauri-plugin-dialog` | MIT OR Apache-2.0 | app shell |
| `tauri-plugin-libmpv` | MPL-2.0 | mpv embedding |
| React, zustand | MIT | user interface |
| Inter, Noto Sans JP, Zen Maru Gothic (via Fontsource) | SIL OFL 1.1 | bundled fonts |

Optional services the app talks to only on this computer (127.0.0.1) when the user runs them:
AnkiConnect (Anki add-on), VOICEVOX (its voices must be credited, e.g. "VOICEVOX:四国めたん"), Ollama.
DeepL is used only with the user's own API key, for lines without English subtitles.
