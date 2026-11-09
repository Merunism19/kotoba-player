//! Word-unit tests: boundaries, readings, base forms, coverage, JSON shape and speed.

use std::sync::OnceLock;
use std::time::Instant;

use jptext::{Tokenizer, WordUnit};

fn tokenizer() -> &'static Tokenizer {
    static T: OnceLock<Tokenizer> = OnceLock::new();
    T.get_or_init(|| Tokenizer::new().expect("embedded UniDic loads"))
}

fn split(line: &str) -> String {
    tokenizer()
        .word_units(line)
        .iter()
        .map(|u| u.text.as_str())
        .collect::<Vec<_>>()
        .join("|")
}

/// (line, expected units joined with `|`). Real anime-style dialogue.
const BOUNDARIES: &[(&str, &str)] = &[
    // The cases from the brief.
    (
        "もう諦めないって決めたんだ。",
        "もう|諦めない|って|決めた|ん|だ|。",
    ),
    (
        "食べさせられなかったのは残念だね。",
        "食べさせられなかった|の|は|残念|だ|ね|。",
    ),
    (
        "夕焼けがこんなに綺麗だなんて知らなかった。",
        "夕焼け|が|こんなに|綺麗|だ|なんて|知らなかった|。",
    ),
    ("明日、一緒に行こうよ。", "明日|、|一緒|に|行こう|よ|。"),
    (
        "カメラを持ってきたから、写真を撮ろう！",
        "カメラ|を|持ってきた|から|、|写真|を|撮ろう|！",
    ),
    (
        "約束だよ。絶対に忘れないでね。",
        "約束|だ|よ|。|絶対|に|忘れないで|ね|。",
    ),
    (
        "ありがとう。また会おうね。",
        "ありがとう|。|また|会おう|ね|。",
    ),
    // Conjugation, contractions, te-form chains.
    ("俺はもう逃げない。", "俺|は|もう|逃げない|。"),
    (
        "お前、まさか忘れちゃったのか？",
        "お前|、|まさか|忘れちゃった|の|か|？",
    ),
    ("ちょっと待ってよ！", "ちょっと|待って|よ|！"),
    (
        "彼女は泣きながら笑っていた。",
        "彼女|は|泣き|ながら|笑っていた|。",
    ),
    (
        "もし明日雨が降ったら、中止にしよう。",
        "もし|明日|雨|が|降ったら|、|中止|に|しよう|。",
    ),
    ("早く帰ってこいよ。", "早く|帰ってこい|よ|。"),
    ("これ、見てみて！", "これ|、|見てみて|！"),
    ("僕は君を守ってみせる。", "僕|は|君|を|守ってみせる|。"),
    (
        "何度も何度も、立ち上がってきた。",
        "何度|も|何度|も|、|立ち上がってきた|。",
    ),
    (
        "お母さんに怒られちゃうよ。",
        "お母さん|に|怒られちゃう|よ|。",
    ),
    (
        "食べたくないなら食べなくてもいいよ。",
        "食べたくない|なら|食べなくて|も|いい|よ|。",
    ),
    (
        "やっぱり行かない方がいいかも。",
        "やっぱり|行かない|方|が|いい|か|も|。",
    ),
    (
        "冷たい飲み物が欲しいなぁ。",
        "冷たい|飲み物|が|欲しい|なぁ|。",
    ),
    ("それは本当に美しかった。", "それ|は|本当|に|美しかった|。"),
    // Copula, nominalising ん, polite forms, honorifics.
    (
        "君のことがずっと好きだった。",
        "君|の|こと|が|ずっと|好き|だった|。",
    ),
    (
        "あの人は先生じゃなかったんだ。",
        "あの|人|は|先生|じゃなかった|ん|だ|。",
    ),
    ("今日はいい天気ですね。", "今日|は|いい|天気|です|ね|。"),
    ("私たちは友達でしょう？", "私たち|は|友達|でしょう|？"),
    ("お願いします。", "お願い|します|。"),
    ("お疲れ様でした！", "お疲れ様|でした|！"),
    ("お待ちください。", "お待ちください|。"),
    ("ナルトくん、助けて！", "ナルト|くん|、|助けて|！"),
    ("田中さんに会いたいです。", "田中|さん|に|会いたい|です|。"),
    ("勉強するのは楽しい。", "勉強|する|の|は|楽しい|。"),
    // Katakana, long vowels, small tsu, slang.
    (
        "すっげー！ マジでかっこいいじゃん！",
        "すっげー|！| |マジ|で|かっこ|いい|じゃん|！",
    ),
    ("ねー、ちょっと聞いてる？", "ねー|、|ちょっと|聞いてる|？"),
    (
        "コーヒーを飲みながら話そう。",
        "コーヒー|を|飲み|ながら|話そう|。",
    ),
    // Latin letters, numbers, mixed.
    ("３話の続きが見たい。", "３話|の|続き|が|見たい|。"),
    ("OK、分かった。", "OK|、|分かった|。"),
    // Multi-line, empty, punctuation only.
    (
        "待って。\nどこへ行くの？",
        "待って|。|\n|どこ|へ|行く|の|？",
    ),
    ("", ""),
    ("……！？", "…|…|！|？"),
    ("「　」", "「　」"),
    ("ああ、そうか。わかった。", "ああ|、|そう|か|。|わかった|。"),
];

#[test]
fn unit_boundaries() {
    let mut failures = Vec::new();
    for (line, want) in BOUNDARIES {
        let got = split(line);
        if got != *want {
            failures.push(format!("{line:?}\n   want {want}\n   got  {got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} lines differ:\n{}",
        failures.len(),
        BOUNDARIES.len(),
        failures.join("\n")
    );
    assert!(BOUNDARIES.len() >= 40);
}

/// (line, unit text, reading, base)
const DETAILS: &[(&str, &str, &str, &str)] = &[
    (
        "もう諦めないって決めたんだ。",
        "諦めない",
        "あきらめない",
        "諦める",
    ),
    ("もう諦めないって決めたんだ。", "決めた", "きめた", "決める"),
    ("もう諦めないって決めたんだ。", "って", "って", "って"),
    (
        "食べさせられなかったのは残念だね。",
        "食べさせられなかった",
        "たべさせられなかった",
        "食べる",
    ),
    (
        "食べさせられなかったのは残念だね。",
        "残念",
        "ざんねん",
        "残念",
    ),
    (
        "夕焼けがこんなに綺麗だなんて知らなかった。",
        "夕焼け",
        "ゆうやけ",
        "夕焼け",
    ),
    (
        "夕焼けがこんなに綺麗だなんて知らなかった。",
        "知らなかった",
        "しらなかった",
        "知る",
    ),
    (
        "夕焼けがこんなに綺麗だなんて知らなかった。",
        "綺麗",
        "きれい",
        "綺麗",
    ),
    ("明日、一緒に行こうよ。", "一緒", "いっしょ", "一緒"),
    ("明日、一緒に行こうよ。", "行こう", "いこう", "行く"),
    ("明日、一緒に行こうよ。", "明日", "あした", "明日"),
    (
        "カメラを持ってきたから、写真を撮ろう！",
        "持ってきた",
        "もってきた",
        "持つ",
    ),
    (
        "カメラを持ってきたから、写真を撮ろう！",
        "カメラ",
        "かめら",
        "カメラ",
    ),
    (
        "カメラを持ってきたから、写真を撮ろう！",
        "撮ろう",
        "とろう",
        "撮る",
    ),
    ("約束だよ。絶対に忘れないでね。", "約束", "やくそく", "約束"),
    (
        "約束だよ。絶対に忘れないでね。",
        "忘れないで",
        "わすれないで",
        "忘れる",
    ),
    (
        "お前、まさか忘れちゃったのか？",
        "忘れちゃった",
        "わすれちゃった",
        "忘れる",
    ),
    ("美しかった", "美しかった", "うつくしかった", "美しい"),
    ("お願いします。", "お願い", "おねがい", "お願い"),
    (
        "お母さんに怒られちゃうよ。",
        "お母さん",
        "おかあさん",
        "お母さん",
    ),
    ("私たちは友達でしょう？", "私たち", "わたしたち", "私たち"),
    ("OK、分かった。", "OK", "おーけー", "OK"),
    ("３話の続きが見たい。", "３話", "さんわ", "３話"),
    ("すっげー！", "すっげー", "すっげー", "すっげー"),
    (
        "コーヒーを飲みながら話そう。",
        "コーヒー",
        "こーひー",
        "コーヒー",
    ),
    (
        "あの人は先生じゃなかったんだ。",
        "じゃなかった",
        "じゃなかった",
        "だ",
    ),
];

#[test]
fn readings_and_base_forms() {
    for (line, text, reading, base) in DETAILS {
        let units = tokenizer().word_units(line);
        let u = units
            .iter()
            .find(|u| u.text == *text)
            .unwrap_or_else(|| panic!("{text} is not a unit of {line}: {units:?}"));
        assert_eq!(
            u.reading.as_deref(),
            Some(*reading),
            "reading of {text} in {line}"
        );
        assert_eq!(u.base.as_deref(), Some(*base), "base of {text} in {line}");
        assert!(u.is_word, "{text} must be a word");
    }
    assert!(DETAILS.len() >= 15);
}

#[test]
fn punctuation_and_space_are_not_words() {
    for line in ["……！？", "「　」 ", "待って。\nどこ", "すっげー！ マジ"] {
        for u in tokenizer().word_units(line) {
            let wordy = u.text.chars().any(|c| {
                (jptext::is_kana(c) && c != 'ー') || jptext::is_kanji(c) || c.is_ascii_alphabetic()
            });
            if !wordy {
                assert!(!u.is_word, "{:?} in {line}", u.text);
                assert!(u.reading.is_none() && u.base.is_none());
            } else {
                assert!(u.is_word, "{:?} in {line}", u.text);
            }
        }
    }
    let units = tokenizer().word_units("待って。\nどこ");
    assert!(units.iter().any(|u| u.text == "\n" && !u.is_word));
}

/// Units must cover the line exactly, in order, with code-point offsets.
#[test]
fn units_cover_the_line() {
    let extra = [
        "",
        " ",
        "   ",
        "a b  c",
        "😀を食べた😀",
        "ｱｲｳ カタカナ",
        "\n\n",
        "text with ＩＤ１２３ and ３話",
        "𠮷野家で食べる",
        "ー",
    ];
    for line in BOUNDARIES.iter().map(|(l, _)| *l).chain(extra) {
        let units = tokenizer().word_units(line);
        let joined: String = units.iter().map(|u| u.text.as_str()).collect();
        assert_eq!(joined, line, "concatenation of units");
        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0;
        for u in &units {
            assert_eq!(u.start, pos, "start of {:?} in {line:?}", u.text);
            assert_eq!(u.len, u.text.chars().count());
            assert!(u.len > 0);
            assert_eq!(
                chars[u.start..u.start + u.len].iter().collect::<String>(),
                u.text
            );
            pos += u.len;
        }
        assert_eq!(pos, chars.len());
    }
}

#[test]
fn serializes_like_the_typescript_type() {
    let units = tokenizer().word_units("知らなかった。");
    let json = serde_json::to_value(&units).unwrap();
    assert_eq!(json[0]["start"], 0);
    assert_eq!(json[0]["len"], 6);
    assert_eq!(json[0]["text"], "知らなかった");
    assert_eq!(json[0]["reading"], "しらなかった");
    assert_eq!(json[0]["base"], "知る");
    assert_eq!(json[0]["isWord"], true);
    // Punctuation: no reading/base keys at all.
    let obj = json[1].as_object().unwrap();
    assert_eq!(obj.len(), 4, "{obj:?}");
    assert_eq!(json[1]["isWord"], false);
    let _: &WordUnit = &units[0];
}

#[test]
fn works_from_many_threads() {
    let t = tokenizer();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..20 {
                    assert_eq!(split("食べさせられなかった。"), "食べさせられなかった|。");
                }
            });
        }
    });
    let _ = t;
}

#[test]
fn speed() {
    let t0 = Instant::now();
    let t = Tokenizer::new().unwrap();
    let new_time = t0.elapsed();
    let line = "夕焼けがこんなに綺麗だなんて知らなかった。もう諦めないって決めたんだ。";
    for _ in 0..20 {
        t.word_units(line);
    }
    let n = 200;
    let t1 = Instant::now();
    for _ in 0..n {
        t.word_units(line);
    }
    let per_line = t1.elapsed() / n;
    eprintln!("Tokenizer::new {new_time:?}; word_units on a 2-sentence line: {per_line:?}");
    assert!(per_line.as_millis() < 20, "{per_line:?} per line");
    assert!(new_time.as_secs() < 5, "{new_time:?}");
}
