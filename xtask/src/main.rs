//! `cargo xtask <command>` — the kiln validation harness. See `docs/VALIDATION.md`.
#![forbid(unsafe_code)]

use std::process::ExitCode;

mod test;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  test        run tests/conformance and tests/differential through the kiln binary
              (`cargo xtask test --help` for options; contract in spec/harness.md)
  bench       run bench/ and compare against bench/baselines
  libcxx      run the libc++ lit suite with kiln as the compiler
  ci          fmt --check, clippy -D warnings, cargo test, xtask test
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("test") => test::main(args),
        Some("bench" | "libcxx") => {
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

fn ci() -> ExitCode {
    let steps: &[&[&str]] = &[
        &["fmt", "--all", "--check"],
        &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        &["test", "--workspace"],
    ];
    for step in steps {
        eprintln!("xtask ci: cargo {}", step.join(" "));
        let status = std::process::Command::new(env!("CARGO")).args(*step).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("xtask ci: failed with {s}");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask ci: could not run cargo: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
