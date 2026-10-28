//! Behavioral tests for `kiln_source` (spec/source.md, "Tests").
//!
//! Standard references: [lex.phases]/1 (physical source file characters,
//! line structure, phase-1 decoding), [cpp.replace]/[cpp.rescan]
//! (macro expansion locations), [cpp.line] (presumed locations),
//! [cpp.predefined] (`__LINE__`/`__FILE__` read presumed locations).

use std::fs;
use std::path::PathBuf;

use kiln_source::{ExpansionKind, FileId, LineCol, LoadError, Loc, SourceManager, Span};

/// A scratch directory under cargo's per-target temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("kiln_source-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Reference model of line/column: terminators `\n`, `\r\n`, lone `\r`.
fn naive_line_col(text: &str, off: usize) -> (u32, u32) {
    let b = text.as_bytes();
    let (mut line, mut line_start, mut i) = (1, 0, 0);
    while i < off {
        let term_end = match b[i] {
            b'\r' if b.get(i + 1) == Some(&b'\n') => Some(i + 2),
            b'\n' | b'\r' => Some(i + 1),
            _ => None,
        };
        match term_end {
            // The `\n` of a `\r\n` pair is still on the line it ends.
            Some(end) if end <= off => {
                line += 1;
                line_start = end;
                i = end;
            }
            _ => i += 1,
        }
    }
    (line, (off - line_start + 1) as u32)
}

const LINE_ENDING_CASES: &[(&str, &str)] = &[
    ("lf", "int a;\nint b;\n\nint c;\n"),
    ("crlf", "int a;\r\nint b;\r\n\r\nint c;\r\n"),
    ("mixed", "a\nb\r\nc\rd\n\re\r\r\nf"),
    ("no_trailing_newline", "int a;\nint b;"),
    ("empty", ""),
    ("only_newline", "\n"),
    ("lone_cr_at_eof", "x\r"),
];

// [lex.phases]/1: line structure of physical source files; round trip
// offset -> (line, col) -> offset for every byte and the end-of-file loc.
#[test]
fn lex_phases_line_col_round_trip() {
    let sm = SourceManager::new();
    for &(name, text) in LINE_ENDING_CASES {
        let f = sm.add_virtual(name, text.to_owned()).unwrap();
        for off in 0..=text.len() {
            let loc = sm.loc(f, off as u32);
            let LineCol { file, line, col } = sm.line_col(loc).unwrap();
            assert_eq!(file, f);
            assert_eq!((line, col), naive_line_col(text, off), "{name} offset {off}");
            // Back from (line, col) to the offset via line_text positions.
            let line_start = (1..line).fold(0usize, |acc, l| {
                let lt = sm.line_text(f, l).unwrap();
                let rest = &text[acc + lt.len()..];
                let term = if rest.starts_with("\r\n") { 2 } else { 1 };
                acc + lt.len() + term
            });
            assert_eq!(line_start + col as usize - 1, off, "{name} offset {off}");
        }
    }
}

// [lex.phases]/1: concatenating every line with its terminator rebuilds the
// file; terminators are stripped from line_text.
#[test]
fn lex_phases_line_text_strips_terminators() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("mixed", "a\nb\r\nc\rd".to_owned()).unwrap();
    assert_eq!(sm.line_count(f), 4);
    assert_eq!(sm.line_text(f, 0), None);
    assert_eq!(sm.line_text(f, 1), Some("a"));
    assert_eq!(sm.line_text(f, 2), Some("b"));
    assert_eq!(sm.line_text(f, 3), Some("c"));
    assert_eq!(sm.line_text(f, 4), Some("d"));
    assert_eq!(sm.line_text(f, 5), None);

    let t = sm.add_virtual("trailing", "x\r\n".to_owned()).unwrap();
    assert_eq!(sm.line_count(t), 2);
    assert_eq!(sm.line_text(t, 1), Some("x"));
    assert_eq!(sm.line_text(t, 2), Some(""));
    // EOF location after a trailing newline is column 1 of the empty line.
    let eof = sm.file_span(t).hi;
    assert_eq!(sm.line_col(eof), Some(LineCol { file: t, line: 2, col: 1 }));

    let e = sm.add_virtual("empty", String::new()).unwrap();
    assert_eq!(sm.line_count(e), 1);
    assert_eq!(sm.line_text(e, 1), Some(""));
    assert_eq!(sm.line_col(sm.loc(e, 0)), Some(LineCol { file: e, line: 1, col: 1 }));
}

// [lex.phases]/1: columns are byte columns, also after non-ASCII text.
#[test]
fn lex_phases_columns_are_bytes() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("u", "é = 1;".to_owned()).unwrap();
    assert_eq!(sm.line_col(sm.loc(f, 3)).unwrap().col, 4);
}

// [lex.phases]/1: invalid UTF-8 is replaced by U+FFFD per maximal invalid
// subsequence and reported with positions in the decoded text.
#[test]
fn lex_phases_invalid_utf8_replacement_positions() {
    let dir = TempDir::new("utf8");
    let path = dir.0.join("bad.cpp");
    fs::write(&path, b"ok\xFFx\n\xE2\x82y\xC0\xAFz").unwrap();
    let sm = SourceManager::new();
    let f = sm.load_file(&path).unwrap();
    assert_eq!(sm.text(f), "ok\u{FFFD}x\n\u{FFFD}y\u{FFFD}\u{FFFD}z");
    let reps = &sm.load_report(f).replacements;
    let got: Vec<(u32, &[u8])> = reps.iter().map(|r| (r.offset, &*r.bytes)).collect();
    assert_eq!(
        got,
        vec![(2, &b"\xFF"[..]), (7, &b"\xE2\x82"[..]), (11, &b"\xC0"[..]), (14, &b"\xAF"[..])]
    );
    for r in reps {
        assert_eq!(&sm.text(f)[r.offset as usize..r.offset as usize + 3], "\u{FFFD}");
    }
    // Positions after a replacement are byte columns in the decoded text.
    let y = sm.text(f).find('y').unwrap() as u32;
    assert_eq!(sm.line_col(sm.loc(f, y)), Some(LineCol { file: f, line: 2, col: 4 }));
    assert!(!sm.load_report(f).is_clean());

    let good = dir.0.join("good.cpp");
    fs::write(&good, "int x;\n").unwrap();
    let g = sm.load_file(&good).unwrap();
    assert!(sm.load_report(g).is_clean());
}

// [cpp.include]: file identity is by canonical path; the name is the path
// as first given.
#[test]
fn cpp_include_file_identity_by_canonical_path() {
    let dir = TempDir::new("identity");
    let a = dir.0.join("a.h");
    let b = dir.0.join("b.h");
    fs::write(&a, "A\n").unwrap();
    fs::write(&b, "B\n").unwrap();
    fs::create_dir(dir.0.join("sub")).unwrap();

    let sm = SourceManager::new();
    let fa = sm.load_file(&a).unwrap();
    assert_eq!(sm.load_file(&a).unwrap(), fa);
    assert_eq!(sm.load_file(&dir.0.join("sub/../a.h")).unwrap(), fa);
    assert_eq!(sm.load_file(&dir.0.join("./a.h")).unwrap(), fa);
    #[cfg(unix)]
    {
        let link = dir.0.join("link.h");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert_eq!(sm.load_file(&link).unwrap(), fa);
    }
    let fb = sm.load_file(&b).unwrap();
    assert_ne!(fa, fb);
    assert_eq!(sm.file_name(fa), a.to_string_lossy());
    assert_eq!(sm.text(fb), "B\n");
    assert_eq!(sm.file_count(), 2);

    match sm.load_file(&dir.0.join("missing.h")) {
        Err(LoadError::Io(e)) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
        other => panic!("expected NotFound, got {other:?}"),
    }
    assert!(matches!(sm.load_file(&dir.0.join("sub")), Err(LoadError::Io(_))));
}

// [cpp.predefined]: virtual buffers (<built-in>, <command line>) are never
// deduplicated and carry their given name.
#[test]
fn virtual_buffers() {
    let sm = SourceManager::new();
    let a = sm.add_virtual("<built-in>", "#define X 1\n".to_owned()).unwrap();
    let b = sm.add_virtual("<built-in>", "#define X 1\n".to_owned()).unwrap();
    assert_ne!(a, b);
    assert_eq!(sm.file_name(a), "<built-in>");
    assert_eq!(sm.text(b), "#define X 1\n");
    assert!(sm.load_report(a).is_clean());
    assert_eq!(a.index(), 0);
    assert_eq!(b.index(), 1);
}

// Loc space partitioning: each buffer owns [start, start + len], so the
// end-of-file location of one buffer is distinct from the next buffer's
// start; offset_of inverts loc for every buffer.
#[test]
fn loc_partitioning_and_offset_of() {
    let sm = SourceManager::new();
    let mut files = Vec::new();
    for i in 0..300u32 {
        let text = "x".repeat((i as usize * 7) % 23);
        files.push(sm.add_virtual(&format!("f{i}"), text).unwrap());
    }
    let mut prev_eof: Option<Loc> = None;
    for &f in &files {
        let span = sm.file_span(f);
        assert!(span.lo.is_file() && !span.lo.is_macro());
        if let Some(p) = prev_eof {
            assert!(p < span.lo, "ranges must not overlap");
        }
        let len = sm.text(f).len() as u32;
        for off in 0..=len {
            assert_eq!(sm.offset_of(sm.loc(f, off)), Some((f, off)));
        }
        assert_eq!(span.hi, sm.loc(f, len));
        prev_eof = Some(span.hi);
    }
    assert!(!Loc::INVALID.is_valid());
    assert_eq!(sm.offset_of(Loc::INVALID), None);
    assert_eq!(sm.line_col(Loc::INVALID), None);
    assert_eq!(sm.presumed_line_col(Loc::INVALID), None);
    // A file location past every buffer is unknown.
    let past = prev_eof.unwrap().with_offset(1);
    assert_eq!(sm.offset_of(past), None);
    assert_eq!(sm.line_col(past), None);
}

#[test]
#[should_panic(expected = "past end")]
fn loc_past_end_panics() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("f", "ab".to_owned()).unwrap();
    let _ = sm.loc(f, 3);
}

// The layout obligation from spec/source.md: Loc 4 bytes, Span 8 bytes.
#[test]
fn loc_and_span_sizes() {
    assert_eq!(size_of::<Loc>(), 4);
    assert_eq!(size_of::<Span>(), 8);
    assert_eq!(size_of::<FileId>(), 4);
}

// Text borrowed from the manager stays valid while the preprocessor keeps
// appending buffers and expansion entries through the shared reference.
#[test]
fn text_borrow_survives_appends() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("main", "int main();\n".to_owned()).unwrap();
    let text = sm.text(f);
    let start = sm.loc(f, 0);
    for i in 0..5000 {
        sm.add_virtual("<scratch space>", format!("tok{i}")).unwrap();
        sm.add_expansion(
            start,
            3,
            Span::new(start, start.with_offset(3)),
            ExpansionKind::Macro { name: Span::new(start, start.with_offset(3)) },
        )
        .unwrap();
    }
    assert_eq!(text, "int main();\n");
    assert_eq!(sm.text(f), text);
}

/// Byte offset of the `n`th occurrence (0-based) of `needle` in `hay`.
fn nth(hay: &str, needle: &str, n: usize) -> u32 {
    hay.match_indices(needle).nth(n).unwrap().0 as u32
}

/// Simulates the preprocessor on
/// ```text
/// #define A(x) x+1
/// #define B A(2)
/// B
/// ```
/// and returns the manager, the file and the result tokens' locations
/// `[2, +, 1]` plus the entry bases `[e_b, e_a, arg]`.
fn nested_expansion() -> (SourceManager, FileId, [Loc; 3], [Loc; 3]) {
    let src = "#define A(x) x+1\n#define B A(2)\nB\n";
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let at = |off: u32| sm.loc(f, off);

    // `B` at line 3 expands to the body of B, "A(2)" on line 2.
    let b_use = nth(src, "B", 1);
    let b_tok = Span::new(at(b_use), at(b_use + 1));
    let b_body = nth(src, "A(2)", 0);
    let e_b = sm.add_expansion(at(b_body), 4, b_tok, ExpansionKind::Macro { name: b_tok }).unwrap();
    // Tokens of B's expansion: A=e_b+0 (=e_b+1 = "(", e_b+2 = "2", e_b+3 = ")").

    // `A(2)` (all macro locations) expands to A's body "x+1" on line 1.
    let a_body = nth(src, "x+1", 0);
    let a_invocation = Span::new(e_b, e_b.with_offset(4));
    let a_name = Span::new(e_b, e_b.with_offset(1));
    let e_a = sm
        .add_expansion(at(a_body), 3, a_invocation, ExpansionKind::Macro { name: a_name })
        .unwrap();

    // The argument `2` (spelled at e_b+2) replaces parameter `x` (e_a+0).
    let arg = sm
        .add_expansion(
            e_b.with_offset(2),
            1,
            Span::new(e_a, e_a.with_offset(1)),
            ExpansionKind::MacroArg,
        )
        .unwrap();

    (sm, f, [arg, e_a.with_offset(1), e_a.with_offset(2)], [e_b, e_a, arg])
}

// [cpp.rescan]: nested expansion; spelling locations go to where each
// character was written, expansion locations go to the outermost
// invocation in the file.
#[test]
fn cpp_rescan_nested_expansion_spelling_and_expansion() {
    let (sm, f, [two, plus, one], [e_b, e_a, arg]) = nested_expansion();
    let src = sm.text(f);
    let lc = |l: Loc| sm.line_col(l).map(|c| (c.line, c.col));

    for l in [two, plus, one, e_b, e_a, arg] {
        assert!(l.is_macro() && sm.is_macro_loc(l));
        assert_eq!(sm.offset_of(l), None);
    }

    // Spelling: `2` is written in B's body on line 2; `+`/`1` in A's body.
    assert_eq!(sm.spelling_loc(two), sm.loc(f, nth(src, "2", 0)));
    assert_eq!(lc(sm.spelling_loc(two)), Some((2, 13)));
    assert_eq!(sm.immediate_spelling_loc(two), e_b.with_offset(2));
    assert!(sm.immediate_spelling_loc(two).is_macro());
    assert_eq!(sm.spelling_loc(plus), sm.loc(f, nth(src, "+", 0)));
    assert_eq!(lc(sm.spelling_loc(one)), Some((1, 16)));

    // Expansion: everything comes from the `B` on line 3.
    let b_loc = sm.loc(f, nth(src, "B", 1));
    for l in [two, plus, one] {
        assert_eq!(sm.expansion_loc(l), b_loc);
        assert_eq!(lc(l), Some((3, 1)));
        assert_eq!(sm.expansion_span(l), Span::new(b_loc, b_loc.with_offset(1)));
    }

    // Token text through spelling.
    assert_eq!(sm.span_text(Span::new(two, two.with_offset(1))), Some("2"));
    assert_eq!(sm.span_text(Span::new(plus, one)), Some("+"));
    assert_eq!(sm.span_text(Span::new(e_b, e_b.with_offset(4))), Some("A(2)"));
    // Ends spelled out of order (`2` on line 2, `1` on line 1).
    assert_eq!(sm.span_text(Span::new(two, one)), None);

    // Entry metadata.
    let info = sm.expansion_info(two).unwrap();
    assert_eq!((info.base, info.len, info.kind), (arg, 1, ExpansionKind::MacroArg));
    assert_eq!(info.spelling, e_b.with_offset(2));
    assert_eq!(sm.expansion_info(b_loc), None);
}

// [cpp.rescan] + clang's "expanded from macro" notes: walking callers from
// a result token names each macro from innermost to outermost.
#[test]
fn cpp_rescan_expanded_from_notes_walk() {
    let (sm, f, [two, plus, _], _) = nested_expansion();
    let notes = |mut l: Loc| {
        let mut names = Vec::new();
        while let Some(info) = sm.expansion_info(l) {
            if let ExpansionKind::Macro { name } = info.kind {
                names.push(sm.span_text(name).unwrap().to_owned());
            }
            l = sm.immediate_macro_caller_loc(l);
        }
        (names, l)
    };
    let b_loc = sm.loc(f, nth(sm.text(f), "B", 1));
    // `+` comes from A's body, invoked inside B's body, invoked in the file.
    assert_eq!(notes(plus), (vec!["A".to_owned(), "B".to_owned()], b_loc));
    // `2` is an argument: its caller is where the argument was written
    // (B's body), whose caller is the `B` in the file.
    assert_eq!(sm.immediate_macro_caller_loc(two), sm.immediate_spelling_loc(two));
    assert_eq!(notes(two), (vec!["B".to_owned()], b_loc));
}

// [cpp.rescan]/1: a macro whose replacement list is another macro's name;
// the inner macro's name token is itself a macro location.
#[test]
fn cpp_rescan_macro_name_from_expansion() {
    let src = "#define F G\n#define G 42\nF\n";
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let at = |o: u32| sm.loc(f, o);
    let f_use = nth(src, "F", 1);
    let f_tok = Span::new(at(f_use), at(f_use + 1));
    let e_f = sm
        .add_expansion(at(nth(src, "G", 0)), 1, f_tok, ExpansionKind::Macro { name: f_tok })
        .unwrap();
    let g_tok = Span::new(e_f, e_f.with_offset(1));
    let e_g = sm
        .add_expansion(at(nth(src, "42", 0)), 2, g_tok, ExpansionKind::Macro { name: g_tok })
        .unwrap();
    assert_eq!(sm.span_text(Span::new(e_g, e_g.with_offset(2))), Some("42"));
    assert_eq!(sm.line_col(sm.spelling_loc(e_g)).map(|c| c.line), Some(2));
    assert_eq!(sm.expansion_loc(e_g), at(f_use));
    assert_eq!(sm.expansion_span(e_g.with_offset(1)), f_tok);
    let ExpansionKind::Macro { name } = sm.expansion_info(e_g).unwrap().kind else { panic!() };
    assert_eq!(sm.span_text(name), Some("G"));
}

// [cpp.replace]/10: a function-like invocation may span lines; the
// expansion span covers the whole invocation.
#[test]
fn cpp_replace_multiline_invocation_span() {
    let src = "#define M(a,b) a\nM(1,\n  2)\n";
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let start = nth(src, "M(1", 0);
    let end = nth(src, ")", 1) + 1;
    let inv = Span::new(sm.loc(f, start), sm.loc(f, end));
    let name = Span::new(inv.lo, inv.lo.with_offset(1));
    let body = nth(src, "a\n", 0);
    let e = sm.add_expansion(sm.loc(f, body), 1, inv, ExpansionKind::Macro { name }).unwrap();
    assert_eq!(sm.expansion_span(e), inv);
    let hi = sm.line_col(sm.expansion_span(e).hi).unwrap();
    assert_eq!((hi.line, hi.col), (3, 5));
}

// Unknown macro locations are treated as invalid rather than panicking.
#[test]
fn unknown_macro_locations() {
    let (sm, _, [_, _, one], _) = nested_expansion();
    let unknown = one.with_offset(100);
    assert!(unknown.is_macro());
    assert_eq!(sm.expansion_info(unknown), None);
    assert_eq!(sm.spelling_loc(unknown), Loc::INVALID);
    assert_eq!(sm.expansion_loc(unknown), Loc::INVALID);
    assert_eq!(sm.expansion_span(unknown), Span::INVALID);
    assert_eq!(sm.immediate_macro_caller_loc(unknown), Loc::INVALID);
    assert_eq!(sm.line_col(unknown), None);
    assert_eq!(sm.span_text(Span::new(unknown, unknown)), None);
}

#[test]
#[should_panic(expected = "bad spelling")]
fn add_expansion_rejects_unallocated_spelling() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("f", "x".to_owned()).unwrap();
    let x = sm.file_span(f);
    let _ = sm.add_expansion(x.hi.with_offset(5), 1, x, ExpansionKind::MacroArg);
}

// [cpp.line]/3-5: `#line` changes the presumed line of the following line
// and optionally the presumed file name; a later `#line N` keeps the name.
#[test]
fn cpp_line_presumed_locations() {
    let src = "a\nb\n#line 100 \"foo.h\"\nc\nd\n#line 7\ne\n";
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let at = |s: &str| sm.loc(f, nth(src, s, 0));
    let off_c = nth(src, "c\n", 0);
    let off_e = nth(src, "e\n", 0);
    sm.add_line_directive(f, off_c, 100, Some("foo.h"));
    sm.add_line_directive(f, off_e, 7, None);

    let p = |l: Loc| {
        let p = sm.presumed_line_col(l).unwrap();
        (p.file_name.to_owned(), p.line, p.col)
    };
    assert_eq!(p(at("a")), ("t.cpp".to_owned(), 1, 1));
    assert_eq!(p(at("b")), ("t.cpp".to_owned(), 2, 1));
    // The directive line itself is still physical.
    assert_eq!(p(at("#line 100")), ("t.cpp".to_owned(), 3, 1));
    assert_eq!(p(at("c")), ("foo.h".to_owned(), 100, 1));
    assert_eq!(p(at("d")), ("foo.h".to_owned(), 101, 1));
    assert_eq!(p(at("7")), ("foo.h".to_owned(), 102, 7));
    assert_eq!(p(at("e\n")), ("foo.h".to_owned(), 7, 1));
    assert_eq!(p(sm.file_span(f).hi), ("foo.h".to_owned(), 8, 1));
    // Physical positions are unaffected.
    assert_eq!(sm.line_col(at("e\n")).unwrap().line, 7);
    assert_eq!(sm.presumed_line_col(at("e\n")).unwrap().file, f);
}

// [cpp.line]: a macro location's presumed position is that of its
// expansion (what `__LINE__` inside a macro body reports).
#[test]
fn cpp_line_presumed_for_macro_locations() {
    let (sm, f, [two, ..], _) = nested_expansion();
    let off_b = nth(sm.text(f), "B\n", 0);
    sm.add_line_directive(f, off_b, 50, Some("x.cpp"));
    let p = sm.presumed_line_col(two).unwrap();
    assert_eq!((p.file_name, p.line, p.col), ("x.cpp", 50, 1));
}

// [cpp.line]: directives may arrive out of order and re-adding at the same
// offset (a header entered twice) replaces rather than duplicates.
#[test]
fn cpp_line_directive_order_and_replacement() {
    let src = "l1\nl2\nl3\nl4\n";
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    sm.add_line_directive(f, nth(src, "l3", 0), 30, None);
    sm.add_line_directive(f, nth(src, "l2", 0), 20, Some("n.h"));
    sm.add_line_directive(f, nth(src, "l2", 0), 25, Some("m.h"));
    let p = |s: &str| {
        let p = sm.presumed_line_col(sm.loc(f, nth(src, s, 0))).unwrap();
        (p.file_name, p.line)
    };
    assert_eq!(p("l1"), ("t.cpp", 1));
    assert_eq!(p("l2"), ("m.h", 25));
    // l3's directive was added before l2's and inherited the original name.
    assert_eq!(p("l3"), ("t.cpp", 30));
    assert_eq!(p("l4"), ("t.cpp", 31));
}

// [cpp.line]/3 bounds line numbers; out-of-range values are pp's
// diagnostic, and the presumed line saturates instead of wrapping.
#[test]
fn cpp_line_presumed_line_saturates() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("t.cpp", "a\nb\n".to_owned()).unwrap();
    sm.add_line_directive(f, 0, u32::MAX, None);
    assert_eq!(sm.presumed_line_col(sm.loc(f, 2)).unwrap().line, u32::MAX);
}

#[test]
fn manager_is_shareable_across_threads() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("t", "a\nb\nc\n".to_owned()).unwrap();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for i in 0..100 {
                    let g = sm.add_virtual("v", format!("x{i}\n")).unwrap();
                    assert_eq!(sm.line_col(sm.loc(g, 1)).unwrap().col, 2);
                    assert_eq!(sm.line_col(sm.loc(f, 4)).unwrap().line, 3);
                }
            });
        }
    });
    assert_eq!(sm.file_count(), 401);
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

    fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n.max(1))) as u32
    }
}

/// Reference distance: walks both spelling chains one link at a time in
/// lockstep (the slow path `kiln_pp` used before WP-056).
fn slow_distance(sm: &SourceManager, lo: Loc, hi: Loc) -> Option<u32> {
    let (mut a, mut b) = (lo, hi);
    while a.is_macro() && b.is_macro() {
        a = sm.immediate_spelling_loc(a);
        b = sm.immediate_spelling_loc(b);
    }
    let (fa, oa) = sm.offset_of(a)?;
    let (fb, ob) = sm.offset_of(b)?;
    (fa == fb && ob >= oa).then(|| ob - oa)
}

// Fast paths for [cpp.rescan] location chains (WP-056): `entry_bounds`
// and `same_entry_distance` (which use a cached last lookup and a chunked
// search) agree with brute force and with the chain-walking reference on
// a randomized set of deep expansion chains, queried in random order.
#[test]
fn cpp_rescan_same_entry_fast_paths_match_slow_walk() {
    let sm = SourceManager::new();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    // Model: every entry's inclusive bounds.
    let mut entries: Vec<(Loc, Loc)> = Vec::new();
    for i in 0..40 {
        let len = 1 + rng.below(200);
        let text: String = (0..len).map(|j| char::from(b'a' + ((i + j) % 26) as u8)).collect();
        let f = sm.add_virtual(&format!("r{i}"), text).unwrap();
        let s = sm.file_span(f);
        entries.push((s.lo, s.hi));
    }
    let file_entries = entries.len();
    for _ in 0..3000 {
        // Spell the new entry inside a random existing entry, preferring
        // recent ones so that chains get deep.
        let n = entries.len() as u32;
        let pick = if rng.below(4) == 0 { rng.below(n) } else { n - 1 - rng.below(n.min(30)) };
        let (lo, hi) = entries[pick as usize];
        let room = hi.to_raw() - lo.to_raw();
        let start = rng.below(room + 1);
        let len = rng.below(room - start + 1);
        let spelling = lo.with_offset(start);
        let exp = Span::new(spelling, spelling);
        let kind = if rng.below(2) == 0 {
            ExpansionKind::MacroArg
        } else {
            ExpansionKind::Macro { name: exp }
        };
        let base = sm.add_expansion(spelling, len, exp, kind).unwrap();
        entries.push((base, base.with_offset(len)));
    }
    let random_loc = |rng: &mut Rng| {
        let (lo, hi) = entries[rng.below(entries.len() as u32) as usize];
        lo.with_offset(rng.below(hi.to_raw() - lo.to_raw() + 1))
    };
    for _ in 0..20_000 {
        let a = random_loc(&mut rng);
        // Half the pairs share an entry.
        let b = if rng.below(2) == 0 {
            let (lo, hi) = sm.entry_bounds(a).unwrap();
            lo.with_offset(rng.below(hi.to_raw() - lo.to_raw() + 1))
        } else {
            random_loc(&mut rng)
        };
        let model = |l: Loc| entries.iter().copied().find(|&(lo, hi)| lo <= l && l <= hi);
        assert_eq!(sm.entry_bounds(a), model(a), "{a:?}");
        let same = model(a) == model(b);
        let fast = sm.same_entry_distance(a, b);
        assert_eq!(fast.is_some(), same && a <= b, "{a:?} {b:?}");
        if let Some(d) = fast {
            assert_eq!(slow_distance(&sm, a, b), Some(d), "{a:?} {b:?}");
            assert_eq!(sm.spelling_loc(b), sm.spelling_loc(a).with_offset(d));
            let slow_text = {
                let (fa, oa) = sm.offset_of(sm.spelling_loc(a)).unwrap();
                let (_, ob) = sm.offset_of(sm.spelling_loc(b)).unwrap();
                &sm.text(fa)[oa as usize..ob as usize]
            };
            assert_eq!(sm.span_text(Span::new(a, b)), Some(slow_text));
        }
    }
    // Locations past every entry are unknown, whatever the cached hint.
    let (_, last_file_end) = entries[file_entries - 1];
    let (_, last_exp_end) = *entries.last().unwrap();
    for l in [last_file_end.with_offset(1), last_exp_end.with_offset(1)] {
        assert_eq!(sm.entry_bounds(l), None);
        assert_eq!(sm.same_entry_distance(l, l), None);
    }
    assert_eq!(sm.entry_bounds(Loc::INVALID), None);
}

// spec: source.md "Inclusions"; [cpp.include]/7 (a file may be included
// more than once; each inclusion is a distinct range of locations).
#[test]
fn cpp_include_inclusion_ranges_alias_the_buffer() {
    let sm = SourceManager::new();
    let main = sm.add_virtual("main.cpp", "#include \"h\"\n#include \"h\"\n".into()).unwrap();
    let h = sm.add_virtual("h", "int a;\n#line 10\nint b;\n".into()).unwrap();
    sm.add_line_directive(h, 16, 10, None);
    let own = sm.loc(h, 0);
    let from1 = sm.loc(main, 9);
    let from2 = sm.loc(main, 22);
    let i1 = sm.add_inclusion(h, from1).unwrap();
    let i2 = sm.add_inclusion(h, from2).unwrap();
    assert!(i1 != own && i2 != own && i1 != i2);
    // `loc` keeps returning the buffer's own range.
    assert_eq!(sm.loc(h, 0), own);
    assert_eq!(sm.include_loc(own), Loc::INVALID);
    assert_eq!(sm.include_loc(sm.loc(main, 3)), Loc::INVALID);
    let len = sm.text(h).len() as u32;
    for (base, from) in [(i1, from1), (i2, from2)] {
        for off in [0, 4, 16, len] {
            let l = base.with_offset(off);
            assert_eq!(sm.offset_of(l), Some((h, off)));
            assert_eq!(sm.line_col(l), sm.line_col(own.with_offset(off)));
            assert_eq!(sm.include_loc(l), from);
            let (p, q) = (
                sm.presumed_line_col(l).unwrap(),
                sm.presumed_line_col(own.with_offset(off)).unwrap(),
            );
            assert_eq!((p.file, p.line, p.col, p.file_name), (q.file, q.line, q.col, q.file_name));
        }
        assert_eq!(sm.entry_bounds(base.with_offset(5)), Some((base, base.with_offset(len))));
        assert_eq!(sm.same_entry_distance(base, base.with_offset(len)), Some(len));
        assert_eq!(sm.span_text(Span::new(base.with_offset(4), base.with_offset(5))), Some("a"));
        assert_eq!(sm.spelling_loc(base.with_offset(4)), base.with_offset(4));
    }
    // The ranges are distinct entries.
    assert_eq!(sm.same_entry_distance(i1, i2), None);
    assert_eq!(sm.same_entry_distance(own, i1), None);
    // A macro location reports the inclusion of its expansion.
    let m = sm
        .add_expansion(
            i2.with_offset(4),
            1,
            Span::new(i2.with_offset(4), i2.with_offset(5)),
            ExpansionKind::Macro { name: Span::new(i2.with_offset(4), i2.with_offset(5)) },
        )
        .unwrap();
    assert_eq!(sm.include_loc(m), from2);
    // The include stack is followed through the includer's own range.
    assert_eq!(sm.include_loc(from1), Loc::INVALID);
}
