#![allow(clippy::unwrap_used)]

use subtitles::{Cue, CueKind, Error, Format, align, cue_index_at, parse};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn kinds(cues: &[Cue]) -> Vec<CueKind> {
    cues.iter().map(|c| c.kind).collect()
}

#[test]
fn anime_ass_cues() {
    let t = parse(&fixture("anime.ass"), None).unwrap();
    assert_eq!(t.format, Format::Ass);
    assert_eq!(t.cues.len(), 12);
    use CueKind::{Dialogue as D, Sign as S, Song as O};
    assert_eq!(kinds(&t.cues), vec![O, S, D, D, S, D, D, D, D, D, S, O]);
    let texts: Vec<&str> = t.cues.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts,
        vec![
            "kagerou",
            "Tokyo, Present Day",
            "Where are we going?",
            "I told you,\nit's a secret.",
            "Keep Out",
            "I wonder if he remembers, after all this time",
            "Top line",
            "Line with bold and nbsp",
            "Overlap A",
            "Overlap B",
            "NOTE: translator comment",
            "yumeni",
        ]
    );
    assert_eq!((t.cues[0].start_ms, t.cues[0].end_ms), (2_000, 6_000));
    assert_eq!(
        (t.cues[11].start_ms, t.cues[11].end_ms),
        (1_330_000, 1_335_000)
    );
    assert_eq!(t.cues[4].style.as_deref(), Some("Default"));
    assert!(t.cues.iter().enumerate().all(|(i, c)| c.id as usize == i));
    assert_eq!(t.style_names().len(), 6);
}

#[test]
fn signs_only_ass_output() {
    let t = parse(&fixture("anime.ass"), None).unwrap();
    let out = t.signs_only_ass().unwrap();
    assert!(out.starts_with("[Script Info]"));
    assert!(out.contains("[V4+ Styles]"));
    assert!(out.contains("Style: Sign,Arial"));
    assert!(out.contains("\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"));
    assert_eq!(out.matches("\nDialogue:").count(), 5);
    assert!(out.contains(r"{\an8\pos(960,120)\fad(300,300)}Tokyo, Present Day"));
    assert!(out.contains(r"{\pos(300,200)}Keep Out"));
    assert!(out.contains(r"{\k40}yu{\ko40}me{\K20}ni"));
    assert!(!out.contains("Where are we going"));
    assert!(!out.contains("Comment:"));
    // The result is itself a parsable ASS with the same sign/song cues.
    let again = parse(out.as_bytes(), None).unwrap();
    assert_eq!(again.cues.len(), 5);
    assert!(
        parse(&fixture("sample.srt"), None)
            .unwrap()
            .signs_only_ass()
            .is_none()
    );
}

#[test]
fn ass_without_signs_has_no_sign_script() {
    let t = parse(&fixture("en.srt"), Some(Format::Srt)).unwrap();
    assert!(t.signs_only_ass().is_none());
    let ass = "[Events]\nFormat: Start, End, Style, Text\nDialogue: 0:00:01.00,0:00:02.00,Default,hi, there\n";
    let t = parse(ass.as_bytes(), None).unwrap();
    assert_eq!(t.cues[0].text, "hi, there");
    assert!(t.signs_only_ass().is_none());
}

#[test]
fn srt_with_crlf_and_tags() {
    let crlf = String::from_utf8(fixture("sample.srt"))
        .unwrap()
        .replace('\n', "\r\n");
    let t = parse(crlf.as_bytes(), None).unwrap();
    assert_eq!(t.format, Format::Srt);
    assert_eq!(t.cues.len(), 3);
    assert_eq!(t.cues[0].text, "Hello there.");
    assert_eq!(t.cues[1].text, "Second line\nover two lines");
    assert_eq!(t.cues[2].text, "Tom & Jerry");
    assert_eq!((t.cues[0].start_ms, t.cues[0].end_ms), (1_000, 3_500));
    assert_eq!((t.cues[2].start_ms, t.cues[2].end_ms), (62_250, 64_000));
    assert!(t.cues.iter().all(|c| c.kind == CueKind::Dialogue));
    // Old Mac line endings too.
    let cr = String::from_utf8(fixture("sample.srt"))
        .unwrap()
        .replace('\n', "\r");
    assert_eq!(parse(cr.as_bytes(), None).unwrap().cues.len(), 3);
}

#[test]
fn vtt_with_cue_settings() {
    let t = parse(&fixture("sample.vtt"), None).unwrap();
    assert_eq!(t.format, Format::Vtt);
    assert_eq!(t.cues.len(), 2);
    assert_eq!(t.cues[0].text, "Hello world");
    assert_eq!((t.cues[0].start_ms, t.cues[0].end_ms), (1_000, 3_000));
    assert_eq!(t.cues[1].text, "Second cue");
    assert_eq!((t.cues[1].start_ms, t.cues[1].end_ms), (4_000, 6_500));
}

#[test]
fn encodings() {
    let srt = "1\n00:00:01,000 --> 00:00:02,000\nこんにちは、世界\n";
    // Shift_JIS, generated here (no binary fixtures).
    let (sjis, _, bad) = encoding_rs::SHIFT_JIS.encode(srt);
    assert!(!bad);
    assert!(std::str::from_utf8(&sjis).is_err());
    assert_eq!(parse(&sjis, None).unwrap().cues[0].text, "こんにちは、世界");
    // UTF-8 with BOM.
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(srt.as_bytes());
    assert_eq!(parse(&bom, None).unwrap().cues[0].text, "こんにちは、世界");
    // UTF-16 LE and BE with BOM.
    let mut le = vec![0xFF, 0xFE];
    le.extend(srt.encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(parse(&le, None).unwrap().cues[0].text, "こんにちは、世界");
    let mut be = vec![0xFE, 0xFF];
    be.extend(srt.encode_utf16().flat_map(u16::to_be_bytes));
    assert_eq!(parse(&be, None).unwrap().cues[0].text, "こんにちは、世界");
}

#[test]
fn errors() {
    assert_eq!(parse(b"", None).unwrap_err(), Error::Empty);
    assert_eq!(parse(b"just some text", None).unwrap_err(), Error::NoCues);
}

#[test]
fn json_shape_matches_ts_cue() {
    let t = parse(&fixture("anime.ass"), None).unwrap();
    assert_eq!(
        serde_json::to_string(&t.cues[1]).unwrap(),
        r#"{"id":1,"startMs":7000,"endMs":10000,"text":"Tokyo, Present Day","kind":"sign"}"#
    );
}

#[test]
fn align_sample_pair() {
    let jp = parse(&fixture("jp.ass"), None).unwrap();
    let en = parse(&fixture("en.srt"), None).unwrap();
    assert_eq!(jp.cues.len(), 8);
    assert_eq!(jp.cues[0].kind, CueKind::Sign);
    assert_eq!(en.cues.len(), 7);
    let pairs = align(&jp.cues, &en.cues);
    let expected: Vec<(u32, Vec<u32>)> = (1..=7).map(|i| (i, vec![i - 1])).collect();
    assert_eq!(pairs, expected);
}

fn cue(id: u32, s: i64, e: i64) -> Cue {
    Cue {
        id,
        start_ms: s,
        end_ms: e,
        text: format!("c{id}"),
        kind: CueKind::Dialogue,
        style: None,
    }
}

#[test]
fn align_threshold_and_sharing() {
    let jp = vec![cue(0, 0, 1000), cue(1, 1000, 3000), cue(2, 5000, 6000)];
    let en = vec![
        cue(0, 100, 1400), // 900 of jp0 (90%) -> jp0; 400 of 1300 vs jp1: shorter is 1300? overlap 400 = 30.7% of 1300 -> no
        cue(1, 600, 1500), // jp0: 400/900 = 44% -> yes; jp1: 500/900 = 55% -> yes (shared)
        cue(2, 2900, 4000), // jp1: 100 of 1100 -> no
    ];
    let pairs = align(&jp, &en);
    assert_eq!(pairs, vec![(0, vec![0, 1]), (1, vec![1]), (2, vec![])]);
    // Exactly 40% counts.
    let jp = vec![cue(0, 0, 1000)];
    let en = vec![cue(0, 600, 5000), cue(1, 700, 5000)];
    assert_eq!(align(&jp, &en), vec![(0, vec![0])]);
    // Non-dialogue cues are ignored on both sides.
    let mut sign = cue(0, 0, 1000);
    sign.kind = CueKind::Sign;
    assert!(align(&[sign.clone()], &en).is_empty());
    assert_eq!(align(&jp, &[sign]), vec![(0, vec![])]);
}

#[test]
fn cue_lookup() {
    let t = parse(&fixture("anime.ass"), None).unwrap();
    assert_eq!(cue_index_at(&t.cues, 18_500), Some(6)); // latest-starting of 5 and 6
    assert_eq!(cue_index_at(&t.cues, 15_000), Some(3)); // sign 4 is skipped
    assert_eq!(cue_index_at(&t.cues, 17_500), Some(5));
    assert_eq!(cue_index_at(&t.cues, 20_000), None); // end is exclusive
    assert_eq!(cue_index_at(&t.cues, 8_000), None); // only a sign
    assert_eq!(cue_index_at(&t.cues, -5), None);
    assert_eq!(cue_index_at(&[], 0), None);
}
