//! The front-end library entry (`kiln_driver::front_end`, spec/driver.md
//! "Front-end library entry", WP-2120 for tooling clients): one `-fsyntax-only` parse
//! as a call, the closure seeing the parsed unit. Every test writes its
//! files to its own temporary directory and names them by absolute path.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use kiln_ast::Ast;
use kiln_ast::ids::DeclId;
use kiln_diag::{DiagId, SharedBuffer};
use kiln_driver::SharedWriter;
use kiln_driver::env::Environment;
use kiln_driver::front_end::{
    Consulted, ConsultedKind, Content, FrontEnd, FrontEndError, FrontEndOptions, FrontEndReport,
    content_digest, run_front_end, std_config,
};
use kiln_pp::{Found, LookupForm};
use kiln_sema::Oven;
use kiln_source::FileOrigin;

// What a caller may move to another thread or share: the AST is both;
// the constant evaluator's state is `Send` but not `Sync` (its caches
// are `RefCell`s), so a caller shares it only by moving it.
const _: () = {
    const fn send<T: Send>() {}
    const fn sync<T: Sync>() {}
    send::<Ast>();
    sync::<Ast>();
    send::<Oven>();
    send::<FrontEndOptions>();
};

/// A fresh temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("kiln-front-end-{name}-{}-{n}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_str().unwrap().to_owned()
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|&s| s.to_owned()).collect()
}

fn ident(ast: &Ast, d: DeclId) -> &str {
    ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s))
}

fn has_decl(ast: &Ast, name: &str) -> bool {
    ast.decls.ids().any(|d| ident(ast, d) == name)
}

fn quiet() -> FrontEndOptions {
    FrontEndOptions::default()
}

fn captured() -> (FrontEndOptions, SharedBuffer) {
    let buf = SharedBuffer::new();
    let mut o = FrontEndOptions::default();
    o.stderr = Some(SharedWriter::new(buf.clone()));
    (o, buf)
}

/// What a test compares across runs: the declarations' names, the
/// files and their origins, the tokens and the consulted paths.
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    decls: Vec<String>,
    files: Vec<(String, FileOrigin)>,
    tokens: u64,
    consulted: Vec<Consulted>,
}

fn seen(fe: &FrontEnd<'_, '_>) -> Seen {
    Seen {
        decls: fe.ast.decls.ids().map(|d| ident(fe.ast, d).to_owned()).collect(),
        files: fe
            .sm
            .file_ids()
            .map(|f| (fe.sm.file_name(f).to_owned(), fe.sm.file_origin(f)))
            .collect(),
        tokens: fe.tokens,
        consulted: fe.consulted.to_vec(),
    }
}

/// Test 1: two headers, one starting with a byte-order mark, one holding
/// invalid UTF-8; the declarations, the files, their bytes, the tokens.
#[test]
fn two_headers_files_bytes_and_tokens() {
    let s = Scratch::new("headers");
    let a_bytes: &[u8] = b"\xEF\xBB\xBF#define A 1\nint a_decl;\n";
    let b_bytes: &[u8] = b"// bad \xFF\xFE bytes\nint b_decl;\n#define B 2\n";
    let a = s.write("a.h", a_bytes);
    let b = s.write("b.h", b_bytes);
    let main_bytes = format!("#include \"{a}\"\n#include \"{b}\"\nint main_v = A + B;\n");
    let main = s.write("main.cpp", main_bytes.as_bytes());
    let cl = argv(&["-fsyntax-only", &main]);
    let run = || {
        run_front_end("kiln++", &cl, quiet(), |fe| {
            assert!(has_decl(fe.ast, "a_decl"));
            assert!(has_decl(fe.ast, "b_decl"));
            assert!(has_decl(fe.ast, "main_v"));
            assert!(fe.main_file.is_some());
            for (path, bytes) in [(&main, main_bytes.as_bytes()), (&a, a_bytes), (&b, b_bytes)] {
                let f = fe
                    .sm
                    .file_ids()
                    .find(|&f| fe.sm.file_name(f) == path.as_str())
                    .unwrap_or_else(|| panic!("{path} is a buffer"));
                assert_eq!(fe.sm.file_origin(f), FileOrigin::Disk);
                assert_eq!(fe.sm.original_bytes(f).as_deref(), Some(bytes), "{path}");
            }
            seen(fe)
        })
        .unwrap()
    };
    let r = run();
    assert_eq!((r.errors, r.fatal, r.driver_errors), (0, false, 0), "{:?}", r.first_error);
    let first = r.value.unwrap();
    // By hand: `int a_decl ;` (3), `int b_decl ;` (3),
    // `int main_v = 1 + 2 ;` (7).
    assert_eq!(first.tokens, 13);
    assert_eq!(run().value.unwrap(), first, "the same on a second run");
}

/// Test 2: `__has_include` of a file never included and an `#embed`
/// resource are consulted, once each; an included file is not, even
/// when `__has_include` found it first.
#[test]
fn has_include_and_embed_are_consulted() {
    let s = Scratch::new("consulted");
    let probe = s.write("probe.h", b"int never;\n");
    let used = s.write("used.h", b"int used;\n");
    let data = s.write("data.bin", b"\x01\x02\x03");
    let main = s.write(
        "main.cpp",
        format!(
            "#if __has_include(\"{probe}\")\n#endif\n#if __has_include(\"{probe}\")\n#endif\n\
             #if __has_include(\"{used}\")\n#include \"{used}\"\n#endif\n\
             const unsigned char d[] = {{\n#embed \"{data}\"\n}};\n\
             static_assert(sizeof d == 3);\n"
        )
        .as_bytes(),
    );
    let r =
        run_front_end("kiln++", &argv(&["-fsyntax-only", "-std=c++26", &main]), quiet(), |fe| {
            fe.consulted.to_vec()
        })
        .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    assert_eq!(
        r.value.unwrap(),
        vec![
            Consulted {
                kind: ConsultedKind::Probe,
                path: probe,
                content: Some(Content::of(b"int never;\n"))
            },
            Consulted {
                kind: ConsultedKind::Embed,
                path: data,
                content: Some(Content { size: 3, digest: content_digest(b"\x01\x02\x03") })
            },
        ]
    );
}

/// Test 3: one error. The closure runs, and the first error is what the
/// text sink and the `kiln` executable print.
#[test]
fn one_error_matches_the_text_sink() {
    let s = Scratch::new("error");
    let main = s.write("main.cpp", b"int ok;\n#line 40 \"named.cpp\"\nint bad = undeclared;\n");
    let (opts, buf) = captured();
    let r = run_front_end("kiln++", &argv(&["-fsyntax-only", &main]), opts, |fe| {
        has_decl(fe.ast, "ok")
    })
    .unwrap();
    assert_eq!(r.value, Some(true));
    assert_eq!((r.errors, r.driver_errors, r.fatal), (1, 0, false));
    let first = r.first_error.unwrap();
    assert_eq!(first.id, Some(DiagId::sema_undeclared_var_use));
    let at = first.at.unwrap();
    let header = format!("{}:{}:{}: error: {}", at.file, at.line, at.column, first.message);
    let text = buf.contents();
    assert_eq!(text.lines().next(), Some(header.as_str()), "{text}");
    assert_eq!((at.file.as_str(), at.line, at.column), ("named.cpp", 40, 11));
    assert!(text.ends_with("1 error generated.\n"), "{text}");
    let out = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-x", "c++", "-fsyntax-only", &main])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stderr), text, "as the executable prints it");
}

/// Test 4: no parse, and the command lines the entry refuses.
#[test]
fn no_parse_and_refusals() {
    let s = Scratch::new("refuse");
    let main = s.write("main.cpp", b"int x;\n");
    let never = |_: &FrontEnd<'_, '_>| -> u8 { panic!("not called") };

    let r =
        run_front_end("kiln++", &argv(&["-fsyntax-only", "-std=c++99x", &main]), quiet(), never)
            .unwrap();
    assert_eq!(r.value, None);
    assert!(r.driver_errors >= 1 && r.driver_errors == r.errors, "{r:?}");
    assert!(r.first_error.unwrap().message.contains("c++99x"));

    let missing = s.path("missing.cpp");
    let r = run_front_end("kiln++", &argv(&["-fsyntax-only", &missing]), quiet(), never).unwrap();
    assert_eq!(r.value, None);
    // As `kiln`: `no such file or directory`, then `no input files`.
    assert_eq!((r.driver_errors, r.errors), (2, 2));
    let first = r.first_error.unwrap();
    assert_eq!(first.id, Some(DiagId::drv_no_such_file));
    assert!(first.message.contains(&missing), "{first:?}");
    assert_eq!(first.at, None);

    let e = run_front_end("kiln++", &argv(&["-c", &main]), quiet(), never).unwrap_err();
    assert!(matches!(e, FrontEndError::NotSyntaxOnly), "{e:?}");
    let e = run_front_end("kiln++", &argv(&["-E", &main]), quiet(), never).unwrap_err();
    assert!(matches!(e, FrontEndError::NotSyntaxOnly), "{e:?}");
    let e = run_front_end("kiln++", &argv(&["-fsyntax-only", &main, &main]), quiet(), never)
        .unwrap_err();
    assert!(matches!(e, FrontEndError::NotOneInput), "{e:?}");
    let e = run_front_end("kiln++", &argv(&["-fsyntax-only"]), quiet(), never).unwrap_err();
    assert!(matches!(e, FrontEndError::NotOneInput), "{e:?}");
    let e = run_front_end("kiln++", &argv(&["-fsyntax-only", "-"]), quiet(), never).unwrap_err();
    assert!(matches!(e, FrontEndError::NotOneInput), "{e:?}");

    for cl in [
        argv(&["-fsyntax-only", "-verify", &main]),
        argv(&["-fsyntax-only", "-include-pch", &s.path("none.pch"), &main]),
    ] {
        let r = run_front_end("kiln++", &cl, quiet(), never).unwrap();
        assert_eq!(r.value, None, "{cl:?}");
        assert!(r.driver_errors >= 1, "{cl:?}");
        let refusals =
            r.first_error.iter().filter(|e| e.id == Some(DiagId::drv_front_end_unsupported));
        assert_eq!(refusals.count(), 1, "{cl:?}: {r:?}");
    }
}

/// Test 5: a panicking closure, on both thread settings; a later call
/// works.
#[test]
fn panics_come_back_and_a_later_call_works() {
    let s = Scratch::new("panic");
    let main = s.write("main.cpp", b"int x;\n");
    let cl = argv(&["-fsyntax-only", &main]);
    for own_thread in [true, false] {
        let mut o = quiet();
        o.own_thread = own_thread;
        let e = run_front_end("kiln++", &cl, o.clone(), |_| -> u8 { panic!("boom {}", 7) })
            .unwrap_err();
        assert!(matches!(&e, FrontEndError::Panicked(m) if m == "boom 7"), "{e:?}");
        let e =
            run_front_end("kiln++", &cl, o.clone(), |_| -> u8 { panic!("literal") }).unwrap_err();
        assert!(matches!(&e, FrontEndError::Panicked(m) if m == "literal"), "{e:?}");
        let r = run_front_end("kiln++", &cl, o, |fe| has_decl(fe.ast, "x")).unwrap();
        assert_eq!((r.value, r.errors), (Some(true), 0));
    }
}

/// Test 6: a module interface unit with every side output asked for:
/// nothing is written.
#[test]
fn no_side_outputs() {
    let s = Scratch::new("outputs");
    let main = s.write("m.cppm", b"export module m;\nexport constexpr int f() { return 1; }\n");
    let report = s.path("report.jsonl");
    let dep = s.path("m.d");
    let out = s.path("m.o");
    let pcm = s.path("m.pcm");
    let cl = argv(&[
        "-fsyntax-only",
        "-std=c++20",
        &format!("-fconstexpr-report={report}"),
        "-MD",
        "-MF",
        &dep,
        "-o",
        &out,
        &format!("-fmodule-output={pcm}"),
        &main,
    ]);
    let r = run_front_end("kiln++", &cl, quiet(), |fe| has_decl(fe.ast, "f")).unwrap();
    assert_eq!(r.value, Some(true));
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    let left: Vec<_> = std::fs::read_dir(&s.0).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from("m.cppm")], "nothing but the input");
}

/// Test 7: eight threads, twice; each report equals the one run alone.
#[test]
fn concurrent_calls_do_not_interfere() {
    let s = Scratch::new("threads");
    let mains: Vec<String> = (0..8)
        .map(|i| {
            let h = s.write(&format!("h{i}.h"), format!("int h{i}_decl;\n").as_bytes());
            let err = if i % 2 == 1 { "int bad = nope;\n" } else { "" };
            let text = format!(
                "#include \"{h}\"\n#if __has_include(\"{}\")\n#endif\n{err}int v{i} = {i};\n",
                s.path(&format!("absent{i}.h"))
            );
            s.write(&format!("m{i}.cpp"), text.as_bytes())
        })
        .collect();
    let run = |main: &str| -> FrontEndReport<Seen> {
        let cl = argv(&["-fsyntax-only", "-fkiln-stack-size=64M", main]);
        run_front_end("kiln++", &cl, quiet(), seen).unwrap()
    };
    let alone: Vec<_> = mains.iter().map(|m| run(m)).collect();
    for _ in 0..2 {
        let together: Vec<_> = std::thread::scope(|sc| {
            let hs: Vec<_> = mains.iter().map(|m| sc.spawn(|| run(m))).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(together, alone);
    }
    assert_eq!(alone[1].errors, 1);
    assert_eq!(alone[0].errors, 0);
}

/// Test 8: nesting deeper than the stack is a fatal diagnostic, not a
/// crash, on a thread of the entry's own.
#[test]
fn deep_nesting_is_fatal_on_own_thread() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/stress/stack-guard-parens.cpp");
    let file = std::fs::canonicalize(file).unwrap();
    let cl = argv(&["-fsyntax-only", "-fkiln-stack-size=8M", file.to_str().unwrap()]);
    let r = run_front_end("kiln++", &cl, quiet(), |fe| fe.tokens).unwrap();
    assert!(r.fatal, "{r:?}");
    assert!(r.value.is_some(), "the closure runs after a fatal error");
    let first = r.first_error.unwrap();
    assert!(first.message.contains("stack exhausted during parsing"), "{first:?}");
    assert_eq!(first.at.map(|p| p.line), Some(22));
}

/// Test 9: `record_lookups` logs a `__has_include` key that was not
/// found.
#[test]
fn record_lookups_logs_a_missing_key() {
    let s = Scratch::new("lookups");
    let main = s.write("main.cpp", b"#if __has_include(\"no_such_header.h\")\n#endif\nint x;\n");
    let cl = argv(&["-fsyntax-only", &main]);
    let mut o = quiet();
    o.record_lookups = true;
    let r = run_front_end("kiln++", &cl, o, |fe| {
        let log = fe.lookups.expect("a lookup log");
        log.lookups.iter().any(|l| {
            l.name == "no_such_header.h"
                && l.form == LookupForm::Include
                && l.found == Found::Missing
        })
    })
    .unwrap();
    assert_eq!(r.value, Some(true));
    let r = run_front_end("kiln++", &cl, quiet(), |fe| fe.lookups.is_none()).unwrap();
    assert_eq!(r.value, Some(true), "no log unless asked");
}

/// WP-2620 (spec/pp.md "Lookup log", [cpp.include]/2): a lookup of an
/// absolute name is recorded, found or not, with `-include` among the
/// forms; `check_absolute` repeats them over the disk and names a
/// header that appeared, or one that went.
#[test]
fn record_lookups_logs_absolute_names() {
    let s = Scratch::new("abs-lookups");
    let pre = s.write("pre.h", b"#define PRE 1\n");
    let h = s.write("h.h", b"int h;\n");
    let (absent, none) = (s.path("absent.h"), s.path("none.bin"));
    let text = format!(
        "#include \"{h}\"\n#if __has_include(\"{absent}\") || __has_embed(\"{none}\")\n#error\n#endif\nstatic_assert(PRE);\n"
    );
    let main = s.write("main.cpp", text.as_bytes());
    let cl = argv(&["-fsyntax-only", "-Wno-everything", "-include", &pre, &main]);
    let mut o = quiet();
    o.record_lookups = true;
    let r = run_front_end("kiln++", &cl, o, |fe| fe.lookups.cloned()).unwrap();
    assert_eq!(r.errors, 0, "{r:?}");
    let log = r.value.flatten().expect("a lookup log");
    let got: Vec<(&str, LookupForm, &Found)> = log
        .lookups
        .iter()
        .filter(|l| l.name.starts_with('/'))
        .map(|l| (l.name.as_str(), l.form, &l.found))
        .collect();
    assert_eq!(
        got,
        [
            (pre.as_str(), LookupForm::Include, &Found::Absolute),
            (h.as_str(), LookupForm::Include, &Found::Absolute),
            (absent.as_str(), LookupForm::Include, &Found::Missing),
            (none.as_str(), LookupForm::Embed, &Found::Missing),
        ]
    );
    let check = || log.check_absolute(&mut kiln_pp::RealFs::new(), &|_| false);
    assert_eq!(check(), Ok(()));
    s.write("absent.h", b"\n");
    assert_eq!(
        check(),
        Err(format!("lookup of \"{absent}\" now finds '{absent}' (was not found)"))
    );
    std::fs::remove_file(&absent).unwrap();
    std::fs::remove_file(&pre).unwrap();
    assert_eq!(check(), Err(format!("lookup of \"{pre}\" now finds nothing (was '{pre}')")));
}

#[test]
fn options_default() {
    let o = FrontEndOptions::default();
    assert!(o.stderr.is_none() && !o.record_lookups && o.own_thread && o.pch_probe);
    assert!(o.response_files);
    assert!(o.environment.is_process());
}

/// Test 10 (WP-2120, the environment): the driver's variables come from
/// `options.environment`, never from the process: `SOURCE_DATE_EPOCH`
/// fixes `__DATE__`, `KILN_RESOURCE_DIR` moves the resource directory.
#[test]
fn environment_is_the_options() {
    let s = Scratch::new("env");
    let main = s.write(
        "main.cpp",
        b"constexpr char d[] = __DATE__;\n\
          static_assert(d[0] == 'J' && d[4] == ' ' && d[5] == '1' && d[10] == '0');\n",
    );
    let cl = argv(&["-fsyntax-only", &main]);
    let mut o = quiet();
    o.environment = Environment::fixed([("SOURCE_DATE_EPOCH", "0")]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 0, "Jan  1 1970: {:?}", r.first_error);
    let mut o = quiet();
    o.environment = Environment::fixed([("SOURCE_DATE_EPOCH", "86400000")]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 1, "Sep 27 1972: {:?}", r.first_error);
    let mut o = quiet();
    o.environment = Environment::fixed([("KILN_RESOURCE_DIR", s.path("no-resources"))]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.value, None);
    assert_eq!(r.first_error.unwrap().id, Some(DiagId::drv_cannot_read_predefines));
}

/// Test 11 (WP-2120): with `pch_probe` off, `-include h` is the header
/// even when `h.pch` is next to it; with it on, the probe makes it
/// `-include-pch`, which the entry refuses.
#[test]
fn pch_probe_can_be_turned_off() {
    let s = Scratch::new("probe");
    let h = s.write(
        "h.h",
        b"int from_header;
",
    );
    s.write("h.h.pch", b"not a precompiled header");
    let main = s.write(
        "main.cpp",
        b"int x = from_header;
",
    );
    let cl = argv(&["-fsyntax-only", "-include", &h, &main]);
    let r = run_front_end("kiln++", &cl, quiet(), |_| ()).unwrap();
    assert_eq!(r.value, None);
    assert_eq!(r.first_error.unwrap().id, Some(DiagId::drv_front_end_unsupported));
    let mut o = quiet();
    o.pch_probe = false;
    let r = run_front_end("kiln++", &cl, o, |fe| has_decl(fe.ast, "from_header")).unwrap();
    assert_eq!((r.value, r.errors), (Some(true), 0), "{:?}", r.first_error);
}

/// Test 12 (WP-2120): the time trace and the statistics log are not
/// written either (spec/driver.md "Front-end library entry": suppressed,
/// not refused).
#[test]
fn no_time_trace_or_statistics_log() {
    let s = Scratch::new("trace");
    let main = s.write("main.cpp", b"constexpr int f() { return 1; }\nstatic_assert(f());\n");
    let cl = argv(&[
        "-fsyntax-only",
        "-ftime-trace",
        &format!("-ftime-trace={}", s.path("trace.json")),
        "-fconstexpr-profile",
        "-o",
        &s.path("main.o"),
        &main,
    ]);
    let mut o = quiet();
    o.environment = Environment::fixed([("KILN_STATS_LOG", s.path("stats.log"))]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!((r.value, r.errors), (Some(()), 0), "{:?}", r.first_error);
    let left: Vec<_> = std::fs::read_dir(&s.0).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from("main.cpp")], "nothing but the input");
}

/// Test 13 (WP-2120): the job configuration of `-std=c++26` with no
/// other flag is the one a parse with that command line uses.
#[test]
fn std_config_is_the_jobs() {
    let r = std_config("c++26", &quiet());
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    let c = r.value.unwrap();
    assert!(c.pp_options.predefines.contains("#define __cplusplus 202400L"));
    assert!(c.pp_options.parser_pragmas && !c.pp_options.preprocessed_output);
    assert!(c.pp_options.modules.is_none());
    assert!(kiln_pp::FeatureQueries::has_builtin(&c.features, "__builtin_expect") > 0);
    assert!(
        c.parse_options.exceptions && c.parse_options.rtti && !c.parse_options.complex_promoted
    );
    let s = Scratch::new("config");
    let main = s.write("main.cpp", b"int x;\n");
    let cl = argv(&["-fsyntax-only", "-std=c++26", &main]);
    let r =
        run_front_end("kiln++", &cl, quiet(), |fe| (format!("{:?}", fe.parse_options), *fe.lang))
            .unwrap();
    let (popts, lang) = r.value.unwrap();
    assert_eq!(popts, format!("{:?}", c.parse_options));
    assert_eq!(lang, c.lang);
    let r = std_config("c++99x", &quiet());
    assert_eq!(r.value.map(|_| ()), None);
    assert!(r.first_error.unwrap().message.contains("c++99x"));
    let promoted =
        kiln_driver::args::Args::parse(&argv(&["-fcomplex-arithmetic=promoted", "a.cpp"]));
    assert!(promoted.complex_promoted());
}

/// The file names under `dir` with their modification times, sorted
/// (empty when `dir` does not exist).
fn tree(dir: &std::path::Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return v };
    for e in rd {
        let p = e.unwrap().path();
        if p.is_dir() {
            v.extend(tree(&p));
        } else {
            v.push((p.clone(), std::fs::metadata(&p).unwrap().modified().unwrap()));
        }
    }
    v.sort();
    v
}

/// Test 14 (WP-2120): the module-file cache of `import std;` is read
/// only under the entry (spec/modules.md §11.12): a miss builds the
/// module in process and stores nothing, a hit uses the entry `kiln++`
/// stored and leaves the cache as it was (no use stamp, no pruning).
/// The libc++ is a miniature in a sysroot, as in `tests/module_cache.rs`.
#[test]
fn module_cache_is_read_only() {
    let s = Scratch::new("cache");
    let sys = s.0.join("sys/usr");
    for d in ["include/c++/v1", "share/libc++/v1", "lib"] {
        std::fs::create_dir_all(sys.join(d)).unwrap();
    }
    s.write("sys/usr/include/c++/v1/answer.h", b"inline int answer() { return 42; }\n");
    s.write(
        "sys/usr/share/libc++/v1/std.cppm",
        b"module;\n#include <answer.h>\nexport module std;\n\
          export namespace std { using ::answer; }\n",
    );
    s.write(
        "sys/usr/share/libc++/v1/std.compat.cppm",
        b"module;\n#include <answer.h>\nexport module std.compat;\n\
          export import std;\nexport using ::answer;\n",
    );
    s.write("sys/usr/lib/libc++.modules.json", b"{}\n");
    let main = s.write("u.cpp", b"import std;\nstatic_assert(sizeof(std::answer()) > 0);\n");
    let sysroot = format!("--sysroot={}", s.path("sys"));
    let cache = s.0.join("cache");
    let cache_flag = format!("-fmodules-cache-path={}", cache.display());
    let kilnpp = std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let o = Command::new(&kilnpp)
        .args([&sysroot, "-stdlib=libc++", "-print-file-name=libc++.modules.json"])
        .output()
        .unwrap();
    if String::from_utf8_lossy(&o.stdout).trim() != s.path("sys/usr/lib/libc++.modules.json") {
        eprintln!("libc++ does not come from the sysroot here; skipping");
        return;
    }
    let flags = ["-v", "-fsyntax-only", "-std=c++20", &sysroot, "-stdlib=libc++", &cache_flag];
    let cl = argv(&[&flags[..], &[main.as_str()]].concat());
    let remarks = |buf: &SharedBuffer| -> Vec<String> {
        buf.contents()
            .lines()
            .filter_map(|l| l.strip_prefix("kiln++: module cache: "))
            .map(str::to_owned)
            .collect()
    };
    let (o, buf) = captured();
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    let r = remarks(&buf);
    let dir = r
        .iter()
        .find_map(|l| {
            l.strip_prefix("no entry in '")?
                .strip_suffix("' for 'std'; building it from its source")
        })
        .unwrap_or_else(|| panic!("{r:?}"))
        .to_owned();
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(!cache.exists(), "a miss stores nothing");
    // The key names the executable, so `kiln++`'s entry is copied to the
    // directory this program's key names.
    let o = Command::new(&kilnpp).args(&cl).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let theirs = tree(&cache);
    let pcm = &theirs.iter().find(|(p, _)| p.ends_with("std.pcm")).expect("kiln++ stored std").0;
    let ours = std::path::Path::new(&dir);
    std::fs::create_dir_all(ours).unwrap();
    std::fs::copy(pcm, ours.join("std.pcm")).unwrap();
    let stamp = std::fs::read_to_string(pcm.with_file_name("std.stamp")).unwrap();
    let stamp = stamp.replace(pcm.to_str().unwrap(), ours.join("std.pcm").to_str().unwrap());
    std::fs::write(ours.join("std.stamp"), stamp).unwrap();
    let stored = tree(&cache);
    let (o, buf) = captured();
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    assert_eq!(remarks(&buf), [format!("using '{}' for 'std'", ours.join("std.pcm").display())]);
    assert_eq!(tree(&cache), stored, "a hit changes nothing");
}

/// `FrontEndOptions::pp_record` (WP-2110, spec/pp.md "Preprocessing
/// record"): off, the preprocessor keeps no record; on, the closure reads
/// it, with one expansion per macro use, each naming its definition.
#[test]
fn preprocessing_record() {
    let s = Scratch::new("pp-record");
    let main = s.write(
        "main.cpp",
        b"#define SQ(x) ((x) * (x))\n#define TWO 2\nint a = SQ(TWO);\nint b = SQ(3) + TWO;\n",
    );
    let cl = argv(&["-fsyntax-only", &main]);
    let off = run_front_end("kiln++", &cl, quiet(), |fe| fe.pp.record().is_none()).unwrap();
    assert_eq!(off.errors, 0);
    assert_eq!(off.value, Some(true));
    let mut on = quiet();
    on.pp_record = true;
    let r = run_front_end("kiln++", &cl, on, |fe| {
        let rec = fe.pp.record().expect("a record with pp_record on");
        assert!(rec.is_complete());
        let interner = fe.pp.interner();
        let mut names: Vec<String> = rec
            .exps()
            .iter()
            .map(|e| interner.as_str(rec.def(e.def).def.name()).to_owned())
            .collect();
        names.sort();
        names
    })
    .unwrap();
    assert_eq!(r.errors, 0);
    assert_eq!(r.value.unwrap(), ["SQ", "SQ", "TWO", "TWO"]);
}

/// WP-2320: `FrontEndOptions::written_refs` records the written-reference
/// table (spec/ast.md §18) and prints nothing; the table the closure reads
/// on the finished tree renders exactly as the executable's
/// `-fkiln-dump-written-refs` output for the same input (spec/driver.md
/// "Front-end library entry"). Off, the table is empty.
#[test]
fn written_refs_equal_the_dump() {
    let s = Scratch::new("written-refs");
    s.write("h.h", b"namespace n { template <class T> T twice(T x) { return x + x; } }\n");
    let main = s.write(
        "main.cpp",
        b"#include \"h.h\"\n#define CALL(f) n::f(2)\nstruct S { int m; };\n\
          int g(S s) { return n::twice(s.m) + CALL(twice<long>); }\n",
    );
    let cl = argv(&["-std=c++26", "-fsyntax-only", &main]);
    let off = run_front_end("kiln++", &cl, quiet(), |fe| fe.ast.written_refs.len()).unwrap();
    assert_eq!((off.errors, off.value), (0, Some(0)));
    let (mut on, buf) = captured();
    on.written_refs = true;
    let r = run_front_end("kiln++", &cl, on, |fe| {
        assert!(fe.parse_options.written_refs);
        fe.ast.written_refs.dump(fe.ast, fe.sm, fe.main_file)
    })
    .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    assert_eq!(buf.contents(), "", "the switch prints nothing");
    let table = r.value.unwrap();
    let kilnpp = std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let mut dump_cl = cl.clone();
    dump_cl.insert(2, "-fkiln-dump-written-refs".to_owned());
    let out = Command::new(&kilnpp).args(&dump_cl).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let dump = String::from_utf8(out.stdout).unwrap();
    assert!(dump.contains("'twice'") && dump.contains("h.h:"), "{dump}");
    assert_eq!(table, dump);
}

/// WP-2122: a developer switch that changes a run's result is read from
/// `FrontEndOptions::environment` (spec/driver.md "Front-end library
/// entry", the environment table). `KILN_WREF_RAW` prefixes each line of
/// the written-reference dump with the name's raw location: two runs in
/// one process with different environments dump differently, and the
/// process's own `KILN_WREF_RAW` reaches only a run under
/// `Environment::process()`. The process variable is set by running this
/// test again in a child (no `set_var`: it is `unsafe`).
#[test]
fn developer_switches_follow_the_environment() {
    if std::env::var_os("KILN_WREF_RAW").is_none() {
        let out = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "developer_switches_follow_the_environment", "--nocapture"])
            .args(["--test-threads=1"])
            .env("KILN_WREF_RAW", "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{text}{}", String::from_utf8_lossy(&out.stderr));
        assert!(text.contains("1 passed"), "the child ran the test: {text}");
        return;
    }
    let s = Scratch::new("dev-switches");
    let main = s.write("main.cpp", b"int v;\nint w = v;\n");
    let cl = argv(&["-std=c++26", "-fsyntax-only", &main]);
    let dump = |environment: Environment| {
        let mut o = quiet();
        o.written_refs = true;
        o.environment = environment;
        let r = run_front_end("kiln++", &cl, o, |fe| {
            fe.ast.written_refs.dump(fe.ast, fe.sm, fe.main_file)
        })
        .unwrap();
        assert_eq!(r.errors, 0, "{:?}", r.first_error);
        r.value.unwrap()
    };
    let raw = dump(Environment::fixed([("KILN_WREF_RAW", "1")]));
    let plain = dump(Environment::fixed::<&str, &str>([]));
    assert!(raw.starts_with('#') && raw.contains(" 2:9-2:10  expr  'v' -> v@1:5"), "{raw}");
    assert_eq!(plain, "2:9-2:10  expr  'v' -> v@1:5 in=w@2:5\n", "the process's variable");
    assert!(dump(Environment::process()).starts_with('#'), "the process environment");
}

/// The process's current directory, which no test of this file changes.
fn process_cwd() -> PathBuf {
    std::env::current_dir().unwrap()
}

/// `FrontEndOptions::working_directory` (WP-2121, spec/driver.md "Working
/// directory"): the options of a run in `dir`.
fn in_dir(dir: &std::path::Path) -> FrontEndOptions {
    let mut o = quiet();
    o.working_directory = Some(dir.to_owned());
    o
}

/// WP-2121: two runs of the same relative command line (`-I inc a.cpp`)
/// in two working directories, inside the front end at the same time,
/// each see their own files; the process's current directory is neither
/// used nor changed ([lex.header]: a quoted name searches the includer's
/// directory, then `-I`).
#[test]
fn working_directories_run_at_the_same_time() {
    let before = process_cwd();
    let dirs: Vec<Scratch> = ["one", "two"]
        .iter()
        .map(|n| {
            let s = Scratch::new(&format!("cwd-{n}"));
            std::fs::create_dir_all(s.0.join("inc")).unwrap();
            s.write("inc/h.h", format!("int from_{n};\n").as_bytes());
            s.write("a.cpp", format!("#include \"h.h\"\nint main_{n} = 1;\n").as_bytes());
            s
        })
        .collect();
    assert!(!std::path::Path::new("a.cpp").exists(), "no a.cpp in the process's directory");
    let cl = argv(&["-fsyntax-only", "-I", "inc", "a.cpp"]);
    let inside = AtomicU32::new(0);
    let run = |s: &Scratch| {
        run_front_end("kiln++", &cl, in_dir(&s.0), |fe| {
            // Wait (bounded) until the other run is inside its closure
            // too: both parses have happened, neither has returned.
            inside.fetch_add(1, Ordering::SeqCst);
            let start = std::time::Instant::now();
            while inside.load(Ordering::SeqCst) < 2 && start.elapsed().as_secs() < 60 {
                std::thread::yield_now();
            }
            let files: Vec<String> =
                fe.sm.file_ids().map(|f| fe.sm.file_name(f).to_owned()).collect();
            (inside.load(Ordering::SeqCst), seen(fe).decls, files)
        })
        .unwrap()
    };
    let reports: Vec<_> = std::thread::scope(|sc| {
        let hs: Vec<_> = dirs.iter().map(|s| sc.spawn(|| run(s))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (r, n) in reports.into_iter().zip(["one", "two"]) {
        assert_eq!((r.errors, r.driver_errors), (0, 0), "{n}: {:?}", r.first_error);
        let (together, decls, files) = r.value.unwrap();
        assert_eq!(together, 2, "{n}: both runs were inside the front end at once");
        assert!(decls.contains(&format!("from_{n}")) && decls.contains(&format!("main_{n}")));
        let other = if n == "one" { "from_two" } else { "from_one" };
        assert!(!decls.iter().any(|d| d == other), "{n}: {decls:?}");
        // The buffers keep the names as written.
        assert!(files.iter().any(|f| f == "a.cpp"), "{files:?}");
        assert!(files.iter().any(|f| f == "inc/h.h"), "{files:?}");
    }
    assert_eq!(process_cwd(), before, "the process's directory is unchanged");
}

/// WP-2121: `__FILE__` and a diagnostic's path stay as written, relative
/// ([cpp.predefined]: `__FILE__` is the presumed name of the source
/// file; [cpp.line] names keep their spelling), while the files open in
/// the working directory.
#[test]
fn file_macro_and_diagnostics_stay_relative() {
    let s = Scratch::new("cwd-names");
    std::fs::create_dir_all(s.0.join("inc")).unwrap();
    s.write("inc/h.h", b"#define HF __FILE__\nint bad = nope;\n");
    s.write(
        "a.cpp",
        b"#include \"h.h\"\n\
          static_assert(__builtin_strcmp(__FILE__, \"a.cpp\") == 0);\n\
          static_assert(__builtin_strcmp(HF, \"a.cpp\") == 0);\n\
          #line 7 \"rel/x.cpp\"\n\
          static_assert(__builtin_strcmp(__FILE__, \"rel/x.cpp\") == 0);\n\
          int also = bad2;\n",
    );
    let cl = argv(&["-fsyntax-only", "-I", "inc", "a.cpp"]);
    let (mut o, buf) = captured();
    o.working_directory = Some(s.0.clone());
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 2, "only the two undeclared names: {}", buf.contents());
    let at = r.first_error.unwrap().at.unwrap();
    assert_eq!((at.file.as_str(), at.line), ("inc/h.h", 2));
    let text = buf.contents();
    assert!(text.contains("inc/h.h:2:11: error: use of undeclared identifier 'nope'"), "{text}");
    assert!(text.contains("rel/x.cpp:8:12: error: use of undeclared identifier 'bad2'"), "{text}");
    assert!(!text.contains(s.0.to_str().unwrap()), "no absolute path: {text}");
}

/// WP-2121: a relative `-include`, `#embed` / `__has_embed` and
/// `--embed-dir=` ([cpp.embed]), and a module file named by a relative
/// `-fmodule-file=` ([module.import]) under the working directory.
#[test]
fn include_embed_and_module_file_in_the_working_directory() {
    let s = Scratch::new("cwd-more");
    std::fs::create_dir_all(s.0.join("res")).unwrap();
    s.write("pre.h", b"#define PRE 3\n");
    s.write("data.bin", b"AB");
    s.write("res/r.bin", b"xyz");
    s.write("m.cppm", b"export module m;\nexport constexpr int twice(int x) { return 2 * x; }\n");
    s.write(
        "a.cpp",
        b"import m;\n\
          constexpr unsigned char d[] = {\n#embed \"data.bin\"\n};\n\
          static_assert(sizeof d == 2 && d[0] == 'A');\n\
          #if __has_embed(<r.bin>) != __STDC_EMBED_FOUND__\n#error r.bin\n#endif\n\
          static_assert(twice(PRE) == 6);\n",
    );
    let kilnpp = std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let o = Command::new(&kilnpp)
        .args(["-std=c++26", "--precompile", "m.cppm", "-o", "m.pcm"])
        .current_dir(&s.0)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let cl = argv(&[
        "-fsyntax-only",
        "-std=c++26",
        "-Wno-c23-extensions",
        "-include",
        "pre.h",
        "--embed-dir=res",
        "-fmodule-file=m=m.pcm",
        "a.cpp",
    ]);
    let (mut o, buf) = captured();
    o.working_directory = Some(s.0.clone());
    let r = run_front_end("kiln++", &cl, o, |fe| fe.consulted.to_vec()).unwrap();
    assert_eq!(r.errors, 0, "{}", buf.contents());
    let consulted = r.value.unwrap();
    let kinds: Vec<(ConsultedKind, &str)> =
        consulted.iter().map(|c| (c.kind, c.path.as_str())).collect();
    // The names as found: the includer's directory (`.`) and `--embed-dir=`.
    assert_eq!(kinds, [(ConsultedKind::Embed, "./data.bin"), (ConsultedKind::Probe, "res/r.bin")]);
    assert_eq!(consulted[0].content, Some(Content::of(b"AB")));
}

/// WP-2121: the environment and the working directory are options of
/// the same run: `SOURCE_DATE_EPOCH` from the fixed set, the files from
/// the directory ([cpp.predefined] `__DATE__`).
#[test]
fn environment_and_working_directory_together() {
    let s = Scratch::new("cwd-env");
    std::fs::create_dir_all(s.0.join("inc")).unwrap();
    s.write("inc/d.h", b"constexpr char d[] = __DATE__;\n");
    s.write("a.cpp", b"#include <d.h>\nstatic_assert(d[0] == 'J' && d[10] == '0');\n");
    let cl = argv(&["-fsyntax-only", "-Iinc", "a.cpp"]);
    let mut o = in_dir(&s.0);
    o.environment = Environment::fixed([("SOURCE_DATE_EPOCH", "0")]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.errors, 0, "Jan  1 1970: {:?}", r.first_error);
    // Without the directory, `a.cpp` is the process's: not there.
    let mut o = quiet();
    o.environment = Environment::fixed([("SOURCE_DATE_EPOCH", "0")]);
    let r = run_front_end("kiln++", &cl, o, |_| ()).unwrap();
    assert_eq!(r.value, None);
    assert_eq!(r.first_error.unwrap().id, Some(DiagId::drv_no_such_file));
}

/// WP-2121: the command line's `-working-directory` wins over the
/// option, a relative one relative to it (clang's rule: the command line
/// names the compile's directory); `@file`s are read from the option's.
#[test]
fn command_line_working_directory_wins() {
    let s = Scratch::new("cwd-cl");
    std::fs::create_dir_all(s.0.join("sub/inc")).unwrap();
    s.write("sub/inc/h.h", b"int in_sub;\n");
    s.write("sub/a.cpp", b"#include \"h.h\"\n");
    s.write("flags.rsp", b"-I inc a.cpp\n");
    for wd in ["-working-directory=sub", "-working-directory"] {
        let mut cl = argv(&["-fsyntax-only", wd]);
        if wd == "-working-directory" {
            cl.push(s.path("sub"));
        }
        cl.push("@flags.rsp".to_owned());
        let r = run_front_end("kiln++", &cl, in_dir(&s.0), |fe| has_decl(fe.ast, "in_sub"));
        let r = r.unwrap();
        assert_eq!(r.errors, 0, "{wd}: {:?}", r.first_error);
        assert_eq!(r.value, Some(true));
    }
}

/// WP-2411: with `response_files` off an `@file` word is an ordinary
/// word: `@defs.rsp` is a second input (so no single `-fsyntax-only`
/// job), never `-DV=5`; an input named `@b.cpp` is parsed. On (the
/// default), `@defs.rsp` is read as a response file, as the executable
/// reads it.
#[test]
fn response_files_can_be_turned_off() {
    let s = Scratch::new("no-rsp");
    s.write("defs.rsp", b"-DV=5\n");
    s.write("a.cpp", b"int v = V;\n");
    s.write("@b.cpp", b"int in_b;\n");
    let words = argv(&["-fsyntax-only", "@defs.rsp", "a.cpp"]);
    let on = run_front_end("kiln++", &words, in_dir(&s.0), |fe| has_decl(fe.ast, "v")).unwrap();
    assert_eq!(on.errors, 0, "{:?}", on.first_error);
    assert_eq!(on.value, Some(true));
    let off = || {
        let mut o = in_dir(&s.0);
        o.response_files = false;
        o
    };
    let r = run_front_end("kiln++", &words, off(), |_| ());
    assert!(matches!(r, Err(FrontEndError::NotOneInput)), "{r:?}");
    let words = argv(&["-fsyntax-only", "@b.cpp"]);
    let r = run_front_end("kiln++", &words, off(), |fe| has_decl(fe.ast, "in_b")).unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    assert_eq!(r.value, Some(true));
}

/// WP-2826: a response file that cannot be read, or that names itself,
/// is `FrontEndError::ResponseFile`, with clang's text; the names are
/// joined onto the working directory.
#[test]
fn response_file_errors() {
    let s = Scratch::new("rsp-err");
    std::fs::create_dir_all(s.0.join("dir")).unwrap();
    s.write("self.rsp", b"-fsyntax-only @self.rsp\n");
    let r = run_front_end("kiln++", &argv(&["@dir"]), in_dir(&s.0), |_| ());
    let Err(FrontEndError::ResponseFile(e)) = r else { panic!("{r:?}") };
    assert_eq!(e.to_string(), format!("cannot not open file '{}': Is a directory", s.path("dir")));
    let r = run_front_end("kiln++", &argv(&["@self.rsp"]), in_dir(&s.0), |_| ());
    let Err(e @ FrontEndError::ResponseFile(_)) = r else { panic!("{r:?}") };
    assert_eq!(e.to_string(), format!("recursive expansion of: '{}'", s.path("self.rsp")));
}

/// WP-2121: the executable's `-working-directory`: inputs, `-I`, `-o`
/// and `-MF` resolve against it, the dependency file and the line
/// markers keep the names as written, and nothing appears in the
/// process's directory.
#[test]
fn executable_working_directory() {
    let s = Scratch::new("cwd-exe");
    let elsewhere = Scratch::new("cwd-exe-elsewhere");
    std::fs::create_dir_all(s.0.join("inc")).unwrap();
    std::fs::create_dir_all(s.0.join("out")).unwrap();
    s.write("inc/h.h", b"int h;\n");
    s.write("a.cpp", b"#include \"h.h\"\nconst char *f = __FILE__;\n");
    let kilnpp = std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let wd = format!("-working-directory={}", s.0.display());
    let run = |args: &[&str]| {
        let o = Command::new(&kilnpp).arg(&wd).args(args).current_dir(&elsewhere.0).output();
        let o = o.unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let e = run(&["-E", "-P", "-I", "inc", "a.cpp"]);
    assert!(e.contains("const char *f = \"a.cpp\";"), "{e}");
    let e = run(&["-E", "-I", "inc", "a.cpp"]);
    assert!(e.contains("# 1 \"inc/h.h\" 1"), "{e}");
    run(&["-fsyntax-only", "-I", "inc", "-MD", "-MF", "out/a.d", "-MT", "a.o", "a.cpp"]);
    let d = std::fs::read_to_string(s.0.join("out/a.d")).unwrap();
    assert_eq!(d, "a.o: a.cpp inc/h.h\n");
    run(&["-E", "-I", "inc", "a.cpp", "-o", "out/a.ii"]);
    assert!(s.0.join("out/a.ii").is_file());
    let left: Vec<_> = std::fs::read_dir(&elsewhere.0).unwrap().collect();
    assert!(left.is_empty(), "nothing in the process's directory: {left:?}");
}

/// WP-2173: the two corpus files whose enumeration's underlying type was
/// an alias of an invalid type ([dcl.typedef]/2, [dcl.enum]/2) panicked
/// in the constant evaluator. Through the entry they come back with the
/// diagnostics the `kiln` executable prints for the same command line:
/// the corpus run's arguments (`-verify` aside, which the entry refuses),
/// and with `-U__SIZE_TYPE__`, the predefined macro whose absence (a
/// parse without the target's predefines) made the alias invalid.
#[test]
fn invalid_alias_enum_base_matches_the_executable() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance")
        .canonicalize()
        .unwrap();
    let path = |rel: &str| corpus.join(rel).to_str().unwrap().to_owned();
    let aware = path("expr.const/type-aware-direct-call.cpp");
    let alias = path("dcl.enum/invalid-alias-base-constexpr-ice.cpp");
    let aware_args = ["-ferror-limit=0", "-Wno-ext-cxx-type-aware-allocators", "-fclang-compat=22"];
    let kilnpp = std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++");
    let kilnpp = kilnpp.as_path();
    let cases: [(Vec<&str>, &str, u32); 3] = [
        (aware_args.to_vec(), &aware, 3),
        ([&aware_args[..], &["-U__SIZE_TYPE__"]].concat(), &aware, 12),
        (Vec::new(), &alias, 3),
    ];
    for (extra, file, errors) in cases {
        let mut cl = argv(&["-std=c++26", "-fsyntax-only", "-fno-crash-diagnostics"]);
        cl.extend(extra.iter().map(|&s| s.to_owned()));
        cl.push(file.to_owned());
        for own_thread in [true, false] {
            let (mut opts, buf) = captured();
            opts.own_thread = own_thread;
            let r = run_front_end("kiln++", &cl, opts, |fe| fe.main_file.is_some());
            let r = r.unwrap_or_else(|e| panic!("{cl:?}: {e}"));
            assert_eq!(r.value, Some(true), "{cl:?}");
            assert_eq!((r.errors, r.fatal), (errors, false), "{cl:?}: {}", buf.contents());
            let out = Command::new(kilnpp).args(&cl).output().unwrap();
            assert_eq!(out.status.code(), Some(1), "{cl:?}");
            assert_eq!(String::from_utf8_lossy(&out.stderr), buf.contents(), "{cl:?}");
        }
    }
}

/// The explicitly defaulted constructor or destructor of class `class`
/// (`dtor`: the destructor).
fn defaulted_member(ast: &Ast, class: &str, dtor: bool) -> DeclId {
    use kiln_ast::decl::{DeclKind, Defaulted};
    let kind = if dtor { DeclKind::CXXDestructor } else { DeclKind::CXXConstructor };
    let mut found = ast.decls.ids().filter(|&d| {
        let n = ast.decls.get(d);
        n.kind() == kind
            && n.ctx().is_some_and(|c| ident(ast, c) == class)
            && ast.decls.function(d).is_some_and(|f| f.spec.defaulted() == Defaulted::Explicit)
    });
    let d = found.next().unwrap_or_else(|| panic!("no defaulted member of {class}"));
    assert!(found.next().is_none(), "one defaulted member of {class}");
    d
}

/// The source of `exception_spec_computed_vs_written`: `W`'s default
/// constructor is written `noexcept(false)`, `C`'s copy constructor is
/// computed to be `noexcept(false)` (the `static_assert` resolves it),
/// `D`'s destructor's specification is computed and left unresolved.
const EXC_SPEC_SRC: &str = "struct M { M(); M(const M &) noexcept(false); };\n\
     struct W { M m; W() noexcept(false) = default; };\n\
     struct C { M m; C(); C(const C &) = default; };\n\
     struct D { ~D() = default; };\n\
     static_assert(!noexcept(C(*static_cast<const C *>(nullptr))));\n";

/// Checks `exception_spec_computed` on `EXC_SPEC_SRC`'s classes.
fn check_exc_spec_computed(ast: &Ast) {
    let w = defaulted_member(ast, "W", false);
    let c = defaulted_member(ast, "C", false);
    let d = defaulted_member(ast, "D", true);
    let nothrow = |f: DeclId| {
        let ty = ast.decls.get(f).ty().unwrap();
        let p = ast.types.proto(ty).unwrap();
        ast.types.exc_spec(p)
    };
    // Written `noexcept(false)` and computed `noexcept(false)`: the same
    // specification, told apart only by the query.
    assert_eq!(nothrow(w), nothrow(c));
    assert!(!ast.exception_spec_computed(w), "written");
    assert!(ast.exception_spec_computed(c), "computed and resolved");
    assert!(ast.exception_spec_computed(d), "computed");
}

/// Test (WP-2505, spec/ast.md §4.2): a tooling client tells an explicitly
/// defaulted function's written exception specification from a computed
/// one, in the unit itself and through a module image of it (the unit's
/// source blanked, so the importer cannot replay it). PCH snapshots
/// carry `FnSpec` whole (`kiln_snap::Snap`), so the bit needs no code there.
#[test]
fn exception_spec_computed_vs_written() {
    let s = Scratch::new("excspec");
    let main = s.write("main.cpp", EXC_SPEC_SRC.as_bytes());
    let r =
        run_front_end("kiln++", &argv(&["-fsyntax-only", "-std=c++26", &main]), quiet(), |fe| {
            check_exc_spec_computed(fe.ast);
        })
        .unwrap();
    assert_eq!((r.value, r.errors), (Some(()), 0), "{:?}", r.first_error);

    let unit = format!("export module m;\nexport {{\n{EXC_SPEC_SRC}}}\n");
    let cppm = s.write("m.cppm", unit.as_bytes());
    let pcm = s.path("m.pcm");
    let out =
        Command::new(std::path::Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++"))
            .args([
                "-v",
                "-std=c++26",
                "-fkiln-bmi-image",
                "--precompile",
                "-x",
                "c++-module",
                &cppm,
                "-o",
                &pcm,
            ])
            .output()
            .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("with a declaration image of "), "{stderr}");
    // The unit's source blanked (same length): only an importer that
    // builds the unit from its image still compiles.
    let mut c = kiln_pp::bmi::read(&std::fs::read(&pcm).unwrap()).unwrap();
    let text = &mut c.files[0].text;
    *text = text.bytes().map(|b| if b == b'\n' { '\n' } else { ' ' }).collect();
    let blank = s.write("blank.pcm", &kiln_pp::bmi::write(&c));
    let user = s.write("user.cpp", b"import m;\nC *p;\n");
    let cl = argv(&[
        "-fsyntax-only",
        "-std=c++26",
        "-fkiln-bmi-image",
        &format!("-fmodule-file=m={blank}"),
        &user,
    ]);
    let r = run_front_end("kiln++", &cl, quiet(), |fe| check_exc_spec_computed(fe.ast)).unwrap();
    assert_eq!((r.value, r.errors), (Some(()), 0), "{:?}", r.first_error);
}

/// `FrontEndOptions::token_table` (spec/parse.md §2.6, WP-2825): the
/// tokens of the main file and of an included one, the split `>>` of the
/// header as two `>`s, nothing in a skipped group; off, no table.
#[test]
fn token_table_of_two_files() {
    let s = Scratch::new("token-table");
    let h_src = "template <class T> struct A {};\nA<A<int>> in_header;\n";
    let main_src = "#include \"h.h\"\nint m = 1;\n#if 0\nint skipped;\n#endif\n";
    s.write("h.h", h_src.as_bytes());
    let main = s.write("main.cpp", main_src.as_bytes());
    let cl = argv(&["-std=c++26", "-fsyntax-only", &main]);
    let off = run_front_end("kiln++", &cl, quiet(), |fe| {
        assert!(!fe.parse_options.token_table);
        (fe.token_table.is_none(), fe.tokens)
    })
    .unwrap();
    assert_eq!((off.errors, off.value), (0, Some((true, 23))), "{:?}", off.first_error);
    let mut on = quiet();
    on.token_table = true;
    let r = run_front_end("kiln++", &cl, on, |fe| {
        assert!(fe.parse_options.token_table);
        let t = fe.token_table.expect("the table");
        assert_eq!(t.delivered(), fe.tokens);
        assert_eq!(t.len() as u64, fe.tokens + 1, "the header's `>>` is two rows");
        let file = |name: &str| {
            fe.sm.file_ids().find(|&f| fe.sm.file_name(f).ends_with(name)).expect(name)
        };
        let (h, m) = (file("/h.h"), file("/main.cpp"));
        let at = |src: &str, n: &str| u32::try_from(src.find(n).unwrap()).unwrap();
        let gg = at(h_src, ">> in_header");
        for o in [0, at(h_src, "A<A"), at(h_src, "<A<"), gg, gg + 1, at(h_src, "in_header")] {
            assert!(t.starts_token(h, o), "h.h:{o}");
        }
        assert!(!t.starts_token(h, at(h_src, "in_header") + 1));
        assert!(!t.starts_token(m, gg), "the query is per file");
        for o in [at(main_src, "int m"), at(main_src, "m ="), at(main_src, "1;")] {
            assert!(t.starts_token(m, o), "main.cpp:{o}");
        }
        for o in [0, 1, at(main_src, "\"h.h\""), at(main_src, "int skipped")] {
            assert!(!t.starts_token(m, o), "main.cpp:{o}: a directive or a skipped group");
        }
        (t.file_tokens(h).count(), t.file_tokens(m).count())
    })
    .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    // `template < class T > struct A { } ;` (10) and `A < A < int > > in_header ;`
    // (9, the `>>` split) in h.h; `int m = 1 ;` (5) in main.cpp.
    assert_eq!(r.value, Some((19, 5)));
}

/// `#embed` data (spec/parse.md §2.6): the bytes are delivered to the
/// parser as numbers and commas spelled in a scratch buffer behind an
/// `#embed` expansion entry, so they are rows, from a macro expansion,
/// and no file token of the file holding the directive.
#[test]
fn token_table_embed_data_are_macro_rows() {
    let s = Scratch::new("token-table-embed");
    s.write("d.bin", b"\x01\x02\x03");
    let main_src = "const unsigned char e[] = {\n#embed \"d.bin\"\n};\n";
    let main = s.write("main.cpp", main_src.as_bytes());
    let cl = argv(&["-std=c++26", "-fsyntax-only", &main]);
    let mut on = quiet();
    on.token_table = true;
    let r = run_front_end("kiln++", &cl, on, |fe| {
        let t = fe.token_table.expect("the table");
        assert_eq!(t.len() as u64, fe.tokens);
        let m = fe.main_file.unwrap();
        let embed: Vec<_> = t.rows().iter().filter(|r| r.from_macro()).collect();
        let lens: Vec<u32> = embed.iter().map(|r| r.len()).collect();
        assert_eq!(lens, [1, 1, 1, 1, 1], "`1 , 2 , 3`");
        assert!(embed.iter().all(|r| !r.from_macro_arg()));
        let at = |n: &str| u32::try_from(main_src.find(n).unwrap()).unwrap();
        assert!(!t.starts_token(m, at("#embed")));
        assert!(!t.starts_token(m, at("\"d.bin\"")));
        assert!(t.starts_token(m, at("};")));
        t.file_tokens(m).count()
    })
    .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    // `const unsigned char e [ ] = {` (8) and `} ;` (2).
    assert_eq!(r.value, Some(10));
}

/// The token table's cost on one large unit (spec/parse.md §2.6 "Cost"):
/// six libstdc++ headers at `-std=c++26`. A measurement, not a check:
/// `cargo test -p kiln_driver --test front_end token_table_cost --
/// --ignored --nocapture` prints the tokens, the rows, the file tokens
/// and the bytes.
#[test]
#[ignore = "a measurement; needs the system's libstdc++"]
fn token_table_cost() {
    let s = Scratch::new("token-table-cost");
    let main = s.write(
        "main.cpp",
        b"#include <algorithm>\n#include <vector>\n#include <string>\n#include <map>\n\
          #include <memory>\n#include <iostream>\nint main() {}\n",
    );
    let cl = argv(&["-std=c++26", "-fsyntax-only", &main]);
    let mut on = quiet();
    on.token_table = true;
    let r = run_front_end("kiln++", &cl, on, |fe| {
        let t = fe.token_table.expect("the table");
        let files: usize = fe.sm.file_ids().map(|f| t.file_tokens(f).count()).sum();
        (fe.tokens, t.len(), files, t.heap_bytes())
    })
    .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    let (tokens, rows, files, bytes) = r.value.unwrap();
    eprintln!("tokens {tokens} rows {rows} file-tokens {files} bytes {bytes}");
}
