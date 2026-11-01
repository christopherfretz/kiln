//! `-verify` directive parsing, matching and the report
//! (`spec/diag.md` section 11), checked against clang 19's
//! `VerifyDiagnosticConsumer` output.

use kiln_diag::verify::{Verifier, VerifyLevel, VerifyOptions};
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, MemorySink, SharedBuffer, TextSink};
use kiln_source::{FileId, Loc, SourceManager, Span};

/// The spans of the comments in `file` (enough of a lexer for tests: no
/// string literals).
fn comments(sm: &SourceManager, file: FileId) -> Vec<Span> {
    let t = sm.text(file).as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < t.len() {
        if t[i] == b'/' && t[i + 1] == b'/' {
            let end = t[i..].iter().position(|&c| c == b'\n').map_or(t.len(), |n| i + n);
            out.push(Span::new(sm.loc(file, i as u32), sm.loc(file, end as u32)));
            i = end;
        } else if t[i] == b'/' && t[i + 1] == b'*' {
            let end = t[i + 2..].windows(2).position(|w| w == b"*/").map_or(t.len(), |n| i + n + 4);
            out.push(Span::new(sm.loc(file, i as u32), sm.loc(file, end as u32)));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// The location of `line`:`col` in `file`.
fn at(sm: &SourceManager, file: FileId, line: u32, col: u32) -> Span {
    let text = sm.text(file);
    let start = sm.line_text(file, line).unwrap().as_ptr() as usize - text.as_ptr() as usize;
    let l = sm.loc(file, start as u32 + col - 1);
    Span::new(l, l.with_offset(1))
}

struct Case {
    sm: SourceManager,
    main: FileId,
    header: Option<FileId>,
}

impl Case {
    fn new(src: &str) -> Case {
        let sm = SourceManager::new();
        let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
        Case { sm, main, header: None }
    }

    fn with_header(src: &str, header: &str) -> Case {
        let mut c = Case::new(src);
        c.header = Some(c.sm.add_virtual("h.h", header.to_owned()).unwrap());
        c
    }

    /// Runs `-verify` with `opts`: comments of every buffer are handled,
    /// then `emit` reports the "compiler's" diagnostics. Returns the
    /// rendered report and the exit status.
    fn run_with(
        &self,
        opts: VerifyOptions,
        emit: impl FnOnce(&DiagEngine<'_>, &SourceManager, FileId),
    ) -> (String, i32) {
        let sm = &self.sm;
        let sink = MemorySink::new();
        let engine =
            DiagEngine::new(sm, &DiagOptions::from_args(["-ferror-limit=0"]), sink.clone());
        let header = self.header;
        let lookup = Box::new(move |_: &SourceManager, _: Loc, name: &str| {
            (name == "h.h").then_some(header).flatten()
        });
        let mut v = Verifier::new(sm, opts, Some(self.main), lookup);
        for f in std::iter::once(self.main).chain(self.header) {
            for c in comments(sm, f) {
                v.handle_comment(c, &engine);
            }
        }
        emit(&engine, sm, self.main);
        v.finish(&engine);
        let report = v.check(&sink.take());
        let dopts = DiagOptions::from_args(["-fno-caret-diagnostics"]);
        let buf = SharedBuffer::new();
        let out = DiagEngine::new(sm, &dopts, TextSink::new(buf.clone(), &dopts));
        report.emit(&out);
        let mut text = buf.contents();
        if let Some(s) = report.summary() {
            text.push_str(&s);
            text.push('\n');
        }
        (text, i32::from(report.problems > 0))
    }

    fn run(&self, emit: impl FnOnce(&DiagEngine<'_>, &SourceManager, FileId)) -> (String, i32) {
        self.run_with(VerifyOptions::default(), emit)
    }
}

fn error(e: &DiagEngine<'_>, span: Span, msg: &str) {
    Diag::new(DiagId::pp_error_directive, span).arg(msg).emit(e);
}

fn warning(e: &DiagEngine<'_>, span: Span, msg: &str) {
    Diag::new(DiagId::pp_warning_directive, span).arg(msg).emit(e);
}

fn opts(prefixes: &[&str]) -> VerifyOptions {
    let mut p: Vec<String> = prefixes.iter().map(|s| (*s).to_owned()).collect();
    p.sort();
    VerifyOptions { prefixes: p, ignore_unexpected: [false; 4] }
}

// spec: diag.md §11.5 (everything matched: exit 0, no output)
#[test]
fn all_matched() {
    let c = Case::new(
        "int a = x; // expected-error {{undeclared}}\n\
         // expected-warning@+1 {{unused}}\n\
         int b;\n",
    );
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 9), "use of undeclared identifier 'x'");
        warning(e, at(sm, f, 3, 5), "unused variable 'b'");
    });
    assert_eq!((out.as_str(), status), ("", 0));
}

// spec: diag.md §11.6 (expected but not seen / seen but not expected)
#[test]
fn mismatch_report() {
    let c = Case::new(
        "int a = x; // expected-error {{nope}}\n\
         // expected-warning@+1 {{whatever}}\n\
         int b;\n",
    );
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 9), "use of undeclared identifier 'x'");
    });
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 1: nope\n\
         error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: use of undeclared identifier 'x'\n\
         error: 'expected-warning' diagnostics expected but not seen: \n  \
         File t.cpp Line 3 (directive at t.cpp:2): whatever\n\
         3 errors generated.\n"
    );
    assert_eq!(status, 1);
}

// spec: diag.md §11.6 (no directives at all)
#[test]
fn no_directives() {
    let c = Case::new("int x;\n");
    let (out, status) = c.run(|_, _, _| {});
    assert_eq!(
        out,
        "error: no expected directives found: consider use of 'expected-no-diagnostics'\n\
         1 error generated.\n"
    );
    assert_eq!(status, 1);
    let c = Case::new("// expected-no-diagnostics\nint x;\n");
    assert_eq!(c.run(|_, _, _| {}), (String::new(), 0));
}

// spec: diag.md §11.2 (no-diagnostics mixed with other directives)
#[test]
fn no_diagnostics_conflicts() {
    let c = Case::new("// expected-no-diagnostics\n// expected-error {{x}}\n");
    let (out, _) = c.run(|_, _, _| {});
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 2: expected directive cannot follow 'expected-no-diagnostics' directive\n\
         1 error generated.\n"
    );
    let c = Case::new("// expected-error {{x}}\n// expected-no-diagnostics\n");
    let (out, _) = c.run(|e, sm, f| error(e, at(sm, f, 1, 1), "x"));
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 2: 'expected-no-diagnostics' directive cannot follow other expected directives\n\
         1 error generated.\n"
    );
}

// spec: diag.md §11.2 (counts N, N+, N-M, +, and the invalid range)
#[test]
fn counts() {
    let c = Case::new(
        "a // expected-error 2 {{e}}\n\
         b // expected-error 1+ {{f}}\n\
         c // expected-error 0-2 {{g}}\n\
         d // expected-error + {{h}}\n\
         e // expected-warning 0+ {{none}}\n",
    );
    let (out, status) = c.run(|e, sm, f| {
        // Distinct columns: the engine drops exact duplicates (§7.4).
        error(e, at(sm, f, 1, 1), "e");
        error(e, at(sm, f, 1, 2), "e");
        for col in 1..=3 {
            error(e, at(sm, f, 2, col), "f");
        }
        error(e, at(sm, f, 4, 1), "h");
        error(e, at(sm, f, 4, 2), "h");
    });
    assert_eq!((out.as_str(), status), ("", 0));

    // Too few and too many: one line per missing occurrence.
    let c = Case::new("a // expected-error 3 {{e}}\nb // expected-error 0-1 {{f}}\n");
    let (out, _) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 1), "e");
        error(e, at(sm, f, 2, 1), "f");
        error(e, at(sm, f, 2, 2), "f");
    });
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 1: e\n  File t.cpp Line 1: e\n\
         error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 2: f\n\
         3 errors generated.\n"
    );

    let c = Case::new("int z; // expected-error 2-1 {{x}}\nint y; // expected-error 2- {{x}}\n");
    let (out, _) = c.run(|_, _, _| {});
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: invalid range following '-' in expected string\n  \
         File t.cpp Line 2: invalid range following '-' in expected string\n\
         2 errors generated.\n"
    );
}

// spec: diag.md §11.2 (@line, @+N, @-N, @file:line, @*, @*:*)
#[test]
fn line_forms() {
    let c = Case::with_header(
        "// expected-error@3 {{abs}}\n\
         // expected-error@+1 {{plus}}\n\
         xyz\n\
         // expected-error@-1 {{minus}}\n\
         // expected-error@h.h:2 {{hdr}}\n\
         // expected-error@h.h:* {{hdrany}}\n\
         // expected-error@* {{anyline}}\n\
         // expected-error@*:* {{anywhere}}\n",
        "h1\nh2\nh3\n",
    );
    let h = c.header.unwrap();
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 3, 1), "abs");
        error(e, at(sm, f, 3, 2), "plus");
        error(e, at(sm, f, 3, 3), "minus");
        error(e, at(sm, h, 2, 1), "hdr");
        error(e, at(sm, h, 3, 1), "hdrany");
        error(e, at(sm, f, 8, 1), "anyline");
        error(e, Span::INVALID, "anywhere");
    });
    assert_eq!((out.as_str(), status), ("", 0));
}

// spec: diag.md §11.2 (bad @ forms)
#[test]
fn bad_line_forms() {
    let c = Case::new(
        "// expected-error@ {{x}}\n\
         // expected-error@-5 {{x}}\n\
         // expected-error@0 {{x}}\n\
         // expected-error@nofile.h:1 {{x}}\n\
         // expected-error@*:1 {{x}}\n\
         // expected-error-re@ {{x}}\n",
    );
    let (out, _) = c.run(|_, _, _| {});
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: missing or invalid line number following '@' in expected string\n  \
         File t.cpp Line 2: missing or invalid line number following '@' in expected string\n  \
         File t.cpp Line 3: missing or invalid line number following '@' in expected string\n  \
         File t.cpp Line 4: file 'nofile.h' could not be located in expected string\n  \
         File t.cpp Line 5: missing or invalid line number following '@' in expected '*'\n  \
         File t.cpp Line 6: missing or invalid line number following '@' in expected regex\n\
         6 errors generated.\n"
    );
}

// spec: diag.md §11.2 (missing `{{`, missing end, nested braces)
#[test]
fn missing_braces() {
    let c = Case::new(
        "// expected-note {{missing end\n\
         // expected-error x {{y}}\n\
         // expected-warning {{{a}} b}}}\n\
         // expected-warning {{c {{d}} e}}\n",
    );
    let (out, _) = c.run(|e, sm, f| {
        warning(e, at(sm, f, 3, 1), "x a}} b y");
        warning(e, at(sm, f, 4, 1), "c {{d}} e");
    });
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: cannot find end ('}}') of expected string\n  \
         File t.cpp Line 2: cannot find start ('{{') of expected string\n\
         2 errors generated.\n"
    );
}

// spec: diag.md §11.4 (-re: regex parts, the missing regex, invalid regex)
#[test]
fn regex_directives() {
    let c = Case::new(
        "a // expected-error-re {{member of '{{.*}}' (aka '{{[a-z]+}}')}}\n\
         b // expected-error-re {{{{^}}starts{{$}}}}\n\
         c // expected-error-re {{x{{[}}y}}\n\
         d // expected-error-re {{no regex}} expected-error {{never parsed}}\n",
    );
    let (out, _) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 1), "no member of 'S<int>' (aka 'foo')");
        error(e, at(sm, f, 2, 1), "starts");
    });
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 3: x{{[}}y\n\
         error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 3: invalid expected regex: brackets ([ ]) not balanced\n  \
         File t.cpp Line 4: cannot find start of regex ('{{') in no regex\n\
         3 errors generated.\n"
    );
}

// spec: diag.md §11.5 (notes are checked; unexpected notes reported)
#[test]
fn notes() {
    let c = Case::new(
        "void f(); // expected-note {{declared here}}\n\
         int x = f(); // expected-error {{cannot}}\n",
    );
    let (out, _) = c.run(|e, sm, f| {
        Diag::new(DiagId::pp_error_directive, at(sm, f, 2, 9))
            .arg("cannot initialize")
            .note("declared here", at(sm, f, 1, 6))
            .note("candidate here", at(sm, f, 1, 6))
            .emit(e);
    });
    assert_eq!(
        out,
        "error: 'expected-note' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: candidate here\n\
         1 error generated.\n"
    );

    // -verify-ignore-unexpected=note
    let mut o = VerifyOptions::default();
    o.ignore_unexpected[VerifyLevel::Note as usize] = true;
    let c = Case::new("int x = f(); // expected-error {{cannot}}\n");
    let (out, status) = c.run_with(o, |e, sm, f| {
        Diag::new(DiagId::pp_error_directive, at(sm, f, 1, 9))
            .arg("cannot initialize")
            .note("candidate here", at(sm, f, 1, 6))
            .emit(e);
    });
    assert_eq!((out.as_str(), status), ("", 0));
}

// spec: diag.md §11.5 (duplicate directives each need their own diagnostic)
#[test]
fn duplicate_directives() {
    let c = Case::new("x // expected-error {{dup}} expected-error {{dup}}\n");
    let (out, _) = c.run(|e, sm, f| error(e, at(sm, f, 1, 1), "dup"));
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 1: dup\n\
         1 error generated.\n"
    );
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 1), "dup");
        error(e, at(sm, f, 1, 3), "dup");
    });
    assert_eq!((out.as_str(), status), ("", 0));
}

// spec: diag.md §11.2 (prefixes; the report names the first prefix)
#[test]
fn prefixes() {
    let c = Case::new(
        "a // foo-error {{a}}\n\
         b // bar-warning {{b}}\n\
         c // expected-error {{not mine}}\n\
         d // foo-bar-error {{not mine either}}\n",
    );
    let (out, _) = c.run_with(opts(&["foo", "bar"]), |e, sm, f| {
        error(e, at(sm, f, 1, 1), "a");
        warning(e, at(sm, f, 2, 1), "b");
        error(e, at(sm, f, 3, 1), "not mine");
    });
    assert_eq!(
        out,
        "error: 'bar-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 3: not mine\n\
         1 error generated.\n"
    );
}

// spec: diag.md §11.3 (markers: @#name, deferred, undefined, ambiguous)
#[test]
fn markers() {
    let c = Case::new(
        "// expected-error@#later {{l}}\n\
         int a; // #first\n\
         // expected-error@#first {{f}}\n\
         int b; // #later\n",
    );
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 2, 1), "f");
        error(e, at(sm, f, 4, 1), "l");
    });
    assert_eq!((out.as_str(), status), ("", 0));

    let c = Case::new(
        "int a; // #m\n\
         int b; // #m\n\
         // expected-error@#m {{f}} expected-error@#nope {{g}}\n",
    );
    let (out, _) = c.run(|_, _, _| {});
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 1 (directive at t.cpp:3): f\n\
         error: 'expected-error' diagnostics seen but not expected: \n  \
         File t.cpp Line 3: reference to marker '#m' is ambiguous\n  \
         File t.cpp Line 3: use of undefined marker '#nope'\n\
         error: 'expected-note' diagnostics seen but not expected: \n  \
         File t.cpp Line 1: ambiguous marker '#m' is defined here\n  \
         File t.cpp Line 2: ambiguous marker '#m' is defined here\n\
         5 errors generated.\n"
    );
}

// spec: diag.md §11.1 (directives must start a word; several per comment)
#[test]
fn word_start_and_multiple() {
    let c = Case::new(
        "x /* expected-error {{one}} */ // notexpected-error {{no}} xexpected-error {{no}}\n\
         y //expected-error {{two}} expected-warning {{three}}\n",
    );
    let (out, status) = c.run(|e, sm, f| {
        error(e, at(sm, f, 1, 1), "one");
        error(e, at(sm, f, 2, 1), "two");
        warning(e, at(sm, f, 2, 1), "three");
    });
    assert_eq!((out.as_str(), status), ("", 0));
}

// spec: diag.md §11.6 (frontend diagnostics and `\n` in directive text)
#[test]
fn frontend_and_newlines() {
    let c = Case::new("// expected-error {{a\\nb\\nc}}\n");
    let (out, _) = c.run(|e, _, _| error(e, Span::INVALID, "unknown argument"));
    assert_eq!(
        out,
        "error: 'expected-error' diagnostics expected but not seen: \n  \
         File t.cpp Line 1: a\nb\n\n\
         error: 'expected-error' diagnostics seen but not expected: \n  \
         (frontend): unknown argument\n\
         2 errors generated.\n"
    );
}

// spec: diag.md §11.2 (prefix validity)
#[test]
fn prefix_validity() {
    for ok in ["expected", "a", "A-b_9", "x-"] {
        assert!(VerifyOptions::is_valid_prefix(ok), "{ok}");
    }
    for bad in ["", "9a", "-a", "a.b", "a b", "_a"] {
        assert!(!VerifyOptions::is_valid_prefix(bad), "{bad}");
    }
}
