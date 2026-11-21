//! WP-1795: the complex-range flags (`-fcomplex-arithmetic=`,
//! `-f[no-]cx-limited-range`, `-f[no-]cx-fortran-rules`) with
//! `-ffp-model=`, `-f[no-]fast-math` and `-Ofast`, as clang's
//! `RenderFloatingPointOptions` resolves them (spec/driver.md
//! "Floating-point flags"): the algorithm a `_Complex double` quotient
//! gets and the driver's, Sema's and codegen's warnings compared with
//! clang++-22 (kiln's default level) and clang++-19 (kiln at
//! `-fclang-compat=19`). The targets are pinned and the source includes
//! nothing, so the tests run on either host. They skip without the clangs.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-complex-range-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("c.cpp"),
        "_Complex double q(_Complex double a, _Complex double b) { return a / b; }\n\
         _Complex long double l(_Complex long double a, _Complex long double b) {\n  \
         return a / b;\n}\n",
    )
    .unwrap();
    d
}

/// The exit code, the diagnostics without the program name, and the
/// algorithm of `q`'s quotient: `full` (`__divdc3`), `promoted` (an
/// `fpext` to the wider type), `improved` (Smith's `llvm.fabs`) or
/// `basic`.
type Outcome = (Option<i32>, Vec<String>, &'static str);

fn outcome(prog: &Path, dir: &Path, args: &[&str]) -> Outcome {
    let mut all = vec!["-S", "-emit-llvm", "-o", "-", "c.cpp"];
    all.extend(args);
    let o = Command::new(prog).args(&all).current_dir(dir).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr)
        .lines()
        .map(|l| {
            ["clang++-19: ", "clang++-22: ", "kiln++: "]
                .iter()
                .fold(l, |l, p| l.strip_prefix(p).unwrap_or(l))
                .to_owned()
        })
        // clang's `-Wdeprecated-ofast` (and its count) is not
        // implemented (a WP-1795 follow-up).
        .filter(|l| !l.contains("[-Wdeprecated-ofast]") && !l.ends_with(" generated."))
        .collect();
    let out = String::from_utf8_lossy(&o.stdout);
    let body = out.split("@_Z1qCdS_(").nth(1).and_then(|b| b.split("\n}\n").next()).unwrap_or("");
    let algo = if body.contains("@__divdc3") {
        "full"
    } else if body.contains("fpext double") {
        "promoted"
    } else if body.contains("llvm.fabs") {
        "improved"
    } else {
        "basic"
    };
    (o.status.code(), err, algo)
}

/// clang 22's `-ffp-model=aggressive` is an unknown value to clang 19,
/// and its warning groups unknown options (kiln knows them at both
/// levels).
const ONLY_22: &[&str] = &[
    "-ffp-model=aggressive",
    "-Wno-higher-precision-for-complex-division",
    "-Werror=higher-precision-for-complex-division",
    "-Wno-overriding-complex-range",
];

#[test]
fn complex_range_flags() {
    let dir = scratch();
    let cases: &[&[&str]] = &[
        &[],
        &["-fcomplex-arithmetic=full"],
        &["-fcomplex-arithmetic=improved"],
        &["-fcomplex-arithmetic=promoted"],
        &["-fcomplex-arithmetic=basic"],
        &["-fcx-limited-range"],
        &["-fcx-fortran-rules"],
        &["-fcx-limited-range", "-fno-cx-limited-range"],
        &["-fcx-fortran-rules", "-fno-cx-fortran-rules"],
        &["-fcx-limited-range", "-fcx-fortran-rules"],
        &["-fcomplex-arithmetic=basic", "-fcomplex-arithmetic=promoted"],
        &["-ffp-model=fast"],
        &["-ffp-model=precise"],
        &["-ffp-model=strict"],
        &["-ffp-model=aggressive"],
        &["-ffast-math"],
        &["-ffast-math", "-fno-fast-math"],
        &["-ffast-math", "-fno-cx-limited-range"],
        &["-ffp-model=fast", "-ffast-math"],
        &["-ffp-model=fast", "-fno-fast-math"],
        &["-ffp-model=fast", "-ffp-model=precise"],
        &["-ffp-model=strict", "-fno-fast-math"],
        &["-ffp-model=fast", "-fcx-limited-range"],
        &["-ffp-model=fast", "-fcx-fortran-rules"],
        &["-fcomplex-arithmetic=basic", "-ffp-model=fast"],
        &["-fcomplex-arithmetic=promoted", "-fno-fast-math"],
        &["-fcomplex-arithmetic=promoted", "-ffp-model=precise"],
        &["-fcomplex-arithmetic=promoted", "-ffp-model=strict"],
        &["-fcomplex-arithmetic=promoted", "-ffast-math"],
        &["-fcx-fortran-rules", "-ffast-math"],
        &["-fcx-limited-range", "-fno-fast-math", "-fcx-fortran-rules"],
        &["-fcx-limited-range", "-fcomplex-arithmetic=promoted"],
        &["-Ofast"],
        &["-Ofast", "-ffp-model=precise"],
        &["-Ofast", "-ffp-model=fast"],
        &["-Ofast", "-ffp-model=strict"],
        &["-ffp-model=fast", "-Ofast"],
        &["-Ofast", "-O2", "-ffp-model=fast"],
        &["-fcomplex-arithmetic=promoted", "-Wno-higher-precision-for-complex-division"],
        &["-fcomplex-arithmetic=promoted", "-Werror=higher-precision-for-complex-division"],
        &["-ffp-model=fast", "-fcx-limited-range", "-Wno-overriding-complex-range"],
        &["-fcx-limited-range", "-fcomplex-arithmetic=basic"],
        &["-fcx-limited-range", "-fcomplex-arithmetic=full"],
        &["-fcx-fortran-rules", "-fcomplex-arithmetic=improved"],
        &["-fno-cx-fortran-rules", "-fcomplex-arithmetic=improved"],
        &["-fno-cx-fortran-rules", "-fcomplex-arithmetic=basic"],
        &["-fno-cx-limited-range", "-fcx-limited-range"],
        &["-fno-cx-limited-range", "-fno-cx-fortran-rules"],
        &["-fno-cx-fortran-rules", "-fno-cx-limited-range"],
        &["-fcomplex-arithmetic=improved", "-fcx-limited-range"],
        &["-fcomplex-arithmetic=basic", "-fcx-limited-range"],
        &["-fcomplex-arithmetic=promoted", "-fno-cx-fortran-rules"],
        &["-fcomplex-arithmetic=full", "-ffast-math"],
        &["-fcx-fortran-rules", "-Ofast"],
        &["-fno-cx-limited-range", "-ffp-model=fast"],
        &["-ffp-model=fast", "-ffp-model=strict", "-Wno-overriding-option"],
    ];
    let mut ran = false;
    for (clang, compat) in
        [("clang++-19", "-fclang-compat=19"), ("clang++-22", "-fclang-compat=22")]
    {
        if !have(clang) {
            continue;
        }
        ran = true;
        for &args in cases {
            let v19 = clang == "clang++-19";
            if v19 && args.iter().any(|a| ONLY_22.contains(a)) {
                continue;
            }
            let pinned =
                ["--target=aarch64-unknown-linux-gnu", "--target=x86_64-unknown-linux-gnu"];
            for target in pinned {
                let a = [&[target][..], args].concat();
                let k = outcome(&kilnpp(), &dir, &[&a[..], &[compat]].concat());
                let c = outcome(Path::new(clang), &dir, &a);
                assert_eq!(k, c, "{clang} {a:?}");
            }
        }
    }
    if !ran {
        eprintln!("skipping: neither clang++-19 nor clang++-22 found");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
