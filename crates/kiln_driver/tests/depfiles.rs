//! `-MD` / `-MMD` while compiling (spec/driver.md "Dependency output",
//! WP-578): `-fsyntax-only`, `-S`, `-c` and linking write a Make rule
//! beside their normal output, named and targeted as clang names them
//! (`-MF`, else the `-o` file with `.d`, else `<stem>.d`; `-MT`/`-MQ`,
//! else the `-o` value, else `<stem>.o`). The files are checked line by
//! line and, where `clang++-19` is installed and no system header is
//! listed, compared with its output byte for byte. Tests that run `llc`
//! or the linker skip without `llc-19` / `clang++-19`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / clang++-19 not found");
    }
    ok
}

/// `local.h` twice (the second skipped by `#pragma once`, still one
/// dependency), a system header, and a `__has_include` probe of a file
/// that exists but is never included (clang lists it; GCC does not).
const MAIN: &str = "#include \"local.h\"\n#include \"local.h\"\n#include <vector>\n\
                    #if __has_include(\"probe.h\")\n#endif\n\
                    #if __has_include(\"absent.h\")\n#endif\n\
                    int two();\nint main() { std::vector<int> v{local(), two()}; return v[0]; }\n";
const LOCAL: &str = "#pragma once\ninline int local() { return 0; }\n";
const TWO: &str = "#include \"two.h\"\nint two() { return TWO; }\n";

/// A scratch directory with the sources (relative names keep the rules
/// short of the 75-column wrap).
fn setup(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-depfiles-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("x")).unwrap();
    std::fs::write(d.join("main.cpp"), MAIN).unwrap();
    std::fs::write(d.join("local.h"), LOCAL).unwrap();
    std::fs::write(d.join("probe.h"), "").unwrap();
    std::fs::write(d.join("two.cpp"), TWO).unwrap();
    std::fs::write(d.join("two.h"), "#define TWO 2\n").unwrap();
    std::fs::write(d.join("bad.cpp"), "#include \"local.h\"\nint f() { return undeclared; }\n")
        .unwrap();
    std::fs::write(d.join("miss.cpp"), "#include \"local.h\"\n#include \"missing.h\"\n").unwrap();
    d
}

fn run(cc: &Path, dir: &Path, args: &[&str]) -> Output {
    Command::new(cc).args(args).current_dir(dir).output().unwrap()
}

fn kiln(dir: &Path, args: &[&str]) -> Output {
    let o = run(&kilnpp(), dir, args);
    assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
    o
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"))
}

/// The same command under clang++-19 writes the same file (when it is
/// installed); the file is removed first so a stale one cannot pass.
fn same_as_clang(dir: &Path, args: &[&str], file: &str, kiln_text: &str) {
    if !have("clang++-19") {
        return;
    }
    let _ = std::fs::remove_file(dir.join(file));
    let _ = run(Path::new("clang++-19"), dir, args);
    assert_eq!(read(dir, file), kiln_text, "clang++-19 {args:?}");
}

#[test]
fn mmd_with_mf_and_mt_omits_system_headers() {
    if !tools() {
        return;
    }
    let d = setup("mmd");
    let args = ["-c", "-MMD", "-MF", "out.d", "-MT", "main.o", "main.cpp"];
    kiln(&d, &args);
    let t = read(&d, "out.d");
    assert_eq!(t.lines().collect::<Vec<_>>(), ["main.o: main.cpp local.h probe.h"], "{t}");
    assert!(d.join("main.o").is_file());
    same_as_clang(&d, &args, "out.d", &t);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn md_lists_system_headers() {
    if !tools() {
        return;
    }
    let d = setup("md");
    kiln(&d, &["-c", "-MD", "main.cpp"]);
    let t = read(&d, "main.d");
    let words: Vec<&str> = t.split_whitespace().filter(|w| *w != "\\").collect();
    assert_eq!(words[..3], ["main.o:", "main.cpp", "local.h"], "{t}");
    // First-inclusion order: `<vector>` and its headers, then the probe.
    let vector = words.iter().position(|w| w.ends_with("/vector")).expect(&t);
    assert_eq!(words.iter().position(|w| *w == "probe.h"), Some(words.len() - 1), "{t}");
    assert!(vector > 2, "{t}");
    assert!(t.lines().skip(1).all(|l| l.starts_with("  ")), "{t}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn default_file_and_target_follow_the_output() {
    if !tools() {
        return;
    }
    let d = setup("default");
    let args = ["-c", "-o", "x/y.o", "-MMD", "main.cpp"];
    kiln(&d, &args);
    let t = read(&d, "x/y.d");
    assert_eq!(t, "x/y.o: main.cpp local.h probe.h\n");
    same_as_clang(&d, &args, "x/y.d", &t);
    let args = ["-S", "-o", "q.s", "-MMD", "two.cpp"];
    kiln(&d, &args);
    assert_eq!(read(&d, "q.d"), "q.s: two.cpp two.h\n");
    let args = ["-c", "-MMD", "-MQ", "a$b", "-MT", "c d", "two.cpp"];
    kiln(&d, &args);
    let t = read(&d, "two.d");
    assert_eq!(t, "a$$b c d: two.cpp two.h\n");
    same_as_clang(&d, &args, "two.d", &t);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn mp_adds_phony_rules() {
    if !tools() {
        return;
    }
    let d = setup("mp");
    let args = ["-c", "-MMD", "-MP", "main.cpp"];
    kiln(&d, &args);
    let t = read(&d, "main.d");
    assert_eq!(t, "main.o: main.cpp local.h probe.h\nlocal.h:\nprobe.h:\n");
    same_as_clang(&d, &args, "main.d", &t);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn syntax_only_mmd() {
    let d = setup("syntax");
    let args = ["-fsyntax-only", "-MMD", "main.cpp"];
    let o = kiln(&d, &args);
    assert_eq!(String::from_utf8_lossy(&o.stdout), "");
    let t = read(&d, "main.d");
    assert_eq!(t, "main.o: main.cpp local.h probe.h\n");
    same_as_clang(&d, &args, "main.d", &t);
    // `-MF -`: the rule on standard output.
    let o = kiln(&d, &["-fsyntax-only", "-MMD", "-MF", "-", "two.cpp"]);
    assert_eq!(String::from_utf8_lossy(&o.stdout), "two.o: two.cpp two.h\n");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn link_from_sources_writes_one_file_per_input() {
    if !tools() {
        return;
    }
    let d = setup("link");
    let args = ["-MMD", "main.cpp", "two.cpp"];
    kiln(&d, &args);
    let (m, t) = (read(&d, "main.d"), read(&d, "two.d"));
    assert_eq!(m, "main.o: main.cpp local.h probe.h\n");
    assert_eq!(t, "two.o: two.cpp two.h\n");
    assert!(d.join("a.out").is_file());
    same_as_clang(&d, &args, "main.d", &m);
    // With `-o`, every input's rule goes to `<output>.d` targeting the
    // output: the last input's survives (clang's naming).
    kiln(&d, &["-MMD", "main.cpp", "two.cpp", "-o", "prog"]);
    assert_eq!(read(&d, "prog.d"), "prog: two.cpp two.h\n");
    std::fs::remove_dir_all(&d).unwrap();
}

/// The rule is written once preprocessing finishes, whatever errors Sema
/// reports later (clang and GCC both keep it); a missing header writes
/// none (clang's `SeenMissingHeader`).
#[test]
fn errors_after_preprocessing_keep_the_file_missing_headers_do_not() {
    let d = setup("errors");
    let o = run(&kilnpp(), &d, &["-fsyntax-only", "-MMD", "bad.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(read(&d, "bad.d"), "bad.o: bad.cpp local.h\n");
    let o = run(&kilnpp(), &d, &["-fsyntax-only", "-MMD", "miss.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!d.join("miss.d").exists());
    if tools() {
        let o = run(&kilnpp(), &d, &["-c", "-MMD", "-MF", "c.d", "bad.cpp"]);
        assert_eq!(o.status.code(), Some(1));
        assert_eq!(read(&d, "c.d"), "bad.o: bad.cpp local.h\n");
        assert!(!d.join("bad.o").exists());
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// Each `#include` is listed by the path it found the file by, once per
/// distinct path, also when the file was not entered (clang's
/// `FileEntryRef` names and `FileSkipped`; WP-820): `#pragma once` and
/// guarded headers reached again through a symlinked directory or a `..`
/// path, and an `#include_next` chain. `__FILE__` and the line markers keep
/// the first path (the `-E` output is compared with clang's too).
#[cfg(unix)]
#[test]
fn every_path_a_header_is_reached_by_is_a_dependency() {
    let dir = std::env::temp_dir().join(format!("kiln-depfiles-{}-paths", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["real", "sub", "a", "b", "c"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::os::unix::fs::symlink("real", dir.join("lnk")).unwrap();
    let files = [
        ("real/once.h", "#pragma once\nconst char *once_file = __FILE__;\n"),
        (
            "real/guard.h",
            "#ifndef GUARD_H\n#define GUARD_H\nconst char *guard_file = __FILE__;\n#endif\n",
        ),
        ("real/plain.h", "__FILE__\n"),
        ("a/n.h", "#include_next <n.h>\nint a_n;\n"),
        ("b/n.h", "#include_next <n.h>\nint b_n;\n"),
        ("c/n.h", "int c_n;\n"),
        (
            "main.cpp",
            "#include \"real/once.h\"\n#include \"lnk/once.h\"\n#include \"sub/../real/once.h\"\n\
             #include \"lnk/guard.h\"\n#include \"real/guard.h\"\n#include \"./real/guard.h\"\n\
             const char *p[] = {\n#include \"lnk/plain.h\"\n,\n#include \"real/plain.h\"\n};\n\
             #include <n.h>\n#include <n.h>\n",
        ),
    ];
    for (f, text) in files {
        std::fs::write(dir.join(f), text).unwrap();
    }
    let args = ["-fsyntax-only", "-MMD", "-MF", "out.d", "-Ia", "-Ib", "-Ic", "main.cpp"];
    let out = run(&kilnpp(), &dir, &args);
    let deps = read(&dir, "out.d");
    assert_eq!(
        deps,
        "main.o: main.cpp real/once.h lnk/once.h sub/../real/once.h lnk/guard.h \\\n  \
         real/guard.h lnk/plain.h real/plain.h a/n.h b/n.h c/n.h\n",
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    same_as_clang(&dir, &args, "out.d", &deps);
    let e = kiln(&dir, &["-E", "-Ia", "-Ib", "-Ic", "main.cpp"]);
    let e = String::from_utf8(e.stdout).unwrap();
    // The second entry, by `real/plain.h`, keeps the first spelling.
    assert!(e.contains("# 1 \"./lnk/plain.h\" 1\n\"./lnk/plain.h\"\n# 11 \"main.cpp\" 2\n"), "{e}");
    if have("clang++-19") {
        let clang = run(Path::new("clang++-19"), &dir, &["-E", "-Ia", "-Ib", "-Ic", "main.cpp"]);
        let clang = String::from_utf8(clang.stdout).unwrap();
        // From the main file on (the predefines' length differs).
        let from_main = |s: &str| s[s.find("# 1 \"main.cpp\" 2").expect(s)..].to_owned();
        assert_eq!(from_main(&e), from_main(&clang), "-E differs from clang++-19");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
