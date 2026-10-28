//! Unicode character classes and data used by the lexer ([lex.name],
//! [lex.charset], [lex.universal.char]): identifier properties, the NFC
//! check, and the character-name table for `\N{name}`.

use std::cmp::Ordering;

use crate::unicode_names::{BLOCK, MAX_NAME_LEN, NAME_BLOCKS, NAMES, NR2_RANGES};
use crate::unicode_tables::{
    CANON_DECOMP, CCC, COMPOSE, NFC_QC_MAYBE, NFC_QC_NO, XID_CONTINUE, XID_START,
};

fn in_table(table: &[(u32, u32)], c: u32) -> bool {
    table
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                Ordering::Less
            } else if lo > c {
                Ordering::Greater
            } else {
                Ordering::Equal
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

/// Non-ASCII characters clang treats as whitespace (with
/// `ext_unicode_whitespace`): the Unicode `White_Space` characters beyond
/// ASCII, except U+0085 and U+180E which clang also accepts.
pub(crate) fn is_unicode_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0085}' | '\u{00A0}' | '\u{1680}' | '\u{180E}' | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

// ---------------------------------------------------------------------------
// Normalization Form C (UAX #15), for [lex.name]/1.

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

/// `Canonical_Combining_Class` of `c`.
pub(crate) fn ccc(c: char) -> u8 {
    let c = c as u32;
    if c < 0x300 {
        return 0;
    }
    match CCC.binary_search_by(|&(lo, hi, _)| {
        if hi < c {
            Ordering::Less
        } else if lo > c {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }) {
        Ok(i) => CCC[i].2,
        Err(_) => 0,
    }
}

/// Whether `s` is in Normalization Form C ([lex.name]/1 requires it of
/// identifiers). Runs the UAX #15 quick check first (`NFC_Quick_Check=No`
/// or a combining-class inversion decides "no" at once); only a string
/// containing an `NFC_Quick_Check=Maybe` character is fully normalized and
/// compared, which allocates.
pub(crate) fn is_nfc(s: &str) -> bool {
    let mut last = 0u8;
    let mut maybe = false;
    for c in s.chars() {
        if c.is_ascii() {
            last = 0;
            continue;
        }
        let cc = ccc(c);
        if cc != 0 && last > cc {
            return false;
        }
        let cp = c as u32;
        if in_table(NFC_QC_NO, cp) {
            return false;
        }
        maybe |= in_table(NFC_QC_MAYBE, cp);
        last = cc;
    }
    !maybe || to_nfc(s).iter().copied().eq(s.chars())
}

fn decompose(c: char, out: &mut Vec<char>) {
    let cp = c as u32;
    if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
        let s = cp - S_BASE;
        let push = |out: &mut Vec<char>, v| out.extend(char::from_u32(v));
        push(out, L_BASE + s / N_COUNT);
        push(out, V_BASE + (s % N_COUNT) / T_COUNT);
        if !s.is_multiple_of(T_COUNT) {
            push(out, T_BASE + s % T_COUNT);
        }
        return;
    }
    match CANON_DECOMP.binary_search_by_key(&cp, |&(k, _, _)| k) {
        Ok(i) => {
            let (_, a, b) = CANON_DECOMP[i];
            decompose(char::from_u32(a).unwrap_or(c), out);
            if b != 0 {
                decompose(char::from_u32(b).unwrap_or(c), out);
            }
        }
        Err(_) => out.push(c),
    }
}

fn compose_pair(a: char, b: char) -> Option<char> {
    let (a, b) = (a as u32, b as u32);
    if (L_BASE..L_BASE + L_COUNT).contains(&a) && (V_BASE..V_BASE + V_COUNT).contains(&b) {
        return char::from_u32(S_BASE + ((a - L_BASE) * V_COUNT + (b - V_BASE)) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&a)
        && (a - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&b)
    {
        return char::from_u32(a + (b - T_BASE));
    }
    COMPOSE
        .binary_search_by(|&(x, y, _)| (x, y).cmp(&(a, b)))
        .ok()
        .and_then(|i| char::from_u32(COMPOSE[i].2))
}

/// The NFC form of `s` (UAX #15: canonical decomposition, canonical
/// ordering, canonical composition).
pub(crate) fn to_nfc(s: &str) -> Vec<char> {
    let mut d = Vec::with_capacity(s.len());
    for c in s.chars() {
        decompose(c, &mut d);
    }
    // Canonical ordering: stable sort of each run of non-starters.
    let mut i = 0;
    while i < d.len() {
        if ccc(d[i]) == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < d.len() && ccc(d[i]) != 0 {
            i += 1;
        }
        d[start..i].sort_by_key(|&c| ccc(c));
    }
    // Canonical composition.
    let mut out: Vec<char> = Vec::with_capacity(d.len());
    let mut starter: Option<usize> = None;
    let mut last_cc: Option<u8> = None; // ccc of the last char appended after the starter
    for c in d {
        let cc = ccc(c);
        if let Some(si) = starter {
            let blocked = matches!(last_cc, Some(l) if l == 0 || l >= cc);
            if !blocked && let Some(comp) = compose_pair(out[si], c) {
                out[si] = comp;
                continue;
            }
        }
        if cc == 0 {
            starter = Some(out.len());
            last_cc = None;
        } else {
            last_cc = Some(cc);
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Character names ([lex.universal.char]/3).

/// How a `\N{…}` name matched a character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameMatch {
    /// The name is exactly the character's Unicode name or one of its
    /// aliases (NameAliases.txt): a valid named-universal-character.
    Exact(char),
    /// The name only matches under UAX #44-LM2 loose matching (case,
    /// whitespace, underscores, medial hyphens). Ill-formed, but it names
    /// the character to recover with.
    Loose(char),
}

impl NameMatch {
    /// The character named, however it matched.
    pub fn char(self) -> char {
        match self {
            NameMatch::Exact(c) | NameMatch::Loose(c) => c,
        }
    }
}

/// A fixed-size byte buffer; names never need more.
struct Buf {
    bytes: [u8; 128],
    len: usize,
}

impl Buf {
    fn new() -> Buf {
        Buf { bytes: [0; 128], len: 0 }
    }
    fn push(&mut self, b: u8) -> bool {
        if self.len == self.bytes.len() {
            return false;
        }
        self.bytes[self.len] = b;
        self.len += 1;
        true
    }
    fn extend(&mut self, s: &[u8]) -> bool {
        s.iter().all(|&b| self.push(b))
    }
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    fn clear(&mut self) {
        self.len = 0;
    }
}

/// The UAX #44-LM2 loose-matching key of `name` into `out`: ASCII letters
/// upper-cased; whitespace, `_` and medial hyphens (between two letters or
/// digits) dropped, except the hyphen of U+1180 HANGUL JUNGSEONG O-E.
/// `false` if `name` has a non-ASCII character or the key overflows.
fn loose_key(name: &[u8], out: &mut Buf) -> bool {
    out.clear();
    for (i, &b) in name.iter().enumerate() {
        match b {
            b' ' | b'\t' | b'\x0B' | b'\x0C' | b'\r' | b'\n' | b'_' => {}
            b'-' if i > 0
                && name[i - 1].is_ascii_alphanumeric()
                && name.get(i + 1).is_some_and(u8::is_ascii_alphanumeric) => {}
            _ if !b.is_ascii() => return false,
            _ => {
                if !out.push(b.to_ascii_uppercase()) {
                    return false;
                }
            }
        }
    }
    if out.as_slice() == b"HANGULJUNGSEONGOE" {
        // Keep the one non-ignorable medial hyphen ("O-E" vs U+116C "OE").
        let trimmed = name
            .iter()
            .rposition(|b| !matches!(b, b' ' | b'\t' | b'_'))
            .map_or(&name[..0], |e| &name[..=e]);
        if trimmed.len() >= 3 && trimmed[trimmed.len() - 3..].eq_ignore_ascii_case(b"O-E") {
            out.clear();
            out.extend(b"HANGULJUNGSEONGO-E");
        }
    }
    true
}

/// One decoded entry of [`NAMES`]: the name (in `name`), the code point,
/// and the offset of the next entry.
fn decode_entry(at: usize, name: &mut Buf) -> Option<(u32, usize)> {
    let bytes = NAMES.as_bytes();
    let shared = usize::from(bytes.get(at)?.checked_sub(0x21)?);
    name.len = shared.min(name.len);
    let mut i = at + 1;
    while bytes[i] != b';' {
        name.push(bytes[i]);
        i += 1;
    }
    i += 1;
    let mut cp = 0u32;
    while let Some(d) = (bytes[i] as char).to_digit(16).filter(|_| !bytes[i].is_ascii_lowercase()) {
        cp = cp * 16 + d;
        i += 1;
    }
    while bytes[i] != b'\n' {
        i += 1; // alias type tag
    }
    Some((cp, i + 1))
}

/// Resolves the name in a `\N{name}` named-universal-character
/// ([lex.universal.char]/3): every Unicode 15.1 character name and every
/// alias in NameAliases.txt (all five types), Hangul syllable names, and
/// the hex-suffixed ideograph names. A name that matches only under
/// UAX #44-LM2 loose matching is reported as [`NameMatch::Loose`] so the
/// caller can diagnose it and recover. Never allocates.
pub fn lookup_char_name(name: &str) -> Option<NameMatch> {
    let mut key = Buf::new();
    if name.is_empty() || !loose_key(name.as_bytes(), &mut key) || key.len > MAX_NAME_LEN {
        return None;
    }
    let key = key.as_slice();
    let (cp, exact) = if let Some(found) = lookup_table(key, name.as_bytes()) {
        found
    } else {
        let cp = lookup_hangul(key).or_else(|| lookup_nr2(key))?;
        (cp, algorithmic_name_eq(cp, name.as_bytes()))
    };
    let c = char::from_u32(cp)?;
    Some(if exact { NameMatch::Exact(c) } else { NameMatch::Loose(c) })
}

/// Finds the table entry (name or alias) with loose key `key`; loose keys
/// are unique, so the match is exact iff that entry's name is `query`.
fn lookup_table(key: &[u8], query: &[u8]) -> Option<(u32, bool)> {
    let mut name = Buf::new();
    let mut k = Buf::new();
    // Last block whose first entry's key is <= `key`.
    let block = NAME_BLOCKS.partition_point(|&off| {
        name.clear();
        decode_entry(off as usize, &mut name);
        loose_key(name.as_slice(), &mut k);
        k.as_slice() <= key
    });
    let mut at = *NAME_BLOCKS.get(block.checked_sub(1)?)? as usize;
    name.clear();
    for _ in 0..BLOCK {
        let (cp, next) = decode_entry(at, &mut name)?;
        loose_key(name.as_slice(), &mut k);
        match k.as_slice().cmp(key) {
            Ordering::Equal => return Some((cp, name.as_slice() == query)),
            Ordering::Greater => return None,
            Ordering::Less => at = next,
        }
    }
    None
}

const JAMO_L: [&str; 19] = [
    "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P",
    "H",
];
const JAMO_V: [&str; 21] = [
    "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO", "WE",
    "WI", "YU", "EU", "YI", "I",
];
const JAMO_T: [&str; 28] = [
    "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH", "M",
    "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
];

/// Longest jamo short name in `table` that prefixes `s`.
fn longest_jamo(s: &[u8], table: &[&str]) -> Option<(usize, usize)> {
    table
        .iter()
        .enumerate()
        .filter(|(_, j)| s.starts_with(j.as_bytes()))
        .max_by_key(|(_, j)| j.len())
        .map(|(i, j)| (i, j.len()))
}

/// NR1 (Unicode ch. 3.12): `HANGUL SYLLABLE` + the jamo short names.
fn lookup_hangul(key: &[u8]) -> Option<u32> {
    let rest = key.strip_prefix(b"HANGULSYLLABLE")?;
    let (l, n) = longest_jamo(rest, &JAMO_L)?;
    let rest = &rest[n..];
    let (v, n) = longest_jamo(rest, &JAMO_V)?;
    let rest = &rest[n..];
    let (t, n) = longest_jamo(rest, &JAMO_T)?;
    (n == rest.len()).then(|| S_BASE + (l as u32 * V_COUNT + v as u32) * T_COUNT + t as u32)
}

/// NR2: a fixed prefix plus the code point in (at least four) hex digits.
fn lookup_nr2(key: &[u8]) -> Option<u32> {
    let mut pk = Buf::new();
    for &(lo, hi, prefix) in NR2_RANGES {
        // The prefix's trailing `-` is medial in a full name: drop it.
        loose_key(prefix.trim_end_matches('-').as_bytes(), &mut pk);
        let Some(hex) = key.strip_prefix(pk.as_slice()) else {
            continue;
        };
        if !(4..=6).contains(&hex.len()) || hex.len() > 4 && hex[0] == b'0' {
            continue;
        }
        let Some(cp) = std::str::from_utf8(hex).ok().and_then(|h| u32::from_str_radix(h, 16).ok())
        else {
            continue;
        };
        if (lo..=hi).contains(&cp) {
            return Some(cp);
        }
    }
    None
}

/// Whether `name` is exactly the algorithmic name of `cp`.
fn algorithmic_name_eq(cp: u32, name: &[u8]) -> bool {
    if (S_BASE..S_BASE + S_COUNT).contains(&cp) {
        let s = cp - S_BASE;
        let mut b = Buf::new();
        b.extend(b"HANGUL SYLLABLE ");
        b.extend(JAMO_L[(s / N_COUNT) as usize].as_bytes());
        b.extend(JAMO_V[((s % N_COUNT) / T_COUNT) as usize].as_bytes());
        b.extend(JAMO_T[(s % T_COUNT) as usize].as_bytes());
        return b.as_slice() == name;
    }
    if let Some(&(_, _, prefix)) = NR2_RANGES.iter().find(|&&(lo, hi, _)| (lo..=hi).contains(&cp)) {
        return name.strip_prefix(prefix.as_bytes()).is_some_and(|hex| {
            hex.len() >= 4
                && !(hex.len() > 4 && hex[0] == b'0')
                && hex.iter().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
                && std::str::from_utf8(hex).ok().and_then(|h| u32::from_str_radix(h, 16).ok())
                    == Some(cp)
        });
    }
    false
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

    /// Every name and alias in the generated table resolves exactly to its
    /// code point; its lower-cased form resolves loosely to the same one.
    #[test]
    fn every_table_name_round_trips() {
        let mut name = Buf::new();
        let mut at = 0;
        let mut count = 0;
        while at < NAMES.len() {
            let (cp, next) = decode_entry(at, &mut name).expect("well-formed table");
            let n = std::str::from_utf8(name.as_slice()).expect("ASCII names");
            let want = char::from_u32(cp).expect("scalar value");
            assert_eq!(lookup_char_name(n), Some(NameMatch::Exact(want)), "{n}");
            if count % 7 == 0 {
                let lower = n.to_ascii_lowercase().replace(' ', "_");
                assert_eq!(lookup_char_name(&lower), Some(NameMatch::Loose(want)), "{lower}");
            }
            count += 1;
            at = next;
        }
        assert!(count > 33_000, "{count} entries");
    }

    /// NR1: all 11172 Hangul syllable names (Unicode ch. 3.12).
    #[test]
    fn hangul_syllable_names() {
        for cp in S_BASE..S_BASE + S_COUNT {
            let s = cp - S_BASE;
            let name = format!(
                "HANGUL SYLLABLE {}{}{}",
                JAMO_L[(s / N_COUNT) as usize],
                JAMO_V[((s % N_COUNT) / T_COUNT) as usize],
                JAMO_T[(s % T_COUNT) as usize]
            );
            let c = char::from_u32(cp).expect("syllable");
            assert_eq!(lookup_char_name(&name), Some(NameMatch::Exact(c)), "{name}");
            if s.is_multiple_of(97) {
                let lower = name.to_ascii_lowercase();
                assert_eq!(lookup_char_name(&lower), Some(NameMatch::Loose(c)), "{lower}");
            }
        }
        assert_eq!(lookup_char_name("HANGUL SYLLABLE"), None);
        assert_eq!(lookup_char_name("HANGUL SYLLABLE GAGX"), None);
    }

    /// UAX44-LM2: the hyphen of U+1180 HANGUL JUNGSEONG O-E is not
    /// ignorable (U+116C is HANGUL JUNGSEONG OE).
    #[test]
    fn o_e_hyphen() {
        assert_eq!(lookup_char_name("HANGUL JUNGSEONG O-E"), Some(NameMatch::Exact('\u{1180}')));
        assert_eq!(lookup_char_name("HANGUL JUNGSEONG OE"), Some(NameMatch::Exact('\u{116C}')));
        assert_eq!(lookup_char_name("hangul jungseong o-e"), Some(NameMatch::Loose('\u{1180}')));
        assert_eq!(lookup_char_name("hangul_jungseong_oe"), Some(NameMatch::Loose('\u{116C}')));
        // Medial hyphens elsewhere are ignorable; others are not.
        assert_eq!(
            lookup_char_name("ZERO WIDTH NO-BREAK SPACE"),
            Some(NameMatch::Exact('\u{FEFF}'))
        );
        assert_eq!(
            lookup_char_name("ZERO WIDTH NO BREAK SPACE"),
            Some(NameMatch::Loose('\u{FEFF}'))
        );
        assert_eq!(
            lookup_char_name("ZERO WIDTH NOBREAK SPACE"),
            Some(NameMatch::Loose('\u{FEFF}'))
        );
        assert_eq!(lookup_char_name("-ZERO WIDTH SPACE"), None);
    }

    /// NR2: names derived from a prefix and the code point in hex.
    #[test]
    fn nr2_names() {
        assert_eq!(
            lookup_char_name("CJK UNIFIED IDEOGRAPH-4E00"),
            Some(NameMatch::Exact('\u{4E00}'))
        );
        assert_eq!(
            lookup_char_name("CJK UNIFIED IDEOGRAPH-4e00"),
            Some(NameMatch::Loose('\u{4E00}'))
        );
        assert_eq!(
            lookup_char_name("cjk unified ideograph 4e00"),
            Some(NameMatch::Loose('\u{4E00}'))
        );
        assert_eq!(
            lookup_char_name("CJK UNIFIED IDEOGRAPH-20000"),
            Some(NameMatch::Exact('\u{20000}'))
        );
        assert_eq!(lookup_char_name("CJK UNIFIED IDEOGRAPH-0041"), None);
        assert_eq!(lookup_char_name("CJK UNIFIED IDEOGRAPH-04E00"), None);
        assert_eq!(lookup_char_name("CJK UNIFIED IDEOGRAPH-"), None);
        assert_eq!(
            lookup_char_name("CJK COMPATIBILITY IDEOGRAPH-F900"),
            Some(NameMatch::Exact('\u{F900}'))
        );
        assert_eq!(lookup_char_name("TANGUT IDEOGRAPH-17000"), Some(NameMatch::Exact('\u{17000}')));
        for (lo, hi, prefix) in NR2_RANGES {
            for cp in [*lo, *hi] {
                let name = format!("{prefix}{cp:04X}");
                let c = char::from_u32(cp).expect("scalar");
                assert_eq!(lookup_char_name(&name), Some(NameMatch::Exact(c)), "{name}");
            }
        }
    }

    /// Aliases of every type resolve; non-names do not.
    #[test]
    fn aliases_and_misses() {
        assert_eq!(lookup_char_name("NULL"), Some(NameMatch::Exact('\0')));
        assert_eq!(lookup_char_name("LINE FEED"), Some(NameMatch::Exact('\n')));
        assert_eq!(lookup_char_name("LF"), Some(NameMatch::Exact('\n')));
        assert_eq!(lookup_char_name("BYTE ORDER MARK"), Some(NameMatch::Exact('\u{FEFF}')));
        assert_eq!(lookup_char_name("LATIN CAPITAL LETTER GHA"), Some(NameMatch::Exact('\u{1A2}')));
        assert_eq!(lookup_char_name(""), None);
        assert_eq!(lookup_char_name("NOT A CHARACTER NAME"), None);
        assert_eq!(lookup_char_name("LATIN SMALL LETTER \u{E9}"), None);
        assert_eq!(lookup_char_name(&"A".repeat(200)), None);
    }

    /// NFC: quick-check and full normalization agree with Unicode's
    /// NormalizationTest.txt. Needs the file:
    /// `KILN_UCD_NORMALIZATION_TEST=<path> cargo test -p kiln_lex -- --ignored nfc`.
    #[test]
    #[ignore = "needs NormalizationTest.txt (KILN_UCD_NORMALIZATION_TEST)"]
    fn nfc_normalization_test() {
        let path = std::env::var("KILN_UCD_NORMALIZATION_TEST")
            .expect("set KILN_UCD_NORMALIZATION_TEST to NormalizationTest.txt");
        let text = std::fs::read_to_string(path).expect("readable");
        let mut lines = 0;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            if line.is_empty() || line.starts_with('@') {
                continue;
            }
            let cols: Vec<String> = line
                .split(';')
                .take(5)
                .map(|col| {
                    col.split_whitespace()
                        .map(|h| {
                            char::from_u32(u32::from_str_radix(h, 16).expect("hex"))
                                .expect("scalar")
                        })
                        .collect()
                })
                .collect();
            let nfc: Vec<char> = cols[1].chars().collect();
            for (i, col) in cols.iter().enumerate() {
                let want = if i < 3 { &nfc } else { &cols[3].chars().collect() };
                assert_eq!(&to_nfc(col), want, "NFC of column {} in {line}", i + 1);
                let is = to_nfc(col).iter().copied().eq(col.chars());
                assert_eq!(is_nfc(col), is, "is_nfc of column {} in {line}", i + 1);
            }
            lines += 1;
        }
        assert!(lines > 10_000, "{lines} test lines");
    }
}
