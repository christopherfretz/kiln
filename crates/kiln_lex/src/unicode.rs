//! Unicode character classes used by the lexer ([lex.name], [lex.charset]).

use crate::unicode_tables::{NFC_QC_NO, XID_CONTINUE, XID_START};

fn in_table(table: &[(u32, u32)], c: u32) -> bool {
    table
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// ASCII identifier-continue: `[A-Za-z0-9_]`.
pub(crate) fn is_ascii_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// [lex.name]: a non-ASCII character with the `XID_Start` property.
pub(crate) fn is_xid_start(c: char) -> bool {
    !c.is_ascii() && in_table(XID_START, c as u32)
}

/// [lex.name]: a non-ASCII character with the `XID_Continue` property.
pub(crate) fn is_xid_continue(c: char) -> bool {
    !c.is_ascii() && in_table(XID_CONTINUE, c as u32)
}

/// Characters with `NFC_Quick_Check=No`: an identifier containing one is not
/// in Normalization Form C ([lex.name]/1).
pub(crate) fn is_nfc_qc_no(c: char) -> bool {
    !c.is_ascii() && in_table(NFC_QC_NO, c as u32)
}

/// Non-ASCII characters clang treats as whitespace (with
/// `ext_unicode_whitespace`): the Unicode `White_Space` characters beyond
/// ASCII, except U+0085 and U+180E which clang also accepts.
pub(crate) fn is_unicode_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{180E}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// Resolves the name in a `\N{name}` named-universal-character
/// ([lex.universal.char]).
///
/// TODO(WP-011): only the algorithmic CJK names are resolved here
/// (`CJK UNIFIED IDEOGRAPH-XXXX`, `CJK COMPATIBILITY IDEOGRAPH-XXXX`); the
/// full Unicode name table (with loose matching diagnostics) belongs to the
/// literal work package, which needs it for `\N{}` escapes in literals too.
pub(crate) fn lookup_char_name(name: &str) -> Option<char> {
    let (prefix_ok, hex) = if let Some(h) = name.strip_prefix("CJK UNIFIED IDEOGRAPH-") {
        (true, h)
    } else if let Some(h) = name.strip_prefix("CJK COMPATIBILITY IDEOGRAPH-") {
        (true, h)
    } else {
        (false, "")
    };
    if !prefix_ok || !(4..=5).contains(&hex.len()) || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let cp = u32::from_str_radix(hex, 16).ok()?;
    let c = char::from_u32(cp)?;
    // Only code points that actually carry such a name.
    let unified = name.starts_with("CJK U") && is_xid_start(c) && is_cjk_unified(cp);
    let compat = name.starts_with("CJK C") && is_cjk_compat(cp);
    (unified || compat).then_some(c)
}

fn is_cjk_unified(cp: u32) -> bool {
    matches!(
        cp,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B739
            | 0x2B740..=0x2B81D
            | 0x2B820..=0x2CEA1
            | 0x2CEB0..=0x2EBE0
            | 0x2EBF0..=0x2EE5D
            | 0x30000..=0x3134A
            | 0x31350..=0x323AF
    )
}

fn is_cjk_compat(cp: u32) -> bool {
    matches!(cp, 0xF900..=0xFA6D | 0xFA70..=0xFAD9 | 0x2F800..=0x2FA1D)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xid_tables() {
        assert!(is_xid_start('é'));
        assert!(is_xid_start('Ω'));
        assert!(is_xid_start('漢'));
        assert!(!is_xid_start('\u{0301}')); // combining acute: continue only
        assert!(is_xid_continue('\u{0301}'));
        assert!(!is_xid_continue('\u{00A0}'));
        assert!(!is_xid_start('€'));
        assert!(!is_xid_continue('😀'));
        assert!(!is_xid_start('a')); // ASCII handled separately
    }

    #[test]
    fn nfc_and_names() {
        assert!(is_nfc_qc_no('\u{212B}')); // ANGSTROM SIGN
        assert!(!is_nfc_qc_no('é'));
        assert_eq!(lookup_char_name("CJK UNIFIED IDEOGRAPH-4E00"), Some('\u{4E00}'));
        assert_eq!(lookup_char_name("CJK COMPATIBILITY IDEOGRAPH-F900"), Some('\u{F900}'));
        assert_eq!(lookup_char_name("CJK UNIFIED IDEOGRAPH-0041"), None);
        assert_eq!(lookup_char_name("LATIN SMALL LETTER A"), None);
    }
}
