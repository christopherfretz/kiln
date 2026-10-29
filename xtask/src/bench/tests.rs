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
    assert_eq!(o.thresholds, ThresholdOverrides::default());

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
        "--threshold-steps=1",
        "--threshold-bytes",
        "3",
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
    assert_eq!(
        o.thresholds,
        ThresholdOverrides {
            time_pct: Some(12.5),
            mem_pct: Some(0.0),
            steps_pct: Some(1.0),
            bytes_pct: Some(3.0)
        }
    );
    assert_eq!(o.paths, [PathBuf::from("bench/oven")]);

    assert_eq!(parse_args(args(&["-h"])).unwrap(), None);
    for bad in [
        &["--runs", "0"][..],
        &["--runs=x"],
        &["--threshold-time", "-1"],
        &["--threshold-mem=nan"],
        &["--threshold-steps", "x"],
        &["--threshold-bytes"],
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
    let loose = ThresholdOverrides {
        time_pct: Some(1000.0),
        mem_pct: Some(1000.0),
        ..ThresholdOverrides::default()
    };
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

/// Constexpr counters, recorded and judged, with a stand-in kiln that
/// compiles with clang and "reports" the canned `<entry>.oven.jsonl`.
#[test]
fn constexpr_counters_with_a_fake_kiln() {
    if identify(Path::new("clang++-19")).is_err() {
        eprintln!("skipping: clang++-19 not available");
        return;
    }
    let dir = scratch("oven");
    let lay = layout(&dir);
    let kiln = lay.root.join("xtask/fixtures/fake-kiln/kiln");
    let counted = [lay.bench.join("oven/counted.cpp")];
    let loose = ThresholdOverrides {
        time_pct: Some(1000.0),
        mem_pct: Some(1000.0),
        ..ThresholdOverrides::default()
    };

    // 1. Record: the report's total lands in the baseline; the report flag
    //    is not part of the recorded flags.
    let opts = Options { runs: 2, record: true, ..Options::default() };
    let r = execute(&opts, &lay, &kiln, &counted).unwrap();
    assert!(!r.failing && r.new == 1, "{r:?}");
    let base_path = lay.baselines.join("oven.json");
    let base = BaselineFile::load(&base_path, "oven").unwrap();
    let m = &base.entries["counted"]["kiln"].measurement;
    let counters = m.oven.expect("counters recorded");
    assert_eq!(
        (counters.steps, counters.bytes, counters.peak_bytes, counters.calls, counters.evaluations),
        (116, 40, 32, 3, 2)
    );
    assert_eq!((counters.peak_arena, counters.frozen_bytes), (32, 32));
    assert_eq!(m.oven_missing, []);
    assert!(!m.flags.contains("constexpr-report"), "{}", m.flags);

    // 2. Unchanged counters compare clean.
    let opts = Options { runs: 2, thresholds: loose, ..Options::default() };
    let r = execute(&opts, &lay, &kiln, &counted).unwrap();
    assert!(!r.failing && r.regressed == 0, "{r:?}");

    // 3. Bytes 33% worse than the baseline is inside the entry's
    //    `threshold-bytes: 50`; one step more is past the 0% default.
    let doctor = |steps: u64, bytes: u64| {
        let mut file = BaselineFile::load(&base_path, "oven").unwrap();
        let entry = file.entries.get_mut("counted").unwrap().get_mut("kiln").unwrap();
        let counters = entry.measurement.oven.as_mut().unwrap();
        (counters.steps, counters.bytes) = (steps, bytes);
        std::fs::write(&base_path, file.to_json("oven").to_pretty()).unwrap();
    };
    doctor(116, 30);
    let r = execute(&opts, &lay, &kiln, &counted).unwrap();
    assert!(!r.failing && r.regressed == 0, "{r:?}");
    doctor(115, 40);
    let r = execute(&opts, &lay, &kiln, &counted).unwrap();
    assert!(r.failing && r.regressed == 1, "{r:?}");
    // ... unless the command line widens the step threshold.
    let wide = ThresholdOverrides { steps_pct: Some(1.0), ..loose };
    let o = Options { runs: 2, thresholds: wide, ..Options::default() };
    let r = execute(&o, &lay, &kiln, &counted).unwrap();
    assert!(!r.failing && r.regressed == 0, "{r:?}");
    // Fewer steps than the baseline is an improvement.
    doctor(200, 40);
    let r = execute(&opts, &lay, &kiln, &counted).unwrap();
    assert!(!r.failing && r.improved == 1, "{r:?}");

    // 4. An `oven:` budget breach fails the entry.
    let over = [lay.bench.join("oven/over-budget.cpp")];
    let r = execute(&opts, &lay, &kiln, &over).unwrap();
    assert!(r.failing && r.failed == 1, "{r:?}");

    // 5. clang reports no counters: measured as usual, none recorded.
    let clang = Path::new("clang++-19");
    let opts = Options { runs: 2, record: true, ..Options::default() };
    let r = execute(&opts, &lay, clang, &counted).unwrap();
    assert!(!r.failing, "{r:?}");
    let base = BaselineFile::load(&base_path, "oven").unwrap();
    assert!(base.entries["counted"]["clang"].measurement.oven.is_none());

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The `inst.*` metrics (spec/sema-templates.md §11.4), recorded and judged
/// for `frontend` entries, with the fake kiln printing the canned
/// `<entry>.inst.jsonl` instantiation report on stderr.
#[test]
fn instantiation_counters_with_a_fake_kiln() {
    if identify(Path::new("clang++-19")).is_err() {
        eprintln!("skipping: clang++-19 not available");
        return;
    }
    let dir = scratch("inst");
    let lay = layout(&dir);
    let kiln = lay.root.join("xtask/fixtures/fake-kiln/kiln");
    let clang = Path::new("clang++-19");
    let inst = [lay.bench.join("frontend/inst.cpp")];
    let loose = ThresholdOverrides {
        time_pct: Some(1000.0),
        mem_pct: Some(1000.0),
        ..ThresholdOverrides::default()
    };

    // 1. Record, with clang as the oracle: the report's totals land in the
    //    baseline and in perf.md; the report flag is not a recorded flag.
    let opts =
        Options { runs: 2, record: true, compare_with: Some(clang.into()), ..Options::default() };
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(!r.failing && r.new == 1, "{r:?}");
    let base_path = lay.baselines.join("frontend.json");
    let base = BaselineFile::load(&base_path, "frontend").unwrap();
    let m = &base.entries["inst"]["kiln"].measurement;
    assert_eq!(m.inst, Some(InstCounters { count: 6, nodes: 51, bytes: 1632 }));
    assert!(!m.flags.contains("instantiation-report"), "{}", m.flags);
    let text = std::fs::read_to_string(&base_path).unwrap();
    assert!(text.contains("\"inst\": {\n          \"count\": 6,"), "{text}");
    let perf = std::fs::read_to_string(&lay.perf_md).unwrap();
    let row = perf.lines().find(|l| l.starts_with("| `frontend/inst`")).expect(&perf);
    assert!(row.contains(" | 6 / 1.6 KiB | "), "{perf}");
    assert!(perf.contains("| kiln instantiations | Recorded |"), "{perf}");

    // 2. Unchanged totals compare clean.
    let opts = Options { runs: 2, thresholds: loose, ..Options::default() };
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(!r.failing && r.regressed == 0 && r.improved == 0, "{r:?}");

    // 3. One instantiation more than the baseline regresses, whatever the
    //    thresholds; bytes 2% over do not, 3% do; fewer is an improvement.
    let doctor = |count: u64, bytes: u64| {
        let mut file = BaselineFile::load(&base_path, "frontend").unwrap();
        let entry = file.entries.get_mut("inst").unwrap().get_mut("kiln").unwrap();
        let i = entry.measurement.inst.as_mut().unwrap();
        (i.count, i.bytes) = (count, bytes);
        std::fs::write(&base_path, file.to_json("frontend").to_pretty()).unwrap();
    };
    doctor(5, 1632);
    let wide = ThresholdOverrides { steps_pct: Some(100.0), bytes_pct: Some(1.0), ..loose };
    let o = Options { runs: 2, thresholds: wide, ..Options::default() };
    let r = execute(&o, &lay, &kiln, &inst).unwrap();
    assert!(r.failing && r.regressed == 1, "{r:?}");
    doctor(6, 1601); // 1632 is +1.9%
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(!r.failing && r.regressed == 0, "{r:?}");
    doctor(6, 1584); // +3.0%
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(r.failing && r.regressed == 1, "{r:?}");
    doctor(7, 1632);
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(!r.failing && r.improved == 1, "{r:?}");

    // 4. A baseline from before the metric loads and is not judged on it.
    let mut file = BaselineFile::load(&base_path, "frontend").unwrap();
    file.entries.get_mut("inst").unwrap().get_mut("kiln").unwrap().measurement.inst = None;
    std::fs::write(&base_path, file.to_json("frontend").to_pretty()).unwrap();
    let r = execute(&opts, &lay, &kiln, &inst).unwrap();
    assert!(!r.failing && r.regressed == 0 && r.improved == 0, "{r:?}");

    // 5. No totals row on stderr fails the entry.
    let missing = [lay.bench.join("frontend/no-report.cpp")];
    let r = execute(&opts, &lay, &kiln, &missing).unwrap();
    assert!(r.failing && r.failed == 1, "{r:?}");

    // 6. clang reports no totals: measured as usual, none recorded.
    let opts = Options { runs: 1, record: true, ..Options::default() };
    let r = execute(&opts, &lay, clang, &inst).unwrap();
    assert!(!r.failing, "{r:?}");
    let base = BaselineFile::load(&base_path, "frontend").unwrap();
    assert!(base.entries["inst"]["clang"].measurement.inst.is_none());

    std::fs::remove_dir_all(&dir).unwrap();
}

/// `--record` with `--compare-with` merges into `perf.md` (spec/bench.md
/// "Recording"): a fresh page when there is none, a filtered recording keeps
/// the other rows, a re-recording replaces its row, stale rows are marked
/// and left out of the means, and a recording of the whole corpus drops them.
/// clang is both compilers (skipped when clang++-19 is not installed).
#[test]
fn perf_md_merges_across_recordings() {
    let clang = Path::new("clang++-19");
    if identify(clang).is_err() {
        eprintln!("skipping: clang++-19 not available");
        return;
    }
    let dir = scratch("perf-md");
    let lay = layout(&dir);
    let all = [lay.bench.clone()];
    let record = |filter: Option<&str>| {
        let opts = Options {
            runs: 1,
            record: true,
            compare_with: Some(clang.into()),
            filter: filter.map(str::to_owned),
            ..Options::default()
        };
        execute(&opts, &lay, clang, &all).unwrap();
        std::fs::read_to_string(&lay.perf_md).unwrap()
    };
    let rows = |page: &str| -> Vec<String> {
        page.lines().filter(|l| l.starts_with("| `")).map(str::to_owned).collect()
    };
    let ids = |page: &str| -> Vec<String> {
        rows(page).iter().filter_map(|l| l[3..].split('`').next().map(str::to_owned)).collect()
    };
    let row = |page: &str, id: &str| -> String {
        let found: Vec<String> =
            rows(page).into_iter().filter(|l| l.starts_with(&format!("| `{id}`"))).collect();
        assert_eq!(found.len(), 1, "{id} in\n{page}");
        found[0].clone()
    };

    // 1. No page yet: written from this run alone.
    assert!(!lay.perf_md.exists());
    let page = record(Some("counted"));
    assert_eq!(ids(&page), ["oven/counted"], "{page}");
    assert!(row(&page, "oven/counted").starts_with("| `oven/counted` | syntax | "), "{page}");
    assert!(page.contains("| test/oracle memory | test instantiations | Recorded |"), "{page}");

    // 2. A filtered recording of another group keeps the oven row verbatim.
    let counted = row(&page, "oven/counted");
    let page = record(Some("tiny"));
    assert_eq!(ids(&page), ["oven/counted", "tiny/answer"], "{page}");
    assert_eq!(row(&page, "oven/counted"), counted);
    assert!(row(&page, "tiny/answer").starts_with("| `tiny/answer` | emit-llvm | "), "{page}");
    assert!(page.contains(" over 2 entries, memory "), "{page}");

    // 3. Re-recording replaces the row; an entry gone from bench/ is kept,
    //    marked stale, and left out of the means (2 entries, not 3).
    let doctored = "| `oven/counted` | syntax | 99999.0 | 1.0 | 99999.00x | 1.0 | 1.0 | 1.00x | - | \
                    2000-01-01T000000 |";
    let gone = "| `gone/old` | syntax | 1.0 | 2.0 | 0.50x | 1.0 | 1.0 | 1.00x | - | \
                2000-01-01T000000 |";
    let tiny = row(&page, "tiny/answer");
    std::fs::write(&lay.perf_md, page.replace(&counted, &format!("{doctored}\n{gone}"))).unwrap();
    let page = record(Some("counted"));
    assert_eq!(ids(&page), ["gone/old", "oven/counted", "tiny/answer"], "{page}");
    let counted = row(&page, "oven/counted");
    assert!(!counted.contains("99999") && !counted.contains("2000-01-01"), "{page}");
    assert_eq!(row(&page, "gone/old"), gone.replace("`gone/old`", "`gone/old` (stale)"));
    assert_eq!(row(&page, "tiny/answer"), tiny);
    assert!(page.contains(" over 2 entries, memory "), "{page}");

    // 4. A recording of the whole corpus drops the stale row; a failed entry
    //    shows FAIL.
    let page = record(None);
    assert_eq!(
        ids(&page),
        [
            "broken/error",
            "frontend/inst",
            "frontend/no-report",
            "oven/counted",
            "oven/over-budget",
            "tiny/answer"
        ]
    );
    assert!(row(&page, "broken/error").contains("| FAIL | FAIL |"), "{page}");

    // 5. A page that cannot be read back is rewritten from this run alone.
    std::fs::write(&lay.perf_md, "hand-written notes\n").unwrap();
    let page = record(Some("tiny"));
    assert_eq!(ids(&page), ["tiny/answer"], "{page}");

    std::fs::remove_dir_all(&dir).unwrap();
}
