//! WP-1863: the `-fsanitize-coverage=` family as clang's `SanitizerArgs`
//! resolves it (spec/driver.md "Sanitizers", spec/codegen.md "Sanitizer
//! coverage"): the driver's diagnostics and exit code, and the
//! instrumentation `SanitizerCoverage` adds (the callback calls, the
//! guard, counter and table globals, the definitions), compared with
//! clang++-22 (kiln's default level) and clang++-19 (kiln at
//! `-fclang-compat=19` with LLVM 19's `opt`). The targets are pinned and
//! the sources include nothing, so the tests run on either host. They
//! skip without the clangs.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

const SRC: &str = "extern \"C\" int g(int);\n\
extern \"C\" int f(int x, int y, int *p, void (*q)()) {\n\
  if (x > y) { q(); return g(x / y) + p[x]; }\n\
  switch (y) { case 1: return 3; case 7: p[2] = x; return 4; }\n\
  return x;\n\
}\n\
extern \"C\" __attribute__((no_sanitize(\"coverage\"))) int nf(int x) { return x ? g(x) : 1; }\n\
extern \"C\" int keep(int x) { return x ? g(x) : 2; }\n\
extern \"C\" int drop(int x) { return x ? g(x) : 3; }\n";

fn scratch() -> PathBuf {
    let d =
        std::env::temp_dir().join(format!("kiln-sanitize-coverage-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("c.cpp"), SRC).unwrap();
    // An allowlist must name the source file too; a list that leaves the
    // whole file out gives kiln's IR unused callback declarations that
    // clang's lacks (spec/codegen.md "Sanitizer coverage"), so the lists
    // here leave out functions only.
    std::fs::write(d.join("allow.txt"), "src:*\nfun:f\nfun:keep\n").unwrap();
    std::fs::write(d.join("ignore.txt"), "fun:drop\n").unwrap();
    std::fs::write(d.join("bad.txt"), "[\n").unwrap();
    d
}

/// The exit code, the diagnostics without the program name, and the
/// instrumentation: the lines naming a coverage callback or global and
/// the definitions, without attribute groups.
type Outcome = (Option<i32>, Vec<String>, Vec<String>);

fn outcome(prog: &Path, dir: &Path, args: &[&str], llvm: Option<&str>) -> Outcome {
    let mut all = vec!["-S", "-emit-llvm", "-o", "-", "c.cpp"];
    all.extend(args);
    let mut cmd = Command::new(prog);
    cmd.args(&all).current_dir(dir);
    if let Some(l) = llvm {
        cmd.env("KILN_LLVM_DIR", l);
    }
    let o = cmd.output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr)
        .lines()
        .map(|l| {
            ["clang++-19: ", "clang++-22: ", "kiln++: "]
                .iter()
                .fold(l, |l, p| l.strip_prefix(p).unwrap_or(l))
                .to_owned()
        })
        .filter(|l| !l.ends_with(" generated."))
        .collect();
    let ir = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| {
            l.starts_with("define ")
                || l.contains("@__sanitizer_cov")
                || l.contains("@__sancov")
                || l.contains("@llvm.used")
                || l.contains("@llvm.compiler.used")
        })
        .map(|l| {
            // `#N` attribute groups differ (clang 22's module constructors
            // carry the target attributes; a WP-1863 follow-up).
            l.split(' ').filter(|w| !w.starts_with('#')).collect::<Vec<_>>().join(" ")
        })
        .collect();
    (o.status.code(), err, ir)
}

/// The flag sets whose diagnostics and instrumentation must match.
const CASES: &[&[&str]] = &[
    &[],
    &["-fsanitize-coverage=trace-pc-guard"],
    &["-fsanitize-coverage=trace-pc"],
    &["-fsanitize-coverage=func,trace-pc-guard"],
    &["-fsanitize-coverage=bb,trace-pc-guard"],
    &["-fsanitize-coverage=edge,trace-pc-guard,indirect-calls"],
    &["-fsanitize-coverage=trace-pc-guard,trace-cmp,trace-div,trace-gep"],
    &["-fsanitize-coverage=inline-8bit-counters,pc-table"],
    &["-fsanitize-coverage=inline-bool-flag,pc-table,no-prune"],
    &["-fsanitize-coverage=trace-loads,trace-stores"],
    &["-fsanitize-coverage=trace-pc-guard,trace-loads,trace-stores"],
    &["-fsanitize-coverage=control-flow"],
    &["-fsanitize-coverage=stack-depth"],
    &["-fsanitize-coverage=trace-pc-guard", "-fno-sanitize-coverage=trace-pc-guard"],
    &["-fsanitize-coverage=trace-pc-guard,trace-cmp", "-fno-sanitize-coverage=trace-cmp"],
    &["-fsanitize-coverage=trace-pc-guard", "-fsanitize-coverage=0"],
    &["-fsanitize-coverage=1"],
    &["-fsanitize-coverage=4"],
    &["-fsanitize-coverage=0x2"],
    &["-fsanitize-coverage=func,bb,edge"],
    &["-fsanitize-coverage=edge"],
    &["-fsanitize-coverage=trace-bb,8bit-counters"],
    &["-fsanitize-coverage=trace-pc-guard,bogus"],
    &["-fno-sanitize-coverage=bogus"],
    &["-fsanitize-coverage=trace-pc-guard,,trace-cmp"],
    &["-fsanitize=fuzzer-no-link"],
    &["-fsanitize=fuzzer-no-link", "-fno-sanitize-coverage=trace-cmp"],
    &["-fsanitize=fuzzer-no-link", "-fsanitize-coverage=trace-pc"],
    &["-fsanitize=fuzzer-no-link", "-fno-sanitize=fuzzer-no-link"],
    // Without `inline-8bit-counters` the fuzzer set's insertion point is
    // `func` (`stack-depth`'s), and `opt`'s `cl::opt`s cannot ask
    // `indirect-calls` with it: kiln leaves out the
    // `__sanitizer_cov_trace_pc_indir` calls (spec/codegen.md "Sanitizer
    // coverage"; a WP-1863 follow-up), so that case is not compared.
    &["-fsanitize-coverage=trace-pc-guard", "-fsanitize-coverage-allowlist=allow.txt"],
    &["-fsanitize-coverage=trace-pc-guard", "-fsanitize-coverage-ignorelist=ignore.txt"],
    &[
        "-fsanitize-coverage=trace-pc-guard",
        "-fsanitize-coverage-allowlist=allow.txt",
        "-fsanitize-coverage-ignorelist=ignore.txt",
    ],
    &["-fsanitize-coverage=trace-pc-guard", "-fsanitize-coverage-allowlist=missing.txt"],
    &["-fsanitize-coverage=trace-pc-guard", "-fsanitize-coverage-ignorelist=bad.txt"],
    &["-fsanitize-coverage-allowlist=allow.txt"],
    &["-fsanitize-coverage-whitelist=allow.txt"],
    // clang 22's; clang 19's unknown argument.
    &["-fsanitize-coverage=stack-depth", "-fsanitize-coverage-stack-depth-callback-min=4"],
    &["-fsanitize-coverage-stack-depth-callback-min=-1", "-fsanitize-coverage=trace-pc"],
    &[
        "-fsanitize-coverage-stack-depth-callback-min=x",
        "-fsanitize-coverage-stack-depth-callback-min=0x10",
        "-fsanitize-coverage=trace-pc-guard,stack-depth",
    ],
    &["-fsanitize-coverage-stack-depth-callback-min=4"],
    &["-O2", "-fsanitize-coverage=trace-pc-guard,trace-cmp"],
    &["-O2", "-fsanitize=fuzzer-no-link"],
];

#[test]
fn sanitize_coverage_flags() {
    let dir = scratch();
    let mut ran = false;
    for (clang, compat, llvm) in [
        ("clang++-19", "-fclang-compat=19", Some("/usr/lib/llvm-19")),
        ("clang++-22", "-fclang-compat=22", None),
    ] {
        if !have(clang) || llvm.is_some_and(|l| !Path::new(l).join("bin").join("opt").exists()) {
            continue;
        }
        ran = true;
        for &args in CASES {
            let pinned =
                ["--target=aarch64-unknown-linux-gnu", "--target=x86_64-unknown-linux-gnu"];
            for target in pinned {
                let a = [&[target][..], args].concat();
                let k = outcome(&kilnpp(), &dir, &[&a[..], &[compat]].concat(), llvm);
                let c = outcome(Path::new(clang), &dir, &a, None);
                assert_eq!(k, c, "{clang} {a:?}");
            }
        }
    }
    if !ran {
        eprintln!("skipping: neither clang++-19 nor clang++-22 found");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
