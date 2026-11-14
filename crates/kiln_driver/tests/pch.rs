//! WP-1023: precompiled headers end to end through `kiln++`
//! (spec/driver.md "Precompiled headers"). A kiln PCH is a textual
//! record; `-include-pch` checks it with clang's diagnostics and then
//! replays the header as an `-include` ([cpp.include]). Covered: `CMake`'s
//! `target_precompile_headers` command lines (producer and consumer), the
//! double-include rule with a header that has no guard, the dependency
//! files (byte-compared with clang++-19 where installed), stale, missing,
//! foreign and mismatched PCHs in clang 19's and clang 22's wording,
//! `-E`, and the output naming rules.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(cc: &Path, dir: &Path, args: &[&str]) -> Out {
    let o: Output =
        Command::new(cc).args(args).current_dir(dir).stdin(Stdio::null()).output().unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "{} {args:?} panicked:\n{stderr}", cc.display());
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    run(&kilnpp(), dir, args)
}

fn ok(dir: &Path, args: &[&str]) -> Out {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    assert_eq!(o.stderr, "", "kiln++ {args:?}");
    o
}

fn read(dir: &Path, f: &str) -> String {
    // Lossy: a PCH carries its binary snapshot section (WP-1404).
    let b = std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    String::from_utf8_lossy(&b).into_owned()
}

/// A header with neither a guard nor `#pragma once`: including it twice
/// redefines `S`, so the consumer must not include it again.
const PCH_HPP: &str = "int helper(int x) { return x + 1; }\nstruct S { int v; };\n";
const MAIN: &str = "int main() { S s{helper(1)}; return s.v - 2; }\n";

/// `CMake` 3.31's layout: `cmake_pch.hxx` (no guard) includes the user's
/// header; `cmake_pch.hxx.cxx` is the producer's (empty) main file.
fn cmake_setup(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("pch.hpp"), PCH_HPP).unwrap();
    std::fs::write(
        d.join("cmake_pch.hxx"),
        format!("#pragma clang system_header\n#include \"{}/pch.hpp\"\n", d.display()),
    )
    .unwrap();
    std::fs::write(d.join("cmake_pch.hxx.cxx"), "/* generated */\n").unwrap();
    std::fs::write(d.join("main.cpp"), MAIN).unwrap();
    d
}

fn producer_args(d: &Path) -> Vec<String> {
    let hxx = d.join("cmake_pch.hxx").display().to_string();
    let cxx = d.join("cmake_pch.hxx.cxx").display().to_string();
    let v = [
        "-Winvalid-pch",
        "-fpch-instantiate-templates",
        "-Xclang",
        "-emit-pch",
        "-Xclang",
        "-include",
        "-Xclang",
        &hxx,
        "-x",
        "c++-header",
        "-MD",
        "-MT",
        "p.pch",
        "-MF",
        "p.pch.d",
        "-o",
        "p.pch",
        "-c",
        &cxx,
    ];
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn consumer_args(d: &Path, action: &[&str]) -> Vec<String> {
    let hxx = d.join("cmake_pch.hxx").display().to_string();
    let pch = d.join("p.pch").display().to_string();
    let mut v: Vec<String> = [
        "-Winvalid-pch",
        "-Xclang",
        "-include-pch",
        "-Xclang",
        &pch,
        "-Xclang",
        "-include",
        "-Xclang",
        &hxx,
        "-MD",
        "-MT",
        "main.o",
        "-MF",
        "main.d",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    v.extend(action.iter().map(|s| (*s).to_owned()));
    v.push("main.cpp".to_owned());
    v
}

/// A Make rule with its line continuations joined.
fn unwrap(rule: &str) -> String {
    rule.replace(" \\\n  ", " ")
}

fn strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

/// `CMake` `target_precompile_headers`: the producer writes the record and
/// a depfile listing its main file, `cmake_pch.hxx` and the header; the
/// consumer compiles with the header seen once although `CMake` also passes
/// `-include cmake_pch.hxx`, and its depfile lists the PCH's input files
/// first, as clang's does.
#[test]
fn cmake_producer_and_consumer() {
    let d = cmake_setup("cmake");
    let p = producer_args(&d);
    ok(&d, &strs(&p));
    let rec = read(&d, "p.pch");
    assert!(rec.starts_with("KILNPCH 2\nkiln "), "{rec}");
    assert!(rec.contains("\nopt b\tC++17\t1\n") && rec.contains("\nend "), "{rec}");
    let dir = d.display();
    let pdeps = read(&d, "p.pch.d");
    assert_eq!(
        unwrap(&pdeps),
        format!("p.pch: {dir}/cmake_pch.hxx.cxx {dir}/cmake_pch.hxx {dir}/pch.hpp\n")
    );
    let c = consumer_args(&d, &["-fsyntax-only"]);
    ok(&d, &strs(&c));
    let mdeps = read(&d, "main.d");
    assert_eq!(
        unwrap(&mdeps),
        format!("main.o: {dir}/cmake_pch.hxx.cxx {dir}/cmake_pch.hxx {dir}/pch.hpp main.cpp\n")
    );
    // Without -include-pch, the guardless header twice is an error: the
    // test above really exercised the double-include rule.
    let hxx = d.join("cmake_pch.hxx").display().to_string();
    let o = kiln(&d, &["-include", &hxx, "-include", &hxx, "-fsyntax-only", "main.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    if have("clang++-19") {
        // clang++-19 writes the same two depfiles for the same commands.
        let clang = Path::new("clang++-19");
        for f in ["p.pch", "p.pch.d", "main.d"] {
            let _ = std::fs::remove_file(d.join(f));
        }
        assert_eq!(run(clang, &d, &strs(&p)).code, 0);
        assert_eq!(read(&d, "p.pch.d"), pdeps, "clang++-19 producer depfile");
        assert_eq!(run(clang, &d, &strs(&c)).code, 0);
        assert_eq!(read(&d, "main.d"), mdeps, "clang++-19 consumer depfile");
    }
}

/// The `CMake` flow through `-c` and a link: the program runs.
#[test]
fn cmake_object_and_link() {
    if !(have("llc-19") && have("clang++-19")) {
        eprintln!("skipping: llc-19 / clang++-19 not found");
        return;
    }
    let d = cmake_setup("link");
    ok(&d, &strs(&producer_args(&d)));
    ok(&d, &strs(&consumer_args(&d, &["-c", "-o", "main.o"])));
    ok(&d, &["main.o", "-o", "app"]);
    let st = Command::new(d.join("app")).status().unwrap();
    assert_eq!(st.code(), Some(0));
}

/// A header input precompiles: `kiln++ h.hpp` writes `h.hpp.pch` beside
/// it, `-o` names the file (with or without `-c`), `-x c++-header` makes
/// any file a header, `-Xclang -emit-pch` precompiles a source to its
/// object name, and `-fsyntax-only` only checks the header (no PCH).
/// `#pragma once` in the header being precompiled is not warned about
/// (clang's `IsHeaderFile`).
#[test]
fn producer_outputs() {
    let d = scratch("outputs");
    std::fs::create_dir_all(d.join("inc")).unwrap();
    std::fs::write(d.join("inc/h.hpp"), "#pragma once\nint h();\n").unwrap();
    std::fs::write(d.join("x.inc"), "int x();\n").unwrap();
    std::fs::write(d.join("s.cpp"), "int s();\n").unwrap();
    ok(&d, &["inc/h.hpp"]);
    assert!(read(&d, "inc/h.hpp.pch").starts_with("KILNPCH 2\n"));
    ok(&d, &["inc/h.hpp", "-o", "a.pch"]);
    ok(&d, &["-c", "inc/h.hpp", "-o", "b.pch"]);
    ok(&d, &["-x", "c++-header", "x.inc", "-o", "c.pch"]);
    ok(&d, &["-Xclang", "-emit-pch", "-c", "s.cpp"]);
    assert!(read(&d, "s.o").starts_with("KILNPCH 2\n"));
    for f in ["a.pch", "b.pch", "c.pch"] {
        assert!(read(&d, f).starts_with("KILNPCH 2\n"), "{f}");
    }
    ok(&d, &["-fsyntax-only", "-x", "c++-header", "x.inc"]);
    assert!(!d.join("x.inc.pch").exists());
    // Two headers and one -o: clang's error.
    let o = kiln(&d, &["-c", "inc/h.hpp", "x.inc", "-x", "c++-header", "s.cpp", "-o", "z.pch"]);
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "kiln++: error: cannot specify -o when generating multiple output files\n"
    );
    // A header with an error writes no PCH.
    std::fs::write(d.join("bad.hpp"), "int f() { return undeclared; }\n").unwrap();
    let o = kiln(&d, &["bad.hpp"]);
    assert_eq!(o.code, 1);
    assert!(!d.join("bad.hpp.pch").exists());
}

/// `-include-pch` replays the producer's `-include`s, then its main
/// file, then the consumer's own `-include`s (one the producer already
/// had is skipped); from another directory the recorded paths resolve
/// against the producer's directory. The PCH's macros carry through.
#[test]
fn replay_order_and_cwd() {
    let d = scratch("replay");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("a.h"), "#define A 1\n").unwrap();
    std::fs::write(d.join("h.hpp"), "#define H (A + 1)\nstruct T {};\n").unwrap();
    std::fs::write(d.join("c.h"), "#define C (H + 1)\n").unwrap();
    std::fs::write(
        d.join("m.cpp"),
        "T t;\nstatic_assert(A == 1 && H == 2 && C == 3);\nint main() { return 0; }\n",
    )
    .unwrap();
    ok(&d, &["-include", "a.h", "h.hpp", "-o", "h.pch"]);
    ok(
        &d,
        &["-include-pch", "h.pch", "-include", "a.h", "-include", "c.h", "-fsyntax-only", "m.cpp"],
    );
    let pch = d.join("h.pch").display().to_string();
    let c = d.join("c.h").display().to_string();
    // Another working directory is another snapshot identity (a
    // `-Wpch-snapshot-unused` warning by design); the replay is under test.
    ok(
        &d.join("sub"),
        &[
            "-fno-kiln-pch-snapshot",
            "-include-pch",
            &pch,
            "-include",
            &c,
            "-fsyntax-only",
            "../m.cpp",
        ],
    );
    // -E does not check the PCH: the header, then the -includes.
    let o = ok(&d, &["-E", "-P", "-include-pch", "h.pch", "-include", "c.h", "m.cpp"]);
    assert!(o.stdout.trim_start().starts_with("struct T {};\n"), "{}", o.stdout);
    assert_eq!(o.stdout.matches("struct T").count(), 1);
}

/// The PCH's checks, in clang 19's and clang 22's wording, compared with
/// the installed clang where there is one (`<pch>` is the PCH name).
fn check_case(compat: &str, d: &Path, args: &[&str], pch: &str) -> String {
    let mut a = vec![compat, "-include-pch", pch];
    a.extend_from_slice(args);
    a.extend_from_slice(&["-fsyntax-only", "m.cpp"]);
    let o = kiln(d, &a);
    assert_eq!(o.code, 1, "{a:?}: {}", o.stderr);
    o.stderr.replace(pch, "<pch>").replace(&d.display().to_string(), "<dir>")
}

#[test]
fn option_mismatches() {
    let d = scratch("mismatch");
    std::fs::write(d.join("h.hpp"), "int h1(int x) { return x; }\n").unwrap();
    std::fs::write(d.join("m.cpp"), "int main() { return h1(0); }\n").unwrap();
    for (compat, pch, file) in [
        ("-fclang-compat=19", "k19.pch", "PCH file"),
        ("-fclang-compat=22", "k22.pch", "precompiled file '<pch>'"),
    ] {
        // The target is named so the case is the same on every host.
        let a64 = "--target=aarch64-unknown-linux-gnu";
        ok(&d, &[compat, a64, "-x", "c++-header", "h.hpp", "-o", pch, "-DFOO=1", "-std=c++17"]);
        let hdr = if compat.ends_with("19") { "precompiled header" } else { file };
        let case = |args: &[&str]| {
            let mut v = args.to_vec();
            if !v.iter().any(|a| a.starts_with("--target=")) {
                v.push(a64);
            }
            check_case(compat, &d, &v, pch)
        };
        assert_eq!(
            case(&["-std=c++20", "-DFOO=1"]),
            format!(
                "error: C++20 was disabled in {file} but is currently enabled\n1 error generated.\n"
            )
        );
        assert_eq!(
            case(&["-std=c++17", "-DFOO=2"]),
            format!(
                "error: definition of macro 'FOO' differs between the {hdr} ('1') and the \
                 command line ('2')\n1 error generated.\n"
            )
        );
        assert_eq!(
            case(&["-std=c++17", "-UFOO"]),
            format!(
                "error: macro 'FOO' was defined in the {hdr} but undef'd on the command line\n\
                 1 error generated.\n"
            )
        );
        // Every category reports its first mismatch, then the compile stops.
        let all = case(&["-std=c++20", "-DFOO=3", "--target=x86_64-unknown-linux-gnu"]);
        let tgt = if compat.ends_with("19") { "PCH file".to_owned() } else { file.to_owned() };
        assert_eq!(
            all,
            format!(
                "error: C++20 was disabled in {file} but is currently enabled\n\
                 error: {tgt} was compiled for the target 'aarch64-unknown-linux-gnu' but the \
                 current translation unit is being compiled for target 'x86_64-unknown-linux-gnu'\n\
                 error: definition of macro 'FOO' differs between the {hdr} ('1') and the \
                 command line ('3')\n3 errors generated.\n"
            )
        );
        let opt = case(&["-std=c++17", "-O2"]);
        if compat.ends_with("19") {
            assert_eq!(
                opt,
                "error: __OPTIMIZE__ predefined macro was disabled in PCH file but is currently \
                 enabled\n1 error generated.\n"
            );
        } else {
            assert_eq!(
                opt,
                "error: OptimizationLevel differs in precompiled file '<pch>' vs. current file\n\
                 1 error generated.\n"
            );
        }
        // A missing or extra -D and a different -I are fine (to the
        // record checks; the snapshot would warn and replay, by design).
        ok(
            &d,
            &[
                "-fno-kiln-pch-snapshot",
                compat,
                a64,
                "-include-pch",
                pch,
                "-std=c++17",
                "-DBAR",
                "-Isub",
                "-fsyntax-only",
                "m.cpp",
            ],
        );
        // Another clang-compat level is another compiler.
        let other = if compat.ends_with("19") { "-fclang-compat=22" } else { "-fclang-compat=19" };
        let o =
            kiln(&d, &[other, a64, "-include-pch", pch, "-std=c++17", "-fsyntax-only", "m.cpp"]);
        assert_eq!(o.code, 1);
        assert!(
            o.stderr.starts_with("error: PCH file built from a different branch ("),
            "{}",
            o.stderr
        );
    }
}

/// Stale, missing and foreign PCHs: clang's fatal errors (kiln compares
/// contents, so a same-size edit is "content changed"; `-fno-validate-pch`
/// skips the checks).
#[test]
fn stale_missing_and_foreign() {
    let d = scratch("stale");
    std::fs::write(d.join("h.hpp"), "int h1(int x) { return x; }\n").unwrap();
    std::fs::write(d.join("m.cpp"), "int main() { return h1(0); }\n").unwrap();
    for (compat, pch, note) in [
        ("-fclang-compat=19", "s19.pch", "precompiled header"),
        ("-fclang-compat=22", "s22.pch", "precompiled file"),
    ] {
        std::fs::write(d.join("h.hpp"), "int h1(int x) { return x; }\n").unwrap();
        ok(&d, &[compat, "-x", "c++-header", "h.hpp", "-o", pch]);
        ok(&d, &[compat, "-include-pch", pch, "-fsyntax-only", "m.cpp"]);
        std::fs::write(d.join("h.hpp"), "int h1(int x) { return x+1; }\n").unwrap();
        assert_eq!(
            check_case(compat, &d, &[], pch),
            format!(
                "fatal error: file '<dir>/h.hpp' has been modified since the precompiled header \
                 '<pch>' was built: size changed (was 28, now 30)\nnote: please rebuild {note} \
                 '<pch>'\n1 error generated.\n"
            )
        );
        ok(
            &d,
            &[
                // The snapshot would warn (changed input) and replay.
                "-fno-kiln-pch-snapshot",
                compat,
                "-Xclang",
                "-fno-validate-pch",
                "-include-pch",
                pch,
                "-fsyntax-only",
                "m.cpp",
            ],
        );
        std::fs::write(d.join("h.hpp"), "int h1(int y) { return y; }\n").unwrap();
        assert!(check_case(compat, &d, &[], pch).contains("was built: content changed\n"));
        std::fs::remove_file(d.join("h.hpp")).unwrap();
        let ast = if compat.ends_with("19") { "AST file" } else { "precompiled file" };
        assert_eq!(
            check_case(compat, &d, &[], pch),
            format!(
                "fatal error: malformed or corrupted {ast}: 'could not find file '<dir>/h.hpp' \
                 referenced by AST file '<pch>''\n1 error generated.\n"
            )
        );
    }
    assert_eq!(
        check_case("-fclang-compat=19", &d, &[], "nope.pch"),
        "error: unable to read PCH file <pch>: 'No such file or directory'\nfatal error: PCH \
         file '<pch>' not found: module file not found\n2 errors generated.\n"
    );
    std::fs::write(d.join("bad.pch"), "garbage\n").unwrap();
    assert_eq!(
        check_case("-fclang-compat=19", &d, &[], "bad.pch"),
        "error: input is not a PCH file: '<pch>'\nfatal error: file '<pch>' is not a valid \
         precompiled PCH file: file doesn't start with AST file magic\n2 errors generated.\n"
    );
    assert_eq!(
        check_case("-fclang-compat=22", &d, &[], "bad.pch"),
        "error: input is not a PCH file: '<pch>'\nfatal error: file '<pch>' is not a valid PCH \
         file: file doesn't start with precompiled file magic\n2 errors generated.\n"
    );
    std::fs::write(d.join("new.pch"), "KILNPCH 9\n").unwrap();
    assert_eq!(
        check_case("-fclang-compat=19", &d, &[], "new.pch"),
        "error: PCH file uses a newer PCH format that cannot be read\n1 error generated.\n"
    );
    std::fs::write(d.join("cut.pch"), "KILNPCH 2\nkiln x\n").unwrap();
    assert!(
        check_case("-fclang-compat=19", &d, &[], "cut.pch")
            .starts_with("fatal error: malformed or corrupted AST file: '")
    );
    std::fs::write(d.join("clang.pch"), b"CPCH\x01\x02").unwrap();
    assert_eq!(
        check_case("-fclang-compat=19", &d, &[], "clang.pch"),
        "fatal error: file '<pch>' is a precompiled header built by clang; kiln reads only its \
         own precompiled headers\n1 error generated.\n"
    );
}

/// WP-1153: `-E` with a PCH it cannot read reports it, as the compile
/// would (one error for a missing file or a non-PCH, as clang's `-E`),
/// goes on without it and exits 1; `-o` is not written, as for any `-E`
/// error. clang's own cases are compared in [`include_probes_for_pch`].
#[test]
fn preprocess_unreadable_pch() {
    let d = scratch("pp-unread");
    std::fs::write(d.join("m.cpp"), "int a;\n#error boom\nint b;\n").unwrap();
    std::fs::write(d.join("bad.pch"), "garbage\n").unwrap();
    std::fs::write(d.join("new.pch"), "KILNPCH 9\n").unwrap();
    std::fs::write(d.join("cut.pch"), "KILNPCH 2\nkiln x\n").unwrap();
    std::fs::write(d.join("clang.pch"), b"CPCH\x01\x02").unwrap();
    let boom = "m.cpp:2:2: error: boom\n    2 | #error boom\n      |  ^\n2 errors generated.\n";
    for (compat, pch, err) in [
        (
            "-fclang-compat=19",
            "nope.pch",
            "unable to read PCH file nope.pch: 'No such file or directory'",
        ),
        (
            "-fclang-compat=22",
            "nope.pch",
            "unable to read PCH file nope.pch: 'No such file or directory'",
        ),
        ("-fclang-compat=19", "bad.pch", "input is not a PCH file: 'bad.pch'"),
        ("-fclang-compat=22", "bad.pch", "input is not a PCH file: 'bad.pch'"),
        ("-fclang-compat=19", "new.pch", "PCH file uses a newer PCH format that cannot be read"),
    ] {
        let o = kiln(&d, &[compat, "-include-pch", pch, "-E", "-P", "m.cpp"]);
        assert_eq!((o.code, o.stdout.as_str()), (1, "int a;\nint b;\n"), "{pch}");
        assert_eq!(o.stderr, format!("error: {err}\n{boom}"), "{pch}");
    }
    // A fatal error (a corrupt record, a clang PCH) drops the later
    // diagnostics, as fatal errors do; the text is still written.
    for (pch, err) in [
        ("cut.pch", "fatal error: malformed or corrupted AST file: 'truncated in kiln PCH record"),
        ("clang.pch", "fatal error: file 'clang.pch' is a precompiled header built by clang"),
    ] {
        let o = kiln(&d, &["-fclang-compat=19", "-include-pch", pch, "-E", "-P", "m.cpp"]);
        assert_eq!((o.code, o.stdout.as_str()), (1, "int a;\nint b;\n"), "{pch}");
        assert!(o.stderr.starts_with(err), "{pch}: {}", o.stderr);
        assert_eq!(o.stderr.lines().count(), 2, "{pch}: {}", o.stderr);
        assert!(o.stderr.ends_with("\n1 error generated.\n"), "{pch}: {}", o.stderr);
    }
    let o = kiln(&d, &["-include-pch", "bad.pch", "-E", "m.cpp", "-o", "m.i"]);
    assert_eq!((o.code, o.stdout.as_str()), (1, ""));
    assert!(!d.join("m.i").exists());
    // `-fsyntax-only` still stops at the PCH.
    let o = kiln(&d, &["-include-pch", "bad.pch", "-fsyntax-only", "m.cpp"]);
    assert_eq!(o.code, 1);
    assert!(!o.stderr.contains("boom"), "{}", o.stderr);
}

/// `-MMD` leaves the PCH's system headers out, as for any compile.
#[test]
fn user_only_depfile() {
    let d = cmake_setup("mmd");
    let mut p = producer_args(&d);
    p.iter_mut().filter(|a| *a == "-MD").for_each(|a| *a = "-MMD".to_owned());
    ok(&d, &strs(&p));
    let dir = d.display();
    assert_eq!(
        unwrap(&read(&d, "p.pch.d")),
        format!("p.pch: {dir}/cmake_pch.hxx.cxx {dir}/cmake_pch.hxx\n")
    );
}

/// WP-1037: clang's driver probe of `-include <h>` (Clang.cpp
/// `AddPreprocessingOptions`). One compiler's results for the probe
/// cases, in a fresh directory: each case's exit code, its `-E -P` output
/// lines and its standard error with the program name replaced by `<cc>`.
/// The PCHs are built by the same compiler, so clang++-19 runs the same
/// cases with its own PCHs as the oracle.
fn probe_cases(cc: &Path, pre: &[&str], name: &str) -> Vec<(String, i32, String, String)> {
    let d = scratch(name);
    let w = |f: &str, s: &str| std::fs::write(d.join(f), s).unwrap();
    std::fs::create_dir_all(d.join("inc")).unwrap();
    for dir in ["e.h.gch", "n.h.pch", "v.h.gch"] {
        std::fs::create_dir_all(d.join(dir)).unwrap();
    }
    w("foo.h", "#define FROM_H 1\n");
    w("pchsrc.h", "#define FROM_PCH 1\n#define FROM_H 1\n");
    w("bar.h", "#define FROM_B 1\n");
    w("inc/q.h", "#define FROM_Q 1\n");
    w("g.h", "#define FROM_G 1\n");
    w("gsrc.h", "#define FROM_GCH 1\n");
    w("j.h", "#define FROM_J 1\n");
    w("j.h.gch", "not a precompiled header\n");
    w("e.h", "#define FROM_E 1\n");
    w("n.h", "#define FROM_N 1\n");
    w("z.h", "#define FROM_Z 1\n");
    w("z.h.pch", "garbage\n");
    w("v.h", "#define FROM_V 1\n");
    w("v1.h", "#define FROM_V1 1\n");
    w("v2.h", "#define FROM_V2 1\n");
    w(
        "m.cpp",
        "pch=FROM_PCH h=FROM_H b=FROM_B q=FROM_Q g=FROM_G gch=FROM_GCH j=FROM_J e=FROM_E \
         v=FROM_V v1=FROM_V1 v2=FROM_V2\n",
    );
    let go = |args: &[&str]| {
        let mut a = pre.to_vec();
        a.extend_from_slice(args);
        run(cc, &d, &a)
    };
    for (src, out, extra) in [
        ("pchsrc.h", "foo.h.pch", None),
        ("pchsrc.h", "inc/q.h.pch", None),
        ("bar.h", "x.pch", None),
        ("gsrc.h", "g.h.gch", None),
        ("v1.h", "v.h.gch/a.pch", Some("-DV=1")),
        ("v2.h", "v.h.gch/b.pch", Some("-DV=2")),
    ] {
        let mut a = vec!["-x", "c++-header", src, "-o", out];
        a.extend(extra);
        let o = go(&a);
        assert_eq!(o.code, 0, "{} {a:?}: {}", cc.display(), o.stderr);
    }
    let prog = cc.file_name().unwrap().to_string_lossy().into_owned();
    let cases: &[&[&str]] = &[
        // The `.pch` beside the header is used, looked up as spelled.
        &["-include", "foo.h"],
        &["-include", "./foo.h"],
        &["-include", "inc/q.h"],
        // Not in the include path, and not through `-Xclang`.
        &["-I", "inc", "-include", "q.h"],
        &["-Xclang", "-include", "-Xclang", "foo.h"],
        // Only the first `-include`.
        &["-include", "bar.h", "-include", "foo.h"],
        &["-include", "j.h", "-include", "foo.h"],
        // The last `-include-pch` wins; `-Xclang` ones come last.
        &["-include", "foo.h", "-include-pch", "x.pch"],
        &["-include-pch", "x.pch", "-include", "foo.h"],
        &["-Xclang", "-include-pch", "-Xclang", "x.pch", "-include", "foo.h"],
        // `.gch`: a PCH file, a file that is not one, an empty directory,
        // a directory of PCHs (the acceptable one is used).
        &["-include", "g.h"],
        &["-include", "j.h"],
        &["-include", "j.h", "-Wno-ignored-gch"],
        &["-include", "j.h", "-Werror"],
        &["-include", "e.h"],
        &["-DV=2", "-include", "v.h"],
        &["-DV=1", "-include", "v.h"],
        // A `.pch` is used whatever it holds (a directory: one in it).
        &["-include", "n.h"],
        // WP-1153: `--include` and `--include=` are `-include`, probed
        // too, and warnings spell them as written.
        &["--include", "foo.h"],
        &["--includefoo.h"],
        &["--include=foo.h"],
        &["-include", "bar.h", "--include", "foo.h"],
        &["-include", "bar.h", "--include=foo.h"],
        // WP-1153: an `-Xclang -include-pch` wins over a driver one on
        // either side of it.
        &["-Xclang", "-include-pch", "-Xclang", "x.pch", "-include-pch", "foo.h.pch"],
        &["-include-pch", "foo.h.pch", "-Xclang", "-include-pch", "-Xclang", "x.pch"],
        &["-Xclang", "-include-pch", "-Xclang", "x.pch", "-include", "bar.h", "-include", "foo.h"],
        // WP-1153: `-E` with a PCH it cannot read reports it and goes on
        // (exit status 1).
        &["-include-pch", "z.h.pch"],
        &["-include-pch", "nope.pch"],
        &["-include", "z.h"],
    ];
    let mut out = Vec::new();
    for c in cases {
        let mut a = c.to_vec();
        a.extend_from_slice(&["-E", "-P", "m.cpp"]);
        let o = go(&a);
        let lines: Vec<&str> = o.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
        let err = o.stderr.replace(&format!("{prog}:"), "<cc>:");
        out.push((c.join(" "), o.code, lines.join("\n"), err));
    }
    // A `.pch` that is not one is the frontend's error. (Not `-E`: there
    // clang reports only `input is not a PCH file` and goes on.)
    let o = go(&["-include", "z.h", "-fsyntax-only", "m.cpp"]);
    out.push(("-fsyntax-only -include z.h".into(), o.code, String::new(), o.stderr));
    // `-M` goes on too, and each `-E` job reports the PCH (WP-1153).
    let o = go(&["-include-pch", "z.h.pch", "-M", "m.cpp"]);
    out.push(("-M z.h.pch".into(), o.code, o.stdout, o.stderr));
    let o = go(&["-include-pch", "nope.pch", "-E", "-P", "m.cpp", "m.cpp"]);
    let lines: Vec<&str> = o.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    out.push(("-E two jobs".into(), o.code, lines.join("\n"), o.stderr));
    // The driver warns once per job.
    let o = go(&["-include", "j.h", "-fsyntax-only", "-x", "c++", "/dev/null", "/dev/null"]);
    out.push(("two jobs".into(), o.code, String::new(), o.stderr.replace(&prog, "<cc>")));
    out
}

/// WP-1037: `-include foo.h` uses `foo.h.pch`, else a `foo.h.gch` that
/// is a PCH, as clang's driver does; only for the first `-include`, with
/// clang's warnings; the last `-include-pch` wins. Compared case by case
/// with clang++-19 where installed.
#[test]
fn include_probes_for_pch() {
    let kiln_res = probe_cases(&kilnpp(), &["-fclang-compat=19"], "probe-kiln");
    let get = |c: &str| kiln_res.iter().find(|r| r.0 == c).unwrap_or_else(|| panic!("{c}"));
    let used = get("-include foo.h");
    assert_eq!((used.1, used.3.as_str()), (0, ""));
    assert!(used.2.starts_with("pch=1 h=1 b=FROM_B"), "{}", used.2);
    assert!(get("-I inc -include q.h").2.starts_with("pch=FROM_PCH h=FROM_H b=FROM_B q=1"));
    let nf = get("-include bar.h -include foo.h");
    assert_eq!(
        nf.3,
        "<cc>: warning: precompiled header 'foo.h.pch' was ignored because '-include foo.h' \
         is not first '-include'\n"
    );
    assert!(nf.2.starts_with("pch=FROM_PCH h=1 b=1"), "{}", nf.2);
    assert!(get("-include foo.h -include-pch x.pch").2.starts_with("pch=FROM_PCH h=FROM_H b=1"));
    assert!(get("-include g.h").2.contains(" g=FROM_G gch=1 "));
    let ign = get("-include j.h");
    assert_eq!(
        ign.3,
        "<cc>: warning: precompiled header 'j.h.gch' was ignored because it is not a clang PCH \
         file [-Wignored-gch]\n"
    );
    assert!(ign.2.contains(" j=1 "));
    assert_eq!(get("-include j.h -Werror").1, 1);
    assert!(get("-include e.h").3.contains("'e.h.gch' was ignored because it contains no clang"));
    assert!(get("-DV=2 -include v.h").2.ends_with("v=FROM_V v1=FROM_V1 v2=1"));
    assert_eq!(
        get("-fsyntax-only -include z.h").3,
        "error: input is not a PCH file: 'z.h.pch'\nfatal error: file 'z.h.pch' is not a valid \
         precompiled PCH file: file doesn't start with AST file magic\n2 errors generated.\n"
    );
    assert_eq!(
        get("-include n.h").3,
        "error: no suitable precompiled header file found in directory 'n.h.pch'\n1 error \
         generated.\n"
    );
    assert_eq!(get("two jobs").3.matches("warning: precompiled header 'j.h.gch'").count(), 2);
    for c in ["--include foo.h", "--includefoo.h", "--include=foo.h"] {
        assert_eq!((get(c).1, get(c).2.as_str(), get(c).3.as_str()), (used.1, used.2.as_str(), ""));
    }
    for (c, spelled) in [
        ("-include bar.h --include foo.h", "--include foo.h"),
        ("-include bar.h --include=foo.h", "--include=foo.h"),
    ] {
        assert_eq!(get(c).2, nf.2);
        assert_eq!(get(c).3, nf.3.replace("'-include foo.h'", &format!("'{spelled}'")));
    }
    for c in [
        "-Xclang -include-pch -Xclang x.pch -include-pch foo.h.pch",
        "-include-pch foo.h.pch -Xclang -include-pch -Xclang x.pch",
    ] {
        assert!(get(c).2.starts_with("pch=FROM_PCH h=FROM_H b=1"), "{c}: {}", get(c).2);
    }
    let x = get("-Xclang -include-pch -Xclang x.pch -include bar.h -include foo.h");
    assert!(x.2.starts_with("pch=FROM_PCH h=1 b=1"), "{}", x.2);
    assert_eq!(x.3, nf.3);
    let junk = get("-include-pch z.h.pch");
    assert_eq!(
        (junk.1, junk.3.as_str()),
        (1, "error: input is not a PCH file: 'z.h.pch'\n1 error generated.\n")
    );
    assert!(junk.2.starts_with("pch=FROM_PCH h=FROM_H b=FROM_B"), "{}", junk.2);
    assert_eq!(get("-include z.h").3, junk.3);
    let gone = get("-include-pch nope.pch");
    assert_eq!(
        (gone.1, gone.2.as_str(), gone.3.as_str()),
        (
            1,
            junk.2.as_str(),
            "error: unable to read PCH file nope.pch: 'No such file or directory'\n1 error \
             generated.\n"
        )
    );
    let m = get("-M z.h.pch");
    assert_eq!((m.1, m.2.as_str(), m.3.as_str()), (1, "m.o: m.cpp\n", junk.3.as_str()));
    let two = get("-E two jobs");
    assert_eq!(two.1, 1);
    assert_eq!(two.2, format!("{}\n{}", gone.2, gone.2));
    assert_eq!(two.3, gone.3.repeat(2));
    if have("clang++-19") {
        let c = probe_cases(Path::new("clang++-19"), &[], "probe-clang");
        for (kr, cr) in kiln_res.iter().zip(&c) {
            assert_eq!(kr, cr, "kiln++ (left) and clang++-19 (right) differ");
        }
    }
}
