//! The token table (spec/parse.md §2.6, WP-2825): with
//! `ParseOptions::token_table` every token the preprocessor delivers to
//! the parser ([lex.pptoken], phase 4's output) is a row, in order, with
//! the kind the parser saw, its location, the length of its spelling and
//! where it came from; a `>>` the parser split for [temp.names]/4 is the
//! two `>`s it consumed; `starts_token` answers per file.

use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_lex::{Punct, TokKind};
use kiln_parse::{ConstexprLimits, ParseOptions, ParsedTu, parse_preprocessed};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, SourceManager};

/// Parses `src` (as `t.cpp`) with the table on or off and calls `f` with
/// the source manager, the file and the parsed unit. No error expected.
fn with_table<R>(src: &str, on: bool, f: impl FnOnce(&SourceManager, FileId, &ParsedTu) -> R) -> R {
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { token_table: on, ..ParseOptions::default() };
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.enter_main_file(file);
    let target = Target::for_arch(Arch::X86_64);
    let (tu, pp) = parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        ConstexprLimits::default(),
    );
    drop(pp);
    let errors: Vec<String> = sink
        .diagnostics()
        .iter()
        .filter(|d| d.level >= Level::Error)
        .map(|d| d.message.clone())
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    f(&sm, file, &tu)
}

/// The byte offset of the `n`th (0-based) occurrence of `needle` in `src`.
fn nth(src: &str, needle: &str, n: usize) -> u32 {
    let at = src.match_indices(needle).nth(n).unwrap_or_else(|| panic!("no {needle:?} #{n}")).0;
    u32::try_from(at).unwrap()
}

fn at(src: &str, needle: &str) -> u32 {
    nth(src, needle, 0)
}

/// The rows as `(kind, file offset or None, length, macro, macro arg)`.
type Row = (TokKind, Option<u32>, u32, bool, bool);

fn rows(sm: &SourceManager, tu: &ParsedTu) -> Vec<Row> {
    let t = tu.token_table.as_ref().expect("the table is on");
    t.rows()
        .iter()
        .map(|r| {
            let off = sm.offset_of(r.loc()).map(|(_, o)| o);
            (r.kind(), off, r.len(), r.from_macro(), r.from_macro_arg())
        })
        .collect()
}

fn p(p: Punct) -> TokKind {
    TokKind::Punct(p)
}

#[test]
fn plain_tokens_kinds_lengths_and_count() {
    let src = "int x = 1 + 22;\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        assert_eq!(t.len() as u64, tu.tokens, "no split: one row per delivered token");
        assert_eq!(t.delivered(), tu.tokens);
        let got = rows(sm, tu);
        let kinds: Vec<TokKind> = got.iter().map(|r| r.0).collect();
        assert!(matches!(kinds[0], TokKind::Identifier(_)));
        assert!(matches!(kinds[1], TokKind::Identifier(_)));
        assert_eq!(
            kinds[2..],
            [p(Punct::Equal), TokKind::PpNumber, p(Punct::Plus), TokKind::PpNumber, p(Punct::Semi)]
        );
        let shape: Vec<(Option<u32>, u32)> = got.iter().map(|r| (r.1, r.2)).collect();
        assert_eq!(
            shape,
            [
                (Some(0), 3),
                (Some(4), 1),
                (Some(6), 1),
                (Some(8), 1),
                (Some(10), 1),
                (Some(12), 2),
                (Some(14), 1)
            ]
        );
        assert!(got.iter().all(|r| !r.3 && !r.4));
        // The query: every start, nothing mid-token or in whitespace.
        for o in [0, 4, 6, 8, 10, 12, 14] {
            assert!(t.starts_token(file, o), "offset {o}");
        }
        for o in [1, 2, 3, 5, 7, 13, 15, 16, 1000] {
            assert!(!t.starts_token(file, o), "offset {o}");
        }
        assert_eq!(t.token_at(file, 13), Some(5), "inside `22`");
        assert_eq!(t.token_at(file, 2), Some(0), "inside `int`");
        assert_eq!(t.token_at(file, 3), None, "whitespace");
        let pairs: Vec<(u32, usize)> = t.file_tokens(file).collect();
        assert_eq!(pairs.len(), 7);
    });
}

#[test]
fn off_by_default_nothing_is_recorded() {
    assert!(!ParseOptions::default().token_table);
    with_table("int x = 1;\n", false, |_, _, tu| {
        assert!(tu.token_table.is_none());
        assert_eq!(tu.tokens, 5);
        assert!(!tu.memory().rows.iter().any(|r| r.name == "token_table"));
    });
    with_table("int x = 1;\n", true, |_, _, tu| {
        assert!(tu.memory().rows.iter().any(|r| r.name == "token_table" && r.count == 5));
    });
}

#[test]
fn a_macro_expansion_is_not_a_file_token() {
    let src = "#define M int y = 2;\nM\nint z;\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        assert_eq!(t.len() as u64, tu.tokens);
        let got = rows(sm, tu);
        // `int y = 2 ;` from the replacement list: macro tokens, each the
        // length of its spelling, no file offset; then `int z ;`.
        assert_eq!(got.len(), 8);
        for r in &got[..5] {
            assert!(r.3 && !r.4, "{r:?}");
            assert_eq!(r.1, None);
        }
        assert_eq!(got[..5].iter().map(|r| r.2).collect::<Vec<_>>(), [3, 1, 1, 1, 1]);
        assert_eq!(got[5].1, Some(at(src, "int z")));
        // The replacement list's `int` and the invocation `M` start no
        // token of the table: neither was delivered as written.
        assert!(!t.starts_token(file, at(src, "int y")));
        assert!(!t.starts_token(file, at(src, "\nM\n") + 1));
        assert!(t.starts_token(file, at(src, "int z")));
    });
}

#[test]
fn macro_argument_tokens_start_where_they_are_written() {
    let src = "#define ID(x) x\n#define TWICE(x) x + x\nint a = ID(3 + 40);\nint b = TWICE(5);\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        let got = rows(sm, tu);
        let arg: Vec<&Row> = got.iter().filter(|r| r.4).collect();
        // `3 + 40` once, `5` twice; all macro tokens.
        assert_eq!(arg.len(), 5, "{got:?}");
        assert!(arg.iter().all(|r| r.3 && r.1.is_none()));
        assert_eq!(arg[2].2, 2, "`40`");
        for needle in ["3 + 40", "+ 40", "40)", "5)"] {
            assert!(t.starts_token(file, at(src, needle)), "{needle}");
        }
        // The invocation's name and parentheses were not delivered; the
        // replacement list's `x` and `+` are not file tokens.
        for o in [at(src, "ID(3"), at(src, "(3"), at(src, "40)") + 2, at(src, "x + x")] {
            assert!(!t.starts_token(file, o), "offset {o}");
        }
        assert!(!t.starts_token(file, at(src, "+ x")));
        assert_eq!(
            t.token_at(file, at(src, "40)") + 1),
            Some(got.iter().position(|r| r.2 == 2 && r.4).unwrap())
        );
        // `5` is spelled once and delivered twice: two entries, one start.
        let five = at(src, "5)");
        assert_eq!(t.file_tokens(file).filter(|&(o, _)| o == five).count(), 2);
    });
}

#[test]
fn a_string_concatenation_is_its_literals() {
    let src = "const char *s = \"ab\" \"cde\" R\"(f)\";\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        let got = rows(sm, tu);
        let lits: Vec<&Row> = got
            .iter()
            .filter(|r| matches!(r.0, TokKind::StringLit(_) | TokKind::RawStringLit(_)))
            .collect();
        assert_eq!(lits.len(), 3);
        assert_eq!(lits.iter().map(|r| r.2).collect::<Vec<_>>(), [4, 5, 6]);
        assert!(t.starts_token(file, at(src, "\"cde")));
        assert!(!t.starts_token(file, at(src, "\"cde") + 2));
    });
}

const TEMPLATES: &str = "template <class T> struct A {};\n\
template <class T> struct B {};\n\
template <class T> int v = 0;\n";

#[test]
fn a_split_greater_greater_is_two_tokens() {
    let src = format!("{TEMPLATES}A<B<int>> x;\n");
    with_table(&src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        assert_eq!(t.delivered(), tu.tokens);
        assert_eq!(t.len() as u64, tu.tokens + 1, "one split");
        let gg = at(&src, ">> x");
        let got = rows(sm, tu);
        let i = got.iter().position(|r| r.1 == Some(gg)).unwrap();
        assert_eq!(got[i], (p(Punct::Greater), Some(gg), 1, false, false));
        assert_eq!(got[i + 1], (p(Punct::Greater), Some(gg + 1), 1, false, false));
        assert!(t.starts_token(file, gg));
        assert!(t.starts_token(file, gg + 1), "the second `>` is a token of its own");
        assert_eq!(t.token_at(file, gg + 1), Some(i + 1));
    });
}

#[test]
fn split_greater_equal_and_greater_greater_equal() {
    // `v<A<int>>= 1`: `>>=` split twice, `>` `>` `=`; `v<int>>= 1`: once,
    // `>` `>=` (the comparison); `v<int>= 2`: `>=` split, `>` `=`.
    let src = format!(
        "{TEMPLATES}int f() {{ return v<A<int>>= 1; }}\n\
         bool g() {{ return v<int>>= 1; }}\n\
         int h() {{ return v<int>= 2; }}\n"
    );
    with_table(&src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        assert_eq!(t.len() as u64, tu.tokens + 4, "2 + 1 + 1 splits");
        let got = rows(sm, tu);
        let from = |o: u32| -> Vec<(TokKind, Option<u32>, u32)> {
            let i = got.iter().position(|r| r.1 == Some(o)).unwrap();
            got[i..i + 3].iter().map(|r| (r.0, r.1, r.2)).collect()
        };
        let a = at(&src, ">>= 1; }\nbool");
        assert_eq!(
            from(a),
            [
                (p(Punct::Greater), Some(a), 1),
                (p(Punct::Greater), Some(a + 1), 1),
                (p(Punct::Equal), Some(a + 2), 1)
            ]
        );
        let b = nth(&src, ">>= 1", 1);
        assert_eq!(
            from(b)[..2],
            [(p(Punct::Greater), Some(b), 1), (p(Punct::GreaterEqual), Some(b + 1), 2)]
        );
        let c = at(&src, ">= 2");
        assert_eq!(
            from(c)[..2],
            [(p(Punct::Greater), Some(c), 1), (p(Punct::Equal), Some(c + 1), 1)]
        );
        for o in [a, a + 1, a + 2, b, b + 1, c, c + 1] {
            assert!(t.starts_token(file, o), "offset {o}");
        }
        assert!(!t.starts_token(file, b + 2), "inside the `>=` left whole");
    });
}

#[test]
fn a_skipped_group_has_no_tokens() {
    let src = "#if 0\nint skipped = 1;\n#endif\nint kept;\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        assert_eq!(tu.tokens, 3);
        let kept = at(src, "kept;");
        let offsets: Vec<Option<u32>> = rows(sm, tu).iter().map(|r| r.1).collect();
        assert_eq!(offsets, [Some(kept - 4), Some(kept), Some(kept + 4)]);
        let skipped = at(src, "int skipped");
        for o in [0, 1, skipped, skipped + 4, at(src, "#endif")] {
            assert!(!t.starts_token(file, o), "offset {o}");
            assert_eq!(t.token_at(file, o), None);
        }
        assert!(t.starts_token(file, at(src, "int kept")));
    });
}

#[test]
fn a_user_defined_literal_is_one_token() {
    let src = "int operator\"\"_k(unsigned long long);\nint u = 12_k;\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        let lit = at(src, "12_k");
        let got = rows(sm, tu);
        let r = got.iter().find(|r| r.1 == Some(lit)).unwrap();
        assert_eq!((r.0, r.2), (TokKind::PpNumber, 4));
        assert!(t.starts_token(file, lit));
        assert!(!t.starts_token(file, lit + 2), "the suffix starts no token");
    });
}

#[test]
fn digraphs_and_alternative_tokens() {
    let src = "struct D <% int m<:2:>; %>;\nbool c = true and false;\n";
    with_table(src, true, |sm, file, tu| {
        let t = tu.token_table.as_ref().unwrap();
        let got = rows(sm, tu);
        let find = |n: &str| got.iter().find(|r| r.1 == Some(at(src, n))).map(|r| (r.0, r.2));
        assert_eq!(find("<%"), Some((p(Punct::LBrace), 2)));
        assert_eq!(find("<:"), Some((p(Punct::LSquare), 2)));
        assert_eq!(find(":>"), Some((p(Punct::RSquare), 2)));
        assert_eq!(find("%>"), Some((p(Punct::RBrace), 2)));
        assert_eq!(find("and"), Some((p(Punct::AmpAmp), 3)));
        assert!(t.starts_token(file, at(src, "<%")));
        assert!(!t.starts_token(file, at(src, "<%") + 1));
    });
}
