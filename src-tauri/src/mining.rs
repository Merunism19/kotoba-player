//! Anki card fields from a clicked word: furigana in Anki syntax, the bolded sentence, meanings, the pitch SVG (pure, tested).

use jpdict::{DictEntry, Furigana, Pitch};

/// The bundled note type (anki/ folder), compiled in.
pub fn note_type() -> crate::anki::NoteType {
    // stores-guard: ok compiled-in note type definition (read-only), not a store
    const FIELDS_JSON: &str = include_str!("../../anki/fields.json");
    let fields: Vec<String> = serde_json::from_str(FIELDS_JSON).unwrap_or_default();
    crate::anki::NoteType {
        fields,
        front: include_str!("../../anki/front.html").to_string(),
        back: include_str!("../../anki/back.html").to_string(),
        css: include_str!("../../anki/style.css").to_string(),
        version: include_str!("../../anki/version.txt")
            .trim()
            .parse()
            .unwrap_or(1),
    }
}

/// Escape text for HTML (subtitle text may contain `<`, `&`, quotes).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Brackets would turn subtitle text into Anki furigana or a fake [sound:...] tag.
            '[' => out.push_str("&#91;"),
            ']' => out.push_str("&#93;"),
            '\n' => out.push_str("<br>"),
            _ => out.push(c),
        }
    }
    out
}

/// Furigana segments in Anki syntax: `諦[あきら]める`. A space goes before every ruby group that
/// follows other text, because Anki's filter takes everything back to the previous space.
pub fn anki_furigana(segs: &[Furigana]) -> String {
    let mut out = String::new();
    for s in segs {
        match &s.ruby {
            Some(r) => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&escape(&s.text));
                out.push('[');
                out.push_str(&escape(r));
                out.push(']');
            }
            None => out.push_str(&escape(&s.text)),
        }
    }
    out
}

/// The sentence with the target (code points `start..start+len`) wrapped in `<b>`.
pub fn bold_sentence(text: &str, start: usize, len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = (
        start.min(chars.len()),
        start.saturating_add(len).min(chars.len()),
    );
    let part = |x: &[char]| escape(&x.iter().collect::<String>());
    format!(
        "{}<b>{}</b>{}",
        part(&chars[..a]),
        part(&chars[a..b]),
        part(&chars[b..])
    )
}

/// One tokenizer unit with its reading, as needed for sentence furigana.
pub struct Unit {
    pub start: usize,
    pub len: usize,
    pub text: String,
    pub reading: Option<String>,
}

fn has_kanji(s: &str) -> bool {
    s.chars().any(jptext::is_kanji)
}

/// Sentence furigana in Anki syntax with `<b>` around the units covering the target range.
pub fn sentence_furigana(units: &[Unit], start: usize, len: usize) -> String {
    let end = start.saturating_add(len);
    let mut out = String::new();
    let mut open = false;
    for u in units {
        let u_end = u.start.saturating_add(u.len);
        if !open && u_end > start && u.start < end {
            out.push_str("<b>");
            open = true;
        }
        let body = match &u.reading {
            Some(r) if has_kanji(&u.text) => {
                let segs = jpdict::furigana(&u.text, r);
                // Every ruby group needs a space before it, also at a unit start that follows text.
                let f = anki_furigana(&segs);
                if segs.first().is_some_and(|s| s.ruby.is_some()) && !out.is_empty() {
                    format!(" {f}")
                } else {
                    f
                }
            }
            _ => escape(&u.text),
        };
        out.push_str(&body);
        if open && u_end >= end {
            out.push_str("</b>");
            open = false;
        }
    }
    if open {
        out.push_str("</b>");
    }
    out
}

/// Meanings as an ordered list; `senses` = indexes to keep (empty = all).
pub fn meaning_html(entry: &DictEntry, senses: &[usize]) -> String {
    let items: Vec<String> = entry
        .senses
        .iter()
        .enumerate()
        .filter(|(i, _)| senses.is_empty() || senses.contains(i))
        .map(|(_, s)| format!("<li>{}</li>", escape(&s.glosses.join("; "))))
        .collect();
    format!("<ol>{}</ol>", items.concat())
}

/// A small pitch graph as inline SVG using `currentColor` (the card colours it by pattern):
/// a dot per mora (high or low) and a hollow dot for the following particle.
pub fn pitch_svg(p: &Pitch) -> String {
    let n = p.morae.len();
    if n == 0 {
        return String::new();
    }
    let high = |i: usize| -> bool {
        // i in 0..=n (n = the particle). Heiban: low, then high to the end incl. particle.
        match p.downstep as usize {
            0 => i > 0,
            1 => i == 0,
            d => i > 0 && i < d,
        }
    };
    let (step, x0, hi, lo) = (22.0, 8.0, 6.0, 20.0);
    let pts: Vec<(f64, f64)> = (0..=n)
        .map(|i| (x0 + step * i as f64, if high(i) { hi } else { lo }))
        .collect();
    let w = x0 * 2.0 + step * n as f64;
    let line = |a: &[(f64, f64)]| {
        a.iter()
            .map(|(x, y)| format!("{x},{y}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut svg = format!(
        "<svg class=\"pitch\" xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"26\" viewBox=\"0 0 {w} 26\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    svg.push_str(&format!("<polyline points=\"{}\"/>", line(&pts[..n])));
    svg.push_str(&format!(
        "<polyline points=\"{}\" stroke-dasharray=\"3 3\" opacity=\".6\"/>",
        line(&pts[n - 1..])
    ));
    for (x, y) in &pts[..n] {
        svg.push_str(&format!(
            "<circle cx=\"{x}\" cy=\"{y}\" r=\"3.5\" fill=\"currentColor\"/>"
        ));
    }
    let (x, y) = pts[n];
    svg.push_str(&format!(
        "<circle cx=\"{x}\" cy=\"{y}\" r=\"3.5\" opacity=\".6\"/></svg>"
    ));
    svg
}

/// "12:34" (or "1:02:03") for the Source field.
pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// A tag-safe version of a show title ("Sousou no Frieren - 07" → "Sousou_no_Frieren").
pub fn show_tag(title: &str) -> String {
    let base = title.split(" - ").next().unwrap_or(title);
    let t: String = base
        .trim()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == ':' || c == '"' {
                '_'
            } else {
                c
            }
        })
        .collect();
    t.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(text: &str, ruby: Option<&str>) -> Furigana {
        Furigana {
            text: text.into(),
            ruby: ruby.map(Into::into),
        }
    }

    #[test]
    fn word_furigana_in_anki_syntax() {
        assert_eq!(
            anki_furigana(&[f("諦", Some("あきら")), f("める", None)]),
            "諦[あきら]める"
        );
        assert_eq!(
            anki_furigana(&[f("お", None), f("母", Some("かあ")), f("さん", None)]),
            "お 母[かあ]さん"
        );
        assert_eq!(anki_furigana(&[f("カメラ", None)]), "カメラ");
    }

    #[test]
    fn bolds_the_target_and_escapes() {
        assert_eq!(
            bold_sentence("もう諦めないって決めたんだ。", 2, 4),
            "もう<b>諦めない</b>って決めたんだ。"
        );
        assert_eq!(bold_sentence("a<b>&", 0, 1), "<b>a</b>&lt;b&gt;&amp;");
        assert_eq!(bold_sentence("二行\n目", 0, 2), "<b>二行</b><br>目");
    }

    fn u(start: usize, text: &str, reading: Option<&str>) -> Unit {
        Unit {
            start,
            len: text.chars().count(),
            text: text.into(),
            reading: reading.map(Into::into),
        }
    }

    #[test]
    fn sentence_furigana_matches_ankis_filter_rules() {
        let units = [
            u(0, "もう", Some("もう")),
            u(2, "諦めない", Some("あきらめない")),
            u(6, "って", Some("って")),
            u(8, "決めた", Some("きめた")),
            u(11, "ん", Some("ん")),
            u(12, "だ", Some("だ")),
            u(13, "。", None),
        ];
        assert_eq!(
            sentence_furigana(&units, 2, 4),
            "もう<b> 諦[あきら]めない</b>って 決[き]めたんだ。"
        );
        // Target at the very start: no leading space needed before the first ruby.
        let units2 = [u(0, "夕焼け", Some("ゆうやけ")), u(3, "が", Some("が"))];
        let s = sentence_furigana(&units2, 0, 3);
        assert!(
            s.starts_with("<b>") && s.contains("[ゆう") && s.ends_with("</b>が"),
            "{s}"
        );
    }

    #[test]
    fn pitch_graph_shapes() {
        let p = |d, morae: &[&str]| Pitch {
            downstep: d,
            pattern: String::new(),
            morae: morae.iter().map(|m| m.to_string()).collect(),
        };
        let naka = pitch_svg(&p(4, &["あ", "き", "ら", "め", "る"]));
        assert!(
            naka.starts_with("<svg")
                && naka.contains("currentColor")
                && naka.matches("<circle").count() == 6
        );
        // あ low, き ら め high, る low, particle low.
        assert!(
            naka.contains("points=\"8,20 30,6 52,6 74,6 96,20\""),
            "{naka}"
        );
        let heiban = pitch_svg(&p(0, &["し", "る"]));
        assert!(
            heiban.contains("points=\"8,20 30,6\"") && heiban.contains("points=\"30,6 52,6\""),
            "{heiban}"
        );
        assert_eq!(pitch_svg(&p(0, &[])), "");
    }

    #[test]
    fn small_helpers() {
        assert_eq!(clock(754_000), "12:34");
        assert_eq!(clock(3_723_000), "1:02:03");
        assert_eq!(show_tag("Sousou no Frieren - 07"), "Sousou_no_Frieren");
        assert_eq!(escape("\"x\""), "&quot;x&quot;");
    }

    #[test]
    fn bundled_note_type_is_complete() {
        let nt = note_type();
        assert!(
            nt.fields.len() >= 13 && nt.fields[0] == "Word",
            "{:?}",
            nt.fields
        );
        assert!(
            nt.front.contains("{{")
                && nt.back.contains("{{")
                && !nt.css.is_empty()
                && nt.version >= 1
        );
        for f in [
            "Word",
            "SentenceFurigana",
            "Screenshot",
            "SentenceAudio",
            "WordAudio",
            "Source",
        ] {
            assert!(nt.fields.iter().any(|x| x == f), "missing field {f}");
        }
    }
}
