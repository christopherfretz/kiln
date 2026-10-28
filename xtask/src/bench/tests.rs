//! CLI parsing and an end-to-end smoke test against clang.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::util::json::Json;

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parses_the_command_line() {
    let o = parse_args(args(&[])).unwrap().unwrap();
    assert_eq!(o, Options::default());
    assert_eq!(o.runs, 5);
    assert_eq!(o.thresholds, Thresholds { time_pct: 5.0, mem_pct: 2.0 });

    let o = parse_args(args(&[
        "--compiler",
        "clang++-19",
        "--baseline=b",
        "--record",
        "--compare-with",
        "clang++-22",
        "--filter=oven",
        "--runs",
        "7",
        "--threshold-time=12.5",
        "--threshold-mem",
        "0",
        "-v",
        "bench/oven",
    ]))
    .unwrap()
    .unwrap();
    assert_eq!(o.compiler, Some(PathBuf::from("clang++-19")));
    assert_eq!(o.baseline, Some(PathBuf::from("b")));
    assert!(o.record && o.verbose);
    assert_eq!(o.compare_with, Some(PathBuf::from("clang++-22")));
    assert_eq!(o.filter.as_deref(), Some("oven"));
    assert_eq!(o.runs, 7);
    assert_eq!(o.thresholds, Thresholds { time_pct: 12.5, mem_pct: 0.0 });
    assert_eq!(o.paths, [PathBuf::from("bench/oven")]);

    assert_eq!(parse_args(args(&["-h"])).unwrap(), None);
    for bad in [
        &["--runs", "0"][..],
        &["--runs=x"],
        &["--threshold-time", "-1"],
        &["--threshold-mem=nan"],
        &["--record=yes"],
        &["--compiler"],
        &["--jobs", "2"],
    ] {
        assert!(parse_args(args(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn results_names_never_collide() {
    let dir = scratch("names");
    let rec = RunRecord {
        timestamp: "2026-10-28T000000".into(),
        host: Host::current(),
        load_avg_1m: None,
        rss_source: "none".into(),
        runs: 1,
        test: CompilerRun {
            compiler: "cc".into(),
            kind: "other".into(),
            version: "cc 1".into(),
            entries: BTreeMap::new(),
        },
        oracle: None,
    };
    let a = write_results(&dir, &rec.timestamp, &rec).unwrap();
    let b = write_results(&dir, &rec.timestamp, &rec).unwrap();
    assert_eq!(a.file_name().unwrap(), "2026-10-28T000000.json");
    assert_eq!(b.file_name().unwrap(), "2026-10-28T000000-1.json");
    let back = RunRecord::from_json(&Json::parse(&std::fs::read_to_string(&b).unwrap()).unwrap());
    assert_eq!(back.unwrap(), rec);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn scratch(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let d = std::env::temp_dir().join(format!("xtask-bench-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn layout(dir: &Path) -> Layout {
    let root = repo_root();
    Layout {
        bench: root.join("xtask/fixtures/bench"),
        baselines: dir.join("baselines"),
        results: dir.join("results"),
        perf_md: dir.join("perf.md"),
        scratch: dir.join("scratch"),
        root,
    }
}

/// Records, re-checks, and then trips a regression with a doctored baseline,
/// using clang as the compiler (skipped when clang++-19 is not installed).
#[test]
fn smoke_record_compare_regress() {
    let clang = Path::new("clang++-19");
    if identify(clang).is_err() {
        eprintln!("skipping: clang++-19 not available");
        return;
    }
    let dir = scratch("smoke");
    let lay = layout(&dir);
    let tiny = [lay.bench.join("tiny/answer.cpp")];

    // 1. Record, with clang as its own oracle: new baseline, perf.md, results.
    let opts =
        Options { runs: 3, record: true, compare_with: Some(clang.into()), ..Options::default() };
    let r = execute(&opts, &lay, clang, &tiny).unwrap();
    assert!(!r.failing, "{r:?}");
    assert_eq!((r.entries, r.new, r.failed), (1, 1, 0));
    let results = Json::parse(&std::fs::read_to_string(&r.results_file).unwrap()).unwrap();
    let rec = RunRecord::from_json(&results).unwrap();
    let m = rec.test.entries["tiny/answer"].measurement().expect("measured").clone();
    assert_eq!(m.mode, entry::Mode::EmitLlvm);
    assert_eq!(m.flags, "-std=c++20 -S -emit-llvm -o /dev/null");
    assert_eq!(m.samples_wall_ms.len(), 3);
    assert!(m.wall_ms_min > 0.0 && m.wall_ms_min <= m.wall_ms_median);
    assert!(rec.oracle.unwrap().entries["tiny/answer"].measurement().is_some());
    let perf = std::fs::read_to_string(&lay.perf_md).unwrap();
    assert!(perf.contains("| `tiny/answer` | emit-llvm |"), "{perf}");
    let base_path = lay.baselines.join("tiny.json");
    let base = BaselineFile::load(&base_path, "tiny").unwrap();
    assert!(base.entries["answer"].contains_key("clang"));

    // 2. Compare against it (thresholds wide enough for any host noise).
    let loose = Thresholds { time_pct: 1000.0, mem_pct: 1000.0 };
    let opts = Options { runs: 3, thresholds: loose, ..Options::default() };
    let r = execute(&opts, &lay, clang, &tiny).unwrap();
    assert!(!r.failing, "{r:?}");
    assert_eq!((r.new, r.regressed, r.failed), (0, 0, 0));

    // 3. A baseline far faster and leaner than reality is a regression.
    let mut doctored = base;
    let b = doctored.entries.get_mut("answer").unwrap().get_mut("clang").unwrap();
    b.measurement.wall_ms_median = 0.01;
    b.measurement.wall_ms_min = 0.01;
    b.measurement.max_rss_kb = b.measurement.max_rss_kb.map(|_| 1);
    std::fs::write(&base_path, doctored.to_json("tiny").to_pretty()).unwrap();
    let opts = Options { runs: 3, ..Options::default() };
    let r = execute(&opts, &lay, clang, &tiny).unwrap();
    assert!(r.failing, "{r:?}");
    assert_eq!(r.regressed, 1);
    // ... but not when re-recording, which replaces the doctored baseline.
    let opts = Options { runs: 3, record: true, ..Options::default() };
    let r = execute(&opts, &lay, clang, &tiny).unwrap();
    assert!(!r.failing && r.regressed == 1, "{r:?}");
    let base = BaselineFile::load(&base_path, "tiny").unwrap();
    assert!(base.entries["answer"]["clang"].measurement.wall_ms_min > 0.01);

    // 4. An entry that does not compile fails the run and is not recorded.
    let broken = [lay.bench.join("broken/error.cpp")];
    let opts = Options { runs: 3, record: true, ..Options::default() };
    let r = execute(&opts, &lay, clang, &broken).unwrap();
    assert!(r.failing && r.failed == 1, "{r:?}");
    assert!(!lay.baselines.join("broken.json").exists());

    assert!(!lay.scratch.exists(), "scratch is cleaned up");
    std::fs::remove_dir_all(&dir).unwrap();
}
