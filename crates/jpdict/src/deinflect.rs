//! Deinflection: reduce a conjugated Japanese form to its dictionary form(s).
//!
//! The rule table is written from scratch for this project (it is generated from the regular
//! conjugation patterns below, not copied from another tool). A rule rewrites a suffix
//! (`from` to `to`) and carries two word-class masks:
//!
//! * `inm`: which classes the *current* form must have for the rule to apply. The text as written
//!   has class [`START`]; after a rule it has that rule's `outm`.
//! * `outm`: the class of the result, e.g. [`V1`] (ichidan) or [`ADJI`]. Intermediate classes such
//!   as [`TE`] or [`MASU`] let chains such as 食べさせられなかった work: past, then negative (an
//!   i-adjective form), then passive and causative (ichidan forms).
//!
//! Candidates are only *possible* readings; the dictionary decides which are real by checking that
//! the entry's part of speech ([`pos_mask`]) overlaps the candidate's class.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Bit set of word classes.
pub type Mask = u32;

/// The text as written (before any rule has applied).
pub const START: Mask = 1 << 0;
pub const V1: Mask = 1 << 1;
pub const V5U: Mask = 1 << 2;
pub const V5K: Mask = 1 << 3;
/// 行く (the irregular godan く verb).
pub const V5KS: Mask = 1 << 4;
pub const V5G: Mask = 1 << 5;
pub const V5S: Mask = 1 << 6;
pub const V5T: Mask = 1 << 7;
pub const V5N: Mask = 1 << 8;
pub const V5B: Mask = 1 << 9;
pub const V5M: Mask = 1 << 10;
pub const V5R: Mask = 1 << 11;
/// いらっしゃる, くださる (godan with い in the masu stem).
pub const V5ARU: Mask = 1 << 12;
pub const VS: Mask = 1 << 13;
pub const VK: Mask = 1 << 14;
pub const ADJI: Mask = 1 << 15;
pub const ADJNA: Mask = 1 << 16;
/// A form ending in て/で.
pub const TE: Mask = 1 << 17;
/// A form ending in ます.
pub const MASU: Mask = 1 << 18;
/// A noun that takes する (JMdict `vs`): 勉強 for 勉強した.
pub const VSN: Mask = 1 << 19;
/// Any other entry (nouns, adverbs, particles...).
pub const OTHER: Mask = 1 << 20;

pub const V5ALL: Mask = V5U | V5K | V5KS | V5G | V5S | V5T | V5N | V5B | V5M | V5R | V5ARU;

/// Word-class mask of a dictionary entry from its raw JMdict part-of-speech tags.
pub fn pos_mask<'a>(tags: impl IntoIterator<Item = &'a str>) -> Mask {
    let mut m = OTHER;
    for t in tags {
        m |= match t {
            "v1" | "v1-s" | "vz" => V1,
            "v5u" | "v5u-s" => V5U,
            "v5k" => V5K,
            "v5k-s" => V5KS,
            "v5g" => V5G,
            "v5s" => V5S,
            "v5t" => V5T,
            "v5n" => V5N,
            "v5b" => V5B,
            "v5m" => V5M,
            "v5r" | "v5r-i" => V5R,
            "v5aru" => V5ARU | V5R,
            "vs-i" | "vs-s" | "vs-c" => VS,
            "vs" => VSN,
            "vk" => VK,
            "adj-i" | "adj-ix" => ADJI,
            "adj-na" => ADJNA,
            _ => 0,
        };
    }
    m
}

/// One suffix rewriting rule.
#[derive(Debug, Clone)]
pub struct Rule {
    pub from: String,
    pub to: String,
    pub inm: Mask,
    pub outm: Mask,
    /// Conjugation steps this rule undoes, innermost first.
    pub names: &'static [&'static str],
}

/// A possible dictionary form of the input and how it got there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub word: String,
    /// Class of `word` after the last rule ([`START`] when no rule was applied).
    pub mask: Mask,
    /// Conjugation steps, innermost first (the last one is the form as written).
    pub chain: Vec<&'static str>,
}

struct Table {
    rules: Vec<Rule>,
    /// Rules by last character of `from`.
    by_last: HashMap<char, Vec<usize>>,
    /// Rules with an empty `from`.
    empty: Vec<usize>,
}

const MAX_DEPTH: usize = 7;
const MAX_CANDIDATES: usize = 400;

/// (u, a, i, e, o, te, ta, class) for each godan verb ending.
type GodanRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Mask,
);
const GODAN: [GodanRow; 11] = [
    ("う", "わ", "い", "え", "お", "って", "った", V5U),
    ("く", "か", "き", "け", "こ", "いて", "いた", V5K),
    ("く", "か", "き", "け", "こ", "って", "った", V5KS),
    ("ぐ", "が", "ぎ", "げ", "ご", "いで", "いだ", V5G),
    ("す", "さ", "し", "せ", "そ", "して", "した", V5S),
    ("つ", "た", "ち", "て", "と", "って", "った", V5T),
    ("ぬ", "な", "に", "ね", "の", "んで", "んだ", V5N),
    ("ぶ", "ば", "び", "べ", "ぼ", "んで", "んだ", V5B),
    ("む", "ま", "み", "め", "も", "んで", "んだ", V5M),
    ("る", "ら", "り", "れ", "ろ", "って", "った", V5R),
    ("る", "ら", "い", "れ", "ろ", "って", "った", V5ARU),
];

struct Builder {
    rules: Vec<Rule>,
}

impl Builder {
    fn add(&mut self, from: &str, to: &str, inm: Mask, outm: Mask, names: &'static [&'static str]) {
        self.rules.push(Rule {
            from: from.to_string(),
            to: to.to_string(),
            inm,
            outm,
            names,
        });
    }

    /// The forms shared by every verb class, given how the stems are spelled.
    /// `base` is the dictionary-form ending, the others are what replaces it in the surface form.
    #[allow(clippy::too_many_arguments)]
    fn verb_common(&mut self, base: &str, cls: Mask, a: &str, i: &str, e: &str, o: &str) {
        let s = START;
        let f = |x: &str, y: &str| format!("{x}{y}");
        self.add(&f(a, "ない"), base, s | ADJI, cls, &["negative"]);
        self.add(&f(a, "ないで"), base, s, cls, &["negative", "te-form"]);
        self.add(&f(a, "ず"), base, s, cls, &["negative (zu)"]);
        self.add(
            &f(a, "ずに"),
            base,
            s,
            cls,
            &["negative (zu)", "without doing"],
        );
        self.add(&f(a, "ぬ"), base, s, cls, &["negative (nu)"]);
        self.add(&f(i, "ます"), base, s | MASU, cls, &["polite"]);
        self.add(i, base, s, cls, &["masu stem"]);
        self.add(&f(i, "たい"), base, s | ADJI, cls, &["want"]);
        self.add(&f(i, "そう"), base, s, cls, &["looks like"]);
        self.add(&f(i, "すぎる"), base, s | V1, cls, &["excessive"]);
        self.add(&f(i, "ながら"), base, s, cls, &["while"]);
        self.add(&f(i, "なさい"), base, s, cls, &["polite command"]);
        self.add(&f(o, "う"), base, s, cls, &["volitional"]);
        self.add(&f(e, "ば"), base, s, cls, &["conditional (ba)"]);
    }

    fn build(mut self) -> Table {
        let s = START;

        // ---- ichidan --------------------------------------------------------------------
        self.verb_common("る", V1, "", "", "れ", "よ");
        self.add("た", "る", s, V1, &["past"]);
        self.add("て", "る", s | TE, V1, &["te-form"]);
        self.add("ろ", "る", s, V1, &["imperative"]);
        self.add("よ", "る", s, V1, &["imperative"]);
        self.add("くれ", "くれる", s, V1, &["imperative"]);
        self.add("られる", "る", s | V1, V1, &["potential or passive"]);
        self.add("れる", "る", s | V1, V1, &["potential"]);
        self.add("させる", "る", s | V1, V1, &["causative"]);
        self.add("させられる", "る", s | V1, V1, &["causative", "passive"]);
        self.add("さす", "る", s | V1, V1, &["causative"]);

        // ---- godan ----------------------------------------------------------------------
        for (u, a, i, e, o, te, ta, cls) in GODAN {
            self.verb_common(u, cls, a, i, e, o);
            self.add(ta, u, s, cls, &["past"]);
            self.add(te, u, s | TE, cls, &["te-form"]);
            self.add(e, u, s, cls, &["imperative"]);
            self.add(&format!("{e}る"), u, s | V1, cls, &["potential"]);
            self.add(&format!("{a}れる"), u, s | V1, cls, &["passive"]);
            self.add(&format!("{a}せる"), u, s | V1, cls, &["causative"]);
            self.add(&format!("{a}す"), u, s | V1, cls, &["causative"]);
            self.add(
                &format!("{a}せられる"),
                u,
                s | V1,
                cls,
                &["causative", "passive"],
            );
            self.add(
                &format!("{a}される"),
                u,
                s | V1,
                cls,
                &["causative", "passive"],
            );
        }
        // 問う, 乞う: te/ta forms keep う.
        self.add("うて", "う", s | TE, V5U, &["te-form"]);
        self.add("うた", "う", s, V5U, &["past"]);
        // くださる, いらっしゃる: imperative and polite with い.
        self.add("い", "る", s, V5ARU, &["imperative"]);

        // ---- する -----------------------------------------------------------------------
        let suru: &[(&str, Mask, &[&'static str])] = &[
            ("した", s, &["past"]),
            ("して", s | TE, &["te-form"]),
            ("しない", s | ADJI, &["negative"]),
            ("しないで", s, &["negative", "te-form"]),
            ("せず", s, &["negative (zu)"]),
            ("せぬ", s, &["negative (nu)"]),
            ("します", s | MASU, &["polite"]),
            ("し", s, &["masu stem"]),
            ("したい", s | ADJI, &["want"]),
            ("しそう", s, &["looks like"]),
            ("しすぎる", s | V1, &["excessive"]),
            ("しながら", s, &["while"]),
            ("しなさい", s, &["polite command"]),
            ("しよう", s, &["volitional"]),
            ("しろ", s, &["imperative"]),
            ("せよ", s, &["imperative"]),
            ("すれば", s, &["conditional (ba)"]),
            ("される", s | V1, &["passive"]),
            ("できる", s | V1, &["potential"]),
            ("させる", s | V1, &["causative"]),
            ("させられる", s | V1, &["causative", "passive"]),
        ];
        for (from, inm, names) in suru {
            self.add(from, "する", *inm, VS, names);
        }

        // ---- 来る (kana and kanji spellings) -----------------------------------------------
        for (pre_a, pre_i, pre_e, base) in
            [("こ", "き", "くれ", "くる"), ("来", "来", "来れ", "来る")]
        {
            let kuru: &[(&str, &str, Mask, &[&'static str])] = &[
                (pre_i, "た", s, &["past"]),
                (pre_i, "て", s | TE, &["te-form"]),
                (pre_a, "ない", s | ADJI, &["negative"]),
                (pre_a, "ないで", s, &["negative", "te-form"]),
                (pre_a, "ず", s, &["negative (zu)"]),
                (pre_i, "ます", s | MASU, &["polite"]),
                (pre_i, "", s, &["masu stem"]),
                (pre_i, "たい", s | ADJI, &["want"]),
                (pre_i, "そう", s, &["looks like"]),
                (pre_i, "すぎる", s | V1, &["excessive"]),
                (pre_i, "ながら", s, &["while"]),
                (pre_i, "なさい", s, &["polite command"]),
                (pre_a, "よう", s, &["volitional"]),
                (pre_a, "い", s, &["imperative"]),
                (pre_e, "ば", s, &["conditional (ba)"]),
                (pre_a, "られる", s | V1, &["potential or passive"]),
                (pre_a, "させる", s | V1, &["causative"]),
                (pre_a, "させられる", s | V1, &["causative", "passive"]),
            ];
            for (pre, post, inm, names) in kuru {
                // 来 forms: the stem character is the same for every form, so the kana-less masu stem "来" must not apply alone to bare 来る.
                let from = format!("{pre}{post}");
                if from.is_empty() || (pre_a == "来" && from == "来") {
                    continue;
                }
                self.add(&from, base, *inm, VK, names);
            }
        }

        // ---- i-adjectives (and the adjective-like forms ない, たい) ------------------------
        let adj: &[(&str, Mask, &[&'static str])] = &[
            ("かった", s, &["past"]),
            ("かったら", s, &["conditional (tara)"]),
            ("くて", s, &["te-form"]),
            ("くない", s | ADJI, &["negative"]),
            ("ければ", s, &["conditional (ba)"]),
            ("く", s, &["adverbial"]),
            ("さ", s, &["noun form (sa)"]),
            ("そう", s, &["looks like"]),
            ("さそう", s, &["looks like"]),
            ("くなる", s, &["become"]),
            ("くする", s, &["make"]),
            ("かろう", s, &["volitional"]),
        ];
        for (from, inm, names) in adj {
            let stem_from = from.to_string();
            self.add(&stem_from, "い", *inm, ADJI, names);
        }
        self.add("すぎる", "い", s | V1, ADJI, &["excessive"]);
        // na-adjectives and nouns with さ / そう: 静かさ, 静かそう.
        self.add("さ", "", s, ADJNA, &["noun form (sa)"]);
        self.add("そう", "", s, ADJNA, &["looks like"]);

        // ---- verb + te-form + auxiliary ------------------------------------------------------
        // (surface of the auxiliary, its own class, steps)
        let aux: &[(&str, Mask, &'static [&'static str])] = &[
            ("いる", V1, &["progressive"]),
            ("る", V1, &["progressive"]),
            ("いく", V5KS, &["te-iku"]),
            ("くる", VK, &["te-kuru"]),
            ("みる", V1, &["try doing"]),
            ("ある", V5R, &["resultative"]),
            ("おく", V5K, &["in advance"]),
            ("しまう", V5U, &["completive"]),
            ("あげる", V1, &["do for someone"]),
            ("もらう", V5U, &["have someone do"]),
            ("くれる", V1, &["do for me"]),
            ("やる", V5R, &["do for someone"]),
            ("来る", VK, &["te-kuru"]),
            ("行く", V5KS, &["te-iku"]),
            ("見る", V1, &["try doing"]),
            ("置く", V5K, &["in advance"]),
            ("ください", s, &["request"]),
        ];
        for (aux_s, cls, name) in aux {
            for t in ["て", "で"] {
                self.add(&format!("{t}{aux_s}"), t, s | *cls, TE, name);
            }
        }
        // Colloquial contractions: ちゃう (てしまう), じゃう (でしまう), とく (ておく), どく (でおく).
        self.add("ちゃう", "て", s | V5U, TE, &["completive"]);
        self.add("じゃう", "で", s | V5U, TE, &["completive"]);
        self.add("とく", "て", s | V5K, TE, &["in advance"]);
        self.add("どく", "で", s | V5K, TE, &["in advance"]);

        // ---- polite endings: 食べました -> 食べます, then ます -> る ----------------------------------
        self.add("ました", "ます", s, MASU, &["past"]);
        self.add("ません", "ます", s, MASU, &["negative"]);
        self.add("ませんでした", "ます", s, MASU, &["negative", "past"]);
        self.add("ましょう", "ます", s, MASU, &["volitional"]);
        self.add("ませ", "ます", s, MASU, &["imperative"]);

        // ---- -tara / -tari / negative te ----------------------------------------------------------
        self.add("たら", "た", s, s, &["conditional (tara)"]);
        self.add("だら", "だ", s, s, &["conditional (tara)"]);
        self.add("たり", "た", s, s, &["tari"]);
        self.add("だり", "だ", s, s, &["tari"]);
        self.add("ないで", "ない", s, ADJI, &["te-form"]);

        let mut by_last: HashMap<char, Vec<usize>> = HashMap::new();
        let mut empty = Vec::new();
        for (idx, r) in self.rules.iter().enumerate() {
            match r.from.chars().next_back() {
                Some(c) => by_last.entry(c).or_default().push(idx),
                None => empty.push(idx),
            }
        }
        Table {
            rules: self.rules,
            by_last,
            empty,
        }
    }
}

static TABLE: LazyLock<Table> = LazyLock::new(|| Builder { rules: Vec::new() }.build());

/// The rule table (for tests and diagnostics).
pub fn rules() -> &'static [Rule] {
    &TABLE.rules
}

/// Steps that are really an auxiliary verb after a te-form; the te-form step itself is then redundant.
const AUX_STEPS: &[&str] = &[
    "progressive",
    "te-iku",
    "te-kuru",
    "try doing",
    "resultative",
    "in advance",
    "completive",
    "do for someone",
    "have someone do",
    "do for me",
    "request",
];

/// Drop steps that only restate the next one: 食べたら is "tara" (not "past, tara"), 食べている is "progressive" (not "te-form, progressive").
fn tidy(steps: Vec<&'static str>) -> Vec<&'static str> {
    let mut out = Vec::with_capacity(steps.len());
    for (i, &n) in steps.iter().enumerate() {
        let next = steps.get(i + 1).copied();
        if (n == "te-form" && next.is_some_and(|x| AUX_STEPS.contains(&x)))
            || (n == "past" && matches!(next, Some("conditional (tara)" | "tari")))
        {
            continue;
        }
        out.push(n);
    }
    out
}

/// Possible dictionary forms of `text`, the text itself first. Breadth-first, so shorter chains come first.
pub fn deinflect(text: &str) -> Vec<Candidate> {
    let mut out = vec![Candidate {
        word: text.to_string(),
        mask: START,
        chain: Vec::new(),
    }];
    let mut seen: HashSet<(String, Mask)> = HashSet::new();
    seen.insert((text.to_string(), START));
    let mut frontier: Vec<(String, Mask, Vec<usize>)> = vec![(text.to_string(), START, Vec::new())];
    let t = &*TABLE;

    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for (word, mask, path) in &frontier {
            let Some(last) = word.chars().next_back() else {
                continue;
            };
            let by_last = t.by_last.get(&last).into_iter().flatten();
            for &ri in by_last.chain(t.empty.iter()) {
                let r = &t.rules[ri];
                if r.inm & mask == 0 {
                    continue;
                }
                let Some(stem) = word.strip_suffix(r.from.as_str()) else {
                    continue;
                };
                if stem.is_empty() && r.to.chars().count() < 2 {
                    continue;
                }
                let new = format!("{stem}{}", r.to);
                if new.is_empty() || !seen.insert((new.clone(), r.outm)) {
                    continue;
                }
                let mut p = path.clone();
                p.push(ri);
                // Names: the rules were applied outermost first, so reverse to innermost first.
                let chain = tidy(
                    p.iter()
                        .rev()
                        .flat_map(|&i| t.rules[i].names.iter().copied())
                        .collect(),
                );
                out.push(Candidate {
                    word: new.clone(),
                    mask: r.outm,
                    chain,
                });
                next.push((new, r.outm, p));
            }
        }
        if next.is_empty() || out.len() >= MAX_CANDIDATES {
            break;
        }
        frontier = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// True when deinflecting `surface` yields `base` with exactly `chain` and a class overlapping `class`.
    fn has(surface: &str, base: &str, chain: &[&str], class: Mask) -> bool {
        deinflect(surface)
            .iter()
            .any(|c| c.word == base && c.chain == chain && c.mask & class != 0)
    }

    #[track_caller]
    fn check(surface: &str, base: &str, chain: &[&str], class: Mask) {
        assert!(
            has(surface, base, chain, class),
            "{surface} -> {base} {chain:?} not found; got {:?}",
            deinflect(surface)
                .iter()
                .map(|c| (c.word.clone(), c.chain.clone()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn sample_lines() {
        check("諦めない", "諦める", &["negative"], V1);
        check("決めた", "決める", &["past"], V1);
        check(
            "食べさせられなかった",
            "食べる",
            &["causative", "passive", "negative", "past"],
            V1,
        );
        check("知らなかった", "知る", &["negative", "past"], V5R);
        check("行こう", "行く", &["volitional"], V5KS);
        check("持ってきた", "持つ", &["te-kuru", "past"], V5T);
        check("撮ろう", "撮る", &["volitional"], V5R);
        check("忘れないで", "忘れる", &["negative", "te-form"], V1);
        check("会おう", "会う", &["volitional"], V5U);
    }

    #[test]
    fn copula_is_not_part_of_the_word() {
        // 綺麗だ: だ is a separate word, so the scan finds 綺麗 first; deinflecting 綺麗だ must not crash or invent a verb.
        let c = deinflect("綺麗だ");
        assert_eq!(c[0].word, "綺麗だ");
        assert!(c.iter().all(|c| c.mask & V5ALL == 0 || c.word != "綺麗"));
    }

    #[test]
    fn unknown_does_not_crash() {
        for s in [
            "すっげー",
            "",
            "ー",
            "あ",
            "ん",
            "ぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁぁ",
            "abc",
            "😀",
        ] {
            let c = deinflect(s);
            assert!(c.len() <= MAX_CANDIDATES + 100);
        }
        assert!(deinflect("すっげー").iter().all(|c| !c.word.is_empty()));
    }

    #[test]
    fn ichidan_forms() {
        check("食べない", "食べる", &["negative"], V1);
        check("食べなかった", "食べる", &["negative", "past"], V1);
        check("食べて", "食べる", &["te-form"], V1);
        check("食べた", "食べる", &["past"], V1);
        check("食べます", "食べる", &["polite"], V1);
        check("食べました", "食べる", &["polite", "past"], V1);
        check("食べません", "食べる", &["polite", "negative"], V1);
        check(
            "食べませんでした",
            "食べる",
            &["polite", "negative", "past"],
            V1,
        );
        check("食べましょう", "食べる", &["polite", "volitional"], V1);
        check("食べよう", "食べる", &["volitional"], V1);
        check("食べられる", "食べる", &["potential or passive"], V1);
        check("食べれる", "食べる", &["potential"], V1);
        check("食べさせる", "食べる", &["causative"], V1);
        check("食べろ", "食べる", &["imperative"], V1);
        check("食べれば", "食べる", &["conditional (ba)"], V1);
        check("食べたら", "食べる", &["conditional (tara)"], V1);
        check("食べたい", "食べる", &["want"], V1);
        check("食べたくない", "食べる", &["want", "negative"], V1);
        check("食べたかった", "食べる", &["want", "past"], V1);
        check("食べそう", "食べる", &["looks like"], V1);
        check("食べすぎる", "食べる", &["excessive"], V1);
        check("食べず", "食べる", &["negative (zu)"], V1);
        check("食べぬ", "食べる", &["negative (nu)"], V1);
        check("食べ", "食べる", &["masu stem"], V1);
        check("食べている", "食べる", &["progressive"], V1);
        check("食べてる", "食べる", &["progressive"], V1);
        check("食べていた", "食べる", &["progressive", "past"], V1);
        check("食べてた", "食べる", &["progressive", "past"], V1);
        check("食べちゃった", "食べる", &["completive", "past"], V1);
        check("食べとく", "食べる", &["in advance"], V1);
        check(
            "食べさせられた",
            "食べる",
            &["causative", "passive", "past"],
            V1,
        );
        check("食べなさい", "食べる", &["polite command"], V1);
        check("食べながら", "食べる", &["while"], V1);
        check("食べないで", "食べる", &["negative", "te-form"], V1);
    }

    #[test]
    fn godan_forms() {
        check("書かない", "書く", &["negative"], V5K);
        check("書いた", "書く", &["past"], V5K);
        check("書いて", "書く", &["te-form"], V5K);
        check("書きます", "書く", &["polite"], V5K);
        check("書こう", "書く", &["volitional"], V5K);
        check("書ける", "書く", &["potential"], V5K);
        check("書かれる", "書く", &["passive"], V5K);
        check("書かせる", "書く", &["causative"], V5K);
        check(
            "書かせられた",
            "書く",
            &["causative", "passive", "past"],
            V5K,
        );
        check("書け", "書く", &["imperative"], V5K);
        check("書けば", "書く", &["conditional (ba)"], V5K);
        check("書きたい", "書く", &["want"], V5K);
        check("泳いだ", "泳ぐ", &["past"], V5G);
        check("泳いで", "泳ぐ", &["te-form"], V5G);
        check("話した", "話す", &["past"], V5S);
        check("話さない", "話す", &["negative"], V5S);
        check("死んだ", "死ぬ", &["past"], V5N);
        check("遊んで", "遊ぶ", &["te-form"], V5B);
        check("飲んだ", "飲む", &["past"], V5M);
        check("買った", "買う", &["past"], V5U);
        check("買わなかった", "買う", &["negative", "past"], V5U);
        check("待った", "待つ", &["past"], V5T);
        check("帰って", "帰る", &["te-form"], V5R);
        check("行った", "行く", &["past"], V5KS);
        check("行って", "行く", &["te-form"], V5KS);
        check("行かない", "行く", &["negative"], V5KS);
        check("読んでる", "読む", &["progressive"], V5M);
        check("読んでいた", "読む", &["progressive", "past"], V5M);
        check("読んじゃう", "読む", &["completive"], V5M);
        check("読んどく", "読む", &["in advance"], V5M);
        check("書いてしまった", "書く", &["completive", "past"], V5K);
        check("いらっしゃいます", "いらっしゃる", &["polite"], V5ARU);
    }

    #[test]
    fn irregular_verbs() {
        check("した", "する", &["past"], VS);
        check("しない", "する", &["negative"], VS);
        check("勉強した", "勉強する", &["past"], VS);
        check("しよう", "する", &["volitional"], VS);
        check("される", "する", &["passive"], VS);
        check("できる", "する", &["potential"], VS);
        check("来た", "来る", &["past"], VK);
        check("きた", "くる", &["past"], VK);
        check("こない", "くる", &["negative"], VK);
        check("来ない", "来る", &["negative"], VK);
        check("来ます", "来る", &["polite"], VK);
        check("来て", "来る", &["te-form"], VK);
        check("こよう", "くる", &["volitional"], VK);
        check("来られる", "来る", &["potential or passive"], VK);
        check("来い", "来る", &["imperative"], VK);
        check("持って来た", "持つ", &["te-kuru", "past"], V5T);
    }

    #[test]
    fn adjectives() {
        check("高くない", "高い", &["negative"], ADJI);
        check("高かった", "高い", &["past"], ADJI);
        check("高くなかった", "高い", &["negative", "past"], ADJI);
        check("高くて", "高い", &["te-form"], ADJI);
        check("高ければ", "高い", &["conditional (ba)"], ADJI);
        check("高く", "高い", &["adverbial"], ADJI);
        check("高さ", "高い", &["noun form (sa)"], ADJI);
        check("高そう", "高い", &["looks like"], ADJI);
        check("高すぎる", "高い", &["excessive"], ADJI);
        check("高くなる", "高い", &["become"], ADJI);
        check("よかった", "よい", &["past"], ADJI);
        check("なさそう", "ない", &["looks like"], ADJI);
        check("静かさ", "静か", &["noun form (sa)"], ADJNA);
    }

    #[test]
    fn unchanged_text_is_first_candidate() {
        let c = deinflect("夕焼け");
        assert_eq!(
            c[0],
            Candidate {
                word: "夕焼け".into(),
                mask: START,
                chain: vec![]
            }
        );
    }

    #[test]
    fn pos_masks() {
        assert_eq!(pos_mask(["v1", "vt"]) & V1, V1);
        assert_eq!(pos_mask(["n"]) & (V1 | V5ALL), 0);
        assert_ne!(pos_mask(["vs", "n"]) & VSN, 0);
        assert_ne!(pos_mask(["v5aru"]) & V5R, 0);
        assert_ne!(pos_mask(["adj-ix"]) & ADJI, 0);
    }

    #[test]
    fn table_is_sane() {
        assert!(rules().len() > 300, "{}", rules().len());
        assert!(
            rules()
                .iter()
                .all(|r| r.from != r.to && !r.names.is_empty())
        );
    }
}
