//! Runner-level tests: CLI parsing, and end-to-end runs with clang over the
//! seed corpus and the self-test fixtures in `xtask/fixtures/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::directives::{self, Expect, StdoutExpect};
use super::runner::Status;
use super::*;

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn cli_parsing() {
    let o = parse_args(strings(&[
        "--compiler=clang++-19",
        "--filter",
        "lex",
        "-j",
        "3",
        "--update",
        "-v",
        "--timeout",
        "2.5",
        "--oracle",
        "clang++",
        "--gcc=/opt/g++",
        "--external",
        "/x/corpus",
        "--no-oracle",
        "--no-gcc",
        "--pp-compare=text",
        "--mem-limit",
        "512",
        "--report-parity",
        "docs/status/pp-parity.md",
        "tests/a",
        "b.cpp",
    ]))
    .unwrap()
    .unwrap();
    assert_eq!(
        o,
        Options {
            compiler: Some("clang++-19".into()),
            oracle: Some("clang++".into()),
            gcc: Some("/opt/g++".into()),
            no_gcc: true,
            no_oracle: true,
            meta_header: false,
            external: Some("/x/corpus".into()),
            filter: Some("lex".into()),
            shard: None,
            update: true,
            jobs: Some(3),
            run_timeout: Duration::from_millis(2500),
            mem_limit_mib: 512,
            verbose: true,
            pp_compare: Some(PpCompare::Text),
            report_parity: Some("docs/status/pp-parity.md".into()),
            paths: vec!["tests/a".into(), "b.cpp".into()],
            toolchain: None,
            require_gcc: false,
            require_debuggers: false,
            debug_sweep: false,
            bmi_kind: None,
        }
    );
    assert_eq!(parse_args(strings(&["--help"])).unwrap(), None);
    assert_eq!(parse_args(Vec::new()).unwrap().unwrap(), Options::default());
    assert_eq!(Options::default().mem_limit_mib, 8192, "the cap is on by default");
    assert_eq!(parse_args(strings(&["--mem-limit=0"])).unwrap().unwrap().mem_limit_mib, 0);
    for bad in [
        &["--jobs", "0"][..],
        &["--compiler"],
        &["--bogus"],
        &["--update=1"],
        &["--timeout", "-1"],
        &["--pp-compare", "bytes"],
        &["--mem-limit", "-1"],
        &["--mem-limit", "8G"],
        &["--mem-limit"],
    ] {
        assert!(parse_args(strings(bad)).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn json_field_extraction() {
    let j = r#"{"packages":[],"target_directory":"/a/b \"c\"\\d\u0041","version":1}"#;
    assert_eq!(json_string_field(j, "target_directory").as_deref(), Some("/a/b \"c\"\\dA"));
    assert_eq!(json_string_field(j, "missing"), None);
}

/// The end-to-end tests need clang 19 (the default oracle and the compiler
/// the fixtures are written for); without it they are skipped with a note.
fn have_clang() -> bool {
    let ok = detect_kind(Path::new("clang++-19")).is_ok();
    if !ok {
        eprintln!("clang++-19 not installed; skipping an end-to-end harness test");
    }
    ok
}

/// clang 19 as both the compiler and the oracle: the fixtures' expected
/// messages name `clang++-19`, whatever the default `KILN_ORACLE` is. The
/// toolchain (`requires: llvm`) is release 19's too, so these tests do not
/// read `KILN_ORACLE` and `ci --oracle-only` need not rerun them
/// (spec/harness.md "CI").
fn clang_opts(paths: &[PathBuf]) -> Options {
    Options {
        compiler: Some("clang++-19".into()),
        oracle: Some("clang++-19".into()),
        toolchain: Some(crate::oracle::Oracle { version: 19 }),
        jobs: Some(2),
        paths: paths.to_vec(),
        ..Options::default()
    }
}

fn report(results: &[TestResult]) -> String {
    results.iter().map(|r| report::format_result(r, true)).collect()
}

/// The hand-written corpus: the default roots without the generated
/// header corpus under `tests/differential/pp` (`cargo xtask gen-pp-corpus`;
/// `xtask ci` runs it) but with the hand-written `pp/torture`.
fn seed_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.join("tests/conformance")];
    if let Ok(dir) = std::fs::read_dir(root.join("tests/differential")) {
        let mut subdirs: Vec<PathBuf> = dir
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "pp"))
            .collect();
        subdirs.sort();
        roots.extend(subdirs);
    }
    roots.push(root.join("tests/differential/pp/torture"));
    roots.push(root.join("tests/stress"));
    roots.retain(|p| p.is_dir());
    roots
}

/// The seed corpus is self-validating: it must pass with clang 19. The run
/// is most of this binary's time, so it uses every CPU rather than
/// `clang_opts`' two jobs.
#[test]
fn seed_corpus_passes_with_clang() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let opts = Options { jobs: None, ..clang_opts(&seed_roots(&root)) };
    let results = execute(&opts).expect("runner infrastructure works");
    let all = report(&results);
    assert!(results.iter().all(|r| !r.status.is_failure()), "{all}");
    let counts = report::Counts::of(&results);
    assert!(counts.get(Status::Pass) >= 6, "{all}");
    assert!(counts.get(Status::XFail) >= 1, "{all}");
    assert!(counts.get(Status::Skip) >= 2, "{all}");
}

/// The seed corpus exercises every directive, so it keeps demonstrating
/// the format (static: it needs no compiler).
#[test]
fn seed_corpus_exercises_every_directive() {
    let root = repo_root();
    let cases = discover::discover(&root, &seed_roots(&root), None).unwrap();
    let mut seen = BTreeSet::new();
    for c in &cases {
        // Lossy, as the runner reads tests: a lexer test may hold bytes
        // that are not UTF-8 (lex.phases/invalid-utf8-in-comment-*).
        let src = String::from_utf8_lossy(&std::fs::read(&c.path).unwrap()).into_owned();
        let s = directives::parse(&src, c.lang).unwrap();
        seen.insert(s.expect.as_str().to_owned());
        let mut flag = |cond: bool, name: &str| _ = cond && seen.insert(name.to_owned());
        flag(src.contains("//@ std:"), "std");
        flag(!s.args.is_empty(), "args");
        flag(s.diags.iter().any(|d| d.severity == directives::Severity::Note), "diag-note");
        flag(!s.diags.is_empty(), "diag");
        flag(s.diags.iter().any(|d| d.clang_dup), "diag-dup");
        flag(s.strict_diags, "strict-diags");
        flag(matches!(s.stdout, Some(StdoutExpect::Inline(_))), "stdout");
        flag(matches!(s.stdout, Some(StdoutExpect::File(_))), "stdout-file");
        flag(s.exit.is_some(), "exit");
        flag(!s.run_args.is_empty(), "run-args");
        flag(matches!(s.stdin, Some(directives::StdinInput::Inline(_))), "run-stdin");
        flag(matches!(s.stdin, Some(directives::StdinInput::File(_))), "run-stdin-file");
        flag(!s.env.is_empty(), "env");
        flag(src.contains("//@ oracle:"), "oracle");
        flag(!s.oven.is_empty(), "oven");
        flag(!s.skip.is_empty(), "skip");
        flag(!s.xfail.is_empty(), "xfail");
        flag(
            s.expect == Expect::Pp
                && c.path
                    .with_extension(format!(
                        "{}.expected",
                        c.path.extension().unwrap().to_string_lossy()
                    ))
                    .is_file(),
            "pp-expected",
        );
        flag(c.is_header, "header-test");
        flag(src.contains("//@ pp-compare:"), "pp-compare");
        flag(s.pp_linemarkers, "pp-linemarkers");
        flag(s.args.iter().any(|a| a.contains("%{")), "placeholder");
        flag(c.lang == directives::Lang::C, "c-test");
        flag(s.ast_filter.is_some(), "ast-filter");
        flag(s.ast_implicit, "ast-implicit");
        flag(s.inst_report.is_some(), "inst-report");
        flag(s.requires_target.is_some(), "requires");
        let clang_req = s.requires_clang.is_some() || s.requires_clang_below.is_some();
        flag(clang_req, "requires-clang");
        flag(s.requires_llvm.is_some() || s.requires_llvm_below.is_some(), "requires-llvm");
    }
    let want = [
        "args",
        "ast",
        "ast-filter",
        "ast-implicit",
        "c-test",
        "compile-fail",
        "compile-ok",
        "diag",
        "diag-dup",
        "diag-note",
        "env",
        "exit",
        "header-test",
        "inst-report",
        "ir",
        "layout",
        "no-crash",
        "oracle",
        "oven",
        "pp",
        "pp-compare",
        "pp-expected",
        "pp-linemarkers",
        "placeholder",
        "requires",
        "requires-clang",
        "requires-llvm",
        "run",
        "run-args",
        "run-stdin",
        "run-stdin-file",
        "skip",
        "std",
        "stdout",
        "stdout-file",
        "strict-diags",
        "vtables",
        "xfail",
    ];
    let missing: Vec<_> = want.iter().filter(|w| !seen.contains(**w)).collect();
    assert!(missing.is_empty(), "seed corpus no longer exercises {missing:?}");
}

/// Each fixture's expected status is its file-name prefix.
fn check_fixture_statuses(results: &[TestResult], expected_count: usize) {
    let all = report(results);
    assert_eq!(results.len(), expected_count, "{all}");
    for r in results {
        let file = r.name.rsplit('/').next().unwrap();
        let want = match file.split('-').next().unwrap() {
            "pass" => Status::Pass,
            "fail" => Status::Fail,
            "xfail" => Status::XFail,
            "xpass" => Status::XPass,
            "skip" => Status::Skip,
            "error" => Status::Error,
            "killed" => Status::Killed,
            p => panic!("fixture {file} has no status prefix ({p})"),
        };
        assert_eq!(
            r.status,
            want,
            "{}\n--- full report ---\n{all}",
            report::format_result(r, true)
        );
    }
}

#[test]
fn fixtures_produce_expected_statuses() {
    if !have_clang() {
        return;
    }
    let dir = repo_root().join("xtask/fixtures/basic");
    let mut o = clang_opts(&[dir]);
    o.run_timeout = Duration::from_secs(1);
    let results = execute(&o).unwrap();
    // 43 tests: helper.h and Inputs/* are not discovered.
    check_fixture_statuses(&results, 43);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    assert!(by_name("fail-timeout.cpp").details.contains("timed out"));
    assert!(by_name("fail-crash.cpp").details.contains("signal"));
    assert!(by_name("fail-no-crash-crash.cpp").details.contains("compiler crashed"));
    assert!(by_name("fail-wrong-stdout.cpp").details.contains("-no\n    +yes"));
    assert!(
        by_name("fail-wrong-exit.cpp").details.contains("expected exit code 0, got exit code 4")
    );
    assert!(by_name("fail-diag-wrong-line.cpp").details.contains("3:11 error"));
    assert!(by_name("fail-strict-unexpected.cpp").details.contains("unexpected diagnostics"));
    assert!(by_name("fail-verify.cpp").details.contains("diagnostics expected but not seen"));
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    has("fail-pp-golden.cpp", "output differs from fail-pp-golden.cpp.expected (token-wise):");
    has("fail-pp-golden.cpp", "-1:5  3  (pp-number)\n    +1:5  2  (pp-number)\n");
    has("fail-pp-tokens-glued.cpp", "-1:1  ++\n    +1:1  +\n    +1:3  +\n");
    has("fail-pp-text-mode.cpp", "(whitespace-normalized text):");
    has("fail-pp-text-mode.cpp", "-f(a+b);\n    +f( a+b );");
    assert_eq!(
        by_name("skip-kiln-specific.cpp").reason.as_deref(),
        Some("not supported by this compiler (kiln-specific `oven:` directive)")
    );
    assert_eq!(
        by_name("skip-requires-pinned-target.cpp").reason.as_deref(),
        Some("requires target-x86_64; the test targets aarch64 (`aarch64-unknown-linux-gnu`)")
    );
    let reason = by_name("skip-requires-clang.cpp").reason.clone().unwrap_or_default();
    assert!(reason.starts_with("requires clang>=999.0.0; the compiler is clang 19."), "{reason}");
    let reason = by_name("skip-min-oracle.cpp").reason.clone().unwrap_or_default();
    assert!(reason.starts_with("min-oracle 999; the compiler is clang 19."), "{reason}");
    assert!(by_name("pass-module.cpp").log.iter().any(|l| l.contains("--precompile")));
    has(
        "error-module-missing-file.cpp",
        "`module: fx_none=Inputs/no-such-unit.cppm`: no such file",
    );
    assert!(by_name("error-bad-directive.cpp").details.contains("unknown directive `frobnicate`"));
    assert!(by_name("error-missing-stdin-file.cpp").details.contains("cannot read run-stdin-file"));
    let hang = by_name("error-oracle-binary-hangs.cpp");
    assert!(hang.details.contains("the oracle's binary") && hang.details.contains("timed out"));
    let crash = by_name("error-oracle-binary-crashes.cpp");
    assert!(crash.details.contains("the oracle's binary") && crash.details.contains("signal"));
}

/// `oven:` budgets against a stand-in kiln that compiles with clang and
/// "reports" the canned `<test>.oven.jsonl` beside each fixture.
#[test]
fn oven_budgets_with_a_fake_kiln() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let mut o = clang_opts(&[root.join("xtask/fixtures/oven")]);
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    o.no_oracle = true;
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 9);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    has("fail-over-total-budget.cpp", "oven: total: steps is 116, over the budget of 115");
    has("fail-over-located-budget.cpp", "oven: at 4:15: calls is 1, over the budget of 0");
    has("fail-no-evaluation-at-location.cpp", "no evaluation reported at 3 of the test file");
    has("fail-located-budget-ignores-other-files.cpp", "no evaluation reported at 5");
    has("fail-no-report.cpp", "wrote no readable constexpr report");
    has("fail-malformed-report.cpp", "the total line disagrees with the evaluations");
    has("fail-compile-error-wins.cpp", "expected compile-ok");
    // The report total is logged for a passing test, too.
    assert!(by_name("pass-budgets.cpp").log.iter().any(|l| l.contains("steps=116")), "log");
}

/// Memory limits (`spec/harness.md` "Resource limits") against a stand-in
/// kiln that allocates what `// fake-hog-mib: N` asks for.
#[test]
fn memory_limits_with_a_fake_hog() {
    if !have_clang() {
        return;
    }
    if process::run(Command::new("python3").arg("--version"), Duration::from_secs(30)).is_err() {
        eprintln!("python3 not installed; skipping the memory-limit fixtures");
        return;
    }
    let root = repo_root();
    let dir = root.join("xtask/fixtures/memory");
    let mut o = clang_opts(std::slice::from_ref(&dir));
    o.compiler = Some(root.join("xtask/fixtures/fake-hog/kiln"));
    o.no_oracle = true;
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 8);
    let by_name = |rs: &[TestResult], n: &str| rs.iter().find(|r| r.name.ends_with(n)).cloned();
    let has = |rs: &[TestResult], n: &str, what: &str| {
        let r = by_name(rs, n).unwrap_or_else(|| panic!("{n} did not run"));
        let text = report::format_result(&r, false);
        assert!(text.contains(what), "{n}: {what:?} not in\n{text}");
    };
    has(
        &results,
        "killed-compiler-hog.cpp",
        "KILLED xtask/fixtures/memory/killed-compiler-hog.cpp (memory limit 8192 MiB)\n",
    );
    has(
        &results,
        "killed-compiler-hog.cpp",
        "the compiler ran out of memory under the 8192 MiB address-space cap + 1024 MiB kiln \
         stack reservation",
    );
    has(&results, "killed-compiler-hog.cpp", "MemoryError");
    has(&results, "killed-compiler-hog.cpp", "[mem<=9216 MiB]");
    has(&results, "killed-xfail-does-not-mask.cpp", "(memory limit 8192 MiB)");
    has(&results, "killed-compiler-over-budget.cpp", "(memory limit 256 MiB)");
    has(&results, "killed-run-binary-over-budget.cpp", "the test binary ran out of memory");
    has(&results, "killed-run-binary-over-budget.cpp", "std::bad_alloc");
    has(&results, "killed-run-binary-over-budget.cpp", "[mem<=256 MiB]");
    let counts = report::Counts::of(&results);
    let summary = report::format_summary(&counts, results.len(), Duration::ZERO);
    assert!(summary.starts_with("test result: FAILED.") && summary.contains("; 4 killed;"));
    assert!(!CorpusOutcome::Ran { counts, total: 8, elapsed: Duration::ZERO }.passes());

    // `--mem-limit 0` lifts the default cap but not a test's `mem<=N`.
    o.mem_limit_mib = 0;
    o.filter = Some("budget".into());
    let results = execute(&o).unwrap();
    let status = |rs: &[TestResult], n: &str| by_name(rs, n).map(|r| r.status);
    assert_eq!(results.len(), 4, "{}", report(&results));
    assert_eq!(status(&results, "error-bad-budget.cpp"), Some(Status::Error));
    assert_eq!(status(&results, "killed-compiler-over-budget.cpp"), Some(Status::Killed));
    assert_eq!(status(&results, "killed-run-binary-over-budget.cpp"), Some(Status::Killed));
    assert_eq!(status(&results, "pass-budget-plus-stack-allowance.cpp"), Some(Status::Pass));

    // A lower `--mem-limit` reaches the test binary, which gets no stack
    // allowance.
    o.mem_limit_mib = 256;
    o.filter = Some("pass-run-binary-fits".into());
    let results = execute(&o).unwrap();
    assert_eq!(status(&results, "pass-run-binary-fits.cpp"), Some(Status::Killed));
    has(&results, "pass-run-binary-fits.cpp", "(memory limit 256 MiB)");
}

/// The GCC oracle (`spec/harness.md` "GCC oracle") against the fake kiln
/// and the fake GCC of `xtask/fixtures/fake-gcc`: verdicts, run output,
/// and the advisory outcome of a GCC crash, ICE or `sorry`.
#[test]
fn gcc_oracle_with_a_fake_gcc() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let dir = root.join("xtask/fixtures/gcc");
    let fake_gcc = root.join("xtask/fixtures/fake-gcc/g++");
    let mut o = clang_opts(std::slice::from_ref(&dir));
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    o.gcc = Some(fake_gcc.clone());
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 13);
    let by_name = |rs: &[TestResult], n: &str| rs.iter().find(|r| r.name.ends_with(n)).cloned();
    let text = |rs: &[TestResult], n: &str| report::format_result(&by_name(rs, n).unwrap(), false);
    let has = |n: &str, what: &str| {
        let t = text(&results, n);
        assert!(t.contains(what), "{n}: {what:?} not in\n{t}");
    };
    has("pass-advisory-ice.cpp", "[advisory: oracle crashed (internal compiler error)]");
    has("pass-advisory-sorry.cpp", "[advisory: oracle crashed (sorry, unimplemented)]");
    has("pass-advisory-signal.cpp", "[advisory: oracle crashed (");
    has("error-gcc-rejects-compile-ok.cpp", "rejects this compile-ok test");
    has("error-gcc-accepts-compile-fail.cpp", "accepts this compile-fail test");
    has("error-xfail-does-not-mask-gcc.cpp", "rejects this compile-ok test");
    has("fail-run-stdout-differs.cpp", "stdout differs from the GCC oracle");
    has("fail-run-exit-differs.cpp", "exit differs from the GCC oracle");
    let counts = report::Counts::of(&results);
    assert_eq!(counts.advisory(), 3);
    // Every fixture GCC gave a verdict on (all but the advisory three and
    // the skipped one) counts as a comparison (spec/harness.md "Output").
    let compared: Vec<&str> =
        results.iter().filter(|r| r.gcc_compared).map(|r| r.name.as_str()).collect();
    assert_eq!(compared.len(), 9, "{compared:?}");
    assert!(results.iter().all(|r| !(r.gcc_compared && r.advisory.is_some())));
    let summary = report::format_summary(&counts, results.len(), Duration::ZERO);
    assert!(summary.contains("; 9 compared with gcc; 3 advisory; 13 tests"), "{summary}");
    // Without a GCC the comparisons are dropped, never failed: only kiln's
    // own checks remain (the `error-` fixtures pass, the run diffs pass).
    o.no_gcc = true;
    let results = execute(&o).unwrap();
    assert!(
        results.iter().all(|r| !r.status.is_failure() && r.advisory.is_none() && !r.gcc_compared),
        "{}",
        report(&results)
    );
    // `skip(gcc)` applies when GCC is the compiler under test.
    let mut g = clang_opts(std::slice::from_ref(&dir));
    g.compiler = Some(fake_gcc);
    g.filter = Some("skip-gcc".into());
    let results = execute(&g).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, Status::Skip, "{}", report(&results));
    // An explicitly named GCC that does not run is an infrastructure error.
    o.no_gcc = false;
    o.gcc = Some(root.join("xtask/fixtures/no-such-gcc"));
    assert!(execute(&o).unwrap_err().contains("the GCC oracle"));
}

/// `KILN_REQUIRE_DEBUGGERS` is required only by exactly `1`, and the
/// debugger corpus is a default root (WP-816, spec/harness.md "Debugger
/// tests"; the SKIP/ERROR decision is `runner::compat_tests`).
#[test]
fn debugger_corpus_is_a_default_root() {
    assert!(require_debuggers_from(Some("1")) && require_debuggers_from(Some(" 1\n")));
    assert!(!require_debuggers_from(None) && !require_debuggers_from(Some("0")));
    assert!(!require_debuggers_from(Some("")) && !require_debuggers_from(Some("true")));
    let root = repo_root();
    let roots = discover::default_roots(&root);
    assert!(roots.contains(&root.join("tests/debuginfo")), "{roots:?}");
    assert!(roots.contains(&root.join("tests/conformance")), "{roots:?}");
}

/// `KILN_REQUIRE_GCC=1` ([`Options::require_gcc`]): a run without a usable
/// GCC oracle is an infrastructure error (exit 2), never a run with the
/// `oracle: gcc` comparisons dropped (spec/harness.md "GCC oracle").
#[test]
fn require_gcc_makes_a_missing_gcc_an_error() {
    let root = repo_root();
    let fake_gcc = root.join("xtask/fixtures/fake-gcc/g++");
    let mut o = Options { gcc: Some(fake_gcc.clone()), require_gcc: true, ..Options::default() };
    let (path, version) = gcc_oracle(&o).unwrap().expect("the fake GCC is a GCC");
    assert_eq!(path, fake_gcc);
    assert_eq!(version, "g++ (GCC) 16.1.0-fake (xtask fixture)");
    // Dropping it explicitly is still an error under the requirement.
    o.no_gcc = true;
    let e = gcc_oracle(&o).unwrap_err();
    assert!(e.starts_with("KILN_REQUIRE_GCC=1 and no usable GCC oracle") && e.contains("--no-gcc"));
    o.no_gcc = false;
    o.no_oracle = true;
    assert!(gcc_oracle(&o).unwrap_err().contains("--no-oracle"));
    // `execute` fails before running a test (no clang needed: --no-oracle).
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    o.paths = vec![root.join("xtask/fixtures/gcc")];
    assert!(execute(&o).unwrap_err().starts_with("KILN_REQUIRE_GCC=1"));
    // The default GCC (`g++-16`, KILN_GCC unset) missing: dropped without
    // the requirement, an error with it. (Skipped where g++-16 is on PATH.)
    let o = Options { require_gcc: true, ..Options::default() };
    if crate::util::identify(Path::new(crate::oracle::GCC_DEFAULT)).is_err() {
        let e = gcc_oracle_with(&o, None).unwrap_err();
        assert!(e.contains("set KILN_GCC or --gcc to GCC 16.1"), "{e}");
        let o = Options { require_gcc: false, ..o };
        assert_eq!(gcc_oracle_with(&o, None), Ok(None));
    }
    // A named GCC that does not run stays the error it was.
    let o = Options { require_gcc: false, ..Options::default() };
    let missing = root.join("xtask/fixtures/no-such-gcc").display().to_string();
    assert!(gcc_oracle_with(&o, Some(&missing)).unwrap_err().contains("the GCC oracle"));
}

#[test]
fn oracle_mismatches_are_detected() {
    if !have_clang() {
        return;
    }
    if detect_kind(Path::new("clang++-22")).is_err() {
        eprintln!("clang++-22 not installed; skipping the oracle-mismatch test");
        return;
    }
    let mut o = clang_opts(&[repo_root().join("xtask/fixtures/oracle")]);
    o.oracle = Some("clang++-22".into());
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 6);
}

#[test]
fn filter_and_no_tests() {
    if !have_clang() {
        return;
    }
    let mut o = clang_opts(&[repo_root().join("xtask/fixtures/basic")]);
    o.filter = Some("pass-qualified".into());
    let results = execute(&o).unwrap();
    assert_eq!(results.len(), 1);
    o.filter = Some("no-such-test".into());
    assert_eq!(execute(&o).unwrap_err(), "no tests selected");
    let mut o = clang_opts(&[]);
    o.compiler = Some("/nonexistent/compiler".into());
    assert!(execute(&o).unwrap_err().contains("cannot run compiler"));
}

/// `expect: debugger` (WP-814): gdb and lldb scripts against clang 19's
/// `-O0 -g` binaries, one fixture per outcome.
#[test]
fn debugger_fixtures_with_clang() {
    let on_path = |n: &str| {
        std::env::var_os("PATH")
            .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(n).is_file()))
    };
    if !have_clang() || !on_path("gdb") || !on_path("lldb-19") {
        return;
    }
    let results = execute(&clang_opts(&[repo_root().join("xtask/fixtures/debugger")])).unwrap();
    check_fixture_statuses(&results, 4);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let d = &by_name("fail-gdb-wrong-value.cpp").details;
    assert!(d.contains("gdb: no line contains `$1 = 43`"), "{d}");
    let d = &by_name("fail-gdb-out-of-order.cpp").details;
    assert!(d.contains("gdb: no line contains `$1 = 1`"), "{d}");
    let d = &by_name("error-debugger-no-check.cpp").details;
    assert!(d.contains("`gdb:` commands without a `gdb-check`"), "{d}");
}

/// `expect: ast` with clang as the compiler under test: its JSON dump is
/// normalized in-process and compared with the golden.
#[test]
fn ast_fixtures_with_clang() {
    if !have_clang() {
        return;
    }
    let results = execute(&clang_opts(&[repo_root().join("xtask/fixtures/ast/clang")])).unwrap();
    check_fixture_statuses(&results, 8);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    has("fail-golden-differs.cpp", "-  `-IntegerLiteral 'int' 2\n");
    has("fail-golden-differs.cpp", "+  `-IntegerLiteral 'int' 1\n");
    has("fail-compile-error.cpp", "expected the AST dump, but the compiler failed");
    has("fail-unexpected-diag.cpp", "unexpected diagnostics");
    has("error-missing-golden.cpp", "`cargo xtask clang-ast <test>`");
    has("error-unsupported-construct.cpp", "unsupported SourceLocExpr");
    let logged = |n: &str, what: &str| by_name(n).log.iter().any(|l| l.contains(what));
    assert!(
        logged("pass-dump.cpp", "-Xclang -ast-dump "),
        "constexpr values come from the text dump"
    );
    assert!(!logged("pass-filter.cpp", "-Xclang -ast-dump "), "no constexpr value, no text dump");
}

/// `--shard K/N` (spec/harness.md "Command line"; the CI fan-out of
/// "CI"): shards 1/3, 2/3 and 3/3 of a fixture directory partition its
/// sorted test list round-robin, each test with the outcome it has in the
/// unsharded run, and a shard past the end of a short list is empty
/// rather than an error.
#[test]
fn shards_partition_the_test_list() {
    assert_eq!(Shard::parse("2/3"), Ok(Shard { index: 2, count: 3 }));
    assert_eq!(Shard::parse("1/1"), Ok(Shard { index: 1, count: 1 }));
    for bad in ["0/3", "4/3", "3", "a/b", "1/0", "", "1/3/5", "-1/3"] {
        assert!(Shard::parse(bad).is_err(), "{bad:?} should be rejected");
    }
    assert_eq!(Shard { index: 2, count: 6 }.to_string(), "2/6");
    let o = parse_args(strings(&["--shard=2/4"])).unwrap().unwrap();
    assert_eq!(o.shard, Some(Shard { index: 2, count: 4 }));
    assert!(parse_args(strings(&["--shard", "5/4"])).is_err());
    assert!(parse_args(strings(&["--shard"])).is_err());
    if !have_clang() {
        return;
    }
    let dir = repo_root().join("xtask/fixtures/ast/clang");
    let outcomes = |shard| {
        let o = Options { shard, ..clang_opts(std::slice::from_ref(&dir)) };
        let results = execute(&o).unwrap();
        results.into_iter().map(|r| (r.name, r.status)).collect::<Vec<_>>()
    };
    let all = outcomes(None);
    assert_eq!(all.len(), 8);
    let mut union = Vec::new();
    for k in 1..=3 {
        let got = outcomes(Some(Shard { index: k, count: 3 }));
        let want: Vec<_> = all.iter().skip(k - 1).step_by(3).cloned().collect();
        assert_eq!(got, want, "shard {k}/3");
        union.extend(got);
    }
    union.sort();
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(union, sorted, "the shards are exactly the unsharded run");
    assert!(outcomes(Some(Shard { index: 9, count: 9 })).is_empty());
}

/// `expect: ast` with kiln (a fake that prints canned dumps) as the compiler
/// under test, and clang's normalized dump as the `oracle: clang` check.
#[test]
fn ast_fixtures_with_a_fake_kiln() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let mut o = clang_opts(&[root.join("xtask/fixtures/ast/kiln")]);
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 7);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    let logged = |n: &str, what: &str| by_name(n).log.iter().any(|l| l.contains(what));
    assert!(logged("pass-dump.cpp", "-fsyntax-only -fdump-ast "));
    assert!(logged("pass-filter-passed.cpp", "-fdump-ast -fdump-ast-filter=x "));
    has("fail-dump-differs.cpp", "dump differs from fail-dump-differs.cpp.expected");
    has("fail-oracle-disagrees.cpp", "the oracle's normalized dump differs");
    has("error-oracle-cannot-normalize.cpp", "cannot normalize the AST dump of clang++-19");
    // Without `oracle: clang` clang is never run.
    assert!(!logged("pass-dump.cpp", "clang++-19"));
    assert!(logged("pass-oracle-agrees.cpp", "clang++-19"));

    // `--no-oracle` drops the oracle half, and with it the oracle failures.
    o.no_oracle = true;
    let results = execute(&o).unwrap();
    for n in ["fail-oracle-disagrees.cpp", "error-oracle-cannot-normalize.cpp"] {
        let r = results.iter().find(|r| r.name.ends_with(n)).unwrap();
        assert_eq!(r.status, Status::Pass, "{}", report::format_result(r, true));
    }
}

/// `skip(clang)` implies `oracle: none` (spec/harness.md, directives): with
/// kiln under test the oracle (a clang) is not run for such a test, and
/// under clang the test is a SKIP.
#[test]
fn skip_clang_drops_the_oracle() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let mut o = clang_opts(&[root.join("xtask/fixtures/skip-clang")]);
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    let results = execute(&o).unwrap();
    let all = report(&results);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let logged = |n: &str, what: &str| by_name(n).log.iter().any(|l| l.contains(what));
    for n in ["run-skip-clang.cpp", "run-oracle.cpp"] {
        assert_eq!(by_name(n).status, Status::Pass, "{all}");
    }
    assert!(!logged("run-skip-clang.cpp", "clang++-19"), "{all}");
    assert!(logged("run-oracle.cpp", "clang++-19"), "{all}");
    let results = execute(&clang_opts(&[root.join("xtask/fixtures/skip-clang")])).unwrap();
    let r = results.iter().find(|r| r.name.ends_with("run-skip-clang.cpp")).unwrap();
    assert_eq!(r.status, Status::Skip);
}

/// `expect: layout` / `expect: vtables` with kiln (a fake that prints canned
/// dumps) as the compiler under test, compared with clang's dumps; without
/// an oracle there is no reference, so every test is a SKIP.
#[test]
fn dump_fixtures_with_a_fake_kiln() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let mut o = clang_opts(&[root.join("xtask/fixtures/dumps/kiln")]);
    o.compiler = Some(root.join("xtask/fixtures/fake-kiln/kiln"));
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 8);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    let logged = |n: &str, what: &str| by_name(n).log.iter().any(|l| l.contains(what));
    assert!(logged("pass-layout.cpp", "-fsyntax-only -fdump-record-layouts "));
    assert!(logged("pass-layout.cpp", "-fsyntax-only -Xclang -fdump-record-layouts-complete "));
    assert!(logged("pass-vtables.cpp", "-fsyntax-only -fdump-vtable-layouts "));
    assert!(logged("pass-vtables.cpp", "-S -emit-llvm -Xclang -fdump-vtable-layouts "));
    // The first differing block, both texts, and a diff.
    has(
        "fail-layout-differs.cpp",
        "record layout block 2 differs from the oracle's (2 blocks, oracle 2):\n\
         --- oracle ---\n             0 | struct A\n",
    );
    has("fail-layout-differs.cpp", "--- actual ---\n             0 | struct A\n");
    has("fail-layout-differs.cpp", "-         4 |   int i\n    +         2 |   int i\n");
    has("xfail-layout-differs.cpp", "record layout block 2 differs");
    has("fail-layout-no-dump.cpp", "the compiler dumped no record layout");
    has("fail-layout-diag-missing.cpp", "no such warning");
    has("fail-vtables-differs.cpp", "as sets of blocks (13 blocks, oracle 14)");
    has(
        "fail-vtables-differs.cpp",
        "block `Vtable for 'D' (13 entries).` differs (-oracle +actual):\n",
    );
    has("fail-vtables-differs.cpp", "-   9 | vcall_offset (-32)\n    +   9 | vcall_offset (-16)");
    has(
        "fail-vtables-differs.cpp",
        "only in the oracle's dump (1):\n    VTable indices for 'V' (1 entries).\n",
    );
    has("error-vtables-oracle-emits-none.cpp", "dumped no vtable; clang dumps a vtable only");

    o.no_oracle = true;
    let results = execute(&o).unwrap();
    assert_eq!(results.len(), 8);
    for r in &results {
        assert_eq!(r.status, Status::Skip, "{}", report::format_result(r, true));
        let reason = r.reason.as_deref().unwrap_or_default();
        assert!(reason.starts_with("no oracle (`expect: "), "{reason}");
    }
}

/// `expect: layout` / `expect: vtables` with clang as both the compiler and
/// the oracle (the corpus self-check): no comparison, but the dump must be
/// produced, non-empty, and the diagnostics must match.
#[test]
fn dump_fixtures_with_clang() {
    if !have_clang() {
        return;
    }
    let results = execute(&clang_opts(&[repo_root().join("xtask/fixtures/dumps/clang")])).unwrap();
    check_fixture_statuses(&results, 5);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    let has = |n: &str, what: &str| {
        let d = &by_name(n).details;
        assert!(d.contains(what), "{n}: {what:?} not in\n{d}");
    };
    has("fail-vtables-not-emitted.cpp", "the compiler dumped no vtable");
    has(
        "fail-layout-compile-error.cpp",
        "expected the record layout dump, but the compiler failed",
    );
    // One compile each: the oracle is the compiler under test.
    for n in ["pass-layout.cpp", "pass-vtables.cpp"] {
        assert_eq!(by_name(n).log.iter().filter(|l| l.starts_with("$ ")).count(), 1, "{n}");
    }
}

/// `--update` rewrites an existing `.expected`; it never creates one.
#[test]
fn update_rewrites_existing_goldens() {
    if !have_clang() {
        return;
    }
    let root = repo_root();
    let dir =
        target_dir(&root).unwrap().join(format!("xtask-selftest-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let test = dir.join("golden.cpp");
    let golden = dir.join("golden.cpp.expected");
    let fresh = dir.join("fresh.cpp");
    std::fs::write(&test, "//@ expect: pp\n#define V 3\nV\n").unwrap();
    std::fs::write(&golden, "stale\n").unwrap();
    std::fs::write(&fresh, "//@ expect: pp\nx\n").unwrap();
    let ast = dir.join("ast.cpp");
    let ast_golden = dir.join("ast.cpp.expected");
    std::fs::write(&ast, "//@ expect: ast\nint v = 3;\n").unwrap();
    std::fs::write(&ast_golden, "stale\n").unwrap();
    let fresh_ast = dir.join("fresh-ast.cpp");
    std::fs::write(&fresh_ast, "//@ expect: ast\nint w;\n").unwrap();

    let mut o = clang_opts(std::slice::from_ref(&dir));
    o.no_oracle = true;
    let status = |r: &[TestResult], n: &str| r.iter().find(|r| r.name.ends_with(n)).unwrap().status;
    // An `ast` test without a golden is broken, `--update` or not.
    let others_pass = |r: &[TestResult]| {
        assert_eq!(status(r, "/fresh-ast.cpp"), Status::Error, "{}", report(r));
        r.iter().filter(|r| !r.name.ends_with("/fresh-ast.cpp")).all(|r| r.status == Status::Pass)
    };
    let r = execute(&o).unwrap();
    assert_eq!(status(&r, "/golden.cpp"), Status::Fail);
    assert_eq!(status(&r, "/ast.cpp"), Status::Fail);

    o.update = true;
    let r = execute(&o).unwrap();
    assert!(others_pass(&r), "{}", report(&r));
    assert_eq!(r.iter().filter(|r| r.updated).count(), 2);
    assert_eq!(std::fs::read_to_string(&golden).unwrap(), "3\n");
    assert_eq!(
        std::fs::read_to_string(&ast_golden).unwrap(),
        "TranslationUnitDecl\n`-VarDecl v 'int' cinit\n  `-IntegerLiteral 'int' 3\n"
    );
    assert!(!dir.join("fresh.cpp.expected").exists());
    assert!(!dir.join("fresh-ast.cpp.expected").exists());

    o.update = false;
    let r = execute(&o).unwrap();
    assert!(others_pass(&r), "{}", report(&r));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// `spec/harness.md` "CI": `cargo xtask ci` passes only if neither corpus
/// run has a FAIL, XPASS or ERROR (XFAIL and SKIP are allowed), an
/// infrastructure error fails it, and a skipped run (the clang self-check
/// under `--quick` or without clang) does not.
/// `xtask ci` reruns a cargo step once on rustc's `error[E0463]` and
/// never on anything else (`spec/harness.md` "CI").
#[test]
fn ci_retries_only_missing_crates_once() {
    let e0463 = "   Compiling kiln_sema v0.0.1\n\
                 error[E0463]: can't find crate for `kiln_ast`\n \
                 --> crates/kiln_sema/src/lib.rs:3:1\n";
    assert!(ci_should_retry(e0463, false));
    assert!(!ci_should_retry(e0463, true), "only one retry");
    assert!(ci_should_retry("warning: x\n  error[E0463]: can't find crate for `std`\n", false));
    for other in [
        "",
        "error[E0425]: cannot find value `x` in this scope\n",
        "error: could not compile `kiln_sema`\n",
        "test foo ... FAILED\n",
        // Only rustc's diagnostic line counts, not a mention of it.
        "note: the test checks that error[E0463] is retried\n",
        "Diff in xtask/src/main.rs: \"error[E0463]\"\n",
    ] {
        assert!(!ci_should_retry(other, false), "{other:?}");
    }
}

#[test]
fn ci_gate_honours_failure_counts() {
    let mk = |status| TestResult {
        name: "tests/conformance/x.cpp".into(),
        status,
        reason: None,
        details: String::new(),
        log: Vec::new(),
        updated: false,
        advisory: None,
        gcc_compared: false,
        elapsed: Duration::ZERO,
    };
    let ran = |statuses: &[Status]| {
        let results: Vec<TestResult> = statuses.iter().map(|&s| mk(s)).collect();
        CorpusOutcome::of(&Ok(results), Duration::from_millis(1250))
    };
    let clean = ran(&[Status::Pass, Status::Pass, Status::XFail, Status::Skip]);
    assert!(clean.passes());
    assert_eq!(
        clean.line(),
        "test result: ok. 2 passed; 0 failed; 1 xfail; 0 xpass; 1 skipped; 0 errors; 0 killed; 4 tests in 1.25s"
    );
    for bad in [Status::Fail, Status::XPass, Status::Error] {
        let run = ran(&[Status::Pass, Status::XFail, bad]);
        assert!(!run.passes(), "{bad:?} must fail the gate");
        assert!(run.line().starts_with("test result: FAILED."), "{}", run.line());
        // One bad run fails ci whichever side it is on.
        assert!(!ci_corpus_passes(&[clean.clone(), run.clone()]), "{bad:?}");
        assert!(!ci_corpus_passes(&[run.clone(), clean.clone()]), "{bad:?}");
    }
    let infra = CorpusOutcome::of(&Err("cargo build -p kiln_driver failed".into()), Duration::ZERO);
    assert!(!infra.passes());
    assert_eq!(infra.line(), "error: cargo build -p kiln_driver failed");
    assert!(!ci_corpus_passes(&[clean.clone(), infra]));

    let skipped = CorpusOutcome::Skipped("--quick".into());
    assert!(skipped.passes());
    assert_eq!(skipped.line(), "skipped (--quick)");
    assert!(ci_corpus_passes(&[skipped.clone(), clean.clone()]));
    assert!(!ci_corpus_passes(&[skipped, ran(&[Status::XPass])]));
    assert!(ci_corpus_passes(&[clean.clone(), clean]));
}

/// `--meta-header` and `meta-header: skip` (spec/harness.md
/// "Header-differential mode").
#[test]
fn bmi_kind_option() {
    assert_eq!(parse_args(strings(&["--bmi-kind", "1"])).unwrap().unwrap().bmi_kind, Some(1));
    assert_eq!(parse_args(strings(&["--bmi-kind=2"])).unwrap().unwrap().bmi_kind, Some(2));
    assert!(parse_args(strings(&["--bmi-kind", "3"])).is_err());
    assert!(parse_args(strings(&["--bmi-kind"])).is_err());
}

#[test]
fn meta_header_mode_selection() {
    let o = parse_args(strings(&["--meta-header"])).unwrap().unwrap();
    assert!(o.meta_header);
    assert!(parse_args(strings(&["--meta-header=1"])).is_err());

    let spec = |src: &str| directives::parse(src, directives::Lang::Cxx).unwrap();
    let skip = |src: &str| runner::meta_header_skip(&spec(src), src);
    let head = "//@ expect: compile-ok\n#include \"kiln_meta.h\"\n";
    assert_eq!(skip(head), None);
    assert_eq!(skip(&format!("{head}auto x = __kiln_data_members<^^int>();\n")), None);
    assert_eq!(
        skip("//@ expect: compile-ok\nint x;\n").as_deref(),
        Some("meta-header mode: does not include kiln_meta.h")
    );
    assert_eq!(
        skip("//@ expect: compile-ok\n//@ meta-header: skip wording names a signature\n#include \"kiln_meta.h\"\n")
            .as_deref(),
        Some("meta-header mode: wording names a signature")
    );
    assert_eq!(
        skip(&format!("{head}auto r = __kiln_define_member(a, b, \"m\");\n")).as_deref(),
        Some("meta-header mode: uses the kiln extension `__kiln_define_member`")
    );
    assert_eq!(
        skip(&format!("{head}auto n = name_of(^^int);\n")).as_deref(),
        Some("meta-header mode: uses the kiln extension `name_of`")
    );
    assert_eq!(skip(&format!("{head}auto n = my_name_of(^^int);\n")), None);
    assert_eq!(
        skip("//@ expect: compile-ok\n//@ args: -stdlib=libc++\n#include \"kiln_meta_traits.h\"\n")
            .as_deref(),
        Some("meta-header mode: the test does not use libstdc++")
    );
    assert!(skip("//@ expect: ast\n#include \"kiln_meta.h\"\n").is_some());
}

/// `--debug-sweep` (spec/harness.md "Debug-info sweep").
#[test]
fn debug_sweep_mode_selection() {
    let o = parse_args(strings(&["--debug-sweep", "tests/conformance"])).unwrap().unwrap();
    assert!(o.debug_sweep);
    assert!(parse_args(strings(&["--debug-sweep=1"])).is_err());
    let skip = |src: &str| {
        runner::debug_sweep_skip(&directives::parse(src, directives::Lang::Cxx).unwrap())
    };
    for kind in ["compile-ok", "run", "ir", "layout", "vtables"] {
        assert_eq!(skip(&format!("//@ expect: {kind}\n")), None, "{kind}");
    }
    assert_eq!(
        skip("//@ expect: compile-fail\n").as_deref(),
        Some("not swept (`expect: compile-fail`, `--debug-sweep`)")
    );
    assert!(skip("//@ expect: ast\n").is_some());
    assert!(skip("//@ expect: pp\n").is_some());
}

#[test]
fn meta_header_directive_errors() {
    for bad in ["//@ meta-header: skip\n", "//@ meta-header: off\n", "//@ meta-header: skipx y\n"] {
        let src = format!("//@ expect: compile-ok\n{bad}");
        assert!(directives::parse(&src, directives::Lang::Cxx).is_err(), "{bad}");
    }
    let dup = "//@ expect: compile-ok\n//@ meta-header: skip a\n//@ meta-header: skip b\n";
    assert!(directives::parse(dup, directives::Lang::Cxx).is_err());
}
