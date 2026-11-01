//! The driver-level IR-diff corpus (spec/codegen.md §6, "IR-diff corpus").
//!
//! Every `tests/codegen/*.cpp` is compiled in-process by the kiln driver,
//! as `kiln++` (`kiln_driver::run`), with
//! `-S -emit-llvm -O0 -std=c++26 --target=<triple>` for each triple in
//! [`TRIPLES`], and by `clang++-19` with the same flags. Both outputs go
//! through `opt-19 -S -passes=verify` and [`normalize`] (as in `lower.rs`)
//! and must be byte-identical, unless the (case, triple) pair is listed in
//! [`DEVIATIONS`] (spec/codegen.md "Deviations"), in which case kiln's IR
//! must still verify. Without `opt-19` / `clang++-19` the comparison is
//! skipped.
//!
//! `KILN_CASE=<name>` (the file stem) runs one case and prints both IR
//! texts for each triple. A `//@ args: <opts>` line in a case adds
//! `<opts>` (whitespace-separated) to both command lines, after the
//! [`pinned_flags`].

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run};

const TRIPLES: &[&str] = &["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];

/// `(case, triple)` pairs whose IR differs from clang's by a documented
/// deviation: they must verify but need not match.
const DEVIATIONS: &[(&str, &str)] = &[
    // clang 19's AArch64 -O0 pipeline runs `loop(loop-idiom-vectorize)`,
    // whose LoopSimplify/LCSSA canonicalization reshapes the array loops
    // of the (non-`optnone`) global initialization functions.
    ("global_init_array_eh", "aarch64-unknown-linux-gnu"),
    // The same canonicalization of `__cxx_global_array_dtor`'s
    // partial-array EH cleanup loop (WP-205).
    ("eh_array_destroy_throwing_global", "aarch64-unknown-linux-gnu"),
    // The same, for the EH cleanup of a static array temporary's
    // initialization (WP-216).
    ("array_temporary_global", "aarch64-unknown-linux-gnu"),
];

/// Flags given to both compilers ahead of a case's own, to pin target
/// defaults that clang derives from the host rather than the triple.
///
/// clang 19 turns `AArch64`'s `+outline-atomics` on by default only when it
/// finds a GCC >= 9.3.1 installation for the target (or compiler-rt):
/// natively on an `AArch64` host it does, cross-compiling from x86-64 (the
/// GitHub runner) it does not. kiln defaults to on; the explicit flag
/// makes the comparison host-independent.
fn pinned_flags(triple: &str) -> &'static [&'static str] {
    if triple.starts_with("aarch64") { &["-moutline-atomics"] } else { &[] }
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/codegen")
}

/// The case's extra options, from its `//@ args:` lines (split on whitespace).
fn extra_flags(file: &Path) -> Vec<String> {
    let src = std::fs::read_to_string(file).unwrap_or_default();
    src.lines()
        .filter_map(|l| l.strip_prefix("//@ args:"))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect()
}

/// Compiles `file` for `triple` with the in-process driver.
fn kiln_ir(file: &Path, triple: &str) -> Result<String, String> {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(out.clone()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    };
    let args: Vec<String> =
        ["-S", "-emit-llvm", "-O0", "-std=c++26", &format!("--target={triple}"), "-o", "-"]
            .iter()
            .chain(pinned_flags(triple))
            .map(|s| (*s).to_owned())
            .chain(extra_flags(file))
            .chain(std::iter::once(file.display().to_string()))
            .collect();
    let code = run(&args, io);
    if code != 0 {
        return Err(format!("kiln exited {code}:\n{}", err.contents()));
    }
    Ok(out.contents())
}

/// Runs `prog args` with `input` on stdin; `None` when `prog` is missing.
fn pipe(prog: &str, args: &[&str], input: &str) -> Option<Result<String, String>> {
    let Ok(mut child) = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return None;
    };
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().expect("wait");
    if out.status.success() {
        Some(Ok(String::from_utf8_lossy(&out.stdout).into_owned()))
    } else {
        Some(Err(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

fn opt(ir: &str) -> Option<Result<String, String>> {
    pipe("opt-19", &["-S", "-passes=verify", "-o", "-"], ir)
}

fn clang_ir(file: &Path, triple: &str) -> Option<Result<String, String>> {
    let target = format!("--target={triple}");
    let path = file.display().to_string();
    let extra = extra_flags(file);
    let mut args = vec!["-S", "-emit-llvm", "-O0", "-std=c++26", target.as_str(), "-o", "-"];
    args.extend(pinned_flags(triple));
    args.extend(extra.iter().map(String::as_str));
    args.push(path.as_str());
    pipe("clang++-19", &args, "")
}

/// Drops the `ModuleID` comment, `source_filename`, the `llvm.ident`
/// named metadata and its string node (clang's or kiln's version).
fn normalize(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        if line.starts_with("; ModuleID")
            || line.starts_with("source_filename")
            || line.starts_with("!llvm.ident")
            || (line.starts_with('!')
                && line.contains(" = !{!\"")
                && (line.contains("clang version") || line.contains("kiln version")))
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A minimal line diff: the first differing region with context.
fn diff(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut i = 0;
    while i < al.len() && i < bl.len() && al[i] == bl[i] {
        i += 1;
    }
    let lo = i.saturating_sub(3);
    let mut s = String::new();
    for l in &al[lo..al.len().min(i + 8)] {
        let _ = writeln!(s, "kiln : {l}");
    }
    for l in &bl[lo..bl.len().min(i + 8)] {
        let _ = writeln!(s, "clang: {l}");
    }
    s
}

enum Outcome {
    Exact,
    Differs(String),
    Skipped,
}

fn run_case(name: &str, file: &Path, triple: &str) -> Result<Outcome, String> {
    let ir = kiln_ir(file, triple)?;
    let Some(v) = opt(&ir) else { return Ok(Outcome::Skipped) };
    let ours = v.map_err(|e| format!("opt-19 rejects kiln IR:\n{e}\n{ir}"))?;
    let Some(c) = clang_ir(file, triple) else { return Ok(Outcome::Skipped) };
    let c = c.map_err(|e| format!("clang++-19 failed: {e}"))?;
    let theirs =
        opt(&c).expect("opt-19 present").map_err(|e| format!("opt-19 rejects clang IR: {e}"))?;
    let (a, b) = (normalize(&ours), normalize(&theirs));
    if std::env::var("KILN_CASE").is_ok_and(|c| c == name) {
        eprintln!("==== kiln ({triple}) ====\n{a}\n==== clang ({triple}) ====\n{b}");
    }
    if a == b { Ok(Outcome::Exact) } else { Ok(Outcome::Differs(diff(&a, &b))) }
}

#[test]
fn ir_matches_clang() {
    let only = std::env::var("KILN_CASE").ok();
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("tests/codegen")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    assert!(only.is_some() || files.len() >= 40, "the corpus has {} cases", files.len());
    let (mut exact, mut total, mut skipped) = (0, 0, 0);
    let mut failures = Vec::new();
    let mut deviations = Vec::new();
    for file in &files {
        let name = file.file_stem().and_then(|s| s.to_str()).expect("file name").to_owned();
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        for &triple in TRIPLES {
            total += 1;
            let deviation = DEVIATIONS.contains(&(name.as_str(), triple));
            match run_case(&name, file, triple) {
                Ok(Outcome::Exact) => {
                    exact += 1;
                    if deviation {
                        deviations
                            .push(format!("{name} ({triple}): now exact; drop the deviation"));
                    }
                }
                Ok(Outcome::Differs(d)) if deviation => {
                    deviations.push(format!("{name} ({triple}): verify-only\n{d}"));
                }
                Ok(Outcome::Differs(d)) => {
                    failures.push(format!("{name} ({triple}): differs from clang\n{d}"));
                }
                Ok(Outcome::Skipped) => skipped += 1,
                Err(e) => failures.push(format!("{name} ({triple}): {e}")),
            }
        }
    }
    eprintln!("ir_vs_clang: {exact}/{total} exact, {skipped} skipped (no opt-19/clang++-19)");
    for d in &deviations {
        eprintln!("  {d}");
    }
    assert!(failures.is_empty(), "{} failing case(s):\n{}", failures.len(), failures.join("\n"));
}
