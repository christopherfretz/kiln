//! The run-time half of the calling-convention matrix (spec/abi.md §8.23,
//! spec/landed/abi/WP-766.md).
//!
//! `tests/abi_run/abi_run.cpp` is compiled twice, as the caller
//! (`-DABI_CALLER`) and as the callee, once by the in-process kiln driver
//! (`kiln_driver::run`, `-c -O0 -std=c++26`, the default
//! `-fclang-compat=22`) and once by `clang++-22`, for the host triple. The
//! objects are linked crosswise — kiln caller with clang callee, clang
//! caller with kiln callee — and run: every shape crosses the call boundary
//! as a named argument, as an argument after the argument registers are
//! used up, through `extern "C"`, through `...`, and as a return value, and
//! both sides check every member. The program exits 0 and prints `ok` when
//! the two compilers agree. CI runs it on aarch64 and x86-64 hosts.
//!
//! The comparison needs structured-binding packs (P1061R10), which clang
//! has since 21, so the oracle is always clang 22, whichever oracle release
//! the harness selects; the compat-19 ABI is covered by `ir_vs_clang`. Without
//! `clang++-22` the test is skipped.

use std::path::{Path, PathBuf};
use std::process::Command;

use kiln_diag::SharedBuffer;
use kiln_driver::{Io, SharedWriter, run};

const CLANG: &str = "clang++-22";

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/abi_run/abi_run.cpp")
}

/// Compiles the caller (`caller`) or the callee to `out` with kiln.
fn kiln_obj(caller: bool, out: &Path) -> Result<(), String> {
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(std::io::sink()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    };
    let mut args: Vec<String> =
        ["-c", "-O0", "-std=c++26", "-o"].iter().map(|s| (*s).to_owned()).collect();
    args.push(out.display().to_string());
    if caller {
        args.push("-DABI_CALLER".to_owned());
    }
    args.push(source().display().to_string());
    match run(&args, io) {
        0 => Ok(()),
        code => Err(format!("kiln exited {code}:\n{}", err.contents())),
    }
}

/// Runs `cmd`, failing with its stderr.
fn sh(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    let text =
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(text) } else { Err(format!("{cmd:?} failed:\n{text}")) }
}

/// Compiles the caller or the callee to `out` with clang.
fn clang_obj(caller: bool, out: &Path) -> Result<(), String> {
    let mut cmd = Command::new(CLANG);
    cmd.args(["-c", "-O0", "-std=c++26", "-o"]).arg(out);
    if caller {
        cmd.arg("-DABI_CALLER");
    }
    sh(cmd.arg(source())).map(drop)
}

/// Links the caller object `from` with the callee object `to` into `exe`
/// and runs it.
fn link_and_run(from: &Path, to: &Path, exe: &Path) -> Result<(), String> {
    sh(Command::new(CLANG).arg(from).arg(to).arg("-o").arg(exe))?;
    let out = sh(&mut Command::new(exe))?;
    if out.trim() == "ok" { Ok(()) } else { Err(format!("unexpected output:\n{out}")) }
}

#[test]
fn kiln_and_clang_objects_agree_at_run_time() {
    if Command::new(CLANG).arg("--version").output().is_err() {
        eprintln!("abi_run_vs_clang: skipped (no {CLANG})");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kiln-abi-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let p = |n: &str| dir.join(n);
    let r = (|| {
        kiln_obj(true, &p("kiln-caller.o"))?;
        kiln_obj(false, &p("kiln-callee.o"))?;
        clang_obj(true, &p("clang-caller.o"))?;
        clang_obj(false, &p("clang-callee.o"))?;
        let mut errs = Vec::new();
        for (caller, callee) in [("kiln", "clang"), ("clang", "kiln"), ("kiln", "kiln")] {
            let exe = p(&format!("{caller}-{callee}"));
            if let Err(e) = link_and_run(
                &p(&format!("{caller}-caller.o")),
                &p(&format!("{callee}-callee.o")),
                &exe,
            ) {
                errs.push(format!("{caller} caller, {callee} callee: {e}"));
            }
        }
        if errs.is_empty() { Ok(()) } else { Err(errs.join("\n")) }
    })();
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = r {
        panic!("{e}");
    }
}
