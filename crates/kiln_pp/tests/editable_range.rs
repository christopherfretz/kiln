//! Location utilities for tooling clients over real expansions
//! (spec/source.md "Editable file ranges" and "Expansion entries",
//! spec/lex.md "End of a token"): `SourceManager::editable_range`,
//! `SourceManager::expansion_entries` and `kiln_lex::token_end` on the
//! tokens the preprocessor produces ([cpp.replace], [cpp.subst],
//! [cpp.stringize], [cpp.concat], [cpp.rescan], [cpp.pragma.op]).
//!
//! Hand-written: clang's `Lexer::makeFileCharRange` is the model; a
//! wrong range would corrupt the file a client edits, so besides the
//! pinned cases every pair of tokens of each source is checked against
//! a soundness property.

use std::cell::RefCell;
use std::rc::Rc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, LexOptions, Std, TokKind, Token};
use kiln_pp::{MemoryResolver, PpCallbacks, PpOptions, PragmaInfo, Preprocessor};
use kiln_source::{ExpansionKind, FileId, Loc, SourceManager, Span};

/// What a run hands to the checks.
struct Run<'a> {
    sm: &'a SourceManager,
    interner: &'a Interner,
    main: FileId,
    toks: Vec<Token>,
    /// The tokens of every pragma offered to the callbacks, per pragma.
    pragmas: Vec<Vec<Token>>,
}

/// Records the tokens of every pragma.
struct Pragmas(Rc<RefCell<Vec<Vec<Token>>>>);

impl PpCallbacks for Pragmas {
    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool {
        self.0.borrow_mut().push(info.tokens.to_vec());
        false
    }
}

/// Preprocesses `src` (headers from `headers`) and runs `f` on its tokens.
fn with_run<R>(src: &str, headers: &[(&str, &str)], f: impl FnOnce(&Run<'_>) -> R) -> R {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    let mut r = MemoryResolver::new();
    for (name, text) in headers {
        r = r.with_header(name, text);
    }
    pp.set_include_resolver(Box::new(r));
    let pragmas = Rc::new(RefCell::new(Vec::new()));
    pp.add_callbacks(Box::new(Pragmas(Rc::clone(&pragmas))));
    pp.enter_main_file(main);
    let mut toks = Vec::new();
    loop {
        let t = pp.next();
        if t.kind == TokKind::Eof {
            break;
        }
        toks.push(t);
    }
    drop(pp);
    let pragmas = pragmas.take();
    f(&Run { sm: &sm, interner: &interner, main, toks, pragmas })
}

impl Run<'_> {
    /// The index of the `n`th (0-based) token spelled `text`.
    #[track_caller]
    fn nth(&self, text: &str, n: usize) -> usize {
        let mut seen = 0;
        for (i, t) in self.toks.iter().enumerate() {
            if self.sm.span_text(t.span) == Some(text) {
                if seen == n {
                    return i;
                }
                seen += 1;
            }
        }
        panic!("no token #{n} spelled {text:?}");
    }

    /// The file text editing tokens `i..=j` edits, `None` if not editable.
    fn edit(&self, i: usize, j: usize) -> Option<String> {
        self.text_of(Span::new(self.toks[i].span.lo, self.toks[j].span.hi))
    }

    /// The file text editing `range` edits, as `file:text`, or the text
    /// alone in the main file.
    fn text_of(&self, range: Span) -> Option<String> {
        let (f, b, e) = self.sm.editable_file_range(range)?;
        let text = &self.sm.text(f)[b as usize..e as usize];
        Some(if f == self.main {
            text.to_owned()
        } else {
            format!("{}:{text}", self.sm.file_name(f))
        })
    }

    /// The main-file offset an insertion before token `i` goes to.
    fn insert_at(&self, i: usize) -> Option<u32> {
        let at = self.toks[i].span.lo;
        let (f, b, e) = self.sm.editable_file_range(Span::empty(at))?;
        assert_eq!((f, b), (self.main, e), "an insertion point is empty");
        Some(b)
    }
}

/// Edits of `src` from `(first, n, last, m)` token pairs (the `n`th token
/// spelled `first` to the `m`th spelled `last`).
fn edits(src: &str, pairs: &[(&str, usize, &str, usize)]) -> Vec<Option<String>> {
    with_run(src, &[], |r| {
        pairs.iter().map(|&(a, n, b, m)| r.edit(r.nth(a, n), r.nth(b, m))).collect()
    })
}

/// Plain file tokens map to themselves; a range out of order, across
/// buffers or with an invalid end maps to nothing.
#[test]
fn file_tokens_and_bad_ranges() {
    with_run("#include \"h.h\"\nint a = b;\n", &[("h.h", "int h;\n")], |r| {
        let (a, b) = (r.nth("a", 0), r.nth("b", 0));
        assert_eq!(r.edit(a, b), Some("a = b".to_owned()));
        assert_eq!(r.edit(a, a), Some("a".to_owned()));
        assert_eq!(r.edit(b, a), None, "end before begin");
        let h = r.nth("h", 0);
        assert_eq!(r.edit(h, h), Some("h.h:h".to_owned()), "an included file is its own buffer");
        assert_eq!(r.edit(h, a), None, "across buffers");
        let lo = r.toks[a].span.lo;
        assert_eq!(r.text_of(Span::new(lo, Loc::INVALID)), None);
        assert_eq!(r.text_of(Span::new(Loc::INVALID, Loc::INVALID)), None);
        assert_eq!(
            r.text_of(Span::new(Loc::from_raw(u32::MAX - 1), Loc::from_raw(u32::MAX))),
            None
        );
    });
}

/// [cpp.replace]: a range of a macro's tokens maps to the invocation
/// only when it begins and ends the expansion; a part of the
/// replacement list is spelled in the `#define` and maps to nothing.
#[test]
fn cpp_replace_whole_expansions_only() {
    let src = "#define ONE 1\n#define P a + b\n#define A B\n#define B 1 2\n\
               int x = ONE; int y = P; int z = A; int w = c + ONE;\n";
    let got = edits(
        src,
        &[
            ("1", 0, "1", 0),
            ("a", 0, "b", 0),
            ("a", 0, "a", 0),
            ("+", 0, "+", 0),
            ("a", 0, "+", 0),
            ("1", 1, "2", 0),
            ("1", 1, "1", 1),
            ("c", 0, "1", 2),
            ("=", 0, "1", 0),
            ("=", 1, "a", 0),
        ],
    );
    assert_eq!(
        got,
        [
            Some("ONE".to_owned()),
            Some("P".to_owned()),
            None,
            None,
            None,
            Some("A".to_owned()),
            None,
            Some("c + ONE".to_owned()),
            Some("= ONE".to_owned()),
            None,
        ]
    );
}

/// [cpp.subst]: a range inside one use of one argument maps to where
/// the argument is written; when the argument is also the whole
/// expansion, the range maps to the invocation (clang
/// `makeFileCharRange` tries the invocation first).
#[test]
fn cpp_subst_argument_ranges() {
    let src = "#define ID(x) x\n#define F(x) (x)\n#define G(x, y) (x + y)\n\
               int a = ID(foo); int b = F(bar baz); int c = G(p1, q1);\n";
    let got = edits(
        src,
        &[
            ("foo", 0, "foo", 0),
            ("bar", 0, "bar", 0),
            ("bar", 0, "baz", 0),
            ("baz", 0, "baz", 0),
            ("(", 0, "baz", 0),
            ("p1", 0, "p1", 0),
            ("q1", 0, "q1", 0),
            // The begin in one argument, the end in another.
            ("p1", 0, "q1", 0),
            ("p1", 0, "+", 0),
        ],
    );
    assert_eq!(
        got,
        [
            Some("ID(foo)".to_owned()),
            Some("bar".to_owned()),
            Some("bar baz".to_owned()),
            Some("baz".to_owned()),
            None,
            Some("p1".to_owned()),
            Some("q1".to_owned()),
            None,
            None,
        ]
    );
}

/// [cpp.subst]: an argument used twice is written once; a range in
/// either use maps to it (editing it changes both uses), a range from
/// one use to the other does not map unless it is the whole expansion.
#[test]
fn cpp_subst_argument_used_twice() {
    let src = "#define T(x) x + x\n#define U(x) (x + x)\nint a = T(v); int b = U(w);\n";
    let got = edits(
        src,
        &[
            ("v", 0, "v", 0),
            ("v", 1, "v", 1),
            ("v", 0, "v", 1),
            ("v", 0, "+", 0),
            ("w", 0, "w", 0),
            ("w", 1, "w", 1),
            ("w", 0, "w", 1),
        ],
    );
    assert_eq!(
        got,
        [
            Some("v".to_owned()),
            Some("v".to_owned()),
            Some("T(v)".to_owned()),
            None,
            Some("w".to_owned()),
            Some("w".to_owned()),
            None
        ]
    );
}

/// [cpp.subst]: an argument that is itself a macro invocation maps to
/// the written invocation; an argument run interrupted by an expansion
/// maps as one range of the argument's text.
#[test]
fn cpp_subst_nested_invocations() {
    let src = "#define ONE 1\n#define F(x) (x)\nint a = F(ONE); int b = F(k ONE m);\n";
    let got = edits(src, &[("1", 0, "1", 0), ("k", 0, "m", 0), ("1", 1, "m", 0), ("k", 0, "1", 1)]);
    assert_eq!(
        got,
        [
            Some("ONE".to_owned()),
            Some("k ONE m".to_owned()),
            Some("ONE m".to_owned()),
            Some("k ONE".to_owned())
        ]
    );
}

/// [cpp.stringize], [cpp.concat]: a `#` or `##` result is spelled in a
/// scratch buffer, so it maps only when it is the whole expansion (the
/// operand's own text is never the edited range).
#[test]
fn cpp_stringize_and_concat_results() {
    let src = "#define S(x) #x\n#define S2(x) f(#x)\n#define CAT(a) a ## _s\n\
               #define CAT2(a) int a ## _t;\n\
               const char *p = S(abc); int q = S2(def); int CAT(foo); CAT2(bar)\n";
    let got = edits(
        src,
        &[
            ("\"abc\"", 0, "\"abc\"", 0),
            ("\"def\"", 0, "\"def\"", 0),
            ("f", 0, ")", 0),
            ("foo_s", 0, "foo_s", 0),
            ("bar_t", 0, "bar_t", 0),
            ("int", 2, ";", 3),
        ],
    );
    assert_eq!(
        got,
        [
            Some("S(abc)".to_owned()),
            None,
            Some("S2(def)".to_owned()),
            None,
            None,
            Some("CAT2(bar)".to_owned())
        ]
    );
}

/// [cpp.pragma.op]: the tokens of a `_Pragma` operand are spelled in a
/// scratch buffer and map to nothing; tokens around it map as usual.
#[test]
fn cpp_pragma_op_tokens() {
    let src = "#define PR _Pragma(\"kiln one two\") int\nPR x;\n_Pragma(\"kiln three\") int y;\n";
    with_run(src, &[], |r| {
        assert_eq!(r.pragmas.len(), 2);
        for p in &r.pragmas {
            for t in p {
                assert_eq!(r.text_of(t.span), None, "{:?}", r.sm.span_text(t.span));
            }
            let (first, last) = (p[0].span.lo, p[p.len() - 1].span.hi);
            assert_eq!(r.text_of(Span::new(first, last)), None);
        }
        let int0 = r.nth("int", 0);
        assert_eq!(r.edit(int0, int0), None, "a token of PR's replacement list after _Pragma");
        assert_eq!(r.edit(r.nth("int", 1), r.nth("y", 0)), Some("int y".to_owned()));
    });
}

/// An empty range is an insertion point (clang `edit::Commit::canInsert`):
/// at the start of an expansion it moves before the invocation, in an
/// argument to where the argument is written, elsewhere in a
/// replacement list nowhere.
#[test]
fn empty_ranges_are_insertion_points() {
    let src = "#define P a + b\n#define F(x) (x)\nint y = P; int z = F(q);\n";
    with_run(src, &[], |r| {
        let off = |s: &str| src.find(s).unwrap() as u32;
        assert_eq!(r.insert_at(r.nth("int", 0)), Some(off("int y")));
        assert_eq!(r.insert_at(r.nth("a", 0)), Some(off("P;")));
        assert_eq!(r.insert_at(r.nth("+", 0)), None);
        assert_eq!(r.insert_at(r.nth("(", 0)), Some(off("F(q)")));
        assert_eq!(r.insert_at(r.nth("q", 0)), Some(off("q)")));
        assert_eq!(r.insert_at(r.nth(")", 0)), None);
    });
}

/// Every location on the chain from `loc` to the code that caused it.
fn callers(sm: &SourceManager, mut loc: Loc) -> Vec<Loc> {
    let mut out = Vec::new();
    for _ in 0..64 {
        out.push(sm.spelling_loc(loc));
        if !loc.is_macro() {
            break;
        }
        loc = sm.immediate_macro_caller_loc(loc);
    }
    out
}

/// True when only macro arguments lie between `loc` and its spelling.
fn only_arguments(sm: &SourceManager, mut loc: Loc) -> bool {
    while let Some(e) = sm.expansion_info(loc) {
        if e.kind != ExpansionKind::MacroArg {
            return false;
        }
        loc = sm.immediate_spelling_loc(loc);
    }
    true
}

/// Soundness of every editable range over every pair of tokens: the
/// edited text holds a location of each token of the range (the token
/// itself, an argument it came through or an invocation it came from),
/// holds no token outside the range that is written there (other than
/// another use of an argument of the range) and no invocation a token
/// outside the range came from.
#[test]
fn editable_ranges_are_sound_for_every_token_pair() {
    let sources = [
        "#define ONE 1\n#define P a + b\n#define F(x) (x)\n#define ID(x) x\n\
         int x = F(ONE) + ID(P) + F(k ONE m) + ID(ID(z));\n",
        "#define T(x) x + x\n#define G(x, y) y - x\n#define H(x) G(x, T(x))\n\
         int a = T(v) * G(p, q) * H(r) * G(T(s), t);\n",
        "#define S(x) #x\n#define CAT(a, b) a ## b\n#define W(x) CAT(x, _w) S(x)\n\
         int CAT(c, d) = W(e) CAT(, f) CAT(g, );\n",
        "#define EMPTY\n#define E2(x) x EMPTY x\n#define L(...) [__VA_ARGS__]\n\
         int n = E2(u) L(1, 2, 3) L() EMPTY L(L(4));\n",
        "#define OBJ F\n#define F(x) <x>\n#define CALL(f, a) f(a)\n\
         int m = OBJ(1) CALL(F, 2) CALL(OBJ, 3) CALL(CALL, (F, 4));\n",
    ];
    let mut checked = 0;
    for src in sources {
        with_run(src, &[], |r| {
            let n = r.toks.len();
            for i in 0..n {
                for j in i..n {
                    let range = Span::new(r.toks[i].span.lo, r.toks[j].span.hi);
                    let Some((f, b, e)) = r.sm.editable_file_range(range) else { continue };
                    checked += 1;
                    let inside = |l: Loc| {
                        r.sm.offset_of(l).is_some_and(|(lf, o)| lf == f && b <= o && o < e)
                    };
                    let what = || format!("{src:?} tokens {i}..={j} -> {b}..{e}");
                    for t in &r.toks[i..=j] {
                        assert!(callers(r.sm, t.span.lo).into_iter().any(inside), "{}", what());
                    }
                    for (k, t) in r.toks.iter().enumerate() {
                        if (i..=j).contains(&k) {
                            continue;
                        }
                        let lo = t.span.lo;
                        assert!(!inside(r.sm.expansion_loc(lo)), "{} took token {k}", what());
                        let sp = r.sm.spelling_loc(lo);
                        if only_arguments(r.sm, lo) && inside(sp) {
                            let twice =
                                r.toks[i..=j].iter().any(|u| r.sm.spelling_loc(u.span.lo) == sp);
                            assert!(twice, "{} took written token {k}", what());
                        }
                    }
                }
            }
        });
    }
    assert!(checked > 100, "only {checked} editable ranges");
}

/// `token_end` lexes one token at a file location, and measures a macro
/// location's token at its spelling, ending in the same entry.
#[test]
fn lex_pptoken_token_end_of_preprocessed_tokens() {
    let src = "#define CAT(a, b) a ## b\n#define F(x) (x + 1.5e+3)\n\
               int CAT(ab, cd) = F(name) >>= R\"d(raw)d\";\n";
    with_run(src, &[], |r| {
        let opts = LexOptions::new(Std::Cxx26);
        for t in &r.toks {
            let end = kiln_lex::token_end(r.sm, r.interner, t.span.lo, opts);
            assert_eq!(end, Some(t.span.hi), "{:?}", r.sm.span_text(t.span));
        }
        // A macro location's end stays in its entry, and so is not
        // clang's end of a token (which needs the end of the expansion).
        let x = r.toks[r.nth("name", 0)].span.lo;
        assert!(x.is_macro());
        let end = kiln_lex::token_end(r.sm, r.interner, x, opts).unwrap();
        assert_eq!(r.sm.same_entry_distance(x, end), Some(4));
        assert_eq!(r.sm.file_loc_at_macro_end(end), None);
    });
}

/// [cpp.rescan]: the expansion entries in creation order, each with its
/// parent (the entry its invocation or parameter token lies in).
#[test]
fn cpp_rescan_expansion_entries_and_parents() {
    let src = "#define ONE 1\n#define F(x) (x)\n#define G(x) F(x)\nint a = G(ONE);\n";
    with_run(src, &[], |r| {
        let sm = r.sm;
        let all: Vec<_> = sm.expansion_entries(0).collect();
        assert_eq!(all.len() as u32, sm.expansion_count());
        for (i, e) in all.iter().enumerate() {
            assert_eq!(e.index as usize, i);
            assert_eq!(sm.expansion_info(e.info.base), Some(e.info));
            let parent = e.parent.map(|p| all[p as usize].info);
            match parent {
                Some(p) => {
                    assert!((e.parent.unwrap() as usize) < i);
                    assert!(p.base <= e.info.expansion.lo);
                    assert!(e.info.expansion.lo <= p.base.with_offset(p.len));
                }
                None => assert!(e.info.expansion.lo.is_file()),
            }
            if i > 0 {
                assert!(all[i - 1].info.base < e.info.base, "base order");
            }
            assert_eq!(e.info.spelling_range().lo, e.info.spelling);
        }
        // From the middle: the same entries.
        let tail: Vec<_> = sm.expansion_entries(2).collect();
        assert_eq!(tail, all[2..]);
        assert_eq!(sm.expansion_entries(u32::MAX).count(), 0);
        // The written invocations are the entries without a parent.
        let roots: Vec<_> = all
            .iter()
            .filter(|e| e.parent.is_none())
            .map(|e| sm.span_text(e.info.expansion).unwrap_or("?").to_owned())
            .collect();
        assert!(roots.iter().any(|s| s == "G(ONE)"), "{roots:?}");
        // The `ONE` written as an argument is expanded before it is
        // substituted ([cpp.subst]/1): its entry is a root, and the `1`
        // reaches `F`'s body through argument entries spelled in it.
        let one = all
            .iter()
            .find(|e| {
                matches!(e.info.kind, ExpansionKind::Macro { name } if sm.span_text(name) == Some("ONE"))
            })
            .unwrap();
        assert_eq!(one.parent, None);
        assert_eq!(sm.span_text(one.info.expansion), Some("ONE"));
        let lit = r.toks[r.nth("1", 0)].span.lo;
        let arg =
            all.iter().find(|e| e.info.base <= lit && lit <= e.info.base.with_offset(e.info.len));
        let arg = arg.unwrap();
        assert_eq!(arg.info.kind, ExpansionKind::MacroArg);
        let p = all[arg.parent.unwrap() as usize];
        assert!(
            matches!(p.info.kind, ExpansionKind::Macro { name } if sm.span_text(name) == Some("F"))
        );
    });
}
