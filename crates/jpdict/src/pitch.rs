//! Pitch accent: mora splitting and accent pattern from a Kanjium downstep number.

use crate::types::Pitch;

/// Split a kana reading into morae: small ゃゅょぁぃぅぇぉゎ join the previous kana; っ, ん and ー are their own mora.
pub fn morae(reading: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in reading.chars() {
        let small = matches!(
            c,
            'ゃ' | 'ゅ'
                | 'ょ'
                | 'ぁ'
                | 'ぃ'
                | 'ぅ'
                | 'ぇ'
                | 'ぉ'
                | 'ゎ'
                | 'ャ'
                | 'ュ'
                | 'ョ'
                | 'ァ'
                | 'ィ'
                | 'ゥ'
                | 'ェ'
                | 'ォ'
                | 'ヮ'
        );
        match out.last_mut() {
            Some(last) if small => last.push(c),
            _ => out.push(c.to_string()),
        }
    }
    out
}

/// Pitch for `reading` with the given downstep (0 heiban, 1 atamadaka, = mora count odaka, else nakadaka).
/// `None` when the downstep is larger than the mora count (bad data).
pub fn pitch_for(reading: &str, downstep: u32) -> Option<Pitch> {
    let morae = morae(reading);
    let n = morae.len() as u32;
    let pattern = match downstep {
        0 => "heiban",
        1 => "atamadaka",
        d if d == n => "odaka",
        d if d < n => "nakadaka",
        _ => return None,
    };
    Some(Pitch {
        downstep,
        pattern: pattern.to_string(),
        morae,
    })
}

/// Parse Kanjium's comma-separated downsteps ("0,2"); invalid parts are skipped, duplicates removed.
pub fn parse_downsteps(s: &str) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    for p in s.split(',') {
        if let Ok(n) = p.trim().parse::<u32>()
            && !v.contains(&n)
        {
            v.push(n);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morae_split() {
        assert_eq!(morae("きょう"), ["きょ", "う"]);
        assert_eq!(morae("がっこう"), ["が", "っ", "こ", "う"]);
        assert_eq!(morae("らーめん"), ["ら", "ー", "め", "ん"]);
        assert_eq!(morae("ファイル"), ["ファ", "イ", "ル"]);
        assert_eq!(morae("あきらめる"), ["あ", "き", "ら", "め", "る"]);
        assert_eq!(morae("しゅっぱつ"), ["しゅ", "っ", "ぱ", "つ"]);
        assert!(morae("").is_empty());
    }

    #[test]
    fn patterns() {
        let pattern = |r, d| pitch_for(r, d).map(|p| p.pattern);
        let p = |r, d| pattern(r, d);
        let p = |r, d| -> Option<&'static str> {
            p(r, d).map(|s| match s.as_str() {
                "heiban" => "heiban",
                "atamadaka" => "atamadaka",
                "nakadaka" => "nakadaka",
                "odaka" => "odaka",
                _ => "?",
            })
        };
        assert_eq!(p("さくら", 0), Some("heiban"));
        assert_eq!(p("いのち", 1), Some("atamadaka"));
        assert_eq!(p("あきらめる", 4), Some("nakadaka"));
        assert_eq!(p("はな", 2), Some("odaka"));
        assert_eq!(p("き", 1), Some("atamadaka"));
        assert_eq!(p("きょう", 2), Some("odaka"));
        assert_eq!(p("はな", 5), None);
    }

    #[test]
    fn downsteps() {
        assert_eq!(parse_downsteps("0,2"), [0, 2]);
        assert_eq!(parse_downsteps("4"), [4]);
        assert_eq!(parse_downsteps("2,2,x"), [2]);
    }
}
