//! The PCH snapshot with a consumer whose include search path differs
//! from the producer's (WP-1403, spec/pch.md "Lookup-tolerant search
//! path" and "Refused snapshots are visible"), end to end through
//! `kiln++`: the corner cases of the lookup comparison, the consumer's
//! own search list in force after a restore, the warning a refused
//! snapshot gives, and the producer's single pass with each mode that
//! writes a snapshot.
//!
//! Every refusal replays the header with the consumer's search list
//! ([cpp.include]/2: as if the header were included), so each refused
//! compile is also checked to have the consumer's result.
#![forbid(unsafe_code)]
#![allow(clippy::many_single_char_names, reason = "d(ir), o(utput), r(eplay), s(napshot)")]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// A fresh scratch directory with `files` (parents created).
fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pchsearch-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (f, t) in files {
        let p = d.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if f.ends_with('/') {
            std::fs::create_dir_all(&p).unwrap();
        } else {
            std::fs::write(p, t).unwrap();
        }
    }
    d
}

struct Out {
    code: i32,
    stderr: String,
}

fn run_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Out {
    let mut c = Command::new(kilnpp());
    c.args(args).current_dir(dir).env("KILN_SNAP_TIMING", "1").stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out { code: o.status.code().unwrap_or(-1), stderr }
}

fn run(dir: &Path, args: &[&str]) -> Out {
    run_env(dir, args, &[])
}

/// Builds `h.pch` from `header` with `flags` and a snapshot.
fn produce(d: &Path, flags: &[&str], header: &str) {
    let mut a: Vec<&str> = flags.to_vec();
    a.extend(["-fkiln-pch-snapshot=write", "-v", "-x", "c++-header", header, "-o", "h.pch"]);
    let o = run(d, &a);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains("snapshot written"), "{}", o.stderr);
}

/// The consumer's command line: `flags`, `h.pch` (with the snapshot
/// loaded when `snap`), `-fsyntax-only main`.
fn consumer<'a>(flags: &[&'a str], snap: bool, main: &'a str) -> Vec<&'a str> {
    let mut a: Vec<&str> = flags.to_vec();
    a.extend(["-include-pch", "h.pch"]);
    if snap {
        a.extend(["-fkiln-pch-snapshot=load", "-v"]);
    }
    a.extend(["-fsyntax-only", main]);
    a
}

fn restored(o: &Out) -> bool {
    o.stderr.lines().any(|l| l.starts_with("snapshot-timing: restore "))
}

fn repeated(o: &Out) -> bool {
    o.stderr
        .lines()
        .any(|l| l.starts_with("snapshot-timing: find ") && l.contains("lookups repeated"))
}

/// The `-v` note after `snapshot `.
fn note(o: &Out) -> &str {
    o.stderr
        .lines()
        .find_map(|l| l.split_once("': snapshot ").map(|(_, w)| w))
        .unwrap_or_else(|| panic!("no snapshot note:\n{}", o.stderr))
}

fn warnings(o: &Out) -> Vec<&str> {
    o.stderr.lines().filter(|l| l.contains("pch-snapshot-unused")).collect()
}

/// The stderr without the `-v` banner and notes and the timing lines.
fn diags(o: &Out) -> String {
    let e = o.stderr.as_str();
    let e = e.split_once("End of search list.\n").map_or(e, |(_, r)| r);
    e.lines()
        .filter(|l| {
            !l.starts_with("snapshot-timing: ")
                && !l.starts_with("note: precompiled header")
                && !l.contains("-Wpch-snapshot-unused")
                && !l.ends_with(" generated.")
        })
        .fold(String::new(), |mut out, l| {
            out.push_str(l);
            out.push('\n');
            out
        })
}

/// The prefix: a header from `inc`, one beside it, and a
/// `__has_include` that fails.
const PREFIX: &[(&str, &str)] = &[
    (
        "p.h",
        "#include <v.h>\n#include \"q.h\"\n#if __has_include(<opt.h>)\n#define OPT 1\n#else\n#define OPT 0\n#endif\n",
    ),
    ("q.h", "#define Q 3\n"),
    ("inc/v.h", "#define V 1\nint v();\n"),
    ("t.cpp", "static_assert(V == 1 && Q == 3 && OPT == 0);\nint main() {}\n"),
    ("extra/", ""),
    ("root/", ""),
];

const STD: &str = "-std=c++20";

/// A compile with `flags`: restored, its lookups repeated when `repeat`,
/// no warning, and the same diagnostics as the replay.
fn restores(d: &Path, flags: &[&str], repeat: bool) {
    let s = run(d, &consumer(flags, true, "t.cpp"));
    assert_eq!(s.code, 0, "{flags:?}:\n{}", s.stderr);
    assert!(restored(&s), "{flags:?}:\n{}", s.stderr);
    assert_eq!(repeated(&s), repeat, "{flags:?}:\n{}", s.stderr);
    assert!(warnings(&s).is_empty(), "{flags:?}:\n{}", s.stderr);
    let r = run(d, &consumer(flags, false, "t.cpp"));
    assert_eq!(diags(&s), diags(&r), "{flags:?}");
}

/// A compile of `main` with `flags`: refused for `why` (the `-v` note
/// after `not used: `), with the warning, and the replay's result.
fn refuses(d: &Path, flags: &[&str], main: &str, why: &str) -> Out {
    let s = run(d, &consumer(flags, true, main));
    assert!(!restored(&s), "{flags:?}:\n{}", s.stderr);
    assert_eq!(note(&s), format!("not used: {why}"), "{flags:?}");
    assert_eq!(
        warnings(&s),
        [format!(
            "warning: precompiled header 'h.pch' was built with a different configuration ({why}); the header is processed again [-Wpch-snapshot-unused]"
        )],
        "{flags:?}:\n{}",
        s.stderr
    );
    let r = run(d, &consumer(flags, false, main));
    assert_eq!((s.code, diags(&s)), (r.code, diags(&r)), "{flags:?}");
    s
}

/// Directories the prefix does not find anything in restore, wherever
/// they go and whatever group they are in; `-iquote`, `-idirafter`,
/// `--sysroot`, `-nostdinc*` change the list the same way.
#[test]
fn unrelated_directories_restore() {
    let d = tree("unrelated", PREFIX);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    restores(&d, &[STD, "-I", "inc"], false);
    restores(&d, &[STD, "-I", "extra", "-I", "inc"], true);
    restores(&d, &[STD, "-I", "inc", "-I", "extra"], true);
    restores(&d, &[STD, "-iquote", "extra", "-I", "inc"], true);
    restores(&d, &[STD, "-I", "inc", "-idirafter", "extra"], true);
    restores(&d, &[STD, "-I", "inc", "-isystem", "extra"], true);
    restores(&d, &[STD, "-I", "inc", "--sysroot=root"], true);
    restores(&d, &[STD, "-I", "inc", "-nostdinc++"], true);
    restores(&d, &[STD, "-I", "inc", "-nostdinc"], true);
    // A reorder that keeps every result: `extra` holds nothing asked for.
    produce(&d, &[STD, "-I", "inc", "-I", "extra"], "p.h");
    restores(&d, &[STD, "-I", "extra", "-I", "inc"], true);
    // kiln reads no CPATH-style variables: the same list, the fast path.
    std::fs::write(d.join("extra/v.h"), "#define V 2\n").unwrap();
    for var in ["CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH"] {
        let s = run_env(
            &d,
            &consumer(&[STD, "-I", "inc", "-I", "extra"], true, "t.cpp"),
            &[(var, "extra")],
        );
        assert!(restored(&s) && !repeated(&s), "{var}:\n{}", s.stderr);
    }
}

/// A directory that shadows a prefix header refuses; the replay
/// re-resolves the header's includes with the consumer's list and finds
/// the shadowing header (the main file sees `V == 2`).
#[test]
fn shadowing_refuses_and_the_replay_re_resolves() {
    let mut files = PREFIX.to_vec();
    files.push(("shadow/v.h", "#define V 2\nint v();\n"));
    files.push(("t2.cpp", "static_assert(V == 2 && Q == 3);\nint main() {}\n"));
    let d = tree("shadow", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let o = refuses(
        &d,
        &[STD, "-I", "shadow", "-I", "inc"],
        "t2.cpp",
        "include search path: lookup of <v.h> now finds 'shadow/v.h' (was 'inc/v.h')",
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    // The same header as a system header.
    refuses(
        &d,
        &[STD, "-isystem", "inc"],
        "t.cpp",
        "include search path: lookup of <v.h> now finds 'inc/v.h' as a system header (was a user header)",
    );
    // Without the directory at all: the replay's error is the result.
    let o = refuses(
        &d,
        &[STD],
        "t.cpp",
        "include search path: lookup of <v.h> now finds nothing (was 'inc/v.h')",
    );
    assert_ne!(o.code, 0);
}

/// A `__has_include` that failed in the prefix and now succeeds refuses.
#[test]
fn has_include_that_now_succeeds_refuses() {
    let mut files = PREFIX.to_vec();
    files.push(("opt/opt.h", "\n"));
    files.push(("t2.cpp", "static_assert(OPT == 1);\nint main() {}\n"));
    let d = tree("hasinc", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let o = refuses(
        &d,
        &[STD, "-I", "inc", "-I", "opt"],
        "t2.cpp",
        "include search path: lookup of <opt.h> now finds 'opt/opt.h' (was not found)",
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
}

/// `#include_next` in the prefix continues after the same directory.
#[test]
fn include_next_in_the_prefix() {
    let d = tree(
        "next",
        &[
            ("p.h", "#include <n.h>\n"),
            ("a/n.h", "#include_next <n.h>\n"),
            ("b/n.h", "#define N 1\n"),
            ("c/n.h", "#define N 2\n"),
            ("extra/", ""),
            ("t.cpp", "static_assert(N == 1);\nint main() {}\n"),
            ("t2.cpp", "static_assert(N == 2);\nint main() {}\n"),
        ],
    );
    produce(&d, &[STD, "-I", "a", "-I", "b"], "p.h");
    restores(&d, &[STD, "-I", "a", "-I", "extra", "-I", "b"], true);
    // A directory between the two that holds `n.h` changes the second
    // lookup only.
    refuses(
        &d,
        &[STD, "-I", "a", "-I", "c", "-I", "b"],
        "t2.cpp",
        "include search path: lookup of #include_next <n.h> now finds 'c/n.h' (was 'b/n.h')",
    );
}

/// `#embed` and `__has_embed` in the prefix are repeated with the
/// consumer's `--embed-dir` list.
#[test]
fn embed_lookups() {
    let d = tree(
        "embed",
        &[
            ("p.h", "#if __has_embed(<d.bin>)\nconst char d[] = {\n#embed <d.bin>\n};\n#endif\n"),
            ("res/d.bin", "Z"),
            ("other/d.bin", "Y"),
            ("extra/", ""),
            ("t.cpp", "static_assert(sizeof d == 1);\nint main() { return d[0] - 'Z'; }\n"),
        ],
    );
    let quiet = [STD, "-Wno-everything"];
    produce(&d, &[STD, "-Wno-everything", "--embed-dir=res"], "p.h");
    restores(&d, &[STD, "-Wno-everything", "--embed-dir=res"], false);
    restores(&d, &[STD, "-Wno-everything", "--embed-dir=extra", "--embed-dir=res"], true);
    let mut a = quiet.to_vec();
    a.extend(["--embed-dir=other", "--embed-dir=res"]);
    let s = run(&d, &consumer(&a, true, "t.cpp"));
    assert!(!restored(&s), "{}", s.stderr);
    assert_eq!(
        note(&s),
        "not used: include search path: lookup of #embed <d.bin> now finds 'other/d.bin' (was 'res/d.bin')"
    );
}

/// A relative `-I` in another working directory: the prefix depends on
/// the working directory, so it refuses there. A prefix built from
/// absolute paths restores from any directory, its lookups repeated.
#[test]
fn working_directory() {
    let mut files = PREFIX.to_vec();
    files.push(("sub/", ""));
    let d = tree("cwd", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let sub = d.join("sub");
    let s = run(
        &sub,
        &[
            STD,
            "-I",
            "../inc",
            "-include-pch",
            "../h.pch",
            "-fkiln-pch-snapshot=load",
            "-v",
            "-fsyntax-only",
            "../t.cpp",
        ],
    );
    assert!(!restored(&s), "{}", s.stderr);
    assert!(note(&s).starts_with("not used: working directory differs"), "{}", s.stderr);
    assert_eq!(warnings(&s).len(), 1, "{}", s.stderr);
    // Absolute: the header, `-I` and the PCH.
    let abs = |p: &str| d.join(p).display().to_string();
    let (inc, header, pch, main) = (abs("inc"), abs("p.h"), abs("h.pch"), abs("t.cpp"));
    let o = run(
        &d,
        &[STD, "-I", &inc, "-fkiln-pch-snapshot=write", "-x", "c++-header", &header, "-o", &pch],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let a = [
        STD,
        "-I",
        &inc,
        "-include-pch",
        &pch,
        "-fkiln-pch-snapshot=load",
        "-v",
        "-fsyntax-only",
        &main,
    ];
    let s = run(&sub, &a);
    assert_eq!(s.code, 0, "{}", s.stderr);
    assert!(restored(&s) && repeated(&s), "{}", s.stderr);
    let r = run(&sub, &[STD, "-I", &inc, "-include-pch", &pch, "-fsyntax-only", &main]);
    assert_eq!(diags(&s), diags(&r));
    // From the producer's own directory nothing is repeated.
    let s = run(&d, &a);
    assert!(restored(&s) && !repeated(&s), "{}", s.stderr);
}

/// After a restore the consumer's search list is in force for the main
/// file: a header only its `-I` finds, a header beside the main file
/// with a prefix header's name, `#include_next` and `__has_include` in
/// the main file, and the `-MD` list.
#[test]
fn the_consumers_list_is_in_force() {
    let mut files = PREFIX.to_vec();
    files.extend([
        ("extra/only.h", "#define ONLY 5\n"),
        ("extra/nx.h", "#define NX 6\n"),
        ("src/q.h", "#define MQ 7\n"),
        (
            "src/m.cpp",
            "#include <only.h>\n#include \"q.h\"\n#include_next <nx.h>\n\
             #if !__has_include(<only.h>) || __has_include(<nope.h>)\n#error lookups\n#endif\n\
             static_assert(ONLY == 5 && MQ == 7 && NX == 6 && Q == 3 && V == 1);\nint main() {}\n",
        ),
    ]);
    let d = tree("inforce", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let dep = |snap: bool, f: &str| {
        let mut a = consumer(&[STD, "-I", "inc", "-I", "extra"], snap, "src/m.cpp");
        a.extend(["-MD", "-MF", f, "-MT", "m.o"]);
        let o = run(&d, &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        let list = std::fs::read_to_string(d.join(f)).unwrap().replace("\\\n", "");
        (o, list.split_whitespace().collect::<Vec<_>>().join(" "))
    };
    let (s, sdeps) = dep(true, "s.d");
    let (r, rdeps) = dep(false, "r.d");
    assert!(restored(&s) && repeated(&s), "{}", s.stderr);
    assert_eq!(diags(&s), diags(&r));
    assert!(diags(&s).contains("#include_next in primary source file"), "{}", s.stderr);
    assert_eq!(sdeps, rdeps);
    assert_eq!(sdeps, "m.o: p.h inc/v.h q.h src/m.cpp extra/only.h src/q.h extra/nx.h");
}

/// The warning: on by default (above), `-Wno-` turns it off,
/// `-Werror=pch-snapshot-unused` makes it an error, and a blanket
/// `-Werror` does not (WP-1397: the group is exempt, spec/pch.md
/// "Consumer").
#[test]
fn the_warning_is_controllable() {
    let mut files = PREFIX.to_vec();
    files.push(("shadow/v.h", "#define V 1\nint v();\n"));
    let d = tree("controls", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let shadow = [STD, "-I", "shadow", "-I", "inc"];
    let with = |extra: &[&str]| {
        let mut a = shadow.to_vec();
        a.extend_from_slice(extra);
        run(&d, &consumer(&a, true, "t.cpp"))
    };
    let o = with(&["-Wno-pch-snapshot-unused"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(warnings(&o).is_empty(), "{}", o.stderr);
    let o = with(&["-Werror=pch-snapshot-unused"]);
    assert_ne!(o.code, 0, "{}", o.stderr);
    assert!(
        warnings(&o).iter().any(|l| l.starts_with("error: precompiled header 'h.pch'")
            && l.ends_with("[-Werror,-Wpch-snapshot-unused]")),
        "{}",
        o.stderr
    );
    let o = with(&["-Werror"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let w = warnings(&o);
    assert_eq!(w.len(), 1, "{}", o.stderr);
    assert!(
        w[0].starts_with("warning: precompiled header 'h.pch'")
            && w[0].ends_with(" [-Wpch-snapshot-unused]"),
        "{}",
        o.stderr
    );
}

/// The cases that replay by design do not warn.
#[test]
fn by_design_replays_do_not_warn() {
    let mut files = PREFIX.to_vec();
    files.push(("shadow/v.h", "#define V 1\nint v();\n"));
    files.push(("mod.cpp", "export module m;\n"));
    let d = tree("bydesign", &files);
    produce(&d, &[STD, "-I", "inc"], "p.h");
    let shadow = [STD, "-I", "shadow", "-I", "inc", "-include-pch", "h.pch"];
    let quiet = |extra: &[&str]| {
        let mut a = shadow.to_vec();
        a.extend_from_slice(extra);
        let o = run(&d, &a);
        assert!(warnings(&o).is_empty(), "{extra:?}:\n{}", o.stderr);
        assert!(!restored(&o), "{extra:?}:\n{}", o.stderr);
    };
    // WP-1404: no option is the snapshot (it warns, above); `-fno-` is
    // the replay by design.
    quiet(&["-fno-kiln-pch-snapshot", "-fsyntax-only", "t.cpp"]);
    quiet(&["-fkiln-pch-snapshot", "-fno-kiln-pch-snapshot", "-fsyntax-only", "t.cpp"]);
    quiet(&["-fkiln-pch-snapshot=write", "-fsyntax-only", "t.cpp"]);
    quiet(&["-fkiln-pch-snapshot=check", "-fsyntax-only", "t.cpp"]);
    quiet(&["-fkiln-pch-snapshot=load", "-E", "t.cpp", "-o", "t.i"]);
    quiet(&["-fkiln-pch-snapshot=load", "-M", "t.cpp"]);
    quiet(&[
        "-fkiln-pch-snapshot=load",
        "-std=c++20",
        "--precompile",
        "-x",
        "c++-module",
        "mod.cpp",
        "-o",
        "m.pcm",
    ]);
    // A PCH without a snapshot section (`-fno-` producer, WP-1404).
    let o = run(
        &d,
        &[STD, "-fno-kiln-pch-snapshot", "-I", "inc", "-x", "c++-header", "p.h", "-o", "plain.pch"],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let o = run(
        &d,
        &[
            STD,
            "-I",
            "inc",
            "-include-pch",
            "plain.pch",
            "-fkiln-pch-snapshot=load",
            "-fsyntax-only",
            "t.cpp",
        ],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(warnings(&o).is_empty() && !restored(&o), "{}", o.stderr);
}

/// WP-1393 single pass: every mode that writes a snapshot (the bare
/// switch, `=write`, `=check`) takes it, and each time the header pass
/// is needed the timing line names the rule.
#[test]
fn single_pass_modes_and_reasons() {
    let d = tree(
        "single",
        &[
            ("p.h", "/*\nimport x;\n*/\nint p();\n"),
            ("sys.h", "int s();\n#pragma system_header\n"),
            ("lvl.h", "#include \"dep.h\"\n"),
            ("dep.h", "int lvl = __INCLUDE_LEVEL__;\n"),
        ],
    );
    let timing = |flags: &[&str], header: &str| {
        let mut a = vec![STD];
        a.extend_from_slice(flags);
        a.extend(["-x", "c++-header", header, "-o", "s.pch"]);
        let o = run(&d, &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        o.stderr
            .lines()
            .find_map(|l| l.strip_prefix("snapshot-timing: producer single pass: "))
            .unwrap_or_else(|| panic!("{flags:?}: no single-pass line:\n{}", o.stderr))
            .to_owned()
    };
    let stands = "stands in for the header pass";
    for mode in ["-fkiln-pch-snapshot", "-fkiln-pch-snapshot=write", "-fkiln-pch-snapshot=check"] {
        // A block comment's `import` line is not a module line.
        assert_eq!(timing(&[mode], "p.h"), stands, "{mode}");
    }
    assert_eq!(timing(&["-fkiln-pch-snapshot", "-v"], "p.h"), "not attempted (-v)");
    assert!(
        timing(&["-fkiln-pch-snapshot"], "sys.h").starts_with(
            "header pass needed (line 2 of the header is read differently in a main file"
        ),
        "{}",
        timing(&["-fkiln-pch-snapshot"], "sys.h")
    );
    assert_eq!(
        timing(&["-fkiln-pch-snapshot"], "lvl.h"),
        "header pass needed (dependency 'dep.h' uses __INCLUDE_LEVEL__ or __BASE_FILE__)"
    );
}
