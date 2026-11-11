//! Operators and punctuators: [lex.operators], [lex.digraph],
//! [lex.pptoken]/3.
mod common;
use common::{cxx26, diag_kinds, dump_of, kinds, kinds_with, lex, lex_with, spellings, std};
use kiln_lex::{LexDiagKind as D, LexOptions, Punct, Std, TokFlags, TokKind};

/// [lex.pptoken]/3.3: maximal munch.
#[test]
fn lex_pptoken_maximal_munch() {
    assert_eq!(spellings("a+++++b\n"), "a ++ ++ + b");
    assert_eq!(
        spellings("x<<=y>>=z ->* .* ... .. &&& |||\n"),
        "x <<= y >>= z ->* .* ... . . && & || |"
    );
}

/// [lex.operators]/1: every operator and punctuator lexes as itself.
#[test]
fn lex_operators_all() {
    for p in Punct::ALL {
        let s = p.as_str();
        let l = lex_with(&format!("{s}\n"), refl(Std::Cxx26));
        assert_eq!(l.tokens.len(), 1, "{s}");
        assert_eq!(l.tokens[0].kind, TokKind::Punct(*p), "{s}");
        assert_eq!(l.spellings[0], s);
    }
}

/// [lex.digraph]: alternative tokens behave as their primary spellings.
#[test]
fn lex_digraph() {
    let l = lex("<% %> <: :> %: %:%:\n");
    let expect = [
        Punct::LBrace,
        Punct::RBrace,
        Punct::LSquare,
        Punct::RSquare,
        Punct::Hash,
        Punct::HashHash,
    ];
    for (t, p) in l.tokens.iter().zip(expect) {
        assert_eq!(t.kind, TokKind::Punct(p));
        assert!(t.has(TokFlags::DIGRAPH));
    }
    assert_eq!(l.spellings, ["<%", "%>", "<:", ":>", "%:", "%:%:"]);
    assert_eq!(spellings("%:%\n"), "%: %");
}

/// [lex.pptoken]/3.2: `<::` not followed by `:` or `>` is `<` `::`.
#[test]
fn lex_pptoken_less_colon_colon() {
    assert_eq!(spellings("a<::b>\n"), "a < :: b >");
    assert_eq!(spellings("<:::\n"), "<: ::");
    assert_eq!(spellings("<::>\n"), "<: :>");
    assert_eq!(kinds_with("<::b\n", std(Std::Cxx03)), "l_square colon identifier");
}

/// `-freflection` options for `std`.
fn refl(s: Std) -> LexOptions {
    LexOptions { reflection: true, ..std(s) }
}

/// [lex.pptoken]/3.2 (C++26, P2996): `[::` is `[` `::` unless followed by `:`,
/// and `[:>` is `[` `:>`. The splice tokens exist only with `-freflection`.
#[test]
fn lex_pptoken_splice_brackets() {
    let r = refl(Std::Cxx26);
    let sp = |t: &str| lex_with(t, r).spellings.join(" ");
    assert_eq!(sp("[: x :]\n"), "[: x :]");
    assert_eq!(kinds_with("[: :]\n", r), "l_splice r_splice");
    assert_eq!(sp("[::x]\n"), "[ :: x ]");
    assert_eq!(sp("[:::]\n"), "[: :: ]");
    assert_eq!(sp("[:>\n"), "[ :>");
    assert_eq!(kinds_with("[: :]\n", refl(Std::Cxx23)), "l_square colon colon r_square");
    assert_eq!(kinds("[: :]\n"), "l_square colon colon r_square");
}

/// [lex.operators] (C++26, P2996): `^^` is a reflection operator token
/// (only with `-freflection`).
#[test]
fn lex_operators_reflect() {
    let r = refl(Std::Cxx26);
    assert_eq!(kinds_with("^^x ^^^\n", r), "caretcaret identifier caretcaret caret");
    assert_eq!(kinds_with("^^x\n", refl(Std::Cxx23)), "caret caret identifier");
    assert_eq!(kinds("^^x\n"), "caret caret identifier");
}

/// [lex.operators] (C++20): `<=>`; before C++20 it is `<=` `>`.
#[test]
fn lex_operators_spaceship() {
    assert_eq!(kinds("a<=>b\n"), "identifier spaceship identifier");
    assert_eq!(kinds_with("a<=>b\n", std(Std::Cxx17)), "identifier lessequal greater identifier");
    assert!(diag_kinds("a<=>b\n", std(Std::Cxx17)).contains(&D::SpaceshipCompat));
}

/// [lex.pptoken]/2: other single characters.
#[test]
fn lex_pptoken_other_char() {
    assert_eq!(kinds("@ ` \\ x\n"), "other other other identifier");
    assert_eq!(dump_of("@\n"), "other '@' [StartOfLine]\n");
}

/// [lex.pptoken]/2: a stray non-ASCII character is ill-formed. Outside a
/// directive and -E it is diagnosed and dropped (clang's recovery); under
/// -E it is a silent "other" token.
#[test]
fn lex_pptoken_stray_non_ascii() {
    let l = lex("a \u{2603} b\n");
    assert_eq!(l.spellings, ["a", "b"]);
    assert_eq!(l.diags[0].0, D::InvalidChar { ch: '\u{2603}' });
    assert_eq!(diag_kinds("\u{2603}\n", cxx26()).len(), 1);
    let e = lex_with("a \u{2603} b\n", LexOptions { preprocessed_output: true, ..cxx26() });
    assert_eq!(e.spellings, ["a", "\u{2603}", "b"]);
    assert!(e.diags.is_empty());
}
