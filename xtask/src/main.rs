//! `cargo xtask <command>` — the kiln validation harness. See `docs/VALIDATION.md`.
#![forbid(unsafe_code)]

use std::process::ExitCode;

mod bench;
mod clang_ast;
mod gen_pp;
mod test;
mod util;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  test        run tests/conformance and tests/differential through the kiln binary
              (`cargo xtask test --help` for options; contract in spec/harness.md)
  bench       measure bench/ compile time and memory against bench/baselines
              (`cargo xtask bench --help` for options; contract in spec/bench.md)
  clang-ast   print clang 19's AST of a file in the kiln dump format (spec/ast-dump.md §6;
              `cargo xtask clang-ast --help` for options)
  gen-pp-corpus
              write the generated per-header preprocessor corpus under
              tests/differential/pp (`cargo xtask gen-pp-corpus --help`)
  libcxx      run the libc++ lit suite with kiln as the compiler
  ci          the merge gate: fmt --check, clippy -D warnings, cargo test, then the
              corpus twice: xtask test --compiler clang++-19 (the corpus self-check;
              skipped when clang++-19 is not on PATH) and xtask test with kiln; fails
              on any FAIL, XPASS or ERROR in either (contract in spec/harness.md \"CI\")
              `--quick` skips the clang self-check (local iteration only)
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("test") => test::main(args),
        Some("bench") => bench::main(args),
        Some("clang-ast") => clang_ast::main(args),
        Some("gen-pp-corpus") => gen_pp::main(args),
        Some("libcxx") => {
            eprintln!("xtask: not implemented yet (M0 work package)");
            ExitCode::FAILURE
        }
        Some("ci") => ci(args),
        _ => {
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// The compiler `ci` self-checks the conformance corpus with (`spec/harness.md`
/// "CI"): the corpus must pass with it, or the corpus itself is wrong.
const CORPUS_COMPILER: &str = "clang++-19";

const CI_USAGE: &str = "\
usage: cargo xtask ci [--quick]

  --quick     skip the clang++-19 corpus self-check (local iteration; merges
              and GitHub CI use the full form)
";

fn ci(args: impl Iterator<Item = String>) -> ExitCode {
    let mut quick = false;
    for arg in args {
        match arg.as_str() {
            "--quick" => quick = true,
            "-h" | "--help" => {
                print!("{CI_USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => {
                eprint!("xtask ci: unknown argument `{arg}`\n\n{CI_USAGE}");
                return ExitCode::from(2);
            }
        }
    }
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
    // The kiln binary the kiln-mode run tests: built here (a no-op when
    // `cargo test` above already built it), from this checkout, into
    // `$CARGO_TARGET_DIR`.
    let root = test::repo_root();
    let kiln = match test::target_dir(&root).and_then(|t| util::build_kiln(&root, &t, "ci", false))
    {
        Ok(k) => k,
        Err(e) => {
            eprintln!("xtask ci: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Both corpus runs are in-process, not a re-exec of `current_exe()`: the
    // `cargo test` step above may have rebuilt and replaced this very binary
    // (its path is then "<exe> (deleted)" and the exec fails). Both always
    // run (when not skipped), so one failing does not hide the other.
    let self_check = if quick {
        test::CorpusOutcome::Skipped("--quick".to_owned())
    } else if let Err(e) = util::identify(std::path::Path::new(CORPUS_COMPILER)) {
        test::CorpusOutcome::Skipped(format!("{CORPUS_COMPILER} not runnable: {e}"))
    } else {
        ci_corpus(&format!("--compiler {CORPUS_COMPILER}"), Some(CORPUS_COMPILER.into()))
    };
    // kiln-mode always runs: kiln as the compiler, clang++-19 as the oracle
    // (and kiln's linker), so without clang it fails rather than passing
    // unchecked.
    let kiln_mode = ci_corpus(&format!("--compiler {}", kiln.display()), Some(kiln));

    let runs = [("clang++-19 self-check", self_check), ("kiln", kiln_mode)];
    for (name, run) in &runs {
        eprintln!("xtask ci: corpus ({name}): {}", run.line());
    }
    let outcomes: Vec<test::CorpusOutcome> = runs.into_iter().map(|(_, r)| r).collect();
    if test::ci_corpus_passes(&outcomes) {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "xtask ci: FAILED: a corpus run has a FAIL, XPASS or ERROR, or could not run \
             (see above)"
        );
        ExitCode::FAILURE
    }
}

/// One `cargo xtask test` run over the default corpus, with `compiler`.
fn ci_corpus(what: &str, compiler: Option<std::path::PathBuf>) -> test::CorpusOutcome {
    eprintln!("xtask ci: cargo xtask test {what}");
    let opts = test::Options { compiler, ..test::Options::default() };
    let start = std::time::Instant::now();
    let run = test::execute(&opts);
    if let Err(e) = &run {
        eprintln!("xtask test: error: {e}");
    }
    test::CorpusOutcome::of(&run, start.elapsed())
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
