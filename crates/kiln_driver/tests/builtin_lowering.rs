//! WP-805 (spec/sema.md §4.5 "Truthful feature tests"): every builtin
//! function kiln's `__has_builtin` reports lowers or folds, and
//! `kiln_codegen::UNLOWERED_BUILTINS`, the builtins kiln declares but
//! cannot lower (which `__has_builtin` reports as absent), is exact.
//!
//! Per target and clang version: the reported set is read back from a
//! translation unit that `#pragma message`s each name of clang's
//! `__has_builtin` table kiln answers 1 for. Each name is called with
//! 0 to 6 operands of a type that converts to anything until sema
//! accepts the call; the calls sema accepts are compiled to IR in
//! chunks. Codegen stops at a translation unit's first error, so an
//! error drops its call and the chunk is compiled again. "not supported
//! yet", "IR lowering failed" and "not a library builtin" fail the test;
//! other errors are operand checks the made-up operands trip (a NEON
//! type code, a shift immediate) and only drop the call.
//!
//! Coverage limit: a builtin no operand list of that type satisfies (the
//! NEON `_v` type-code forms with vector operands, the x86 builtins with
//! immediate operands, the type traits and other keywords) is not
//! exercised; neither are the hand-written coroutine builtins, which
//! need the shape of a coroutine (`__builtin_coro_id` first).
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The operand type: converts to any type sema asks for.
const ANY: &str =
    "struct Any { template <class T> constexpr operator T() const { return T(); } };\n";

/// Codegen errors that mean a builtin does not lower.
const UNLOWERED: [&str; 3] = ["not supported yet", "IR lowering failed", "not a library builtin"];

struct Probe {
    dir: PathBuf,
    target: &'static str,
    compat: u32,
}

/// An `error:` diagnostic located in `probe.cpp`: its line and message.
fn errors(stderr: &str) -> Vec<(usize, String)> {
    stderr
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("probe.cpp:")?;
            let (line, rest) = rest.split_once(':')?;
            let (_, msg) = rest.split_once(": error: ")?;
            Some((line.parse().ok()?, msg.to_string()))
        })
        .collect()
}

impl Probe {
    fn new(target: &'static str, compat: u32) -> Probe {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("builtin-lowering-{}-{target}-{compat}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Probe { dir, target, compat }
    }

    /// Compiles `src` as `probe.cpp` with `mode` (`-fsyntax-only`, or
    /// `-S -emit-llvm`); the stderr.
    fn run(&self, src: &str, mode: &[&str]) -> String {
        std::fs::write(self.dir.join("probe.cpp"), src).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_kiln"))
            .args(["-std=c++26", "-ferror-limit=0", "-Wno-everything"])
            .arg(format!("-fclang-compat={}", self.compat))
            .arg(format!("--target={}", self.target))
            .args(mode)
            .arg("probe.cpp")
            .current_dir(&self.dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stderr).into_owned()
    }

    /// The names of clang's `__has_builtin` table kiln reports.
    fn reported(&self) -> Vec<String> {
        let mut src = String::new();
        for n in kiln_pp::table_keys(kiln_pp::FeatureTable::Builtin) {
            src += &format!("#if __has_builtin({n})\n#pragma message \"HB {n}\"\n#endif\n");
        }
        std::fs::write(self.dir.join("probe.cpp"), &src).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_kiln"))
            .args(["-std=c++26", "-fsyntax-only"])
            .arg(format!("-fclang-compat={}", self.compat))
            .arg(format!("--target={}", self.target))
            .arg("probe.cpp")
            .current_dir(&self.dir)
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        let names: Vec<String> = err
            .lines()
            .filter_map(|l| {
                l.split_once("warning: HB ")?.1.split_once(' ').map(|(n, _)| n.to_string())
            })
            .collect();
        assert!(names.len() > 500, "{}: only {} builtins reported", self.target, names.len());
        names
    }

    /// The calls `return name(Any{}, ...)` sema accepts, with the fewest
    /// operands that work; the names no operand count satisfies drop out.
    fn callable(&self, names: &[String]) -> Vec<(String, usize)> {
        let mut left: Vec<&String> = names.iter().collect();
        let mut ok = Vec::new();
        for k in 0..7 {
            let args = vec!["Any{}"; k].join(", ");
            let mut src = ANY.to_string();
            for (i, n) in left.iter().enumerate() {
                src += &format!("auto f{i}() {{ return {n}({args}); }}\n");
            }
            let bad: BTreeSet<usize> = errors(&self.run(&src, &["-fsyntax-only"]))
                .into_iter()
                .filter_map(|(l, _)| l.checked_sub(2))
                .collect();
            let mut next = Vec::new();
            for (i, n) in left.into_iter().enumerate() {
                if bad.contains(&i) {
                    next.push(n);
                } else {
                    ok.push((n.clone(), k));
                }
            }
            left = next;
        }
        ok
    }

    /// The calls of `calls` codegen rejects as unlowered, with the error.
    fn unlowered(&self, calls: &[(String, usize)]) -> Vec<(String, String)> {
        let mut bad = Vec::new();
        for chunk in calls.chunks(64) {
            let mut chunk = chunk.to_vec();
            while !chunk.is_empty() {
                let mut src = ANY.to_string();
                for (i, (n, k)) in chunk.iter().enumerate() {
                    let args = vec!["Any{}"; *k].join(", ");
                    src += &format!("auto f{i}() {{ return {n}({args}); }}\n");
                }
                let err = self.run(&src, &["-S", "-emit-llvm", "-o", "probe.ll"]);
                let Some((line, msg)) = errors(&err).into_iter().next() else {
                    assert!(!err.contains("error"), "{}: unlocated error:\n{err}", self.target);
                    break;
                };
                let i = line.checked_sub(2).filter(|&i| i < chunk.len());
                let i = i.unwrap_or_else(|| panic!("{}: error outside a call: {msg}", self.target));
                let (n, _) = chunk.remove(i);
                if UNLOWERED.iter().any(|u| msg.contains(u)) {
                    bad.push((n, msg));
                }
            }
        }
        bad
    }
}

fn unlowered_list() -> Vec<&'static str> {
    kiln_codegen::UNLOWERED_BUILTINS.split_ascii_whitespace().collect()
}

/// The reported builtins of `target` at `compat` all lower; at clang 22
/// each list entry sema accepts on `target` is confirmed unlowered.
fn check(target: &'static str, compat: u32) -> BTreeSet<&'static str> {
    let p = Probe::new(target, compat);
    let names: Vec<String> =
        p.reported().into_iter().filter(|n| !n.starts_with("__builtin_coro_")).collect();
    let calls = p.callable(&names);
    let bad = p.unlowered(&calls);
    assert!(
        bad.is_empty(),
        "{target} @{compat}: builtins __has_builtin reports that do not lower \
         (lower them, or list them in kiln_codegen::UNLOWERED_BUILTINS): {bad:#?}"
    );
    let mut confirmed = BTreeSet::new();
    if compat == 22 {
        let listed: Vec<String> = unlowered_list().into_iter().map(String::from).collect();
        for (n, k) in p.callable(&listed) {
            let rejected = p.unlowered(&[(n.clone(), k)]);
            assert!(
                !rejected.is_empty(),
                "{target}: {n} lowers now: drop it from kiln_codegen::UNLOWERED_BUILTINS"
            );
            confirmed.insert(*unlowered_list().iter().find(|&&m| m == n).unwrap());
        }
    }
    let _ = std::fs::remove_dir_all(&p.dir);
    confirmed
}

#[test]
fn aarch64_19() {
    check("aarch64-unknown-linux-gnu", 19);
}

#[test]
fn x86_64_19() {
    check("x86_64-unknown-linux-gnu", 19);
}

/// Both targets at clang 22, and every list entry is confirmed on one.
#[test]
fn clang22_and_list_exact() {
    let a = std::thread::spawn(|| check("aarch64-unknown-linux-gnu", 22));
    let mut confirmed = check("x86_64-unknown-linux-gnu", 22);
    confirmed.extend(a.join().unwrap());
    let stale: Vec<_> = unlowered_list().into_iter().filter(|n| !confirmed.contains(n)).collect();
    assert!(stale.is_empty(), "UNLOWERED_BUILTINS entries no probe confirms: {stale:?}");
}

#[test]
fn list_sorted_and_declared() {
    let l = unlowered_list();
    assert!(l.windows(2).all(|w| w[0] < w[1]), "UNLOWERED_BUILTINS must be sorted, no duplicates");
    for n in l {
        assert!(
            kiln_ast::decl::builtin::by_name(n).is_some()
                || kiln_ast::decl::builtin::lib_by_name(n).is_some(),
            "{n} is not a builtin kiln declares"
        );
    }
}
