//! WP-1995: evaluation sessions opened from a driver command line
//! (spec/eval-session.md §2): include paths, predefines and the
//! standard library apply as for `-fsyntax-only`, the host-call builtin
//! is visible, and a translation unit with errors is refused.
#![forbid(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kiln_diag::{DiagId, Level};
use kiln_driver::front_end::{ConsultedKind, Content, FrontEndOptions, run_front_end};
use kiln_driver::session::{SessionRefused, with_eval_session};
use kiln_driver::{Io, SharedWriter};
use kiln_parse::{CancelToken, EvalSession, HostCalls, HostError, HostOut, RunBudget, RunValue};

/// Standard error, kept for the test to read.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

fn io(err: &Captured) -> Io {
    Io {
        stdout: Box::new(std::io::sink()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    }
}

fn kilnpp() -> String {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++").to_string_lossy().into_owned()
}

/// A fresh directory holding `files`.
fn scratch(test: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-eval-session-{}-{test}", std::process::id()));
    for (name, src) in files {
        let p = d.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, src).unwrap();
    }
    d
}

/// Op 1 returns 7.
struct Seven;

impl HostCalls for Seven {
    fn call(
        &mut self,
        op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        if op == 1 { Ok(7) } else { Err(HostError::new("unknown op")) }
    }
}

#[test]
fn opened_from_a_command_line() {
    let d = scratch(
        "open",
        &[
            ("inc/lib.h", "#include <vector>\nconstexpr int scale = SCALE;\n"),
            (
                "t.cpp",
                "#include \"lib.h\"\n\
                 consteval int entry() {\n\
                   std::vector<int> v;\n\
                   for (int i = 0; i < 1000; ++i) v.push_back(i * scale);\n\
                   return v.back() + (int)__builtin_kiln_host_call(1, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
                 }\n",
            ),
        ],
    );
    let err = Captured::default();
    let t = d.join("t.cpp").to_string_lossy().into_owned();
    let inc = format!("-I{}", d.join("inc").display());
    let cl = ["-std=c++26".to_owned(), inc, "-DSCALE=3".to_owned(), t];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        assert!(fe.main.is_some());
        let e = fe.session.entry_named("entry").expect("entry");
        let mut out = Vec::new();
        for _ in 0..3 {
            let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
            out.push((r.result, r.steps));
        }
        out
    });
    assert_eq!(err.text(), "");
    assert_eq!(got[0].0, Ok(RunValue::Signed(999 * 3 + 7)));
    assert!(got.iter().all(|g| *g == got[0]), "{got:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

/// WP-2072: a `constexpr` entry over the standard library's own
/// constant-evaluation paths (`std::is_constant_evaluated()` in
/// `std::vector` and `std::string`, [meta.const.eval]): the run is
/// manifestly constant-evaluated (spec/eval-session.md §4).
#[test]
fn a_constexpr_entry_over_the_standard_library() {
    let d = scratch(
        "constexpr",
        &[(
            "t.cpp",
            "#include <string>\n\
             #include <type_traits>\n\
             #include <vector>\n\
             constexpr int entry() {\n\
               std::vector<int> v;\n\
               for (int i = 0; i < 1000; ++i) v.push_back(i);\n\
               std::string s = \"abc\";\n\
               s += \"defgh\";\n\
               return v.back() + (int)s.size() + (std::is_constant_evaluated() ? 1000 : 0);\n\
             }\n\
             constexpr int leaks() { return *new int(1); }\n",
        )],
    );
    let err = Captured::default();
    let cl = ["-std=c++26".to_owned(), d.join("t.cpp").to_string_lossy().into_owned()];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        let e = fe.session.entry_named("entry").expect("a constexpr entry");
        let mut out = Vec::new();
        for _ in 0..3 {
            let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
            out.push((r.result, r.steps, r.calls));
        }
        let e = fe.session.entry_named("leaks").expect("a constexpr entry");
        let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
        let text: Vec<&str> = r.diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(text, ["call to constexpr function 'leaks' is not a constant expression"]);
        out
    });
    assert_eq!(err.text(), "");
    assert_eq!(got[0].0, Ok(RunValue::Signed(999 + 8 + 1000)));
    assert!(got.iter().all(|g| *g == got[0]), "{got:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_translation_unit_with_errors_is_refused() {
    let d = scratch("refused", &[("t.cpp", "int x = ;\n")]);
    let err = Captured::default();
    let cl = ["-std=c++26".to_owned(), d.join("t.cpp").to_string_lossy().into_owned()];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| fe.map(drop).err());
    let Some(SessionRefused::TuHasErrors { errors: 1, diagnostics }) = got else {
        panic!("{got:?}")
    };
    let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["expected expression"]);
    let text = err.text();
    assert!(text.contains("t.cpp:1:9: error: expected expression"), "{text}");
    assert!(
        text.contains(
            "error: cannot open an evaluation session: the translation unit has 1 error(s)"
        ),
        "{text}"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_command_line_without_one_source_is_refused() {
    let err = Captured::default();
    let got =
        with_eval_session(&kilnpp(), &["-std=c++26".to_owned()], io(&err), |fe| fe.map(drop).err());
    let message = "the command line does not name exactly one C++ source input".to_owned();
    assert_eq!(got, Some(SessionRefused::CommandLine { message }));
    let missing = ["no-such-file.cpp".to_owned()];
    let got = with_eval_session(&kilnpp(), &missing, io(&err), |fe| fe.map(drop).err());
    let message = "no such file or directory: 'no-such-file.cpp'".to_owned();
    assert_eq!(got, Some(SessionRefused::CommandLine { message }));
    assert!(err.text().contains("no-such-file.cpp"), "{}", err.text());
}

/// WP-2322 (spec/eval-session.md §2 "Refusal"): why no session was
/// opened, case by case: the driver's message for a command line it
/// rejects; the records of a unit with errors (the error limit's fatal
/// error included); the records of a front end that stopped (a fatal
/// error); a panic of the front end; and a panic of the closure, which
/// propagates.
#[test]
fn refusals_say_why() {
    let d = scratch(
        "refusals",
        &[
            ("ok.cpp", "consteval int f() { return 1; }\n"),
            ("errors.cpp", "int a = ;\nint b = ;\nint c = ;\n"),
            ("fatal.cpp", "int a = ;\n#include \"missing.h\"\nint b;\n"),
        ],
    );
    let path = |n: &str| d.join(n).to_string_lossy().into_owned();
    let refused = |extra: &[&str], file: &str| {
        let err = Captured::default();
        let mut cl: Vec<String> = extra.iter().map(|&s| s.to_owned()).collect();
        cl.push(path(file));
        let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| fe.map(drop).err());
        (got.expect("refused"), err.text())
    };
    // The driver's message.
    let (r, text) = refused(&["-std=c++99x"], "ok.cpp");
    let message = "invalid value 'c++99x' in '-std=c++99x'".to_owned();
    assert_eq!(r, SessionRefused::CommandLine { message }, "{text}");
    assert!(text.contains("error: invalid value 'c++99x' in '-std=c++99x'"), "{text}");
    // A unit with errors, and one stopped at the error limit.
    let (r, _) = refused(&[], "errors.cpp");
    assert!(
        matches!(&r, SessionRefused::TuHasErrors { errors: 3, diagnostics } if diagnostics.len() == 3),
        "{r:?}"
    );
    let (r, _) = refused(&["-ferror-limit=2"], "errors.cpp");
    let SessionRefused::TuHasErrors { errors: 3, diagnostics } = &r else { panic!("{r:?}") };
    let last = diagnostics.last().unwrap();
    assert_eq!((last.level, last.id), (Level::Fatal, Some(DiagId::diag_too_many_errors)));
    // A fatal error stops the front end: its records, the fatal one last.
    let (r, text) = refused(&[], "fatal.cpp");
    let SessionRefused::Stopped { diagnostics } = &r else { panic!("{r:?}") };
    let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["expected expression", "'missing.h' file not found"]);
    assert_eq!(diagnostics[1].level, Level::Fatal);
    assert!(text.contains("fatal error: 'missing.h' file not found"), "{text}");
    // A panic of the front end comes back as `Panicked`.
    let (r, _) = refused(&["-std=c++26", "-fkiln-crash-for-testing=parse"], "ok.cpp");
    assert!(matches!(&r, SessionRefused::Panicked(m) if !m.is_empty()), "{r:?}");
    // The same command line without the crash opens, on the same thread.
    let err = Captured::default();
    let ok = ["-std=c++26".to_owned(), path("ok.cpp")];
    let opened = with_eval_session(&kilnpp(), &ok, io(&err), |fe| fe.is_ok());
    assert!(opened, "{}", err.text());
    // A panic of the closure is the caller's.
    let caught = std::panic::catch_unwind(|| {
        let err = Captured::default();
        with_eval_session(&kilnpp(), &ok, io(&err), |_| -> u8 { panic!("the closure's") })
    });
    let payload = caught.unwrap_err();
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"the closure's"));
    std::fs::remove_dir_all(&d).unwrap();
}

/// WP-2324 (spec/eval-session.md §7): the public renderer renders records
/// exactly as the driver prints them, in text and JSON. The unit's own
/// warning (in a header, with a note) is rendered as `kiln++
/// -fsyntax-only` prints it on standard error, and as the session printed
/// it; a run's failure is rendered with its notes and its call stack.
#[test]
fn run_diagnostics_render_as_the_driver() {
    let d = scratch(
        "render",
        &[
            (
                "h.h",
                "[[deprecated(\"use new\")]] constexpr int old() { return 1; }\n\
                 inline int w = old();\n",
            ),
            (
                "t.cpp",
                "#include \"h.h\"\n\
                 constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
                 consteval int bad() { return at(5); }\n",
            ),
        ],
    );
    let t = d.join("t.cpp").to_string_lossy().into_owned();
    for json in [false, true] {
        let mut cl = vec!["-std=c++26".to_owned()];
        if json {
            cl.push("-fdiagnostics-format=json".to_owned());
        }
        cl.push(t.clone());
        let err = Captured::default();
        let (tu, run) = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
            let mut fe = fe.expect("session opens");
            assert_eq!(fe.tu_diagnostics.len(), 1, "{:?}", fe.tu_diagnostics);
            let e = fe.session.entry_named("bad").unwrap();
            let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.result.is_err());
            (fe.render(&fe.tu_diagnostics), fe.render(&r.diagnostics))
        });
        let mut exe_cl = vec!["-fsyntax-only".to_owned()];
        exe_cl.extend(cl.iter().cloned());
        let exe = std::process::Command::new(kilnpp()).args(&exe_cl).output().unwrap();
        assert!(exe.status.success());
        let exe = String::from_utf8(exe.stderr).unwrap();
        let summary = if json { "" } else { "1 warning generated.\n" };
        assert_eq!(format!("{tu}{summary}"), exe, "json: {json}");
        // The session printed the same records; the summary line is the
        // driver's at exit, which an opened session has not reached.
        assert_eq!(err.text(), tu, "json: {json}");
        if json {
            let lines: Vec<&str> = run.lines().collect();
            assert_eq!(lines.len(), 1, "{run}");
            assert!(lines[0].starts_with('{') && lines[0].ends_with('}'), "{run}");
            assert!(
                lines[0].contains("call to consteval function 'bad' is not a constant expression")
            );
            assert!(lines[0].contains("in call to 'at(5)'"), "{run}");
            assert!(lines[0].contains("in call to 'bad()'"), "{run}");
        } else {
            assert!(run.starts_with(&format!("{t}:3:15: error: call to consteval function 'bad' is not a constant expression\n")), "{run}");
            assert!(run.contains(&format!("{t}:2:46: note: cannot refer to element 5")), "{run}");
            assert!(run.contains("note: in call to 'at(5)'"), "{run}");
            assert!(run.contains("note: in call to 'bad()'"), "{run}");
            assert!(run.contains("    3 | consteval int bad() { return at(5); }\n"), "{run}");
        }
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// WP-2411 (spec/eval-session.md §2): a session's command line is words,
/// with no `@file` expansion. `@defs.rsp` (holding `-DV=5`) is a second
/// input, so the line names no one C++ source; an input named `@u.cpp`
/// is a source.
#[test]
fn a_sessions_words_are_not_response_files() {
    let d = scratch(
        "words",
        &[
            ("t.cpp", "consteval int f() { return V; }\n"),
            ("defs.rsp", "-DV=5\n"),
            ("@u.cpp", "consteval int g() { return 3; }\n"),
        ],
    );
    let path = |n: &str| d.join(n).to_string_lossy().into_owned();
    let err = Captured::default();
    let cl = ["-std=c++26".to_owned(), format!("@{}", path("defs.rsp")), path("t.cpp")];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| fe.map(drop).err());
    let message = "the command line does not name exactly one C++ source input".to_owned();
    assert_eq!(got, Some(SessionRefused::CommandLine { message }));
    let cl = [
        "-std=c++26".to_owned(),
        format!("-working-directory={}", d.display()),
        "@u.cpp".to_owned(),
    ];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        let e = fe.session.entry_named("g").unwrap();
        fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new()).result
    });
    assert_eq!(got, Ok(RunValue::Signed(3)), "{}", err.text());
    std::fs::remove_dir_all(&d).unwrap();
}

/// WP-2412 (spec/eval-session.md §2): `SessionFrontEnd::consulted` lists
/// the `#embed` resource and the `__has_include` / `__has_embed` hits with
/// their contents, as `FrontEnd::consulted` does for the same command
/// line; an included header is not listed.
#[test]
fn consulted_resources_are_listed() {
    let d = scratch(
        "consulted",
        &[
            ("probe.h", "int never;\n"),
            ("used.h", "int used;\n"),
            ("hit.bin", "xyz"),
            ("data.bin", "AB"),
            (
                "t.cpp",
                "#if __has_include(\"probe.h\") && __has_embed(\"hit.bin\")\n#endif\n\
                 #include \"used.h\"\n\
                 constexpr unsigned char d[] = {\n#embed \"data.bin\"\n};\n\
                 consteval int f() { return d[0] + d[1]; }\n",
            ),
        ],
    );
    let cl = ["-std=c++26".to_owned(), d.join("t.cpp").to_string_lossy().into_owned()];
    let err = Captured::default();
    let (consulted, v) = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        let e = fe.session.entry_named("f").unwrap();
        let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
        (fe.consulted.clone(), r.result)
    });
    assert_eq!(v, Ok(RunValue::Signed(i128::from(b'A') + i128::from(b'B'))), "{}", err.text());
    let kinds: Vec<(ConsultedKind, String, Option<Content>)> = consulted
        .iter()
        .map(|c| {
            let name = Path::new(&c.path).file_name().unwrap().to_string_lossy().into_owned();
            (c.kind, name, c.content)
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (ConsultedKind::Probe, "probe.h".to_owned(), Some(Content::of(b"int never;\n"))),
            (ConsultedKind::Probe, "hit.bin".to_owned(), Some(Content::of(b"xyz"))),
            (ConsultedKind::Embed, "data.bin".to_owned(), Some(Content::of(b"AB"))),
        ]
    );
    let mut fe_cl = vec!["-fsyntax-only".to_owned()];
    fe_cl.extend(cl.iter().cloned());
    let r =
        run_front_end(&kilnpp(), &fe_cl, FrontEndOptions::default(), |fe| fe.consulted.to_vec())
            .unwrap();
    assert_eq!(r.value, Some(consulted));
    std::fs::remove_dir_all(&d).unwrap();
}

/// WP-2413 (spec/eval-session.md §7): an exception leaving a run's entry
/// is reported with the call stack it left from, innermost first, under
/// the backtrace limit; the same exception in the translation unit's own
/// evaluation keeps clang's shape (the throw only).
#[test]
fn an_uncaught_exception_has_its_call_stack() {
    let src = "struct Err { int code; };\n\
               constexpr int thrower(int n) { if (n == 0) throw Err{42}; return thrower(n - 1) + 1; }\n\
               consteval int shallow() { return thrower(2); }\n\
               consteval int deep() { return thrower(30); }\n\
               constexpr int catcher() { try { return thrower(1); } catch (...) { throw; } }\n\
               constexpr int rethrows() { return catcher() + 1; }\n";
    let d = scratch(
        "uncaught",
        &[("t.cpp", src), ("tu.cpp", &format!("{src}constexpr int v = shallow();\n"))],
    );
    let t = d.join("t.cpp").to_string_lossy().into_owned();
    let cl = ["-std=c++26".to_owned(), t.clone()];
    let err = Captured::default();
    let shown = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        ["shallow", "deep", "rethrows"].map(|name| {
            let e = fe.session.entry_named(name).unwrap();
            let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(matches!(r.result, Err(kiln_parse::RunError::Uncaught { .. })), "{name}");
            fe.render(&r.diagnostics).replace(&t, "t.cpp")
        })
    });
    // The diagnostics' lines, without the source excerpts.
    let heads = |s: &str| -> Vec<String> {
        s.lines().filter(|l| l.starts_with("t.cpp:")).map(str::to_owned).collect()
    };
    let throw =
        "t.cpp:2:50: note: unhandled exception of type 'Err' with content {42} thrown from here";
    assert_eq!(
        heads(&shown[0]),
        [
            "t.cpp:3:15: error: call to consteval function 'shallow' is not a constant expression",
            throw,
            "t.cpp:2:66: note: in call to 'thrower(0)'",
            "t.cpp:2:66: note: in call to 'thrower(1)'",
            "t.cpp:3:34: note: in call to 'thrower(2)'",
            "t.cpp:3:15: note: in call to 'shallow()'",
        ],
        "{}",
        shown[0]
    );
    // 32 frames, the default limit of 10: the 5 innermost and outermost.
    let mut want = vec![
        "t.cpp:4:15: error: call to consteval function 'deep' is not a constant expression"
            .to_owned(),
        throw.to_owned(),
    ];
    want.extend((0..5).map(|n| format!("t.cpp:2:66: note: in call to 'thrower({n})'")));
    want.push(
        "t.cpp:2:66: note: (skipping 22 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
            .to_owned(),
    );
    want.extend((27..30).map(|n| format!("t.cpp:2:66: note: in call to 'thrower({n})'")));
    want.push("t.cpp:4:31: note: in call to 'thrower(30)'".to_owned());
    want.push("t.cpp:4:15: note: in call to 'deep()'".to_owned());
    assert_eq!(heads(&shown[1]), want, "{}", shown[1]);
    // A rethrow: the note at the throw, the frames of the rethrow.
    assert_eq!(
        heads(&shown[2]),
        [
            "t.cpp:6:15: error: call to constexpr function 'rethrows' is not a constant expression",
            throw,
            "t.cpp:6:35: note: in call to 'catcher()'",
            "t.cpp:6:15: note: in call to 'rethrows()'",
        ],
        "{}",
        shown[2]
    );
    // The unit's own evaluation: the throw's note only, as clang.
    let tu = d.join("tu.cpp").to_string_lossy().into_owned();
    let cl = ["-std=c++26".to_owned(), tu];
    let refused = with_eval_session(&kilnpp(), &cl, io(&err), |fe| fe.map(drop).err());
    let Some(SessionRefused::TuHasErrors { diagnostics, .. }) = refused else {
        panic!("{refused:?}")
    };
    let notes: Vec<&str> =
        diagnostics.iter().flat_map(|d| d.notes.iter()).map(|n| n.message.as_str()).collect();
    assert!(
        notes.contains(&"unhandled exception of type 'Err' with content {42} thrown from here"),
        "{notes:?}"
    );
    assert!(!notes.iter().any(|n| n.starts_with("in call to")), "{notes:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn reflection_in_runs() {
    let d = scratch(
        "meta",
        &[(
            "t.cpp",
            "#include \"kiln_meta.h\"\n\
             using namespace std::meta;\n\
             struct A;\n\
             struct B { int x; int y; long z; };\n\
             consteval int members() {\n\
               return (int)nonstatic_data_members_of(^^B, access_context::unchecked()).size();\n\
             }\n\
             consteval int define_in_run() {\n\
               define_aggregate(^^A, {data_member_spec(^^int, {.name = \"i\"})});\n\
               return 1;\n\
             }\n",
        )],
    );
    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support");
    let err = Captured::default();
    let cl = [
        "-std=c++26".to_owned(),
        "-freflection".to_owned(),
        format!("-I{}", support.display()),
        d.join("t.cpp").to_string_lossy().into_owned(),
    ];
    with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.unwrap_or_else(|e| panic!("{e:?}: {}", err.text()));
        let m = fe.session.entry_named("members").unwrap();
        let def = fe.session.entry_named("define_in_run").unwrap();
        let mut first = None;
        for _ in 0..3 {
            let r = fe.session.run(m, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert_eq!(r.result, Ok(RunValue::Signed(3)), "{:?}", r.diagnostics);
            // A run cannot inject a declaration ([expr.const]): every run
            // fails the same way, and nothing it tried is kept.
            let r = fe.session.run(def, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.result.is_err());
            let msgs: Vec<String> = r.diagnostics.iter().map(|d| d.message.clone()).collect();
            assert!(!msgs.is_empty());
            assert_eq!(first.get_or_insert_with(|| msgs.clone()), &msgs);
        }
    });
    assert_eq!(err.text(), "");
    std::fs::remove_dir_all(&d).unwrap();
}

/// `VmRSS` and `VmHWM` of this process in KiB (Linux; 0 elsewhere).
fn rss_kib() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |k: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// Op 1 returns its call count.
#[derive(Default)]
struct Count(u64);

impl HostCalls for Count {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        self.0 += 1;
        Ok(self.0)
    }
}

const TRANSFORM: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op) {\n\
  return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
}\n\
consteval void empty() {}\n\
consteval int one() { return 1; }\n\
consteval u64 hosts() { u64 s = 0; for (int i = 0; i < 100; ++i) s += hc(1); return s; }\n\
consteval int spin() { for (;;) {} return 0; }\n";

/// `runs` runs of `name` with a fresh handler each; their total time.
fn time_runs(session: &mut EvalSession<'_, '_>, name: &str, runs: u32) -> Duration {
    let entry = session.entry_named(name).unwrap();
    let token = CancelToken::new();
    let start = Instant::now();
    for _ in 0..runs {
        let report = session.run(entry, &mut Count::default(), RunBudget::PROGRAM, &token);
        assert!(report.result.is_ok());
    }
    start.elapsed()
}

/// From `cancel()` on another thread to the end of a `for (;;) {}` run.
fn cancel_latency(session: &mut EvalSession<'_, '_>) -> Duration {
    let spin = session.entry_named("spin").unwrap();
    let token = CancelToken::new();
    std::thread::scope(|scope| {
        let shared = &token;
        let canceller = scope.spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            shared.cancel();
            Instant::now()
        });
        let report = session.run(spin, &mut Count::default(), RunBudget::PROGRAM, &token);
        let done = Instant::now();
        assert_eq!(report.result, Err(kiln_parse::RunError::Cancelled));
        done.saturating_duration_since(canceller.join().unwrap())
    })
}

/// Runs the 10^6-element vector entry up to 1000 times (at most 10
/// minutes): is `kept()` after the last run `kept()` after run 2?
fn vector_flat(session: &mut EvalSession<'_, '_>) {
    let Ok(vec) = session.entry_named("vec") else { return };
    let token = CancelToken::new();
    let start = Instant::now();
    let first = session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
    println!(
        "1e6-element vector: {:?} per run, {} steps, peak {} KiB ({:?})",
        start.elapsed(),
        first.steps,
        first.peak_heap / 1024,
        first.result
    );
    session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
    let second = session.kept();
    let (rss_before, _) = rss_kib();
    let start = Instant::now();
    let mut runs = 2;
    while runs < 1000 && start.elapsed().as_secs() < 600 {
        session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
        runs += 1;
    }
    let (rss_after, hwm) = rss_kib();
    println!(
        "kept after run {runs} == after run 2: {} ({:?}); RSS {rss_before} -> {rss_after} KiB, \
         peak {hwm} KiB",
        session.kept() == second,
        session.kept()
    );
}

/// The per-run numbers of one session.
fn measure_runs(session: &mut EvalSession<'_, '_>) {
    let start = Instant::now();
    let empty = session.entry_named("empty").unwrap();
    let report = session.run(empty, &mut Count::default(), RunBudget::PROGRAM, &CancelToken::new());
    println!(
        "first empty run (open to first instruction): {:?} ({:?})",
        start.elapsed(),
        report.result
    );
    let runs = 100_000u32;
    println!("empty run: {:?} each", time_runs(session, "empty", runs) / runs);
    let trivial = time_runs(session, "one", runs);
    println!("trivial entry: {:.0} runs/s", f64::from(runs) / trivial.as_secs_f64());
    time_runs(session, "hosts", 2);
    let kept = session.kept();
    let hosts = time_runs(session, "hosts", 50_000);
    println!(
        "50,000 runs x 100 host calls: {hosts:?} ({:?} per run); kept flat: {}",
        hosts / 50_000,
        session.kept() == kept
    );
    println!("cancel latency (for (;;) {{}}): {:?}", cancel_latency(session));
    vector_flat(session);
}

/// The numbers of spec/eval-session.md §10 (a release build:
/// `cargo test --release -p kiln_driver --test eval_session -- --ignored
/// --nocapture measure`).
#[test]
#[ignore = "measurement: run in a release build with --ignored --nocapture"]
fn measure() {
    let vector = "consteval unsigned long vec() { std::vector<int> v; \
                  for (int i = 0; i < 1000000; ++i) v.push_back(i); return v.size(); }\n";
    for (label, pre) in
        [("small transform", ""), ("<bits/stdc++.h> + transform", "#include <bits/stdc++.h>\n")]
    {
        let body =
            if pre.is_empty() { TRANSFORM.to_owned() } else { format!("{pre}{TRANSFORM}{vector}") };
        let dir = scratch("measure", &[("t.cpp", &body)]);
        let err = Captured::default();
        let cl = [
            "-std=c++26".to_owned(),
            "-O0".to_owned(),
            dir.join("t.cpp").to_string_lossy().into_owned(),
        ];
        let (rss_start, _) = rss_kib();
        let start = Instant::now();
        with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
            let opened = start.elapsed();
            let mut fe = fe.unwrap_or_else(|e| panic!("{e:?}: {}", err.text()));
            let (rss_open, _) = rss_kib();
            let kept = fe.session.kept();
            println!("== {label}");
            println!(
                "parse + open: {opened:?}; RSS +{} KiB; kept {kept:?} (total {} KiB)",
                rss_open.saturating_sub(rss_start),
                kept.total() / 1024
            );
            measure_runs(&mut fe.session);
        });
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// WP-2074: reflection queries whose operands a run built (a vector, a
/// `define_static_string`, a filtered `members_of`). `Old<int>::get` is
/// deprecated: instantiating `v<int>` (or `w<s>` of a string starting
/// with 'i') warns, once per translation unit, so a later run reports
/// that warning only by replaying it (spec/eval-session.md §7). Every
/// entry allocates its operands first, at the same heap address; the
/// `later_*` entries allocate before them, so theirs land elsewhere.
const RUN_BUILT: &str = "#include <string_view>\n\
#include <vector>\n\
#include \"kiln_meta.h\"\n\
using namespace std::meta;\n\
template <class T> struct Old { static constexpr int get() { return 2; } };\n\
template <> struct Old<int> { [[deprecated(\"use new\")]] static constexpr int get() { return 1; } };\n\
template <class T> constexpr int v = Old<T>::get();\n\
template <bool> struct Pick { using type = long; };\n\
template <> struct Pick<true> { using type = int; };\n\
template <const char* P> constexpr int w = Old<typename Pick<P[0] == 'i'>::type>::get();\n\
struct S { long a; int b; char c; };\n\
consteval int sub(info t) {\n\
  std::vector<info> a;\n\
  a.push_back(t);\n\
  return extract<int>(substitute(^^v, a));\n\
}\n\
consteval int a_int() { return sub(^^int); }\n\
consteval int a_long() { return sub(^^long); }\n\
consteval int later_int() {\n\
  std::vector<int> pad(100, 1);\n\
  return sub(^^int) + pad[0] - 1;\n\
}\n\
consteval int str(char c) {\n\
  char* p = new char[3]{c, 'n', 't'};\n\
  const char* s = std::define_static_string(std::string_view(p, 3));\n\
  delete[] p;\n\
  std::vector<info> a;\n\
  a.push_back(reflect_constant(s));\n\
  return extract<int>(substitute(^^w, a));\n\
}\n\
consteval int str_i() { return str('i'); }\n\
consteval int str_x() { return str('x'); }\n\
consteval int later_str_i() {\n\
  std::vector<int> pad(7, 1);\n\
  return str('i') + pad[0] - 1;\n\
}\n\
consteval int member_typed(int size) {\n\
  std::vector<info> a;\n\
  for (info m : nonstatic_data_members_of(^^S, access_context::unchecked()))\n\
    if (size_of(m) == sizeof(int) * size) a.push_back(type_of(m));\n\
  return extract<int>(substitute(^^v, a));\n\
}\n\
consteval int filtered_int() { return member_typed(1); }\n\
consteval int later_filtered_int() {\n\
  std::vector<int> pad(3, 1);\n\
  return member_typed(1) + pad[0] - 1;\n\
}\n\
consteval int rd(info r) { return extract<int>(r); }\n\
consteval int local_k(int k) { int x = k; return rd(^^x); }\n\
consteval int local_one() { return local_k(1); }\n\
consteval int local_two() { return local_k(2); }\n\
consteval int many() {\n\
  int s = 0;\n\
  for (int i = 0; i < 2000; ++i) s += sub(i % 2 ? ^^int : ^^long);\n\
  return s;\n\
}\n\
consteval int many_long() {\n\
  int s = 0;\n\
  for (int i = 0; i < 2000; ++i) s += sub(^^long);\n\
  return s;\n\
}\n";

/// A run's result, steps and diagnostics (level, message, span).
type Seen = (Result<RunValue, kiln_parse::RunError>, u64, Vec<String>);

fn seen(session: &mut EvalSession<'_, '_>, name: &str) -> Seen {
    let e = session.entry_named(name).unwrap();
    let r = session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
    let diags = r.diagnostics.iter().map(|d| format!("{:?} {} {:?}", d.level, d.message, d.span));
    (r.result, r.steps, diags.collect())
}

/// Opens a session on [`RUN_BUILT`] and runs `f` on it.
fn run_built<R>(test: &str, f: impl FnOnce(&mut EvalSession<'_, '_>) -> R) -> R {
    let d = scratch(test, &[("t.cpp", RUN_BUILT)]);
    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support");
    let err = Captured::default();
    let cl = [
        "-std=c++26".to_owned(),
        "-freflection".to_owned(),
        format!("-I{}", support.display()),
        d.join("t.cpp").to_string_lossy().into_owned(),
    ];
    let r = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.unwrap_or_else(|e| panic!("{e:?}: {}", err.text()));
        f(&mut fe.session)
    });
    assert_eq!(err.text(), "");
    std::fs::remove_dir_all(&d).unwrap();
    r
}

fn deprecated(s: &Seen) -> bool {
    s.2.iter().any(|d| d.contains("'get' is deprecated"))
}

#[test]
fn run_built_operands_replay_by_content() {
    run_built("content", |s| {
        for (first, later, other) in [
            ("a_int", "later_int", "a_long"),
            ("str_i", "later_str_i", "str_x"),
            // After `a_int`: the same query from another function (and
            // another loop) replays `a_int`'s record.
            ("filtered_int", "later_filtered_int", "a_long"),
        ] {
            let a = seen(s, first);
            assert_eq!(a.0, Ok(RunValue::Signed(1)), "{first}: {:?}", a.2);
            assert!(deprecated(&a), "{first}: {:?}", a.2);
            // The same content at another address: the warning replays.
            let l = seen(s, later);
            assert_eq!((&l.0, &l.2), (&a.0, &a.2), "{later}");
            // Other content at the same address: no warning.
            let o = seen(s, other);
            assert_eq!(o.0, Ok(RunValue::Signed(2)), "{other}: {:?}", o.2);
            assert!(o.2.is_empty(), "{other}: {:?}", o.2);
            assert_eq!(seen(s, first), a, "{first} again");
        }
    });
}

#[test]
fn a_run_does_not_depend_on_the_runs_before_it() {
    for (a, b) in [("a_int", "a_long"), ("str_i", "str_x"), ("filtered_int", "a_long")] {
        let (aba, fresh) = (
            run_built("aba", |s| {
                let first = seen(s, a);
                assert!(deprecated(&first), "{a}: {:?}", first.2);
                // At the address `a`'s operands had: none of its warnings.
                let other = seen(s, b);
                assert_eq!((other.0, other.2), (Ok(RunValue::Signed(2)), vec![]), "{b}");
                seen(s, a)
            }),
            run_built("fresh", |s| seen(s, a)),
        );
        assert_eq!(aba, fresh, "{a} after {a} {b}");
        let (ba, fresh) = (
            run_built("ba", |s| {
                assert!(seen(s, b).0.is_ok());
                seen(s, a)
            }),
            run_built("fresh", |s| seen(s, b)),
        );
        assert!(deprecated(&ba), "{a} after {b}: {:?}", ba.2);
        assert!(fresh.2.is_empty(), "{b}: {:?}", fresh.2);
    }
}

/// The memoization cache keys calls by scalar arguments only: a function
/// given a pointer or a reference (an address of the run) is not
/// memoized, so the same address with other content is computed again.
#[test]
fn memoized_results_are_not_keyed_by_addresses() {
    let d = scratch(
        "memo",
        &[(
            "t.cpp",
            "constexpr int sum(const int* p, int n) { int s = 0; for (int i = 0; i < n; ++i) s += p[i]; return s; }\n\
             constexpr int first(const int& r) { return r * 3; }\n\
             constexpr int sq(int x) { int s = 0; for (int i = 0; i < x; ++i) s += x; return s; }\n\
             consteval int with(int k) {\n\
               int* p = new int[3]{k, k, k};\n\
               int r = sum(p, 3) * 100 + first(p[0]) * 10 + sq(k);\n\
               delete[] p;\n\
               return r;\n\
             }\n\
             consteval int ones() { return with(1); }\n\
             consteval int twos() { return with(2); }\n",
        )],
    );
    let err = Captured::default();
    let cl = ["-std=c++26".to_owned(), d.join("t.cpp").to_string_lossy().into_owned()];
    with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let s = &mut fe.expect("session opens").session;
        let (one, two) = (seen(s, "ones"), seen(s, "twos"));
        assert_eq!(one.0, Ok(RunValue::Signed(300 + 30 + 1)));
        assert_eq!(two.0, Ok(RunValue::Signed(600 + 60 + 4)));
        // `with` and `sq` are memoized (scalar arguments), `sum` and
        // `first` are not.
        let entries = s.kept().memo_entries;
        assert!(entries > 0);
        for _ in 0..3 {
            assert_eq!(seen(s, "ones"), one);
            assert_eq!(seen(s, "twos"), two);
        }
        assert_eq!(s.kept().memo_entries, entries);
    });
    assert_eq!(err.text(), "");
    std::fs::remove_dir_all(&d).unwrap();
}

/// A reflection of an automatic variable is the same `info` in every run
/// (and every call), but `extract` of it reads the variable of the call
/// that runs: a function given it is not memoized by its bits.
#[test]
fn reflections_of_locals_are_not_memoized() {
    run_built("locals", |s| {
        for _ in 0..3 {
            assert_eq!(seen(s, "local_one").0, Ok(RunValue::Signed(1)));
            assert_eq!(seen(s, "local_two").0, Ok(RunValue::Signed(2)));
        }
    });
}

/// What content keying costs a run that makes 2000 `substitute` calls,
/// each over a vector it built (spec/eval-session.md §10): `many_long`
/// computes no key while no `substitute` call has a record, and one per
/// call once one has (`a_int`'s).
#[test]
#[ignore = "measurement: run in a release build with --ignored --nocapture"]
fn measure_content_keys() {
    run_built("measure-keys", |s| {
        let time = |s: &mut EvalSession<'_, '_>, name: &str| {
            let start = Instant::now();
            let r = seen(s, name);
            assert!(r.0.is_ok(), "{name}: {:?}", r.2);
            start.elapsed()
        };
        time(s, "many_long");
        let unkeyed = (0..20).map(|_| time(s, "many_long")).min().unwrap();
        time(s, "a_int");
        let keyed = (0..20).map(|_| time(s, "many_long")).min().unwrap();
        let mixed = (0..20).map(|_| time(s, "many")).min().unwrap();
        println!(
            "2000 substitute calls per run: no key {unkeyed:?}, content keys {keyed:?}, \
             half replaying {mixed:?}; kept {:?}",
            s.kept()
        );
    });
}
