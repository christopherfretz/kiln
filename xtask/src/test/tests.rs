//! Runner-level tests: CLI parsing, and end-to-end runs with clang over the
//! seed corpus and the self-test fixtures in `xtask/fixtures/`.

use std::collections::BTreeSet;
use std::path::PathBuf;

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
        "--no-oracle",
        "tests/a",
        "b.cpp",
    ]))
    .unwrap()
    .unwrap();
    assert_eq!(
        o,
        Options {
            compiler: Some("clang++-19".into()),
            oracle: "clang++".into(),
            no_oracle: true,
            filter: Some("lex".into()),
            update: true,
            jobs: Some(3),
            run_timeout: Duration::from_millis(2500),
            verbose: true,
            paths: vec!["tests/a".into(), "b.cpp".into()],
        }
    );
    assert_eq!(parse_args(strings(&["--help"])).unwrap(), None);
    assert_eq!(parse_args(Vec::new()).unwrap().unwrap(), Options::default());
    for bad in
        [&["--jobs", "0"][..], &["--compiler"], &["--bogus"], &["--update=1"], &["--timeout", "-1"]]
    {
        assert!(parse_args(strings(bad)).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn json_field_extraction() {
    let j = r#"{"packages":[],"target_directory":"/a/b \"c\"\\d\u0041","version":1}"#;
    assert_eq!(json_string_field(j, "target_directory").as_deref(), Some("/a/b \"c\"\\dA"));
    assert_eq!(json_string_field(j, "missing"), None);
}

fn clang_opts(paths: &[PathBuf]) -> Options {
    Options {
        compiler: Some("clang++-19".into()),
        jobs: Some(2),
        paths: paths.to_vec(),
        ..Options::default()
    }
}

fn report(results: &[TestResult]) -> String {
    results.iter().map(|r| report::format_result(r, true)).collect()
}

/// The seed corpus is self-validating: it must pass with clang 19, and it
/// must exercise every directive.
#[test]
fn seed_corpus_passes_with_clang() {
    let results = execute(&clang_opts(&[])).expect("runner infrastructure works");
    let all = report(&results);
    assert!(results.iter().all(|r| !r.status.is_failure()), "{all}");
    let counts = report::Counts::of(&results);
    assert!(counts.get(Status::Pass) >= 6, "{all}");
    assert!(counts.get(Status::XFail) >= 1, "{all}");
    assert!(counts.get(Status::Skip) >= 2, "{all}");

    // Directive coverage, so the corpus keeps demonstrating the format.
    let root = repo_root();
    let cases = discover::discover(&root, &discover::default_roots(&root), None).unwrap();
    let mut seen = BTreeSet::new();
    for c in &cases {
        let src = std::fs::read_to_string(&c.path).unwrap();
        let s = directives::parse(&src, c.lang).unwrap();
        seen.insert(s.expect.as_str().to_owned());
        let mut flag = |cond: bool, name: &str| {
            if cond {
                seen.insert(name.to_owned());
            }
        };
        flag(src.contains("//@ std:"), "std");
        flag(!s.args.is_empty(), "args");
        flag(s.diags.iter().any(|d| d.severity == directives::Severity::Note), "diag-note");
        flag(!s.diags.is_empty(), "diag");
        flag(s.strict_diags, "strict-diags");
        flag(matches!(s.stdout, Some(StdoutExpect::Inline(_))), "stdout");
        flag(matches!(s.stdout, Some(StdoutExpect::File(_))), "stdout-file");
        flag(s.exit.is_some(), "exit");
        flag(src.contains("//@ oracle:"), "oracle");
        flag(s.oven.is_some(), "oven");
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
        flag(c.lang == directives::Lang::C, "c-test");
    }
    let want = [
        "args",
        "c-test",
        "compile-fail",
        "compile-ok",
        "diag",
        "diag-note",
        "exit",
        "header-test",
        "oracle",
        "oven",
        "pp",
        "pp-expected",
        "run",
        "skip",
        "std",
        "stdout",
        "stdout-file",
        "strict-diags",
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
    let dir = repo_root().join("xtask/fixtures/basic");
    let mut o = clang_opts(&[dir]);
    o.run_timeout = Duration::from_secs(1);
    let results = execute(&o).unwrap();
    // 21 tests: helper.h and Inputs/aux.h are not discovered.
    check_fixture_statuses(&results, 21);
    let by_name = |n: &str| results.iter().find(|r| r.name.ends_with(n)).unwrap();
    assert!(by_name("fail-timeout.cpp").details.contains("timed out"));
    assert!(by_name("fail-crash.cpp").details.contains("signal"));
    assert!(by_name("fail-wrong-stdout.cpp").details.contains("-no\n    +yes"));
    assert!(
        by_name("fail-wrong-exit.cpp").details.contains("expected exit code 0, got exit code 4")
    );
    assert!(by_name("fail-diag-wrong-line.cpp").details.contains("3:11 error"));
    assert!(by_name("fail-strict-unexpected.cpp").details.contains("unexpected diagnostics"));
    assert!(by_name("fail-pp-golden.cpp").details.contains("-1 + 3\n    +1 + 2"));
    assert_eq!(
        by_name("skip-kiln-specific.cpp").reason.as_deref(),
        Some("not supported by this compiler (kiln-specific `oven:` directive)")
    );
    assert!(by_name("error-bad-directive.cpp").details.contains("unknown directive `frobnicate`"));
}

#[test]
fn oracle_mismatches_are_detected() {
    if detect_kind(Path::new("clang++-22")).is_err() {
        eprintln!("clang++-22 not installed; skipping the oracle-mismatch test");
        return;
    }
    let mut o = clang_opts(&[repo_root().join("xtask/fixtures/oracle")]);
    o.oracle = "clang++-22".into();
    let results = execute(&o).unwrap();
    check_fixture_statuses(&results, 6);
}

#[test]
fn filter_and_no_tests() {
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

/// `--update` rewrites an existing `.expected`; it never creates one.
#[test]
fn update_rewrites_existing_goldens() {
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

    let mut o = clang_opts(std::slice::from_ref(&dir));
    o.no_oracle = true;
    let r = execute(&o).unwrap();
    assert_eq!(r.iter().find(|r| r.name.ends_with("golden.cpp")).unwrap().status, Status::Fail);

    o.update = true;
    let r = execute(&o).unwrap();
    assert!(r.iter().all(|r| r.status == Status::Pass), "{}", report(&r));
    assert_eq!(r.iter().filter(|r| r.updated).count(), 1);
    assert_eq!(std::fs::read_to_string(&golden).unwrap(), "3\n");
    assert!(!dir.join("fresh.cpp.expected").exists());

    o.update = false;
    let r = execute(&o).unwrap();
    assert!(r.iter().all(|r| r.status == Status::Pass), "{}", report(&r));
    std::fs::remove_dir_all(&dir).unwrap();
}
