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
    std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"))
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
    assert!(rec.starts_with("KILNPCH 1\nkiln "), "{rec}");
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
    assert!(read(&d, "inc/h.hpp.pch").starts_with("KILNPCH 1\n"));
    ok(&d, &["inc/h.hpp", "-o", "a.pch"]);
    ok(&d, &["-c", "inc/h.hpp", "-o", "b.pch"]);
    ok(&d, &["-x", "c++-header", "x.inc", "-o", "c.pch"]);
    ok(&d, &["-Xclang", "-emit-pch", "-c", "s.cpp"]);
    assert!(read(&d, "s.o").starts_with("KILNPCH 1\n"));
    for f in ["a.pch", "b.pch", "c.pch"] {
        assert!(read(&d, f).starts_with("KILNPCH 1\n"), "{f}");
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
    ok(&d.join("sub"), &["-include-pch", &pch, "-include", &c, "-fsyntax-only", "../m.cpp"]);
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
        // A missing or extra -D and a different -I are fine.
        ok(
            &d,
            &[
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
    std::fs::write(d.join("cut.pch"), "KILNPCH 1\nkiln x\n").unwrap();
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
