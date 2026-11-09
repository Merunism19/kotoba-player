//! Small kana/kanji helpers (kept local so this crate stands alone).

/// Katakana to hiragana (ァ..ヴ and the iteration marks); everything else is left alone, including ー.
pub fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ァ'..='ヴ' | 'ヽ' | 'ヾ' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

pub fn is_hiragana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{309F}')
}

pub fn is_katakana(c: char) -> bool {
    matches!(c, '\u{30A0}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}')
}

pub fn is_kana(c: char) -> bool {
    is_hiragana(c) || is_katakana(c)
}

/// CJK ideographs plus the repetition mark 々, 〆 and 〇.
pub fn is_kanji(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FFFF}' | '々' | '〆' | '〇')
}

pub fn has_katakana(s: &str) -> bool {
    s.chars().any(|c| matches!(c, 'ァ'..='ヴ'))
}

/// Characters a dictionary word can consist of (so a scan stops at punctuation, spaces, brackets...).
pub fn is_word_char(c: char) -> bool {
    is_kana(c)
        || is_kanji(c)
        || c == 'ヶ'
        || c == 'ヵ'
        || c.is_ascii_alphanumeric()
        || matches!(c, '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn katakana_converts() {
        assert_eq!(to_hiragana("ケーキ"), "けーき");
        assert_eq!(to_hiragana("サボる"), "さぼる");
        assert_eq!(to_hiragana("諦める"), "諦める");
    }

    #[test]
    fn word_chars() {
        assert!(is_word_char('夕') && is_word_char('け') && is_word_char('ー'));
        assert!(!is_word_char('、') && !is_word_char(' ') && !is_word_char('「'));
    }
}
