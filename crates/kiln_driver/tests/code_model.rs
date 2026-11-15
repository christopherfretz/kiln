//! WP-1419: `-mcmodel=` and `-mlarge-data-threshold=` as clang's driver
//! checks them (spec/driver.md "Codegen flags"): every value against
//! both targets and every relocation model, kiln's errors, warnings and
//! `Code Model` / `Large Data Threshold` / `PIC Level` / `PIE Level`
//! module flags compared with clang++-19 (kiln at `-fclang-compat=19`)
//! and, when it is installed, clang++-22 (kiln's default level). The
//! targets are pinned and the source includes nothing, so the tests run
//! on either host. They skip without the clangs.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const A64: &str = "--target=aarch64-unknown-linux-gnu";
const X86: &str = "--target=x86_64-unknown-linux-gnu";

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn scratch(name: &str) -> PathBuf {
    let d =
        std::env::temp_dir().join(format!("kiln-code-model-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("m.cpp"), "int g;\nint f() { return g; }\n").unwrap();
    std::fs::write(d.join("n.cpp"), "int h() { return 1; }\n").unwrap();
    d
}

/// The exit code, the diagnostics without the program name and the
/// code-model and relocation module flags of `prog args -S -emit-llvm`.
type Outcome = (Option<i32>, Vec<String>, Vec<String>);

fn outcome(prog: &Path, dir: &Path, args: &[&str], inputs: &[&str]) -> Outcome {
    // One output file per input: several inputs are only checked.
    let mut all = if inputs.len() > 1 {
        vec!["-fsyntax-only"]
    } else if args.contains(&"-fsyntax-only") {
        vec![]
    } else {
        vec!["-S", "-emit-llvm", "-o", "-"]
    };
    all.extend(args);
    all.extend(inputs);
    let o = Command::new(prog).args(&all).current_dir(dir).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr)
        .lines()
        .map(|l| {
            let l = ["clang++-19: ", "clang++-22: ", "kiln++: "]
                .iter()
                .fold(l, |l, p| l.strip_prefix(p).unwrap_or(l));
            l.to_owned()
        })
        .collect();
    let flags = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| {
            ["Code Model", "Large Data Threshold", "PIC Level", "PIE Level"]
                .iter()
                .any(|f| l.contains(&format!("!\"{f}\"")))
        })
        .map(|l| l.split_once(" = ").map_or(l, |x| x.1).to_owned())
        .collect();
    (o.status.code(), err, flags)
}

/// The clangs to compare with: (program, kiln's `-fclang-compat=`).
fn oracles() -> Vec<(&'static str, &'static str)> {
    let v: Vec<_> = [("clang++-19", "-fclang-compat=19"), ("clang++-22", "-fclang-compat=22")]
        .into_iter()
        .filter(|(c, _)| have(c))
        .collect();
    if v.is_empty() {
        eprintln!("skipping: neither clang++-19 nor clang++-22 found");
    }
    v
}

/// Compares kiln with each clang on `cases`; `inputs` are the sources.
fn compare(dir: &Path, cases: &[Vec<&str>], inputs: &[&str]) {
    for (clang, compat) in oracles() {
        for args in cases {
            // clang 19's driver passes x86-64 `tiny` on and its backend
            // stops with a fatal error; kiln reports clang 22's error at
            // both levels.
            if clang == "clang++-19" && args.contains(&X86) && args.contains(&"-mcmodel=tiny") {
                continue;
            }
            let k = outcome(&kilnpp(), dir, &[&args[..], &[compat]].concat(), inputs);
            let c = outcome(Path::new(clang), dir, args, inputs);
            assert_eq!(k, c, "{clang} {args:?} {inputs:?}");
        }
    }
}

/// Every `-mcmodel=` value (clang's, other targets', unknown ones) on
/// both targets with every relocation model: `large` on `AArch64` only
/// with `-fno-pic` (the default PIE and `-static`, which keeps it, are
/// errors), x86-64 `large` with PIE accepted.
#[test]
fn code_model_values_and_pic_modes() {
    let d = scratch("values");
    let models =
        ["tiny", "small", "kernel", "medium", "large", "medlow", "medany", "huge", "", "Large"];
    let pics: [&[&str]; 8] = [
        &[],
        &["-fno-pic"],
        &["-fpic"],
        &["-fPIC"],
        &["-fPIE"],
        &["-fpie", "-fno-pic"],
        &["-fno-pic", "-fpie"],
        &["-static"],
    ];
    let mut cases = Vec::new();
    for t in [A64, X86] {
        for m in models {
            let flag = format!("-mcmodel={m}");
            let flag: &'static str = Box::leak(flag.into_boxed_str());
            for p in pics {
                cases.push([&[t, flag][..], p].concat());
            }
        }
        // The last `-mcmodel=` counts.
        cases.push(vec![t, "-mcmodel=large", "-mcmodel=small"]);
        cases.push(vec![t, "-mcmodel=small", "-mcmodel=large"]);
        cases.push(vec![t, "-fno-pic", "-mcmodel=large", "-mcmodel=medium"]);
        // Every action that runs a compile job checks it.
        cases.push(vec![t, "-fsyntax-only", "-mcmodel=large"]);
        cases.push(vec![t, "-fsyntax-only", "-mcmodel=kernel"]);
        // Before the `-mtls-dialect=` error, as clang.
        cases.push(vec![t, "-mcmodel=large", "-mtls-dialect=desc"]);
        cases.push(vec![t, "-mcmodel=tiny", "-mtls-dialect=desc"]);
    }
    compare(&d, &cases, &["m.cpp"]);
    // clang 19's x86-64 `tiny`: clang 22's error at both levels.
    for compat in ["-fclang-compat=19", "-fclang-compat=22"] {
        let k = outcome(&kilnpp(), &d, &[X86, "-mcmodel=tiny", compat], &["m.cpp"]);
        let want = "error: unsupported argument 'tiny' to option '-mcmodel=' for target \
                    'x86_64-unknown-linux-gnu'";
        assert_eq!(k, (Some(1), vec![want.to_owned()], vec![]), "{compat}");
    }
}

/// `-mlarge-data-threshold=`: on x86-64 the `Large Data Threshold` flag
/// with `-mcmodel=medium` / `large` (`-cc1`'s 32-bit integer, radix 0;
/// a bad one is its error, before the other `-cc1` errors), clang's
/// `-Wunused-command-line-argument` warning with any other model, once
/// per job; on `AArch64` an unsupported option, which clang 19 reports
/// only when the driver saw no other error.
#[test]
fn large_data_threshold() {
    let d = scratch("threshold");
    let mut cases: Vec<Vec<&str>> = Vec::new();
    for m in [None, Some("-mcmodel=small"), Some("-mcmodel=kernel"), Some("-mcmodel=medium")] {
        for v in [
            "-mlarge-data-threshold=5",
            "-mlarge-data-threshold=0x10",
            "-mlarge-data-threshold=010",
            "-mlarge-data-threshold=0b101",
            "-mlarge-data-threshold=4294967295",
            "-mlarge-data-threshold=4294967296",
            "-mlarge-data-threshold=abc",
            "-mlarge-data-threshold=",
            "-mlarge-data-threshold=-1",
            "-mlarge-data-threshold=+5",
        ] {
            cases.push([&[X86][..], m.as_slice(), &[v]].concat());
        }
    }
    for extra in [
        &["-mcmodel=large", "-mlarge-data-threshold=100"][..],
        &["-mcmodel=large", "-mlarge-data-threshold=7", "-mlarge-data-threshold=9"],
        &["-mlarge-data-threshold=100", "-mcmodel=large", "-mcmodel=small"],
        &["-mcmodel=large", "-mlarge-data-threshold=1", "-Wno-unused-command-line-argument"],
        &["-mlarge-data-threshold=1", "-Wno-unused-command-line-argument"],
        &["-mlarge-data-threshold=1", "-Werror=unused-command-line-argument"],
        &["-mlarge-data-threshold=1", "-Qunused-arguments"],
        &["-mlarge-data-threshold=1", "-fsyntax-only"],
        &["-mcmodel=bogus", "-mlarge-data-threshold=1"],
        &["-mcmodel=medium", "-mlarge-data-threshold=x", "-mfunction-return=bad"],
        &["-mcmodel=medium", "-mlarge-data-threshold=x", "-ftls-model=bad"],
    ] {
        cases.push([&[X86][..], extra].concat());
    }
    for extra in [
        &["-mlarge-data-threshold=5"][..],
        &["-mlarge-data-threshold=x"],
        &["-mcmodel=small", "-mlarge-data-threshold=5"],
        &["-mcmodel=large", "-mlarge-data-threshold=5"],
        &["-mcmodel=large", "-fno-pic", "-mlarge-data-threshold=5"],
        &["-mcmodel=kernel", "-mlarge-data-threshold=5"],
        &["-mlarge-data-threshold=5", "-mcpu=bogus"],
        &["-mlarge-data-threshold=5", "-mtls-dialect=gnu2"],
        &["-mlarge-data-threshold=5", "-fsyntax-only"],
    ] {
        cases.push([&[A64][..], extra].concat());
    }
    compare(&d, &cases, &["m.cpp"]);
    // Two inputs: the x86-64 warning once per job; clang 19's `AArch64`
    // error once (clang 22's is once per job, as its `-mcmodel=` and
    // `-mtls-dialect=` errors, where kiln reports each once).
    let two: Vec<Vec<&str>> = vec![
        vec![X86, "-mlarge-data-threshold=5"],
        vec![X86, "-mcmodel=medium", "-mlarge-data-threshold=5"],
    ];
    compare(&d, &two, &["m.cpp", "n.cpp"]);
    if have("clang++-19") {
        let args = [A64, "-mlarge-data-threshold=5"];
        let inputs = ["m.cpp", "n.cpp"];
        let k = outcome(&kilnpp(), &d, &[&args[..], &["-fclang-compat=19"]].concat(), &inputs);
        assert_eq!(k, outcome(Path::new("clang++-19"), &d, &args, &inputs));
    }
}
