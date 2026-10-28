//! Shared helpers for the `kiln_llvm` integration tests (`spec/llvm.md` §9).
//!
//! The LLVM tools are found on `PATH` (or under `/usr/lib/llvm-19/bin`).
//! When they are missing the tool-backed half of a test is skipped with a
//! note on stderr, unless `KILN_REQUIRE_LLVM=1`, which turns the skip into a
//! failure (release-candidate validation).
#![allow(dead_code)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use kiln_llvm::Module;

/// Locate an LLVM 19 tool (`llvm-as`, `opt`, `clang`).
pub fn tool(name: &str) -> Option<PathBuf> {
    let versioned = format!("{name}-19");
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let p = dir.join(&versioned);
        if p.is_file() {
            return Some(p);
        }
    }
    let p = PathBuf::from("/usr/lib/llvm-19/bin").join(name);
    p.is_file().then_some(p)
}

fn missing(name: &str) -> Option<PathBuf> {
    let t = tool(name);
    if t.is_none() {
        assert!(
            std::env::var("KILN_REQUIRE_LLVM").as_deref() != Ok("1"),
            "{name}-19 not found and KILN_REQUIRE_LLVM=1"
        );
        eprintln!("note: {name}-19 not found; skipping tool check");
    }
    t
}

fn numbered(text: &str) -> String {
    text.lines().enumerate().map(|(i, l)| format!("{:4} | {l}\n", i + 1)).collect()
}

/// Run `tool args...` with `input` on stdin; return stdout, or panic with
/// stderr and the numbered input.
fn run(tool: &PathBuf, args: &[&str], input: &str) -> String {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn LLVM tool");
    child.stdin.take().expect("stdin").write_all(input.as_bytes()).expect("write stdin");
    let out = child.wait_with_output().expect("wait for LLVM tool");
    assert!(
        out.status.success(),
        "{} {} rejected the module:\n{}\n--- input ---\n{}",
        tool.display(),
        args.join(" "),
        String::from_utf8_lossy(&out.stderr),
        numbered(input)
    );
    String::from_utf8(out.stdout).expect("utf-8 tool output")
}

/// Assemble with `llvm-as-19` and verify with `opt-19 -passes=verify`.
/// Returns false if the tools are unavailable.
pub fn llvm_accepts(text: &str) -> bool {
    let (Some(as_), Some(opt)) = (missing("llvm-as"), missing("opt")) else { return false };
    run(&as_, &["-", "-o", "/dev/null"], text);
    run(&opt, &["-passes=verify", "-disable-output"], text);
    true
}

/// `opt-19 -S -passes=verify` normal form of a module's text.
pub fn canonical(text: &str) -> Option<String> {
    let opt = missing("opt")?;
    Some(run(&opt, &["-S", "-passes=verify"], text))
}

/// Print `m`, check every `expect` line occurs in the output (in order),
/// and round-trip it through the LLVM tools. Returns the text.
pub fn check(m: &Module, expect: &[&str]) -> String {
    let mut buf = Vec::new();
    m.print(&mut buf).expect("print to Vec");
    let text = String::from_utf8(buf).expect("printer emits UTF-8");
    assert_eq!(text, m.to_text(), "print() and to_text() agree");
    let mut from = 0;
    for e in expect {
        match text[from..].find(e) {
            Some(p) => from += p + e.len(),
            None => panic!("expected (in order) {e:?} in:\n{}", numbered(&text)),
        }
    }
    llvm_accepts(&text);
    text
}

/// Compile a C file with clang 19 at -O0 for aarch64 Linux.
pub fn clang_ir(path: &std::path::Path) -> Option<String> {
    let clang = missing("clang")?;
    let out = Command::new(clang)
        .args(["-S", "-emit-llvm", "-O0", "-fno-ident", "--target=aarch64-unknown-linux-gnu"])
        .arg(path)
        .args(["-o", "-"])
        .output()
        .expect("spawn clang");
    assert!(out.status.success(), "clang failed: {}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8(out.stdout).expect("utf-8"))
}
