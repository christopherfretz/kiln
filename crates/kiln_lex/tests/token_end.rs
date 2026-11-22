//! `kiln_lex::token_end` at file locations (spec/lex.md "End of a
//! token"): exactly one token is lexed from the location, with phases
//! 1-2 applied ([lex.phases], [lex.pptoken]).

use kiln_lex::{Interner, LexOptions, Lexer, Std, TokKind, token_end};
use kiln_source::{Loc, SourceManager};

/// The byte length of the token at each offset in `at` of `src`, or
/// `None`, under `opts`.
fn lens(src: &str, at: &[usize], opts: LexOptions) -> Vec<Option<u32>> {
    let sm = SourceManager::new();
    let interner = Interner::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    at.iter()
        .map(|&o| {
            let lo = sm.loc(f, o as u32);
            token_end(&sm, &interner, lo, opts).map(|e| sm.same_entry_distance(lo, e).unwrap())
        })
        .collect()
}

fn cxx() -> LexOptions {
    LexOptions::new(Std::Cxx26)
}

/// [lex.pptoken]/3: the longest token, whatever its kind.
#[test]
fn lex_pptoken_longest_token() {
    let src = "a1_b >>= 1'000 1.5e+3x \"s\\\"t\" 'c' u8R\"d(r)\")d\" <=> ... .5 ->* x";
    let at = |s: &str| src.find(s).unwrap();
    let got = lens(
        src,
        &[
            0,
            at(">>="),
            at("1'000"),
            at("1.5e"),
            at("\"s"),
            at("'c'"),
            at("u8R"),
            at("<=>"),
            at("..."),
            at(" .5") + 1,
            at("->*"),
        ],
        cxx(),
    );
    let want = [4, 3, 5, 7, 6, 3, 12, 3, 3, 2, 3];
    assert_eq!(got, want.map(Some));
}

/// [lex.phases]/1.2: a token continued over a `\`-newline ends after
/// its last spliced character.
#[test]
fn lex_phases_splice_inside_a_token() {
    assert_eq!(lens("ab\\\ncd e", &[0], cxx()), [Some(6)]);
    assert_eq!(lens("x +\\\n= y", &[2], cxx()), [Some(4)]);
}

/// No token starts at whitespace, a comment, the end of the buffer or
/// inside a UTF-8 character.
#[test]
fn no_token_starts_there() {
    let src = "a /* c */ // d\n\u{e9}t";
    let at = |s: &str| src.find(s).unwrap();
    assert_eq!(
        lens(src, &[1, at("/*"), at("//"), src.len(), at("\u{e9}") + 1], cxx()),
        [None, None, None, None, None]
    );
    assert_eq!(
        lens(src, &[at("\u{e9}")], cxx()),
        [Some(3)],
        "an identifier with a non-ASCII start"
    );
}

/// The language decides the token: digit separators are C++14.
#[test]
fn lex_pptoken_follows_the_language() {
    assert_eq!(lens("1'0'", &[0], LexOptions::new(Std::Cxx11)), [Some(1)]);
    assert_eq!(lens("1'0'", &[0], cxx()), [Some(3)]);
}

/// A location in an inclusion range ends in that range, and the token is
/// the one the lexer forms there.
#[test]
fn inclusion_ranges_and_lexer_agreement() {
    let sm = SourceManager::new();
    let interner = Interner::new();
    let src = "int x = y <<= 0x1p-3 ... R\"(a\nb)\" @ $q;";
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let inc = sm.add_inclusion(f, sm.loc(f, 0)).unwrap();
    let mut lx = Lexer::new(&sm, &interner, f, sm.loc(f, 0), cxx());
    loop {
        let t = lx.next();
        if t.kind == TokKind::Eof {
            break;
        }
        assert_eq!(token_end(&sm, &interner, t.span.lo, cxx()), Some(t.span.hi));
        let lo = Loc::from_raw(inc.to_raw() + sm.offset_of(t.span.lo).unwrap().1);
        let hi = token_end(&sm, &interner, lo, cxx()).unwrap();
        assert_eq!(sm.same_entry_distance(lo, hi), sm.same_entry_distance(t.span.lo, t.span.hi));
    }
    assert_eq!(token_end(&sm, &interner, Loc::INVALID, cxx()), None);
    assert_eq!(token_end(&sm, &interner, Loc::from_raw(u32::MAX), cxx()), None);
}
