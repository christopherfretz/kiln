//! WP-1990: `__builtin_kiln_host_call` through the driver's hidden test
//! handler (spec/host-calls.md §4 "Run time", §6, §7): a call that would
//! run at run time is a compile error, never an undefined symbol, and no
//! precompiled header or module file is written with a handler.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

const FLAG: &str = "-fkiln-host-call-test-handler";

const SRC: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op) {\n\
  return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
}\n";

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// A fresh directory holding `name` with `src`.
fn scratch(test: &str, name: &str, src: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-host-calls-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(name), src).unwrap();
    d
}

/// The exit code and stderr of kiln++ `args` in `dir`.
fn kiln(dir: &Path, args: &[&str]) -> (Option<i32>, String) {
    let o = Command::new(kilnpp()).args(args).current_dir(dir).output().unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn a_run_time_call_is_a_compile_error() {
    let d = scratch("run", "t.cpp", &format!("{SRC}int main() {{ return (int)hc(1); }}\n"));
    let (code, err) = kiln(&d, &["-std=c++26", FLAG, "-c", "t.cpp", "-o", "t.o"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("t.cpp:3:8: error: host call outside constant evaluation"), "{err}");
    assert!(!d.join("t.o").exists());
    // The constant one is folded and emits nothing to call.
    std::fs::write(
        d.join("u.cpp"),
        format!("{SRC}constexpr u64 v = hc(1);\nint main() {{ return (int)v; }}\n"),
    )
    .unwrap();
    let (code, err) = kiln(&d, &["-std=c++26", FLAG, "-c", "u.cpp", "-o", "u.o"]);
    assert_eq!(code, Some(0), "{err}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn without_the_flag_the_name_is_undeclared() {
    let d = scratch("absent", "t.cpp", SRC);
    let (code, err) = kiln(&d, &["-std=c++26", "-fsyntax-only", "t.cpp"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("use of undeclared identifier '__builtin_kiln_host_call'"), "{err}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn no_artifact_is_written_with_a_handler() {
    let d = scratch("artifact", "h.h", SRC);
    std::fs::write(d.join("m.cppm"), format!("module;\n{SRC}export module m;\n")).unwrap();
    let cases: [(&[&str], &str); 3] = [
        (&["-x", "c++-header", "h.h", "-o", "h.pch"], "a precompiled header"),
        (&["--precompile", "m.cppm", "-o", "m.pcm"], "a module file"),
        (
            &["-c", "m.cppm", "-fmodule-output=m.pcm", "-o", "m.o"],
            "a module file (-fmodule-output)",
        ),
    ];
    for (args, what) in cases {
        let mut all = vec!["-std=c++26", FLAG];
        all.extend(args);
        let (code, err) = kiln(&d, &all);
        assert_eq!(code, Some(1), "{args:?}: {err}");
        let want = format!("error: {FLAG} cannot be used when writing {what}");
        assert!(err.contains(&want), "{args:?}: {err}");
    }
    for f in ["h.pch", "m.pcm", "m.o"] {
        assert!(!d.join(f).exists(), "{f} was written");
    }
    std::fs::remove_dir_all(&d).unwrap();
}
