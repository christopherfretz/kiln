//! Values of character and string literals: [lex.ccon], [lex.string],
//! [lex.string.uneval], [lex.ext], and `#if` character values
//! ([cpp.cond]/12). Values match clang 19 on the named targets.
use kiln_lex::literal::{
    CharType, CodeUnits, EncodingParams, LiteralOptions, decode_char, decode_string,
    decode_unevaluated_string, encode_string,
};
use kiln_lex::{Encoding, LexDiagKind as D, Std};

const X86: LiteralOptions = LiteralOptions::new(Std::Cxx26, EncodingParams::LINUX_X86_64);
const ARM: LiteralOptions = LiteralOptions::new(Std::Cxx26, EncodingParams::LINUX_AARCH64);
const WIN: LiteralOptions = LiteralOptions::new(Std::Cxx26, EncodingParams::WINDOWS);

fn kinds(d: &[kiln_lex::literal::LitDiag]) -> Vec<D> {
    d.iter().map(|d| d.kind).collect()
}

/// [lex.string]/10-12: code units per encoding (UTF-8, UTF-16 with
/// surrogate pairs, UTF-32; wide per target).
#[test]
fn lex_string_encodings() {
    let s = decode_string("u8\"\u{e9}\u{1F600}\"", X86);
    assert_eq!(s.encoding, Encoding::Utf8);
    assert_eq!(s.units, CodeUnits::U8(vec![0xC3, 0xA9, 0xF0, 0x9F, 0x98, 0x80]));
    let s = decode_string("u\"a\u{1F600}\"", X86);
    assert_eq!(s.units, CodeUnits::U16(vec![0x61, 0xD83D, 0xDE00]));
    let s = decode_string("U\"a\u{1F600}\"", X86);
    assert_eq!(s.units, CodeUnits::U32(vec![0x61, 0x1_F600]));
    assert_eq!(decode_string("L\"\u{1F600}\"", X86).units, CodeUnits::U32(vec![0x1_F600]));
    assert_eq!(decode_string("L\"\u{1F600}\"", WIN).units, CodeUnits::U16(vec![0xD83D, 0xDE00]));
    // Ordinary literals use UTF-8 (the execution character set is UTF-8).
    assert_eq!(decode_string("\"\u{e9}\"", X86).units.len(), 2);
    // The value excludes the terminating null.
    assert!(decode_string("\"\"", X86).units.is_empty());
}

/// [lex.string]/4: raw strings keep their body verbatim; a CR LF line
/// end is one LF (as clang), a lone CR is kept.
#[test]
fn lex_string_raw_values() {
    let s = decode_string("R\"x(a\r\nb\rc)x\"", X86);
    assert!(s.raw);
    assert_eq!(s.units, CodeUnits::U8(b"a\nb\rc".to_vec()));
    let s = decode_string("uR\"(\u{1F600})\"_s", X86);
    assert_eq!(s.units, CodeUnits::U16(vec![0xD83D, 0xDE00]));
    assert_eq!(s.ud_suffix, Some("_s"));
    // `)"` inside the body does not end it when the delimiter differs.
    let s = decode_string("R\"d()\")d\"", X86);
    assert_eq!(s.units, CodeUnits::U8(b")\"".to_vec()));
}

/// [lex.ext]/1: ud-suffixes are returned as spelled.
#[test]
fn lex_ext_ud_suffix() {
    assert_eq!(decode_string("\"a\"_x", X86).ud_suffix, Some("_x"));
    assert_eq!(decode_string("\"a\"sv", X86).ud_suffix, Some("sv"));
    assert_eq!(decode_char("'a'_c", X86).ud_suffix, Some("_c"));
    assert_eq!(decode_char("'a'", X86).ud_suffix, None);
}

/// [lex.string.uneval]: no prefix, no ud-suffix, no numeric or unknown
/// escapes; the value is UTF-8.
#[test]
fn lex_string_uneval() {
    let s = decode_unevaluated_string("\"ok \u{e9}\\n\"", X86);
    assert_eq!(s.units, CodeUnits::U8(b"ok \xC3\xA9\n".to_vec()));
    assert_eq!(s.diags, []);
    let s = decode_unevaluated_string("u8\"x\"", X86);
    assert_eq!(kinds(&s.diags), [D::UnevaluatedStringPrefix]);
    assert_eq!(s.units, CodeUnits::U8(b"x".to_vec()));
    assert_eq!(
        kinds(&decode_unevaluated_string("\"x\"_s", X86).diags),
        [D::UnevaluatedStringUdSuffix]
    );
    assert_eq!(
        kinds(&decode_unevaluated_string("\"\\x41\"", X86).diags),
        [D::UnevaluatedStringEscape { ch: 'x' }]
    );
    assert_eq!(
        kinds(&decode_unevaluated_string("\"\\q\"", X86).diags),
        [D::UnknownEscape { ch: 'q' }, D::UnevaluatedStringEscape { ch: 'q' }]
    );
}

/// [lex.ccon]/2, /5: a single c-char: type and value; plain `char` is
/// signed on x86-64 and unsigned on `AArch64` Linux.
#[test]
fn lex_ccon_single_values() {
    let c = decode_char("'a'", X86);
    assert_eq!((c.ty, c.chars, c.value, c.pp_value), (CharType::Char, 1, 97, 97));
    assert_eq!(decode_char("'\\xff'", X86).value, -1);
    assert_eq!(decode_char("'\\xff'", ARM).value, 255);
    assert_eq!(decode_char("L'\\xffffffff'", X86).value, -1);
    assert_eq!(decode_char("L'\\xffffffff'", ARM).value, 0xFFFF_FFFF);
    assert_eq!(decode_char("L'\\xffff'", WIN).value, 0xFFFF);
    assert_eq!(decode_char("u'\\xffff'", X86).value, 0xFFFF);
    assert_eq!(decode_char("U'\u{1F600}'", X86).value, 0x1_F600);
    assert_eq!(decode_char("u8'\\xff'", X86).ty, CharType::Char8);
    assert_eq!(decode_char("u8'\\xff'", X86).value, 255);
    let cxx17 = LiteralOptions::new(Std::Cxx17, EncodingParams::LINUX_X86_64);
    assert_eq!(decode_char("u8'a'", cxx17).ty, CharType::Char);
    assert_eq!(decode_char("u8'\\xff'", cxx17).value, -1);
}

/// [lex.ccon]/3: a c-char that needs more than one code unit is
/// ill-formed.
#[test]
fn lex_ccon_too_large() {
    assert_eq!(kinds(&decode_char("'\u{e9}'", X86).diags), [D::CharTooLarge]);
    assert_eq!(kinds(&decode_char("u8'\u{e9}'", X86).diags), [D::CharTooLarge]);
    assert_eq!(kinds(&decode_char("u'\u{1F600}'", X86).diags), [D::CharTooLarge]);
    assert_eq!(kinds(&decode_char("L'\u{1F600}'", WIN).diags), [D::CharTooLarge]);
    assert_eq!(decode_char("L'\u{1F600}'", X86).diags, []);
    assert_eq!(decode_char("u'\u{e9}'", X86).diags, []);
}

/// [lex.ccon]/2 (conditionally-supported multicharacter literals): type
/// `int`, packed 8 bits per c-char as clang; non-ordinary ones are
/// ill-formed.
#[test]
fn lex_ccon_multichar() {
    let c = decode_char("'ab'", X86);
    assert_eq!((c.ty, c.chars, c.value, c.pp_value), (CharType::Int, 2, 0x6162, 0x6162));
    assert_eq!(kinds(&c.diags), [D::MultiCharLiteral]);
    let c = decode_char("'abcd'", X86);
    assert_eq!((c.value, kinds(&c.diags)), (0x6162_6364, vec![D::FourCharLiteral]));
    let c = decode_char("'abcde'", X86);
    assert_eq!(c.value, 0x6263_6465);
    assert_eq!(kinds(&c.diags), [D::MultiCharLiteral, D::CharConstantTooLong]);
    assert_eq!(decode_char("'\\x80\\x81'", X86).value, 0x8081);
    assert_eq!(decode_char("'\\xff\\xff\\xff\\xff'", X86).value, -1);
    assert_eq!(kinds(&decode_char("u'ab'", X86).diags), [D::MultiCharNotAllowed]);
    assert!(decode_char("u'ab'", X86).has_error);
    assert_eq!(kinds(&decode_char("''", X86).diags), [D::EmptyCharLiteral]);
}

/// [cpp.cond]/12: in `#if`, a character literal's value is computed in
/// the width of its kind and extended by the character type's signedness
/// (clang's `PPExpressions`).
#[test]
fn cpp_cond_char_values() {
    let c = decode_char("'\\xff'", X86);
    assert_eq!((c.pp_value, c.pp_unsigned), (-1, false));
    let c = decode_char("'\\xff'", ARM);
    assert_eq!((c.pp_value, c.pp_unsigned), (255, true));
    let c = decode_char("u8'\\xff'", X86);
    assert_eq!((c.pp_value, c.pp_unsigned), (255, true));
    let c = decode_char("L'\\xffffffff'", X86);
    assert_eq!((c.pp_value, c.pp_unsigned), (-1, false));
    let c = decode_char("'\\xff\\xff\\xff\\xff'", X86);
    assert_eq!(c.pp_value, -1);
    let c = decode_char("'\\xff\\xff\\xff\\xff'", ARM);
    assert_eq!((c.pp_value, c.pp_unsigned), (0xFFFF_FFFF, true));
    let c = decode_char("u'\\xffff'", X86);
    assert_eq!((c.pp_value, c.pp_unsigned), (0xFFFF, true));
}

/// Malformed spellings never panic and report an error.
#[test]
fn lex_ccon_malformed_spellings() {
    for sp in ["", "'", "'a", "\"", "u8", "R\"(", "R\"d(x)e\"", "x", "'\\", "\"\\"] {
        let c = decode_char(sp, X86);
        let s = decode_string(sp, X86);
        assert!(c.has_error || s.has_error || !c.diags.is_empty(), "{sp:?}");
    }
}

/// Tiny xorshift generator (no dependencies).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// [lex.string]: property: for arbitrary code units, `encode_string`
/// spells a literal whose value `decode_string` gives back exactly, in
/// every encoding and language mode from C++11.
#[test]
fn lex_string_encode_decode_round_trip() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for round in 0..2000 {
        let enc =
            [Encoding::Ordinary, Encoding::Utf8, Encoding::Utf16, Encoding::Utf32, Encoding::Wide]
                [round % 5];
        let opts = [X86, WIN][round / 5 % 2];
        let opts = LiteralOptions { std: [Std::Cxx11, Std::Cxx17, Std::Cxx26][round % 3], ..opts };
        let len = (rng.next() % 12) as usize;
        let mut units = match (enc, opts.params.wchar_bits) {
            (Encoding::Ordinary | Encoding::Utf8, _) => CodeUnits::U8(Vec::new()),
            (Encoding::Utf16, _) | (Encoding::Wide, 16) => CodeUnits::U16(Vec::new()),
            _ => CodeUnits::U32(Vec::new()),
        };
        let bits = units.unit_bits();
        for _ in 0..len {
            let r = rng.next();
            // Bias towards ASCII, digits and boundary values.
            let u = match r % 4 {
                0 => u32::from(b"0aF\"\\?x\n"[(r >> 8) as usize % 8]),
                1 => (r >> 8) as u32 & 0x7F,
                _ => (r >> 8) as u32,
            };
            let u = if bits == 32 { u } else { u & ((1 << bits) - 1) };
            match &mut units {
                CodeUnits::U8(v) => v.push(u as u8),
                CodeUnits::U16(v) => v.push(u as u16),
                CodeUnits::U32(v) => v.push(u),
            }
        }
        let spelled = encode_string(&units, enc);
        let back = decode_string(&spelled, opts);
        assert!(back.diags.is_empty(), "{spelled}: {:?}", back.diags);
        assert_eq!(back.units, units, "{spelled}");
        assert_eq!(back.encoding, enc);
    }
}

/// [lex.string]: decoding UTF-8 text and re-encoding gives the same
/// value (decode -> encode -> decode is the identity on values).
#[test]
fn lex_string_decode_encode_decode() {
    for sp in [
        "\"plain\"",
        "u8\"\u{e9}\\u00e9\\N{GREEK SMALL LETTER ALPHA}\"",
        "u\"\\x{D800}\\U0001F600\"",
        "U\"\\o{17777777777}\"",
        "L\"\\0\\00\\000\\1234\"",
    ] {
        let a = decode_string(sp, X86);
        assert!(a.diags.is_empty(), "{sp}: {:?}", a.diags);
        let spelled = encode_string(&a.units, a.encoding);
        let b = decode_string(&spelled, X86);
        assert_eq!(a.units, b.units, "{sp}");
    }
}

/// [lex.phases]/1.2: a literal diagnostic's spelling offsets map back to
/// the source through splices and trigraphs (`LitDiag::locate`).
#[test]
fn lex_string_diag_locate_through_splices() {
    use kiln_lex::{Interner, LexOptions, Lexer, TokKind};
    use kiln_source::SourceManager;
    // Source:  x "a\<newline>\x100??/q"   (trigraph `??/` is `\`).
    let src = "x \"a\\\n\\x100??/q\"\n";
    let sm = SourceManager::new();
    let file = sm.add_virtual("<t>", src.to_owned()).unwrap();
    let interner = Interner::new();
    let lopts = LexOptions { trigraphs: true, ..LexOptions::default() };
    let mut lx = Lexer::new(&sm, &interner, file, sm.loc(file, 0), lopts);
    let _x = lx.next();
    let tok = lx.next();
    assert_eq!(tok.kind, TokKind::StringLit(Encoding::Ordinary));
    let spelling = lx.spelling(&tok).into_owned();
    assert_eq!(spelling, "\"a\\x100\\q\"");
    let lit = decode_string(&spelling, LiteralOptions::default());
    assert_eq!(kinds(&lit.diags), [D::HexEscapeTooLarge, D::UnknownEscape { ch: 'q' }]);
    let text = sm.span_text(tok.span).unwrap();
    let at = |d: &kiln_lex::literal::LitDiag| {
        let l = d.locate(&tok, text, &lopts);
        (sm.offset_of(l.span.lo).unwrap().1, sm.offset_of(l.span.hi).unwrap().1)
    };
    // `\x100` is source bytes 6..11; `??/q` is 11..15.
    assert_eq!(at(&lit.diags[0]), (6, 11));
    assert_eq!(at(&lit.diags[1]), (11, 15));
}
