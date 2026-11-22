//! The front-end library entry (`kiln_driver::front_end`, spec/driver.md
//! "Front-end library entry", WP-020 for Slip): one `-fsyntax-only` parse
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
use kiln_driver::front_end::{
    Consulted, ConsultedKind, FrontEnd, FrontEndError, FrontEndOptions, FrontEndReport,
    run_front_end,
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
            Consulted { kind: ConsultedKind::Probe, path: probe },
            Consulted { kind: ConsultedKind::Embed, path: data },
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

#[test]
fn options_default() {
    let o = FrontEndOptions::default();
    assert!(o.stderr.is_none() && !o.record_lookups && o.own_thread);
}
