//! In-crate golden token dumps (`kind 'spelling' [flags]`, see [`crate::dump`]).
//!
//! The expected dumps were cross-checked against `clang-19 -cc1 -std=c++26
//! -dump-tokens`; they agree except for the documented deviations in
//! `spec/lex.md` (C++26 reflection tokens, `p+` in pp-numbers, kiln-only
//! kind names and flags such as `other`, `raw_string_literal`, `[UnClean]`).

#![allow(clippy::too_many_lines, reason = "golden data")]

use std::fmt::Write as _;

use kiln_source::SourceManager;

use crate::literal::{LiteralOptions, NumberClass, classify_number, decode_char, decode_string};
use crate::{Interner, LexOptions, Lexer, TokKind, dump_source};

fn check(src: &str, expected: &[&str]) {
    let got = dump_source(src, LexOptions::default());
    let want: String = expected.concat();
    assert_eq!(got, want, "\n--- got ---\n{got}");
}

/// One line per literal token of `src`: its spelling and what the
/// `literal` module makes of it (x86-64 Linux, C++26).
fn literal_summary(src: &str) -> String {
    let sm = SourceManager::new();
    let file = sm.add_virtual("<golden>", src.to_owned()).expect("small");
    let interner = Interner::new();
    let opts = LexOptions::default();
    let mut lx = Lexer::new(&sm, &interner, file, sm.loc(file, 0), opts);
    let lopts = LiteralOptions::default();
    let mut out = String::new();
    loop {
        let t = lx.next();
        let sp = lx.spelling(&t);
        let diags = |d: &[crate::literal::LitDiag]| {
            d.iter().fold(String::new(), |mut s, d| {
                let _ = write!(s, " {:?}", d.kind);
                s
            })
        };
        let line = match t.kind {
            TokKind::Eof => break,
            TokKind::StringLit(_) | TokKind::RawStringLit(_) => {
                let l = decode_string(&sp, lopts);
                let units: Vec<String> = l.units.iter().map(|u| format!("{u:X}")).collect();
                format!("[{}] sfx={:?}{}", units.join(" "), l.ud_suffix, diags(&l.diags))
            }
            TokKind::CharLit(_) => {
                let l = decode_char(&sp, lopts);
                format!("{:?} {} pp={}{}", l.ty, l.value, l.pp_value, diags(&l.diags))
            }
            TokKind::PpNumber => {
                let n = classify_number(&sp, lopts.std);
                let class = match &n.class {
                    NumberClass::Integer(i) => {
                        format!("int {:?} {:?} sfx={:?}", i.value(), i.suffix, i.ud_suffix)
                    }
                    NumberClass::Floating(f) => {
                        format!("float {:e} {:?} sfx={:?}", f.parse_f64(), f.suffix, f.ud_suffix)
                    }
                    NumberClass::Invalid => "invalid".to_owned(),
                };
                format!("{class}{}", diags(&n.diags))
            }
            _ => continue,
        };
        let _ = writeln!(out, "{} => {line}", sp.replace('\n', "\\n"));
    }
    out
}

/// [lex.pptoken], [lex.phases]: A realistic mix: directives, templates, `<::`, splices, comments,
/// digraphs, literals, UCNs and C++26 reflection tokens.
#[test]
fn golden_lex_pptoken_mixed() {
    check(
        "#include <vector>\ntemplate <class T> auto f(T t) -> decltype(t <=> t) { return t[0] <:1:>; }\nint a<::b>::c; x\\\ny  /* c\n */ z\n  q <=> %:%: <: :> u8\"x\"_s R\"d(a\\\nb)d\" 1e-5'0 a\\u00e9b \"abc\"xyz \"s\"s\nconstexpr auto r = ^^int; typename [: r :] v = 0x1.8p+3;\n",
        &[
            "hash '#' [StartOfLine]\n",
            "identifier 'include'\n",
            "less '<' [LeadingSpace]\n",
            "identifier 'vector'\n",
            "greater '>'\n",
            "identifier 'template' [StartOfLine]\n",
            "less '<' [LeadingSpace]\n",
            "identifier 'class'\n",
            "identifier 'T' [LeadingSpace]\n",
            "greater '>'\n",
            "identifier 'auto' [LeadingSpace]\n",
            "identifier 'f' [LeadingSpace]\n",
            "l_paren '('\n",
            "identifier 'T'\n",
            "identifier 't' [LeadingSpace]\n",
            "r_paren ')'\n",
            "arrow '->' [LeadingSpace]\n",
            "identifier 'decltype' [LeadingSpace]\n",
            "l_paren '('\n",
            "identifier 't'\n",
            "spaceship '<=>' [LeadingSpace]\n",
            "identifier 't' [LeadingSpace]\n",
            "r_paren ')'\n",
            "l_brace '{' [LeadingSpace]\n",
            "identifier 'return' [LeadingSpace]\n",
            "identifier 't' [LeadingSpace]\n",
            "l_square '['\n",
            "numeric_constant '0'\n",
            "r_square ']'\n",
            "l_square '<:' [LeadingSpace] [Digraph]\n",
            "numeric_constant '1'\n",
            "r_square ':>' [Digraph]\n",
            "semi ';'\n",
            "r_brace '}' [LeadingSpace]\n",
            "identifier 'int' [StartOfLine]\n",
            "identifier 'a' [LeadingSpace]\n",
            "less '<'\n",
            "coloncolon '::'\n",
            "identifier 'b'\n",
            "greater '>'\n",
            "coloncolon '::'\n",
            "identifier 'c'\n",
            "semi ';'\n",
            "identifier 'xy' [LeadingSpace] [UnClean]\n",
            "identifier 'z' [LeadingSpace]\n",
            "identifier 'q' [StartOfLine] [LeadingSpace]\n",
            "spaceship '<=>' [LeadingSpace]\n",
            "hashhash '%:%:' [LeadingSpace] [Digraph]\n",
            "l_square '<:' [LeadingSpace] [Digraph]\n",
            "r_square ':>' [LeadingSpace] [Digraph]\n",
            "utf8_string_literal 'u8\"x\"_s' [LeadingSpace] [UDSuffix]\n",
            "raw_string_literal 'R\"d(a\\\\nb)d\"' [LeadingSpace]\n",
            "numeric_constant '1e-5'0' [LeadingSpace]\n",
            "identifier 'a\\u00e9b' [LeadingSpace] [UCN]\n",
            "string_literal '\"abc\"' [LeadingSpace]\n",
            "identifier 'xyz'\n",
            "string_literal '\"s\"s' [LeadingSpace] [UDSuffix]\n",
            "identifier 'constexpr' [StartOfLine]\n",
            "identifier 'auto' [LeadingSpace]\n",
            "identifier 'r' [LeadingSpace]\n",
            "equal '=' [LeadingSpace]\n",
            "caretcaret '^^' [LeadingSpace]\n",
            "identifier 'int'\n",
            "semi ';'\n",
            "identifier 'typename' [LeadingSpace]\n",
            "l_splice '[:' [LeadingSpace]\n",
            "identifier 'r' [LeadingSpace]\n",
            "r_splice ':]' [LeadingSpace]\n",
            "identifier 'v' [LeadingSpace]\n",
            "equal '=' [LeadingSpace]\n",
            "numeric_constant '0x1.8p+3' [LeadingSpace]\n",
            "semi ';'\n",
            "eof '' [StartOfLine]\n",
        ],
    );
}

/// [lex.pptoken]/3, [lex.digraph], [lex.ppnumber], [lex.string]: Maximal munch, digraphs, pp-numbers,
/// every literal prefix, UCNs and comments.
#[test]
fn golden_lex_pptoken_munch_numbers_literals() {
    check(
        "a+++++b a-->b a->*b .* ... .. . x<<=y>>=z <<<>>> &&&|||\n<% %> <: :> %: %:%: %:% <:: <::: <::> [:: [:> [: :] ^^x ^ ^= ^^^\n0x1p-3 1.e+5 .5e-x 1_a 0b1'0'1 1''2 1'a 1e 1.2.3.4 0xe+1 1p+2 12ns\nu8'a' u'x' U'y' L'z' 'ab' u8R\"(x)\" LR\"x(y)x\" uR\"(a)\" UR\"(b)\" R\"()\" \"x\"_y \"x\" _y\n'a'_q \"\\\"\" '\\'' \"\\\\\" L\"w\" u\"\\u0041\" a\\U0001F600b \u{300}x \\u0300y \\u00e9t\n/**/x/***/y/*/ */z // line \\\ncont\nw\n\ttab\tf\n",
        &[
            "identifier 'a' [StartOfLine]\n",
            "plusplus '++'\n",
            "plusplus '++'\n",
            "plus '+'\n",
            "identifier 'b'\n",
            "identifier 'a' [LeadingSpace]\n",
            "minusminus '--'\n",
            "greater '>'\n",
            "identifier 'b'\n",
            "identifier 'a' [LeadingSpace]\n",
            "arrowstar '->*'\n",
            "identifier 'b'\n",
            "periodstar '.*' [LeadingSpace]\n",
            "ellipsis '...' [LeadingSpace]\n",
            "period '.' [LeadingSpace]\n",
            "period '.'\n",
            "period '.' [LeadingSpace]\n",
            "identifier 'x' [LeadingSpace]\n",
            "lesslessequal '<<='\n",
            "identifier 'y'\n",
            "greatergreaterequal '>>='\n",
            "identifier 'z'\n",
            "lessless '<<' [LeadingSpace]\n",
            "less '<'\n",
            "greatergreater '>>'\n",
            "greater '>'\n",
            "ampamp '&&' [LeadingSpace]\n",
            "amp '&'\n",
            "pipepipe '||'\n",
            "pipe '|'\n",
            "l_brace '<%' [StartOfLine] [Digraph]\n",
            "r_brace '%>' [LeadingSpace] [Digraph]\n",
            "l_square '<:' [LeadingSpace] [Digraph]\n",
            "r_square ':>' [LeadingSpace] [Digraph]\n",
            "hash '%:' [LeadingSpace] [Digraph]\n",
            "hashhash '%:%:' [LeadingSpace] [Digraph]\n",
            "hash '%:' [LeadingSpace] [Digraph]\n",
            "percent '%'\n",
            "less '<' [LeadingSpace]\n",
            "coloncolon '::'\n",
            "l_square '<:' [LeadingSpace] [Digraph]\n",
            "coloncolon '::'\n",
            "l_square '<:' [LeadingSpace] [Digraph]\n",
            "r_square ':>' [Digraph]\n",
            "l_square '[' [LeadingSpace]\n",
            "coloncolon '::'\n",
            "l_square '[' [LeadingSpace]\n",
            "r_square ':>' [Digraph]\n",
            "l_splice '[:' [LeadingSpace]\n",
            "r_splice ':]' [LeadingSpace]\n",
            "caretcaret '^^' [LeadingSpace]\n",
            "identifier 'x'\n",
            "caret '^' [LeadingSpace]\n",
            "caretequal '^=' [LeadingSpace]\n",
            "caretcaret '^^' [LeadingSpace]\n",
            "caret '^'\n",
            "numeric_constant '0x1p-3' [StartOfLine]\n",
            "numeric_constant '1.e+5' [LeadingSpace]\n",
            "numeric_constant '.5e-x' [LeadingSpace]\n",
            "numeric_constant '1_a' [LeadingSpace]\n",
            "numeric_constant '0b1'0'1' [LeadingSpace]\n",
            "numeric_constant '1' [LeadingSpace]\n",
            "unknown ''''\n",
            "numeric_constant '2'\n",
            "numeric_constant '1'a' [LeadingSpace]\n",
            "numeric_constant '1e' [LeadingSpace]\n",
            "numeric_constant '1.2.3.4' [LeadingSpace]\n",
            "numeric_constant '0xe+1' [LeadingSpace]\n",
            "numeric_constant '1p+2' [LeadingSpace]\n",
            "numeric_constant '12ns' [LeadingSpace]\n",
            "utf8_char_constant 'u8'a'' [StartOfLine]\n",
            "utf16_char_constant 'u'x'' [LeadingSpace]\n",
            "utf32_char_constant 'U'y'' [LeadingSpace]\n",
            "wide_char_constant 'L'z'' [LeadingSpace]\n",
            "char_constant ''ab'' [LeadingSpace]\n",
            "utf8_raw_string_literal 'u8R\"(x)\"' [LeadingSpace]\n",
            "wide_raw_string_literal 'LR\"x(y)x\"' [LeadingSpace]\n",
            "utf16_raw_string_literal 'uR\"(a)\"' [LeadingSpace]\n",
            "utf32_raw_string_literal 'UR\"(b)\"' [LeadingSpace]\n",
            "raw_string_literal 'R\"()\"' [LeadingSpace]\n",
            "string_literal '\"x\"_y' [LeadingSpace] [UDSuffix]\n",
            "string_literal '\"x\"' [LeadingSpace]\n",
            "identifier '_y' [LeadingSpace]\n",
            "char_constant ''a'_q' [StartOfLine] [UDSuffix]\n",
            "string_literal '\"\\\"\"' [LeadingSpace]\n",
            "char_constant ''\\''' [LeadingSpace]\n",
            "string_literal '\"\\\\\"' [LeadingSpace]\n",
            "wide_string_literal 'L\"w\"' [LeadingSpace]\n",
            "utf16_string_literal 'u\"\\u0041\"' [LeadingSpace]\n",
            "identifier 'a\\U0001F600b' [LeadingSpace] [UCN]\n",
            "other '\u{300}' [LeadingSpace]\n",
            "identifier 'x'\n",
            "unknown '\\u0300' [LeadingSpace] [UCN]\n",
            "identifier 'y'\n",
            "identifier '\\u00e9t' [LeadingSpace] [UCN]\n",
            "identifier 'x' [StartOfLine] [LeadingSpace]\n",
            "identifier 'y' [LeadingSpace]\n",
            "identifier 'z' [LeadingSpace]\n",
            "identifier 'w' [StartOfLine]\n",
            "identifier 'tab' [StartOfLine] [LeadingSpace]\n",
            "identifier 'f' [LeadingSpace]\n",
            "eof '' [StartOfLine]\n",
        ],
    );
}

/// [lex.pptoken]/2, [lex.phases]: Error recovery: `$`, disabled trigraphs, unterminated literals,
/// stray characters, Unicode whitespace, NUL and a bad raw string.
#[test]
fn golden_lex_pptoken_recovery() {
    check(
        "$abc a$b\n??= ??( ??/\n\"unterminated\n'q\n''\n@ ` \\ \u{2603} \u{3000} x\0y\nR\"abc\n",
        &[
            "identifier '$abc' [StartOfLine]\n",
            "identifier 'a$b' [LeadingSpace]\n",
            "question '?' [StartOfLine]\n",
            "question '?'\n",
            "equal '='\n",
            "question '?' [LeadingSpace]\n",
            "question '?'\n",
            "l_paren '('\n",
            "question '?' [LeadingSpace]\n",
            "question '?'\n",
            "slash '/'\n",
            "unknown '\"unterminated' [StartOfLine]\n",
            "unknown ''q' [StartOfLine]\n",
            "unknown '''' [StartOfLine]\n",
            "other '@' [StartOfLine]\n",
            "other '`' [LeadingSpace]\n",
            "other '\\' [LeadingSpace]\n",
            "other '\u{2603}' [LeadingSpace]\n",
            "identifier 'x' [LeadingSpace]\n",
            "identifier 'y' [LeadingSpace]\n",
            "unknown 'R\"abc\\n' [StartOfLine]\n",
            "eof ''\n",
        ],
    );
}

const LITERALS_SRC: &str = "auto a = u8\"\\N{GREEK SMALL LETTER ALPHA}\\x{41}\\o{101}\"_s + L\"\\U0001F600\" U\"\\u{1F600}\";\nchar b[] = {'\\0', '\\377', 'ab', u8'x', u'\\xFFFF', U'\\N{LATIN SMALL LETTER E WITH ACUTE}', L'\\e'};\nauto c = R\"x(a\\b\"c)x\"sv LR\"(\\N{X})\" u8R\"--(\n)--\"_raw;\nint d = 0b1010'1010uz + 0x1p-3f + 1.e+10L + 0x1.8p3 + 017'7ull + 1'000'000 + 2.5bf16 + 3f128 + 12_km;\n\"\\N{latin small letter a}\" '\\x{}' \"a\" \"b\" u\"c\" \"\\q\"\n";

/// [lex.ccon], [lex.string], [lex.icon], [lex.fcon], [lex.ext]: literal
/// tokens of every kind (prefixes, raw strings, ud-suffixes, delimited and
/// named escapes, digit separators, extended float suffixes, ill-formed
/// escapes). Token dump cross-checked against clang 19 `-dump-tokens`.
#[test]
fn golden_lex_literals_tokens() {
    check(
        LITERALS_SRC,
        &[
            "identifier 'auto' [StartOfLine]\n",
            "identifier 'a' [LeadingSpace]\n",
            "equal '=' [LeadingSpace]\n",
            "utf8_string_literal 'u8\"\\N{GREEK SMALL LETTER ALPHA}\\x{41}\\o{101}\"_s' [LeadingSpace] [UDSuffix]\n",
            "plus '+' [LeadingSpace]\n",
            "wide_string_literal 'L\"\\U0001F600\"' [LeadingSpace]\n",
            "utf32_string_literal 'U\"\\u{1F600}\"' [LeadingSpace]\n",
            "semi ';'\n",
            "identifier 'char' [StartOfLine]\n",
            "identifier 'b' [LeadingSpace]\n",
            "l_square '['\n",
            "r_square ']'\n",
            "equal '=' [LeadingSpace]\n",
            "l_brace '{' [LeadingSpace]\n",
            "char_constant ''\\0''\n",
            "comma ','\n",
            "char_constant ''\\377'' [LeadingSpace]\n",
            "comma ','\n",
            "char_constant ''ab'' [LeadingSpace]\n",
            "comma ','\n",
            "utf8_char_constant 'u8'x'' [LeadingSpace]\n",
            "comma ','\n",
            "utf16_char_constant 'u'\\xFFFF'' [LeadingSpace]\n",
            "comma ','\n",
            "utf32_char_constant 'U'\\N{LATIN SMALL LETTER E WITH ACUTE}'' [LeadingSpace]\n",
            "comma ','\n",
            "wide_char_constant 'L'\\e'' [LeadingSpace]\n",
            "r_brace '}'\n",
            "semi ';'\n",
            "identifier 'auto' [StartOfLine]\n",
            "identifier 'c' [LeadingSpace]\n",
            "equal '=' [LeadingSpace]\n",
            "raw_string_literal 'R\"x(a\\b\"c)x\"sv' [LeadingSpace] [UDSuffix]\n",
            "wide_raw_string_literal 'LR\"(\\N{X})\"' [LeadingSpace]\n",
            "utf8_raw_string_literal 'u8R\"--(\\n)--\"_raw' [LeadingSpace] [UDSuffix]\n",
            "semi ';'\n",
            "identifier 'int' [StartOfLine]\n",
            "identifier 'd' [LeadingSpace]\n",
            "equal '=' [LeadingSpace]\n",
            "numeric_constant '0b1010'1010uz' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '0x1p-3f' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '1.e+10L' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '0x1.8p3' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '017'7ull' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '1'000'000' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '2.5bf16' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '3f128' [LeadingSpace]\n",
            "plus '+' [LeadingSpace]\n",
            "numeric_constant '12_km' [LeadingSpace]\n",
            "semi ';'\n",
            "string_literal '\"\\N{latin small letter a}\"' [StartOfLine]\n",
            "char_constant ''\\x{}'' [LeadingSpace]\n",
            "string_literal '\"a\"' [LeadingSpace]\n",
            "string_literal '\"b\"' [LeadingSpace]\n",
            "utf16_string_literal 'u\"c\"' [LeadingSpace]\n",
            "string_literal '\"\\q\"' [LeadingSpace]\n",
            "eof '' [StartOfLine]\n",
        ],
    );
}

/// The same literals through `kiln_lex::literal` (x86-64 Linux, C++26):
/// values, types, `#if` values and diagnostics. Values cross-checked with
/// clang 19 `static_assert`s and `#if`; diagnostics with `-fsyntax-only`.
#[test]
fn golden_lex_literals_values() {
    let want: String = [
        "u8\"\\N{GREEK SMALL LETTER ALPHA}\\x{41}\\o{101}\"_s => [CE B1 41 41] sfx=Some(\"_s\")\n",
        "L\"\\U0001F600\" => [1F600] sfx=None\n",
        "U\"\\u{1F600}\" => [1F600] sfx=None\n",
        "'\\0' => Char 0 pp=0\n",
        "'\\377' => Char -1 pp=-1\n",
        "'ab' => Int 24930 pp=24930 MultiCharLiteral\n",
        "u8'x' => Char8 120 pp=120\n",
        "u'\\xFFFF' => Char16 65535 pp=65535\n",
        "U'\\N{LATIN SMALL LETTER E WITH ACUTE}' => Char32 233 pp=233\n",
        "L'\\e' => WChar 27 pp=27 NonstandardEscape { ch: 'e' }\n",
        "R\"x(a\\b\"c)x\"sv => [61 5C 62 22 63] sfx=Some(\"sv\")\n",
        "LR\"(\\N{X})\" => [5C 4E 7B 58 7D] sfx=None\n",
        "u8R\"--(\\n)--\"_raw => [A] sfx=Some(\"_raw\")\n",
        "0b1010'1010uz => int Some(170) IntSuffix { unsigned: true, size: Size } sfx=None\n",
        "0x1p-3f => float 1.25e-1 F sfx=None\n",
        "1.e+10L => float 1e10 L sfx=None\n",
        "0x1.8p3 => float 1.2e1 None sfx=None\n",
        "017'7ull => int Some(127) IntSuffix { unsigned: true, size: LongLong } sfx=None\n",
        "1'000'000 => int Some(1000000) IntSuffix { unsigned: false, size: None } sfx=None\n",
        "2.5bf16 => float 2.5e0 BF16 sfx=None\n",
        "3f128 => invalid InvalidDigit { ch: 'f', radix: 10 }\n",
        "12_km => int Some(12) IntSuffix { unsigned: false, size: None } sfx=Some(\"_km\")\n",
        "\"\\N{latin small letter a}\" => [61] sfx=None UcnNameLooseMatch { ch: 'a' }\n",
        "'\\x{}' => Int 125 pp=125 DelimitedEscapeEmpty MultiCharLiteral\n",
        "\"a\" => [61] sfx=None\n",
        "\"b\" => [62] sfx=None\n",
        "u\"c\" => [63] sfx=None\n",
        "\"\\q\" => [71] sfx=None UnknownEscape { ch: 'q' }\n",
    ]
    .concat();
    let got = literal_summary(LITERALS_SRC);
    assert_eq!(got, want, "\n--- got ---\n{got}");
}
