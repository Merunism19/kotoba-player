# Kotoba Mining note type

`version.txt` is bumped whenever a template or the CSS changes. When it goes up, the app calls
AnkiConnect `createModel` (first install) or `updateModelTemplates` + `updateModelStyling`
(existing "Kotoba Mining" model). Fields are created in the order of `fields.json`.
One card template, "Card 1": `front.html` / `back.html`; styling is `style.css`.

| Field | Holds |
|---|---|
| Word | dictionary form, e.g. 諦める |
| WordFurigana | Anki furigana syntax, `諦[あきら]める` (a space before a kanji group separates it from preceding text) |
| WordReading | plain kana reading (katakana kept for katakana words) |
| PitchPattern | HTML: inline SVG pitch graph (use `currentColor`; the card colours it by PitchClass) |
| PitchClass | `heiban`, `atamadaka`, `nakadaka`, `odaka`, or empty |
| Meaning | `<ol><li>…</li></ol>` |
| Sentence | sentence with the target word in `<b>…</b>` |
| SentenceFurigana | same with furigana syntax (`<b>` kept) |
| SentenceTranslation | English translation |
| Screenshot | `<img src="kotoba_x.webp">` |
| SentenceAudio, WordAudio | `[sound:kotoba_x.mp3]` |
| Source | "Show · S01E07 · 12:34" |

Front: word, sentence, screenshot. Back: the word and sentence stay; reading, pitch and meanings
go under the word, the translation under the sentence, then a small screenshot and the source
(`{{FrontSide}}` is not used, so the back can interleave blocks). Readings (`<rt>`) show on hover
or tap (a tiny idempotent script toggles `.on` on the tapped `ruby`). Night mode: `.nightMode` and
`.night_mode`. No external resources; system Japanese fonts only.
