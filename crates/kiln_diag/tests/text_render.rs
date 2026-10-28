//! Text-renderer goldens. Every expected string is clang 19's stderr for
//! the same source and the same locations (`spec/diag.md` section 9),
//! with only the message text substituted where kiln has no such id yet.

mod common;

use common::{arg, assert_text, at, mac, source, sp, sub, text};
use kiln_diag::{Diag, DiagId, FixIt};
use kiln_source::Span;

const INVALID_OPERANDS: &str = "invalid operands to binary expression ('const char[2]' and 'int')";

fn err(msg: &str, span: Span) -> Diag {
    Diag::new(DiagId::pp_error_directive, span).arg(msg)
}

// spec: diag.md §9.1, §9.3 (single-line snippet, ranges, caret)
#[test]
fn single_line_with_ranges() {
    let (sm, f) = source("t.cpp", "int x = \"a\" * 2;\n");
    let out = text(&sm, &[], |e| {
        err(INVALID_OPERANDS, sp(&sm, f, 1, 13, 1))
            .range(sp(&sm, f, 1, 9, 3))
            .range(sp(&sm, f, 1, 15, 1))
            .emit(e);
    });
    assert_text(
        &out,
        "t.cpp:1:13: error: invalid operands to binary expression ('const char[2]' and 'int')\n\
         \x20   1 | int x = \"a\" * 2;\n\
         \x20     |         ~~~ ^ ~\n\
         1 error generated.\n",
    );
}

const R_CPP: &str = "int a1 = 0;\n\tint x = \t\"a\" * 2;\nint y = (\"a\"\n         *\n\n   2);\n#line 100 \"foo.h\"\nint z = \"a\" *\n   2;\n#line 9998\nint q =\n \"b\"\n *\n 3;\n";

const R_CPP_OUT: &str = "\
r.cpp:2:15: error: invalid operands to binary expression ('const char[2]' and 'int')
    2 |         int x =         \"a\" * 2;
      |                         ~~~ ^ ~
r.cpp:4:10: error: invalid operands to binary expression ('const char[2]' and 'int')
    3 | int y = (\"a\"
      |          ~~~
    4 |          *
      |          ^
    5 | 
    6 |    2);
      |    ~
foo.h:100:13: error: invalid operands to binary expression ('const char[2]' and 'int')
  100 | int z = \"a\" *
      |         ~~~ ^
  101 |    2;
      |    ~
foo.h:10000:2: error: invalid operands to binary expression ('const char[2]' and 'int')
  9999 |  \"b\"
       |  ~~~
 10000 |  *
       |  ^
 10001 |  3;
       |  ~
4 errors generated.
";

fn render_r(name: &str, src: &str) -> String {
    let (sm, f) = source(name, src);
    let line_start = |l: u32| common::offset(&sm, f, l, 1);
    sm.add_line_directive(f, line_start(8), 100, Some("foo.h"));
    sm.add_line_directive(f, line_start(11), 9998, None);
    text(&sm, &[], |e| {
        err(INVALID_OPERANDS, sp(&sm, f, 2, 15, 1))
            .range(sp(&sm, f, 2, 11, 3))
            .range(sp(&sm, f, 2, 17, 1))
            .emit(e);
        err(INVALID_OPERANDS, sp(&sm, f, 4, 10, 1))
            .range(sp(&sm, f, 3, 10, 3))
            .range(sp(&sm, f, 6, 4, 1))
            .emit(e);
        err(INVALID_OPERANDS, sp(&sm, f, 8, 13, 1))
            .range(sp(&sm, f, 8, 9, 3))
            .range(sp(&sm, f, 9, 4, 1))
            .emit(e);
        err(INVALID_OPERANDS, sp(&sm, f, 13, 2, 1))
            .range(sp(&sm, f, 12, 2, 3))
            .range(sp(&sm, f, 14, 2, 1))
            .emit(e);
    })
}

// spec: diag.md §9.3, §9.4 (tabs, multi-line ranges, blank lines,
// presumed `#line` numbers and gutter width)
#[test]
fn tabs_multiline_and_line_directives() {
    assert_text(&render_r("r.cpp", R_CPP), R_CPP_OUT);
}

// spec: diag.md §9.3 (CRLF and lone-CR line terminators render as LF)
#[test]
fn crlf_and_cr() {
    assert_text(&render_r("r.cpp", &R_CPP.replace('\n', "\r\n")), R_CPP_OUT);
    assert_text(&render_r("r.cpp", &R_CPP.replace('\n', "\r")), R_CPP_OUT);
}

const M_CPP: &str = "\
#define INNER(x) (x + \"a\" * 2)
#define OUTER(y) INNER(y)
#define BAD bad_name
int v = OUTER(1);
int w = BAD;
#define ID(x) x
int u = ID(nope);
int t = ID(BAD);
#define CALL OUTER(2)
int s = CALL;
";

// spec: diag.md §8.2-§8.4 (macro backtrace, range mapping, argument
// expansions)
#[test]
#[allow(clippy::similar_names)]
fn nested_macro_chain() {
    let (sm, f) = source("m.cpp", M_CPP);
    let s = &sm;
    // int v = OUTER(1);
    let outer = mac(s, at(s, f, 2, 18), 8, sp(s, f, 4, 9, 8), sp(s, f, 4, 9, 5));
    let y_arg = arg(s, at(s, f, 4, 15), 1, sub(outer, 6, 7));
    let inner = mac(s, at(s, f, 1, 18), 13, sub(outer, 0, 8), sub(outer, 0, 5));
    let _x_arg = arg(s, y_arg, 1, sub(inner, 1, 2));
    // int w = BAD;
    let bad = mac(s, at(s, f, 3, 13), 8, sp(s, f, 5, 9, 3), sp(s, f, 5, 9, 3));
    // int u = ID(nope);
    let id1 = mac(s, at(s, f, 6, 15), 1, sp(s, f, 7, 9, 8), sp(s, f, 7, 9, 2));
    let nope = arg(s, at(s, f, 7, 12), 4, sub(id1, 0, 1));
    // int t = ID(BAD);  (BAD is pre-expanded inside the argument)
    let id2 = mac(s, at(s, f, 6, 15), 1, sp(s, f, 8, 9, 7), sp(s, f, 8, 9, 2));
    let bad2 = mac(s, at(s, f, 3, 13), 8, sp(s, f, 8, 12, 3), sp(s, f, 8, 12, 3));
    let bad2_arg = arg(s, bad2, 8, sub(id2, 0, 1));
    // int s = CALL;
    let call = mac(s, at(s, f, 9, 14), 8, sp(s, f, 10, 9, 4), sp(s, f, 10, 9, 4));
    let outer2 = mac(s, at(s, f, 2, 18), 8, sub(call, 0, 8), sub(call, 0, 5));
    let y2 = arg(s, call.with_offset(6), 1, sub(outer2, 6, 7));
    let inner2 = mac(s, at(s, f, 1, 18), 13, sub(outer2, 0, 8), sub(outer2, 0, 5));
    let _x2 = arg(s, y2, 1, sub(inner2, 1, 2));

    let out = text(s, &[], |e| {
        err(INVALID_OPERANDS, sub(inner, 9, 10))
            .range(sub(inner, 5, 8))
            .range(sub(inner, 11, 12))
            .emit(e);
        err("use of undeclared identifier 'bad_name'", sub(bad, 0, 8)).emit(e);
        err("use of undeclared identifier 'nope'", sub(nope, 0, 4)).emit(e);
        err("use of undeclared identifier 'bad_name'", sub(bad2_arg, 0, 8)).emit(e);
        err(INVALID_OPERANDS, sub(inner2, 9, 10))
            .range(sub(inner2, 5, 8))
            .range(sub(inner2, 11, 12))
            .emit(e);
    });
    assert_text(
        &out,
        "\
m.cpp:4:9: error: invalid operands to binary expression ('const char[2]' and 'int')
    4 | int v = OUTER(1);
      |         ^~~~~~~~
m.cpp:2:18: note: expanded from macro 'OUTER'
    2 | #define OUTER(y) INNER(y)
      |                  ^~~~~~~~
m.cpp:1:27: note: expanded from macro 'INNER'
    1 | #define INNER(x) (x + \"a\" * 2)
      |                       ~~~ ^ ~
m.cpp:5:9: error: use of undeclared identifier 'bad_name'
    5 | int w = BAD;
      |         ^
m.cpp:3:13: note: expanded from macro 'BAD'
    3 | #define BAD bad_name
      |             ^
m.cpp:7:12: error: use of undeclared identifier 'nope'
    7 | int u = ID(nope);
      |            ^
m.cpp:8:12: error: use of undeclared identifier 'bad_name'
    8 | int t = ID(BAD);
      |            ^
m.cpp:3:13: note: expanded from macro 'BAD'
    3 | #define BAD bad_name
      |             ^
m.cpp:10:9: error: invalid operands to binary expression ('const char[2]' and 'int')
   10 | int s = CALL;
      |         ^~~~
m.cpp:9:14: note: expanded from macro 'CALL'
    9 | #define CALL OUTER(2)
      |              ^~~~~~~~
m.cpp:2:18: note: expanded from macro 'OUTER'
    2 | #define OUTER(y) INNER(y)
      |                  ^~~~~~~~
m.cpp:1:27: note: expanded from macro 'INNER'
    1 | #define INNER(x) (x + \"a\" * 2)
      |                       ~~~ ^ ~
5 errors generated.
",
    );
}

// spec: diag.md §8.2 (backtrace limit: first half, skip note, last half)
#[test]
fn macro_backtrace_limit() {
    let src = "#define A1 A2\n#define A2 A3\n#define A3 A4\n#define A4 A5\n#define A5 A6\n#define A6 A7\n#define A7 A8\n#define A8 zz\nint i = A1;\n";
    let (sm, f) = source("deep.cpp", src);
    let s = &sm;
    let mut exp = sp(s, f, 9, 9, 2);
    let mut last = None;
    for line in 1..=8 {
        let e = mac(s, at(s, f, line, 12), 2, exp, exp);
        exp = sub(e, 0, 2);
        last = Some(e);
    }
    let zz = last.unwrap();
    let emit = |e: &kiln_diag::DiagEngine<'_>| {
        err("use of undeclared identifier 'zz'", sub(zz, 0, 2)).emit(e);
    };
    let note = |l: u32| {
        format!("deep.cpp:{l}:12: note: expanded from macro 'A{l}'\n    {l} | #define A{l} A{}\n      |            ^\n", l + 1)
            .replace("A9", "zz")
    };
    let head = "deep.cpp:9:9: error: use of undeclared identifier 'zz'\n    9 | int i = A1;\n      |         ^\n";
    let want = format!(
        "{head}{}{}{}note: (skipping 2 expansions in backtrace; use -fmacro-backtrace-limit=0 to see all)\n{}{}{}1 error generated.\n",
        note(1),
        note(2),
        note(3),
        note(6),
        note(7),
        note(8)
    );
    assert_text(&text(s, &[], emit), &want);
    let all: String = (1..=8).map(note).collect();
    assert_text(
        &text(s, &["-fmacro-backtrace-limit=0"], emit),
        &format!("{head}{all}1 error generated.\n"),
    );
}

// spec: diag.md §9.1, §9.2 (colors), §5 (option suffix), §9.3 (note
// after a diagnostic at the same location still shows its snippet)
#[test]
fn colors_and_option_names() {
    let (sm, f) = source("col2.cpp", "#warning hi\n#define A x\n#define A y\nint a\u{1}b;\n");
    let s = &sm;
    let run = |args: &[&str]| {
        text(s, args, |e| {
            Diag::new(DiagId::pp_warning_directive, sp(s, f, 1, 2, 7)).arg("hi").emit(e);
            Diag::new(DiagId::pp_macro_redefined, sp(s, f, 3, 9, 1))
                .arg("A")
                .note(DiagId::note_previous_definition, sp(s, f, 2, 9, 1))
                .emit(e);
            err("expected ';' after top level declarator", sp(s, f, 4, 6, 0))
                .fixit(FixIt::insert(at(s, f, 4, 6), ";"))
                .emit(e);
        })
    };
    // clang additionally highlights the keyword `int` on line 4
    // (`\x1b[0;34mint\x1b[0m`); kiln does not (deviation D1).
    assert_text(
        &run(&["-fcolor-diagnostics"]),
        "\x1b[1mcol2.cpp:1:2: \x1b[0m\x1b[0;1;35mwarning: \x1b[0m\x1b[1mhi [-W#warnings]\x1b[0m\n\
         \x20   1 | #warning hi\x1b[0m\n\
         \x20     | \x1b[0;1;32m ^\n\
         \x1b[0m\x1b[1mcol2.cpp:3:9: \x1b[0m\x1b[0;1;35mwarning: \x1b[0m\x1b[1m'A' macro redefined [-Wmacro-redefined]\x1b[0m\n\
         \x20   3 | #define A y\x1b[0m\n\
         \x20     | \x1b[0;1;32m        ^\n\
         \x1b[0m\x1b[1mcol2.cpp:2:9: \x1b[0m\x1b[0;1;36mnote: \x1b[0mprevious definition is here\x1b[0m\n\
         \x20   2 | #define A x\x1b[0m\n\
         \x20     | \x1b[0;1;32m        ^\n\
         \x1b[0m\x1b[1mcol2.cpp:4:6: \x1b[0m\x1b[0;1;31merror: \x1b[0m\x1b[1mexpected ';' after top level declarator\x1b[0m\n\
         \x20   4 | int a\x1b[7m<U+0001>\x1b[0mb;\x1b[0m\n\
         \x20     | \x1b[0;1;32m     ^\n\
         \x1b[0m      | \x1b[0;32m     ;\n\
         \x1b[0m2 warnings and 1 error generated.\n",
    );
    assert_text(
        &run(&["-Werror"]),
        "col2.cpp:1:2: error: hi [-Werror,-W#warnings]\n\
         \x20   1 | #warning hi\n\
         \x20     |  ^\n\
         col2.cpp:3:9: error: 'A' macro redefined [-Werror,-Wmacro-redefined]\n\
         \x20   3 | #define A y\n\
         \x20     |         ^\n\
         col2.cpp:2:9: note: previous definition is here\n\
         \x20   2 | #define A x\n\
         \x20     |         ^\n\
         col2.cpp:4:6: error: expected ';' after top level declarator\n\
         \x20   4 | int a<U+0001>b;\n\
         \x20     |      ^\n\
         \x20     |      ;\n\
         3 errors generated.\n",
    );
}

// spec: diag.md §9.1, §9.2 (locationless diagnostics; unknown options;
// fatal errors)
#[test]
fn locationless_and_fatal() {
    let (sm, f) = source("pp4.cpp", "#include <nonexist>\n");
    let s = &sm;
    let out = text(s, &["-fcolor-diagnostics", "-Wfoo", "-Wmacro-redefine"], |e| {
        Diag::new(DiagId::pp_file_not_found, sp(s, f, 1, 10, 10))
            .arg("nonexist")
            .range(sp(s, f, 1, 10, 10))
            .emit(e);
        // Dropped: a fatal error stops everything.
        err("after fatal", sp(s, f, 1, 1, 1)).emit(e);
    });
    assert_text(
        &out,
        "\x1b[0;1;35mwarning: \x1b[0m\x1b[1munknown warning option '-Wfoo' [-Wunknown-warning-option]\x1b[0m\n\
         \x1b[0;1;35mwarning: \x1b[0m\x1b[1munknown warning option '-Wmacro-redefine'; did you mean '-Wmacro-redefined'? [-Wunknown-warning-option]\x1b[0m\n\
         \x1b[1mpp4.cpp:1:10: \x1b[0m\x1b[0;1;31mfatal error: \x1b[0m\x1b[1m'nonexist' file not found\x1b[0m\n\
         \x20   1 | #include <nonexist>\x1b[0m\n\
         \x20     | \x1b[0;1;32m         ^~~~~~~~~~\n\
         \x1b[0m2 warnings and 1 error generated.\n",
    );
}

// spec: diag.md §9.3 (display flags)
#[test]
fn display_flags() {
    let (sm, f) = source("t.cpp", "int x = \"a\" * 2;\n");
    let s = &sm;
    let emit = |e: &kiln_diag::DiagEngine<'_>| {
        Diag::new(DiagId::pp_macro_redefined, sp(s, f, 1, 13, 1))
            .arg("A")
            .range(sp(s, f, 1, 9, 3))
            .emit(e);
    };
    assert_text(
        &text(s, &["-fno-caret-diagnostics"], emit),
        "t.cpp:1:13: warning: 'A' macro redefined [-Wmacro-redefined]\n1 warning generated.\n",
    );
    assert_text(
        &text(
            s,
            &[
                "-fno-diagnostics-show-line-numbers",
                "-fno-diagnostics-show-option",
                "-fno-show-column",
            ],
            emit,
        ),
        "t.cpp:1: warning: 'A' macro redefined\nint x = \"a\" * 2;\n        ~~~ ^\n1 warning generated.\n",
    );
    assert_text(&text(s, &["-w"], emit), "");
}

// spec: diag.md §9.4 (invalid UTF-8 shows the original bytes as <XX>)
#[test]
fn invalid_utf8_bytes() {
    let sm = kiln_source::SourceManager::new();
    let dir = std::env::temp_dir().join(format!("kiln_diag_utf8_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("u.cpp");
    std::fs::write(&path, b"int \xff\xfe = 1;\n").unwrap();
    let f = sm.load_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    // Decoded text is "int \u{FFFD}\u{FFFD} = 1;" (3 bytes per FFFD); the
    // `=` is decoded offset 11, raw column 8.
    let eq = sm.loc(f, 11);
    let name = sm.file_name(f).to_string();
    let out = text(&sm, &[], |e| err("bad", Span::new(eq, eq.with_offset(1))).emit(e));
    assert_text(
        &out,
        &format!(
            "{name}:1:8: error: bad\n    1 | int <FF><FE> = 1;\n      |              ^\n1 error generated.\n"
        ),
    );
}
