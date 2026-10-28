//! `cargo xtask <command>` — the kiln validation harness. See `docs/VALIDATION.md`.
#![forbid(unsafe_code)]

use std::process::ExitCode;

mod bench;
mod test;
mod util;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  test        run tests/conformance and tests/differential through the kiln binary
              (`cargo xtask test --help` for options; contract in spec/harness.md)
  bench       measure bench/ compile time and memory against bench/baselines
              (`cargo xtask bench --help` for options; contract in spec/bench.md)
  libcxx      run the libc++ lit suite with kiln as the compiler
  ci          fmt --check, clippy -D warnings, cargo test, and (when clang++-19 is
              on PATH) xtask test --compiler clang++-19 as a corpus self-check
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("test") => test::main(args),
        Some("bench") => bench::main(args),
        Some("libcxx") => {
            eprintln!("xtask: not implemented yet (M0 work package)");
            ExitCode::FAILURE
        }
        Some("ci") => ci(),
        _ => {
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// The compiler `ci` self-checks the conformance corpus with (`spec/harness.md`
/// "CI"): the corpus must pass with it, or the corpus itself is wrong.
const CORPUS_COMPILER: &str = "clang++-19";

fn ci() -> ExitCode {
    let steps: &[&[&str]] = &[
        &["fmt", "--all", "--check"],
        &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        &["test", "--workspace"],
    ];
    for step in steps {
        eprintln!("xtask ci: cargo {}", step.join(" "));
        let mut cmd = std::process::Command::new(env!("CARGO"));
        cmd.args(*step);
        if !ci_step(&mut cmd) {
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = util::identify(std::path::Path::new(CORPUS_COMPILER)) {
        eprintln!(
            "xtask ci: skipping the corpus self-check (cargo xtask test --compiler \
             {CORPUS_COMPILER}): {e}"
        );
        return ExitCode::SUCCESS;
    }
    eprintln!("xtask ci: cargo xtask test --compiler {CORPUS_COMPILER}");
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("xtask ci: cannot locate the xtask binary: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["test", "--compiler", CORPUS_COMPILER]);
    if ci_step(&mut cmd) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn ci_step(cmd: &mut std::process::Command) -> bool {
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("xtask ci: failed with {s}");
            false
        }
        Err(e) => {
            eprintln!("xtask ci: could not run {cmd:?}: {e}");
            false
        }
    }
}
