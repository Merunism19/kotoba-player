//! Japanese text tools: tokenizing a subtitle line into word units (whole words, readings, base forms) and kana utilities.
//!
//! The tokenizer is Lindera with the embedded UniDic (mecab 2.1.2) dictionary. UniDic cuts text into
//! short morphemes; [`Tokenizer::word_units`] glues them back into the "words" a learner would click.

use std::borrow::Cow;
use std::fmt;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use serde::Serialize;

/// One hoverable piece of a subtitle line. Serialises to the TS `WordUnit` in `src/lib/backend.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordUnit {
    /// Offset in the line, in Unicode code points.
    pub start: usize,
    /// Length in code points.
    pub len: usize,
    pub text: String,
    /// Hiragana reading of the whole unit, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    /// Dictionary form of the head word (知らなかった -> 知る), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// False for punctuation, spaces, symbols and bare numbers: shown but not hoverable.
    pub is_word: bool,
}

/// Tokenizer set-up failure (the embedded dictionary could not be loaded).
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tokenizer error: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// Katakana (and the iteration marks ヽヾ) to hiragana; everything else, including ー, is kept.
pub fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' | '\u{30FD}' | '\u{30FE}' => {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            }
            _ => c,
        })
        .collect()
}

/// Hiragana, katakana (incl. ー and small extensions) and half-width katakana.
pub fn is_kana(c: char) -> bool {
    matches!(c,
        '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309F}' | '\u{30A1}'..='\u{30FA}'
        | '\u{30FC}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}')
}

/// CJK ideographs (incl. extensions A-G, compatibility ideographs and the marks 々 〆 〇).
pub fn is_kanji(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{323AF}' | '\u{3005}' | '\u{3006}' | '\u{3007}')
}

fn is_latin_alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}')
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{FF10}'..='\u{FF19}')
}

/// Kana (other than a lone long-vowel mark), kanji or a Latin letter: something worth looking up.
fn is_word_char(c: char) -> bool {
    (is_kana(c) && c != 'ー') || is_kanji(c) || (is_latin_alnum(c) && !is_digit(c))
}

/// Splits Japanese subtitle lines into word units. `Send + Sync` and immutable after `new`.
pub struct Tokenizer {
    segmenter: Segmenter,
}

impl Tokenizer {
    /// Loads the UniDic dictionary that is compiled into the binary (memory-mapped, well under 1 ms).
    pub fn new() -> Result<Self, Error> {
        let dictionary = load_dictionary("embedded://unidic").map_err(|e| Error(e.to_string()))?;
        Ok(Self {
            segmenter: Segmenter::new(Mode::Normal, dictionary, None),
        })
    }

    /// Splits one subtitle line into word units. The units cover the line in order with no gaps and
    /// concatenate back to exactly `line`; `start`/`len` count Unicode code points.
    ///
    /// # Rules (UniDic morphemes are glued left to right)
    ///
    /// Joined into one unit:
    /// - A verb or i-adjective plus every following auxiliary verb (助動詞: ない, た, ます, れる,
    ///   させる, たい, う, ぬ, ちゃう, てる ...) and verbal/adjectival suffix (らしい, っぽい, げ):
    ///   `知らなかった`, `食べさせられなかった`, `食べてる`, `行っちゃった`.
    /// - The te-form: a て/で after such a chain, and then the helper verbs (いる, しまう, おく,
    ///   ある, くる, いく, みる, あげる, くれる, もらう, やる, くださる and ない/ほしい):
    ///   `食べて`, `持ってきた`, `忘れないで`. The conditional ば after a 仮定形 (`食べれば`) and
    ///   the polite request `お待ちください` (連用形 + ください) join too.
    /// - The copula forms stay one unit with their own endings: `だった`, `だろう`, `でした`.
    /// - An お/ご (or any 接頭辞) prefix with the word that follows: `お願い`, `ご飯`.
    /// - Kanji-only nouns next to each other, but only when UniDic itself knows the joined string as
    ///   one dictionary word (never across particles, never a guess like 明日雨), and a noun with a non-honorific
    ///   suffix (`私たち`, `美しさ`, `３話`). Honorifics (さん, ちゃん, くん, 様...) stay apart.
    /// - A numeral and its counter (`何度`, `二人`), runs of Latin letters/digits (`OK`, `３D`), and
    ///   こんな/そんな/あんな/どんな + に.
    /// - A long-vowel mark ー or a stretched ending after a word: `すっげー`, `ねー`.
    ///
    /// Kept separate: particles (は, が, を, に, って, の, よ, ね, から ...), the nominalising `ん`/`の`,
    /// the copula だ/です/なら after a noun or na-adjective (`綺麗 | だ`, `決めた | ん | だ`),
    /// a noun and the する after it (`勉強 | する`), and all punctuation, symbols and whitespace
    /// (each token its own unit, `is_word = false`).
    ///
    /// `reading` is the hiragana of the whole unit (None if any part is unknown). `base` is the
    /// dictionary form of the head word (`知る`, `食べる`, `綺麗`); for a unit with a prefix it is the
    /// prefix plus the head as written (`お願い`); None for non-words.
    pub fn word_units(&self, line: &str) -> Vec<WordUnit> {
        if line.is_empty() {
            return Vec::new();
        }
        let morphs = self.morphemes(line);
        let groups = group(&morphs, &|word| self.is_dictionary_word(word));
        let mut units = Vec::with_capacity(groups.len());
        let mut start = 0;
        for g in &groups {
            let parts = &morphs[g.first..=g.last];
            let text: String = parts.iter().map(|m| m.surface.as_str()).collect();
            let len = text.chars().count();
            let is_word = text.chars().any(is_word_char);
            let (reading, base) = if is_word {
                (
                    unit_reading(parts, &text),
                    Some(unit_base(&morphs, g, &text)),
                )
            } else {
                (None, None)
            };
            units.push(WordUnit {
                start,
                len,
                text,
                reading,
                base,
                is_word,
            });
            start += len;
        }
        units
    }

    /// True when UniDic knows `word` as a single dictionary entry (not an unknown-word guess).
    fn is_dictionary_word(&self, word: &str) -> bool {
        let Ok(mut tokens) = self.segmenter.segment(Cow::Borrowed(word)) else {
            return false;
        };
        tokens.len() == 1
            && tokens[0].byte_end == word.len()
            && !tokens[0]
                .get_detail(6)
                .is_none_or(|l| l == "*" || l.is_empty())
    }

    /// Lindera morphemes covering the whole line; text the tokenizer skipped (spaces, newlines) becomes
    /// its own morpheme so nothing is lost.
    fn morphemes(&self, line: &str) -> Vec<Morph> {
        let mut out = Vec::new();
        let mut cursor = 0;
        if let Ok(tokens) = self.segmenter.segment(Cow::Borrowed(line)) {
            for mut tok in tokens {
                let (bs, be) = (tok.byte_start, tok.byte_end);
                if bs < cursor
                    || be > line.len()
                    || bs >= be
                    || !line.is_char_boundary(bs)
                    || !line.is_char_boundary(be)
                {
                    continue;
                }
                if bs > cursor {
                    out.push(Morph::bare(&line[cursor..bs]));
                }
                let d: Vec<String> = tok
                    .details()
                    .iter()
                    .map(|s| {
                        if *s == "*" {
                            String::new()
                        } else {
                            (*s).to_string()
                        }
                    })
                    .collect();
                out.push(Morph::from_details(&line[bs..be], &d));
                cursor = be;
            }
        }
        if cursor < line.len() {
            out.push(Morph::bare(&line[cursor..]));
        }
        out
    }
}

/// One UniDic morpheme (fields by index in the mecab 2.1.2 layout).
struct Morph {
    surface: String,
    pos1: String,
    pos2: String,
    pos3: String,
    cform: String,
    lform: String,
    lemma: String,
    pron: String,
    orth_base: String,
}

impl Morph {
    /// A piece without dictionary data (whitespace, tokenizer failure).
    fn bare(surface: &str) -> Self {
        let pos1 = if surface.chars().all(char::is_whitespace) {
            "空白"
        } else {
            "未知語"
        };
        Self::from_details(surface, &[pos1.to_string()])
    }

    fn from_details(surface: &str, d: &[String]) -> Self {
        let f = |i: usize| d.get(i).cloned().unwrap_or_default();
        Self {
            surface: surface.to_string(),
            pos1: f(0),
            pos2: f(1),
            pos3: f(2),
            cform: f(5),
            lform: f(6),
            lemma: f(7),
            pron: f(9),
            orth_base: f(10),
        }
    }

    fn is_latin(&self) -> bool {
        !self.surface.is_empty() && self.surface.chars().all(is_latin_alnum)
    }

    fn is_kanji_noun(&self) -> bool {
        matches!(self.pos1.as_str(), "名詞" | "代名詞") && self.surface.chars().all(is_kanji)
    }

    fn is_copula(&self) -> bool {
        self.pos1 == "助動詞" && (self.lemma == "だ" || self.lemma == "です")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Punctuation, symbols, spaces: never merged.
    Punct,
    /// 接頭辞 waiting for its word.
    Prefix,
    /// Verb / adjective / auxiliary chain that can still take endings.
    Pred,
    /// Noun-like (nouns, numerals, Latin runs): can take suffixes and nouns.
    Noun,
    /// Anything else (particles, adverbs, na-adjectives ...).
    Closed,
}

struct Group {
    first: usize,
    last: usize,
    /// Morpheme the base form comes from (after a prefix).
    head: usize,
    prefix: bool,
    kind: Kind,
    /// The chain just took a て/で and may take helper verbs.
    te: bool,
}

const HONORIFICS: [&str; 10] = [
    "さん",
    "ちゃん",
    "くん",
    "君",
    "様",
    "さま",
    "殿",
    "氏",
    "たん",
    "っち",
];
const DEMONSTRATIVE_NA: [&str; 4] = ["こんな", "そんな", "あんな", "どんな"];
const HELPER_VERBS: [&str; 27] = [
    "居る",
    "いる",
    "しまう",
    "仕舞う",
    "おく",
    "置く",
    "有る",
    "ある",
    "来る",
    "くる",
    "行く",
    "いく",
    "見る",
    "みる",
    "上げる",
    "あげる",
    "呉れる",
    "くれる",
    "貰う",
    "もらう",
    "遣る",
    "やる",
    "下さる",
    "頂く",
    "いただく",
    "見せる",
    "みせる",
];

/// UniDic's default readings that anime dialogue almost never uses (あす, わたくし).
const READING_OVERRIDES: [(&str, &str); 3] =
    [("明日", "あした"), ("私", "わたし"), ("日本", "にほん")];

/// Whole-word readings UniDic gets wrong when it glues family terms from お + 母 + さん.
const UNIT_READING_OVERRIDES: [(&str, &str); 8] = [
    ("お母さん", "おかあさん"),
    ("お父さん", "おとうさん"),
    ("お兄さん", "おにいさん"),
    ("お姉さん", "おねえさん"),
    ("お兄ちゃん", "おにいちゃん"),
    ("お姉ちゃん", "おねえちゃん"),
    ("お爺さん", "おじいさん"),
    ("お婆さん", "おばあさん"),
];

fn starts_kind(m: &Morph) -> Kind {
    match m.pos1.as_str() {
        "接頭辞" => Kind::Prefix,
        "動詞" | "形容詞" => Kind::Pred,
        // Copula as a particle-like に / で / な: never takes endings.
        "助動詞" if matches!(m.surface.as_str(), "に" | "で" | "な") => Kind::Closed,
        // Other auxiliaries and the copula (だ, です) start a chain so だった / でした / だろう stay whole.
        "助動詞" => Kind::Pred,
        "名詞" | "代名詞" => Kind::Noun,
        _ if m.is_latin() => Kind::Noun,
        _ if !m.surface.chars().any(|c| is_word_char(c) || is_digit(c)) => Kind::Punct,
        _ => Kind::Closed,
    }
}

fn group(morphs: &[Morph], is_dict_word: &dyn Fn(&str) -> bool) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (i, m) in morphs.iter().enumerate() {
        if let Some(g) = groups.last_mut()
            && try_merge(g, morphs, i, is_dict_word)
        {
            continue;
        }
        groups.push(Group {
            first: i,
            last: i,
            head: i,
            prefix: false,
            kind: starts_kind(m),
            te: false,
        });
    }
    groups
}

/// Tries to append morpheme `i` to group `g`; updates the group state when it does.
fn try_merge(
    g: &mut Group,
    morphs: &[Morph],
    i: usize,
    is_dict_word: &dyn Fn(&str) -> bool,
) -> bool {
    let m = &morphs[i];
    let prev = &morphs[g.last];
    if g.kind == Kind::Punct {
        return false;
    }
    // Stretched ending: すっげー, ねー.
    if m.surface.chars().all(|c| c == 'ー')
        && m.pos1 == "補助記号"
        && prev.surface.chars().last().is_some_and(is_kana)
    {
        g.last = i;
        g.te = false;
        return true;
    }
    if g.kind == Kind::Prefix {
        return match m.pos1.as_str() {
            "名詞" | "動詞" | "形容詞" | "形状詞" | "代名詞" => {
                g.kind = starts_kind(m);
                g.prefix = true;
                g.head = i;
                g.last = i;
                true
            }
            _ => false,
        };
    }
    if m.pos1 == "接尾辞" {
        // お母さん, お疲れ様: an honorific after an お/ご word is part of it; 山田さん stays two units.
        let honorific = HONORIFICS.contains(&m.surface.as_str()) && !g.prefix;
        let new_kind = match m.pos2.as_str() {
            "動詞的" | "形容詞的" => Some(Kind::Pred),
            "形状詞的" => Some(Kind::Closed),
            "名詞的" if !honorific && g.kind == Kind::Noun => Some(Kind::Noun),
            "名詞的" if g.kind == Kind::Pred && matches!(m.surface.as_str(), "さ" | "み") => {
                Some(Kind::Closed)
            }
            _ => None,
        };
        let Some(k) = new_kind else { return false };
        g.kind = k;
        g.last = i;
        g.te = false;
        return true;
    }
    match g.kind {
        Kind::Pred => merge_pred(g, prev, m, i),
        Kind::Noun => {
            // 何 + 度, 二 + 人, 二 + 十: a numeral takes a counter or another numeral.
            let numeral = prev.pos2 == "数詞"
                && m.pos1 == "名詞"
                && (m.pos2 == "数詞" || m.pos3 == "助数詞可能");
            let joins = numeral
                || (m.is_latin() && prev.is_latin())
                || (m.is_kanji_noun() && prev.is_kanji_noun() && {
                    let joined: String = morphs[g.first..=g.last]
                        .iter()
                        .chain([m])
                        .map(|x| x.surface.as_str())
                        .collect();
                    is_dict_word(&joined)
                });
            if joins {
                g.last = i;
            }
            joins
        }
        Kind::Closed => {
            // こんな + に
            let joins = m.surface == "に"
                && m.lemma == "だ"
                && DEMONSTRATIVE_NA.contains(&prev.surface.as_str());
            if joins {
                g.last = i;
            }
            joins
        }
        Kind::Punct | Kind::Prefix => false,
    }
}

fn merge_pred(g: &mut Group, prev: &Morph, m: &Morph, i: usize) -> bool {
    let renyou = prev.cform.starts_with("連用");
    let ok = match m.pos1.as_str() {
        "助動詞" => !m.is_copula(),
        "助詞" if m.pos2 == "接続助詞" && m.lemma == "て" => !g.te,
        "助詞" if m.surface == "ば" && prev.cform.contains("仮定") => true,
        "動詞" if g.te => {
            HELPER_VERBS.contains(&m.orth_base.as_str()) || HELPER_VERBS.contains(&m.lemma.as_str())
        }
        "動詞" => renyou && matches!(m.lemma.as_str(), "下さる" | "くださる"),
        "形容詞" => {
            (g.te || renyou) && matches!(m.lemma.as_str(), "無い" | "ない" | "欲しい" | "ほしい")
        }
        _ => false,
    };
    if ok {
        g.te = m.pos1 == "助詞" && m.surface != "ば";
        g.last = i;
    }
    ok
}

fn unit_base(morphs: &[Morph], g: &Group, text: &str) -> String {
    let head = &morphs[g.head];
    let verbal = matches!(head.pos1.as_str(), "動詞" | "形容詞" | "助動詞");
    if g.prefix {
        if g.last > g.head && !verbal {
            return text.to_string(); // お母さん, お疲れ様: prefix + noun + honorific is one word
        }
        let tail = if verbal || head.orth_base.is_empty() {
            head.surface.as_str()
        } else {
            head.orth_base.as_str()
        };
        let prefix: String = morphs[g.first..g.head]
            .iter()
            .map(|m| m.surface.as_str())
            .collect();
        format!("{prefix}{tail}")
    } else if g.first == g.last || verbal {
        if head.orth_base.is_empty() {
            head.surface.clone()
        } else {
            head.orth_base.clone()
        }
    } else {
        text.to_string()
    }
}

fn unit_reading(ms: &[Morph], text: &str) -> Option<String> {
    if let Some((_, reading)) = UNIT_READING_OVERRIDES
        .iter()
        .find(|(word, _)| *word == text)
    {
        return Some((*reading).to_string());
    }
    let mut out = String::new();
    for m in ms {
        out.push_str(&morph_reading(m)?);
    }
    (!out.is_empty()).then_some(out)
}

fn trailing_kana(s: &str) -> &str {
    let cut = s
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_kana(*c))
        .last()
        .map_or(s.len(), |(i, _)| i);
    &s[cut..]
}

/// Hiragana reading of one morpheme. Surface kana are kept as written (は stays は); kanji readings
/// come from the dictionary, with the okurigana of conjugated forms taken from the surface.
fn morph_reading(m: &Morph) -> Option<String> {
    let s = m.surface.as_str();
    if s.chars().all(is_kana) {
        return Some(katakana_to_hiragana(s));
    }
    if !s.chars().any(|c| is_kanji(c) || is_latin_alnum(c)) {
        // Punctuation or other symbols inside a word unit: no reading of their own.
        return Some(String::new());
    }
    let from_pron = || (!m.pron.is_empty()).then(|| katakana_to_hiragana(&m.pron));
    if let Some((_, reading)) = READING_OVERRIDES.iter().find(|(word, _)| *word == s) {
        return Some((*reading).to_string());
    }
    if m.orth_base.is_empty() || s == m.orth_base {
        return (!m.lform.is_empty())
            .then(|| katakana_to_hiragana(&m.lform))
            .or_else(from_pron);
    }
    // Conjugated: reading of the kanji part from the lemma, okurigana from the surface.
    let tail = trailing_kana(s);
    let lemma_tail = katakana_to_hiragana(trailing_kana(&m.orth_base));
    let lform = katakana_to_hiragana(&m.lform);
    if !tail.is_empty()
        && !lemma_tail.is_empty()
        && let Some(stem) = lform.strip_suffix(lemma_tail.as_str())
        && !stem.is_empty()
    {
        return Some(format!("{stem}{}", katakana_to_hiragana(tail)));
    }
    from_pron()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kana_helpers() {
        assert_eq!(katakana_to_hiragana("カメラ・コーヒー"), "かめら・こーひー");
        assert!(is_kana('あ') && is_kana('ア') && is_kana('ー') && !is_kana('a') && !is_kana('漢'));
        assert!(is_kanji('漢') && is_kanji('々') && !is_kanji('あ') && !is_kanji('A'));
    }

    #[test]
    fn tokenizer_is_send_sync() {
        fn check<T: Send + Sync>() {}
        check::<Tokenizer>();
    }
}
