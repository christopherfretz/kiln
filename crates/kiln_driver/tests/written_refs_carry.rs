//! WP-1934: the written-reference table (spec/ast.md §18) carried in PCH
//! snapshots and module images (stage 5, §18.10), end to end through
//! `kiln++`. No C++ standard section applies beyond [cpp.include] and
//! [module.import]: the table reports what the source names, and a
//! precompiled prefix or an imported unit must not change that report.
//!
//! - Every file of `tests/conformance/written_refs/` that compiles is
//!   split at a top-level boundary near its middle; the first part is
//!   precompiled with the table on and the rest compiled with
//!   `-include-pch`. Its dump must equal the dump of the same rest
//!   compiled with the first part as an `-include` (§18.10.1: the
//!   restored table continues the prefix's, with the same entries), and
//!   the snapshot must have been restored, not the header replayed.
//! - A precompiled header made with the table on, used by a compile that
//!   does not record, compiles as without it (§18.10.4).
//! - A module unit compiled without the table, imported from its image
//!   by a compile that records, is listed as unrecorded (§18.10.3).
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/written_refs")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-written-refs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(kilnpp())
        .args(args)
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

/// The `-std=` and `args:` flags of a corpus file's directives.
fn flags(src: &str) -> Vec<String> {
    let mut std = "c++26".to_owned();
    let mut out = Vec::new();
    for l in src.lines() {
        if let Some(s) = l.strip_prefix("//@ std:") {
            s.trim().clone_into(&mut std);
        } else if let Some(a) = l.strip_prefix("//@ args:") {
            out.extend(a.split_whitespace().map(str::to_owned));
        }
    }
    out.insert(0, format!("-std={std}"));
    out
}

/// A line's code: without a `//` comment and with string and character
/// literals blanked (enough for the corpus's braces).
fn code(line: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == '\\' => {
                let _ = (q, chars.next());
            }
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '/' && chars.peek() == Some(&'/') => break,
            None => out.push(c),
        }
    }
    out
}

/// The line index nearest the middle at which a top-level declaration
/// starts: brace and parenthesis depth 0, the code before ending in `;`
/// or `}`, the line itself starting a declaration. `None` for a file
/// with no such line after its first declaration.
fn split_point(src: &str) -> Option<usize> {
    let lines: Vec<&str> = src.lines().collect();
    let mut depth = 0i32;
    let mut last_code = String::new();
    let mut in_block = false;
    let mut cands = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let starts_decl = !l.starts_with([' ', '\t', '}', ')', ':', ',', '#', '/', '{'])
            && !l.is_empty()
            && !l.starts_with("else")
            && !l.starts_with("catch");
        if depth == 0
            && !in_block
            && starts_decl
            && (last_code.ends_with(';') || last_code.ends_with('}'))
        {
            cands.push(i);
        }
        if l.contains("/*") || l.contains("*/") {
            in_block = l.contains("/*") && !l.contains("*/");
        }
        let c = code(l);
        for ch in c.chars() {
            match ch {
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
        if !c.trim().is_empty() {
            c.trim().clone_into(&mut last_code);
        }
    }
    let mid = lines.len() / 2;
    cands.into_iter().min_by_key(|&i| i.abs_diff(mid))
}

/// Whether the corpus file is one this test splits: it compiles, and
/// has no module, PCH or `#include` lines of its own.
fn splittable(src: &str) -> bool {
    !src.contains("//@ expect: compile-fail")
        && !src.contains("//@ module:")
        && !src.contains("//@ pch:")
        && !src.lines().any(|l| l.trim_start().starts_with("#include"))
}

#[test]
fn corpus_through_a_pch_of_its_own_prefix() {
    let dir = scratch("corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    let mut failures = String::new();
    let mut checked = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        if !splittable(&src) {
            continue;
        }
        let Some(k) = split_point(&src) else {
            let _ = writeln!(failures, "{name}: no top-level split point");
            continue;
        };
        let lines: Vec<&str> = src.lines().collect();
        let mut prefix = lines[..k].join("\n");
        prefix.push('\n');
        // The rest keeps its line numbers (as a PCH user drops the
        // includes it holds).
        let mut rest = "\n".repeat(k);
        rest.push_str(&lines[k..].join("\n"));
        rest.push('\n');
        let h = format!("{name}.h");
        let main = format!("{name}.cpp");
        let pch = format!("{name}.pch");
        std::fs::write(dir.join(&h), prefix).unwrap();
        std::fs::write(dir.join(&main), rest).unwrap();
        let fl = flags(&src);
        let fl: Vec<&str> = fl.iter().map(String::as_str).collect();
        let dump = ["-fsyntax-only", "-fkiln-dump-written-refs"];
        let plain = kiln(&dir, &[&fl[..], &dump, &["-include", &h, &main]].concat());
        let made = kiln(
            &dir,
            &[&fl[..], &["-fkiln-written-refs", "-x", "c++-header", &h, "-o", &pch]].concat(),
        );
        if made.code != 0 || plain.code != 0 {
            let _ = writeln!(
                failures,
                "{name}: split at line {}: producer {} / plain {}:\n{}{}",
                k + 1,
                made.code,
                plain.code,
                made.stderr,
                plain.stderr
            );
            continue;
        }
        let used = kiln(&dir, &[&fl[..], &dump, &["-include-pch", &pch, &main]].concat());
        if !used.stderr.contains("snapshot-timing: restore") {
            let _ = writeln!(failures, "{name}: the snapshot was not restored:\n{}", used.stderr);
            continue;
        }
        if used.stdout != plain.stdout {
            let _ = writeln!(
                failures,
                "{name}: split at line {}: the dump through the PCH differs\n--- plain\n{}--- pch\n{}",
                k + 1,
                plain.stdout,
                used.stdout
            );
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{failures}");
    assert!(checked >= 40, "only {checked} corpus files were split");
}

const HEADER: &str = "namespace n { struct S { int v; }; inline int f(S s) { return s.v; } }\n";
const MAIN: &str = "int g() { return n::f(n::S{1}); }\n";

/// §18.10.4: a snapshot with the table, used by a compile that does not
/// record, is skipped: the compile is the one without the table.
#[test]
fn a_recorded_snapshot_serves_a_compile_that_does_not_record() {
    let dir = scratch("off-consumer");
    std::fs::write(dir.join("p.h"), HEADER).unwrap();
    std::fs::write(dir.join("m.cpp"), MAIN).unwrap();
    let made = kiln(&dir, &["-fkiln-written-refs", "-x", "c++-header", "p.h", "-o", "p.pch"]);
    assert_eq!(made.code, 0, "{}", made.stderr);
    let used = kiln(&dir, &["-fsyntax-only", "-include-pch", "p.pch", "m.cpp"]);
    assert_eq!(used.code, 0, "{}", used.stderr);
    assert!(used.stderr.contains("snapshot-timing: restore"), "{}", used.stderr);
    assert_eq!(used.stdout, "");
    // And one that does record sees the prefix's names first.
    let dump = kiln(
        &dir,
        &["-fsyntax-only", "-fkiln-dump-written-refs", "-include-pch", "p.pch", "m.cpp"],
    );
    assert_eq!(dump.code, 0, "{}", dump.stderr);
    assert!(dump.stdout.starts_with("p.h:1:"), "{}", dump.stdout);
    assert!(dump.stdout.contains("\n1:21-1:22  expr  'f' -> n::f@p.h:1:47"), "{}", dump.stdout);
}

const MODULE: &str = "export module rm;\nexport namespace rm { inline int one() { return 1; } }\n\
                      export inline int two() { return rm::one() + 1; }\n";
const IMPORTER: &str = "import rm;\nint g() { return rm::one(); }\n";

/// §18.10.3: a unit compiled without the table, imported from its image
/// by a compile that records, is a listed gap, and the importer's own
/// names are recorded; a unit compiled with it is merged.
#[test]
fn a_module_compiled_without_the_table_is_unrecorded() {
    let dir = scratch("module-off");
    std::fs::write(dir.join("rm.cppm"), MODULE).unwrap();
    std::fs::write(dir.join("use.cpp"), IMPORTER).unwrap();
    for (on, pcm) in [(false, "off.pcm"), (true, "on.pcm")] {
        let mut args = vec!["-std=c++20", "-fkiln-bmi-image", "--precompile"];
        if on {
            args.push("-fkiln-written-refs");
        }
        args.extend(["-x", "c++-module", "rm.cppm", "-o", pcm]);
        let made = kiln(&dir, &args);
        assert_eq!(made.code, 0, "{}", made.stderr);
        let file = format!("-fmodule-file=rm={pcm}");
        let used = kiln(
            &dir,
            &[
                "-std=c++20",
                "-fsyntax-only",
                "-v",
                "-fkiln-bmi-image",
                "-fkiln-dump-written-refs",
                &file,
                "use.cpp",
            ],
        );
        assert_eq!(used.code, 0, "{}", used.stderr);
        assert!(
            used.stderr.contains("imported 'rm' from its declaration image"),
            "{}",
            used.stderr
        );
        let first = used.stdout.lines().next().unwrap_or_default();
        if on {
            assert!(first.starts_with("rm.cppm:3:"), "{}", used.stdout);
        } else {
            assert!(first.starts_with("unrecorded  module rm (rm.cppm, "), "{}", used.stdout);
        }
        assert!(used.stdout.contains("2:18-2:20  nns  'rm' -> rm@rm.cppm:2:18"), "{}", used.stdout);
        // A compile that does not record skips the section either way.
        let quiet =
            kiln(&dir, &["-std=c++20", "-fsyntax-only", "-fkiln-bmi-image", &file, "use.cpp"]);
        assert_eq!((quiet.code, quiet.stdout.as_str()), (0, ""), "{}", quiet.stderr);
    }
}

/// §18.8 (WP-2170): the names of a thread-safety attribute's arguments
/// are entries but not pattern nodes (no instantiation rebuilds them), so
/// a dependent or undeclared one in a module unit compiled with the
/// table on does not cost the unit its image.
#[test]
fn thread_safety_arguments_keep_the_image() {
    let dir = scratch("module-tsa");
    let module = "export module tm;\n\
                  export struct M { int mu; };\n\
                  export template <class T> struct G {\n\
                  \x20 int x __attribute__((guarded_by(T::mu)));\n\
                  \x20 int y __attribute__((guarded_by(no_such_mutex)));\n\
                  };\n";
    std::fs::write(dir.join("tm.cppm"), module).unwrap();
    std::fs::write(dir.join("use.cpp"), "import tm;\nG<M> g;\n").unwrap();
    let made = kiln(
        &dir,
        &[
            "-std=c++20",
            "-fkiln-bmi-image",
            "-fkiln-written-refs",
            "--precompile",
            "-x",
            "c++-module",
            "tm.cppm",
            "-o",
            "tm.pcm",
        ],
    );
    assert_eq!(made.code, 0, "{}", made.stderr);
    let used = kiln(
        &dir,
        &[
            "-std=c++20",
            "-fsyntax-only",
            "-v",
            "-fkiln-bmi-image",
            "-fkiln-dump-written-refs",
            "-fmodule-file=tm=tm.pcm",
            "use.cpp",
        ],
    );
    assert_eq!(used.code, 0, "{}", used.stderr);
    assert!(used.stderr.contains("imported 'tm' from its declaration image"), "{}", used.stderr);
    assert!(used.stdout.contains("tm.cppm:4:35-4:36  nns  'T'"), "{}", used.stdout);
}

/// A dump without the substitution keys' own lines (spec/ast.md
/// §18.14.6): the keyed rows, the `keys` block and the key gaps.
fn without_key_lines(dump: &str) -> String {
    let is_key_line = |l: &str| {
        l.contains(" key=")
            || l.starts_with("keys  ")
            || (l.starts_with("unrecorded  ") && l.contains(" keys "))
    };
    let mut out = String::with_capacity(dump.len());
    for l in dump.lines().filter(|l| !is_key_line(l)) {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// spec/ast.md §18.14.8: the sub-switch `-fkiln-written-ref-keys`
/// changes no line of the dump but its own, on every compiling file of
/// the corpus: with keys on, the only lines added are key lines.
#[test]
fn keys_leave_the_corpus_dump_unchanged() {
    let dir = scratch("keys-corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    let mut failures = String::new();
    let mut checked = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        if src.contains("//@ expect: compile-fail")
            || src.contains("//@ module:")
            || src.contains("//@ pch:")
        {
            continue;
        }
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let path = f.to_string_lossy().into_owned();
        let fl = flags(&src);
        let fl: Vec<&str> = fl.iter().map(String::as_str).collect();
        // Off: a file whose `args:` turn keys on (subst-partial-*.cpp) is
        // compiled with the table alone.
        let plain: Vec<&str> = fl
            .iter()
            .map(|&a| if a == "-fkiln-written-ref-keys" { "-fkiln-written-refs" } else { a })
            .collect();
        let dump = ["-fsyntax-only", "-fkiln-dump-written-refs"];
        let off = kiln(&dir, &[&plain[..], &dump, &[&path]].concat());
        let on = kiln(&dir, &[&fl[..], &dump, &["-fkiln-written-ref-keys", &path]].concat());
        if off.code != 0 || on.code != 0 {
            let _ = writeln!(failures, "{name}: exit {} / {}:\n{}", off.code, on.code, on.stderr);
            continue;
        }
        if without_key_lines(&on.stdout) != off.stdout {
            let _ = writeln!(
                failures,
                "{name}: the dump with keys differs\n--- off\n{}--- on\n{}",
                off.stdout, on.stdout
            );
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{failures}");
    assert!(checked >= 40, "only {checked} corpus files were compared");
}

const TEMPLATED: &str = "namespace n { struct S { using t = int; }; }\n\
                         template <class T> using A = typename T::t;\n\
                         template <class T> int f(T x) { return g(x); }\n";
const TEMPLATED_MAIN: &str = "A<n::S> v = 0;\n";

/// spec/ast.md §18.14.5: a snapshot made with the table but without
/// keys, restored by a compile that records keys, is a `PrefixKeys` gap
/// (printed first); one made with keys is none; and a compile that
/// records the table without keys sees no key gap.
#[test]
fn a_prefix_without_keys_is_a_key_gap() {
    let dir = scratch("keys-prefix");
    std::fs::write(dir.join("p.h"), TEMPLATED).unwrap();
    std::fs::write(dir.join("m.cpp"), format!("\n\n\n{TEMPLATED_MAIN}")).unwrap();
    let dump = ["-fsyntax-only", "-fkiln-dump-written-refs", "-include-pch"];
    for (producer, pch) in [
        (&["-fkiln-written-refs"][..], "plain.pch"),
        (&["-fkiln-written-ref-keys"][..], "keys.pch"),
    ] {
        let made = kiln(&dir, &[producer, &["-x", "c++-header", "p.h", "-o", pch]].concat());
        assert_eq!(made.code, 0, "{}", made.stderr);
        let keyed = kiln(&dir, &[&dump[..], &[pch, "-fkiln-written-ref-keys", "m.cpp"]].concat());
        assert_eq!(keyed.code, 0, "{}", keyed.stderr);
        assert!(keyed.stderr.contains("snapshot-timing: restore"), "{}", keyed.stderr);
        let first = keyed.stdout.lines().next().unwrap_or_default();
        if pch == "plain.pch" {
            assert!(first.starts_with("unrecorded  precompiled prefix keys ("), "{}", keyed.stdout);
        } else {
            assert!(!keyed.stdout.contains("unrecorded"), "{}", keyed.stdout);
        }
        let plain = kiln(&dir, &[&dump[..], &[pch, "m.cpp"]].concat());
        assert_eq!(plain.code, 0, "{}", plain.stderr);
        assert!(!plain.stdout.contains("unrecorded"), "{}", plain.stdout);
        assert_eq!(without_key_lines(&keyed.stdout), plain.stdout);
    }
}

/// spec/ast.md §18.14.5, §18.14.10: a snapshot of a prefix with a
/// partial specialization match, made without keys, restored by a
/// compile that records keys: a `PrefixKeys` gap, and the only key is
/// the main file's new match (with its root); the prefix's match, used
/// again, has none. Made with keys, both matches have keys.
#[test]
fn a_prefix_without_keys_has_no_partial_match_keys() {
    let dir = scratch("keys-partial-prefix");
    let header = std::fs::read_to_string(corpus().join("Inputs/subst_partial_pch.h")).unwrap();
    std::fs::write(dir.join("p.h"), header).unwrap();
    std::fs::write(
        dir.join("m.cpp"),
        "bool again = can_swap<sp::S>::value;\nbool anew = can_swap<sp::V>::value;\n",
    )
    .unwrap();
    let key = |dump: &str, of: &str| {
        dump.lines().any(|l| l.starts_with("keys  ") && l.contains(&format!("<sp::{of}, void>")))
    };
    for (producer, pch) in
        [("-fkiln-written-refs", "plain.pch"), ("-fkiln-written-ref-keys", "keys.pch")]
    {
        let made = kiln(&dir, &["-std=c++26", producer, "-x", "c++-header", "p.h", "-o", pch]);
        assert_eq!(made.code, 0, "{}", made.stderr);
        let keyed = kiln(
            &dir,
            &[
                "-std=c++26",
                "-fsyntax-only",
                "-fkiln-dump-written-refs",
                "-fkiln-written-ref-keys",
                "-include-pch",
                pch,
                "m.cpp",
            ],
        );
        assert_eq!(keyed.code, 0, "{}", keyed.stderr);
        let first = keyed.stdout.lines().next().unwrap_or_default();
        let plain = pch == "plain.pch";
        assert_eq!(
            first.starts_with("unrecorded  precompiled prefix keys ("),
            plain,
            "{}",
            keyed.stdout
        );
        assert_eq!(key(&keyed.stdout, "S"), !plain, "{}", keyed.stdout);
        assert!(key(&keyed.stdout, "V"), "{}", keyed.stdout);
        assert!(
            keyed.stdout.lines().any(|l| l.contains("<sp::V, void> root[1]=")),
            "{}",
            keyed.stdout
        );
    }
}

/// spec/ast.md §18.14.5: a module unit compiled with the table but
/// without keys, imported from its image by a compile that records keys,
/// is a `ModuleKeys` gap; one compiled with keys is none, and its names
/// are merged as without keys.
#[test]
fn a_module_compiled_without_keys_is_a_key_gap() {
    let dir = scratch("keys-module");
    std::fs::write(dir.join("rm.cppm"), MODULE).unwrap();
    std::fs::write(dir.join("use.cpp"), IMPORTER).unwrap();
    for (flag, pcm) in
        [("-fkiln-written-refs", "plain.pcm"), ("-fkiln-written-ref-keys", "keys.pcm")]
    {
        let made = kiln(
            &dir,
            &[
                "-std=c++20",
                "-fkiln-bmi-image",
                "--precompile",
                flag,
                "-x",
                "c++-module",
                "rm.cppm",
                "-o",
                pcm,
            ],
        );
        assert_eq!(made.code, 0, "{}", made.stderr);
        let file = format!("-fmodule-file=rm={pcm}");
        let used = kiln(
            &dir,
            &[
                "-std=c++20",
                "-fsyntax-only",
                "-v",
                "-fkiln-bmi-image",
                "-fkiln-dump-written-refs",
                "-fkiln-written-ref-keys",
                &file,
                "use.cpp",
            ],
        );
        assert_eq!(used.code, 0, "{}", used.stderr);
        assert!(
            used.stderr.contains("imported 'rm' from its declaration image"),
            "{}",
            used.stderr
        );
        let first = used.stdout.lines().next().unwrap_or_default();
        if pcm == "plain.pcm" {
            assert!(first.starts_with("unrecorded  module rm keys (rm.cppm, "), "{}", used.stdout);
        } else {
            assert!(first.starts_with("rm.cppm:3:"), "{}", used.stdout);
        }
        assert!(used.stdout.contains("2:18-2:20  nns  'rm' -> rm@rm.cppm:2:18"), "{}", used.stdout);
    }
}
