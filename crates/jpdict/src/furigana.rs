//! Furigana: align the kanji runs of a written form with its kana reading.
//!
//! A kanji run (e.g. 夕焼) gets one ruby over the whole run (ゆうや); kana in the term (け) never goes
//! into ruby text. If the alignment is not unique, only the kana prefix/suffix that surely matches is
//! split off and the middle gets one ruby.

use crate::kana::{is_kanji, to_hiragana};
use crate::types::Furigana;

/// Kanji-run characters: ideographs, plus ヶ/ヵ as in 三ヶ月.
fn is_run_char(c: char) -> bool {
    is_kanji(c) || c == 'ヶ' || c == 'ヵ'
}

/// Split `term` into (is_kanji_run, text) runs.
fn runs(term: &str) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = Vec::new();
    for c in term.chars() {
        let k = is_run_char(c);
        match out.last_mut() {
            Some((lk, s)) if *lk == k => s.push(c),
            _ => out.push((k, c.to_string())),
        }
    }
    out
}

/// Furigana segments for `term` read as `reading`.
pub fn furigana(term: &str, reading: &str) -> Vec<Furigana> {
    let plain = |t: &str| Furigana {
        text: t.to_string(),
        ruby: None,
    };
    if term.is_empty() {
        return Vec::new();
    }
    if !term.chars().any(is_run_char) || reading.is_empty() {
        return vec![plain(term)];
    }
    let runs = runs(term);
    let rd_orig: Vec<char> = reading.chars().collect();
    let rd: Vec<char> = to_hiragana(reading).chars().collect();
    let kana_runs: Vec<Vec<char>> = runs
        .iter()
        .map(|(k, s)| {
            if *k {
                Vec::new()
            } else {
                to_hiragana(s).chars().collect()
            }
        })
        .collect();

    // Minimum reading length needed from run i onwards.
    let mut need = vec![0usize; runs.len() + 1];
    for i in (0..runs.len()).rev() {
        need[i] = need[i + 1] + if runs[i].0 { 1 } else { kana_runs[i].len() };
    }

    let mut solutions: Vec<Vec<usize>> = Vec::new();
    let mut cuts: Vec<usize> = Vec::new(); // reading position where each run starts
    align(
        &runs,
        &kana_runs,
        &rd,
        &need,
        0,
        0,
        &mut cuts,
        &mut solutions,
    );

    if solutions.len() == 1 {
        let cuts = &solutions[0];
        let mut out = Vec::new();
        for (i, (k, text)) in runs.iter().enumerate() {
            let end = cuts.get(i + 1).copied().unwrap_or(rd.len());
            if *k {
                out.push(Furigana {
                    text: text.clone(),
                    ruby: Some(rd_orig[cuts[i]..end].iter().collect()),
                });
            } else {
                out.push(plain(text));
            }
        }
        return out;
    }

    // Ambiguous or inconsistent: split off a matching kana prefix and suffix, one ruby over the middle.
    let mut first = 0usize; // runs[..first] are leading kana
    let mut lead = 0usize; // reading chars consumed by them
    if !runs[0].0 && rd.starts_with(&kana_runs[0]) {
        first = 1;
        lead = kana_runs[0].len();
    }
    let mut last = runs.len(); // runs[last..] are trailing kana
    let mut trail = 0usize;
    if last > first && !runs[last - 1].0 {
        let k = &kana_runs[last - 1];
        if rd.len() > lead + k.len() && rd.ends_with(k) {
            last -= 1;
            trail = k.len();
        }
    }
    if first >= last || lead + trail >= rd.len() {
        return vec![Furigana {
            text: term.to_string(),
            ruby: Some(reading.to_string()),
        }];
    }
    let mut out = Vec::new();
    if first == 1 {
        out.push(plain(&runs[0].1));
    }
    let mid: String = runs[first..last].iter().map(|(_, s)| s.as_str()).collect();
    out.push(Furigana {
        text: mid,
        ruby: Some(rd_orig[lead..rd.len() - trail].iter().collect()),
    });
    if last < runs.len() {
        out.push(plain(&runs[last].1));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn align(
    runs: &[(bool, String)],
    kana_runs: &[Vec<char>],
    rd: &[char],
    need: &[usize],
    i: usize,
    pos: usize,
    cuts: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if out.len() > 1 {
        return; // already ambiguous
    }
    if i == runs.len() {
        if pos == rd.len() {
            out.push(cuts.clone());
        }
        return;
    }
    cuts.push(pos);
    if runs[i].0 {
        // A kanji run takes between 1 and (rest - what later runs need) kana.
        let max = rd.len().saturating_sub(pos + need[i + 1]);
        for l in 1..=max {
            align(runs, kana_runs, rd, need, i + 1, pos + l, cuts, out);
        }
    } else {
        let k = &kana_runs[i];
        if rd.len() >= pos + k.len() && rd[pos..pos + k.len()] == k[..] {
            align(runs, kana_runs, rd, need, i + 1, pos + k.len(), cuts, out);
        }
    }
    cuts.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(term: &str, reading: &str) -> Vec<(String, Option<String>)> {
        furigana(term, reading)
            .into_iter()
            .map(|s| (s.text, s.ruby))
            .collect()
    }
    fn seg(t: &str, r: Option<&str>) -> (String, Option<String>) {
        (t.to_string(), r.map(str::to_string))
    }

    #[test]
    fn okurigana() {
        assert_eq!(
            f("諦める", "あきらめる"),
            [seg("諦", Some("あきら")), seg("める", None)]
        );
        assert_eq!(
            f("食べる", "たべる"),
            [seg("食", Some("た")), seg("べる", None)]
        );
        assert_eq!(
            f("買い物", "かいもの"),
            [
                seg("買", Some("か")),
                seg("い", None),
                seg("物", Some("もの"))
            ]
        );
    }

    #[test]
    fn kanji_run_with_trailing_kana() {
        assert_eq!(
            f("夕焼け", "ゆうやけ"),
            [seg("夕焼", Some("ゆうや")), seg("け", None)]
        );
    }

    #[test]
    fn all_kanji_and_all_kana() {
        assert_eq!(f("綺麗", "きれい"), [seg("綺麗", Some("きれい"))]);
        assert_eq!(f("ありがとう", "ありがとう"), [seg("ありがとう", None)]);
        assert_eq!(f("", "あ"), []);
    }

    #[test]
    fn kana_never_in_ruby() {
        for (t, r) in [
            ("振り返り", "ふりかえり"),
            ("お願いします", "おねがいします"),
            ("言い出しっぺ", "いいだしっぺ"),
            ("取り扱い", "とりあつかい"),
        ] {
            for s in furigana(t, r) {
                if let Some(ruby) = s.ruby {
                    assert!(
                        s.text.chars().all(is_run_char),
                        "{t}: ruby over kana in {:?}",
                        s.text
                    );
                    assert!(!ruby.is_empty());
                }
            }
            let joined: String = furigana(t, r).iter().map(|s| s.text.as_str()).collect();
            assert_eq!(joined, t);
        }
    }

    #[test]
    fn leading_kana_and_katakana_reading() {
        assert_eq!(
            f("お願い", "おねがい"),
            [seg("お", None), seg("願", Some("ねが")), seg("い", None)]
        );
        assert_eq!(f("今日", "キョウ"), [seg("今日", Some("キョウ"))]);
        assert_eq!(
            f("三ヶ月", "さんかげつ"),
            [seg("三ヶ月", Some("さんかげつ"))]
        );
    }

    #[test]
    fn ambiguous_falls_back_to_whole_or_trimmed() {
        // The kana "い" appears twice in the reading, so the alignment is ambiguous: one ruby over the kanji.
        let r = f("行い", "おこない");
        assert_eq!(r, [seg("行", Some("おこな")), seg("い", None)]);
        // Reading does not contain the okurigana at all: whole term gets the ruby.
        assert_eq!(f("食べる", "くう"), [seg("食べる", Some("くう"))]);
    }

    #[test]
    fn serialized_shape() {
        let v = serde_json::to_value(furigana("諦める", "あきらめる")).unwrap();
        assert_eq!(v[0]["text"], "諦");
        assert_eq!(v[0]["ruby"], "あきら");
        assert!(v[1].get("ruby").is_none());
    }
}
