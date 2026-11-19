//! The release gate's step manifest (WP-1219; spec/harness.md "Release
//! gate"): the one place that says what a gate IS. Each step names the
//! existing command it runs, the host it runs on, how its counts are read,
//! its pass rule, its minimum unit count (a step that ran fewer units
//! FAILS: exit status 0 is not a pass), its rough time and scratch space,
//! and the steps it needs first.

use std::fmt::Write as _;

use crate::util::json::Json;

/// Where a step runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    /// The aarch64 development host, by `release-gate run`.
    Local,
    /// GitHub's x86-64 runners (`sweep.yml`), read back by `import-x86`.
    Hosted,
}

/// What a step runs and how its result is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `cargo xtask dist --check`, `sha256sum -c`, unpack, `--version`.
    Archive,
    /// `cargo xtask ci` with extra arguments and environment.
    Ci { args: &'static [&'static str], oracle19: bool },
    /// `scripts/pch-release-gate.sh <kiln++> <dir> all`.
    Pch,
    /// `cargo xtask gcc-torture` at one `-std`.
    Torture { std: &'static str },
    /// `cargo xtask libcxx std libcxx` (Local) or the sweep's shards (Hosted).
    Libcxx,
    /// `cargo xtask gxx-dg` in full (Local) or the sweep's shards (Hosted).
    GxxDg,
    /// `scripts/oss/oss-build.sh <project>` (Local, one step per row of
    /// scripts/oss/projects.toml) or the sweep's `oss` matrix (Hosted).
    Oss { project: Option<String> },
    /// `--external <id>=<command>`: a check outside the repository.
    External { command: String },
}

/// One step of the gate.
#[derive(Clone, Debug)]
pub struct Step {
    pub id: String,
    pub host: Host,
    pub kind: Kind,
    /// The record section the step's row goes to.
    pub section: &'static str,
    /// How the counts are read.
    pub counts: &'static str,
    /// The pass rule, as the record and `plan` print it.
    pub rule: &'static str,
    pub min_units: u64,
    /// The minimum of a `--smoke` gate's restricted run.
    pub smoke_min_units: u64,
    pub minutes: u32,
    pub scratch_gb: u32,
    pub deps: Vec<String>,
    /// The parallel track (`a` or `b`, as the 1.2.1 driver ran them).
    pub track: char,
}

impl Step {
    /// Whether the step runs the archive's `kiln++` (and its record is
    /// tied to that binary's hash).
    pub fn uses_kiln(&self) -> bool {
        self.host == Host::Local
            && matches!(
                self.kind,
                Kind::Pch | Kind::Torture { .. } | Kind::Libcxx | Kind::GxxDg | Kind::Oss { .. }
            )
    }
}

struct Def {
    id: &'static str,
    host: Host,
    kind: Kind,
    section: &'static str,
    counts: &'static str,
    rule: &'static str,
    min_units: u64,
    smoke_min_units: u64,
    minutes: u32,
    scratch_gb: u32,
    deps: &'static [&'static str],
    track: char,
}

const NO_FAIL: &str = "exit 0, every corpus line `ok.` with 0 failed";
const NOT_BELOW: &str =
    "kiln's passes not below the stored baseline (docs/status/release-gate-baseline.tsv)";
const GXX_RULE: &str = "no case from pass to a failing class that is not in docs/status/gxx-dg-explained.txt, \
     by case against docs/status/gxx-dg-baseline.txt (the percentage is reported, not gated)";

#[allow(clippy::too_many_lines, reason = "the manifest table")]
/// The fixed steps, in dependency order. The OSS rows and the externals
/// are added by [`steps`].
fn defs() -> Vec<Def> {
    let d =
        |id, host, kind, section, counts, rule, min: u64, smoke: u64, minutes, gb, deps, track| {
            Def {
                id,
                host,
                kind,
                section,
                counts,
                rule,
                min_units: min,
                smoke_min_units: smoke,
                minutes,
                scratch_gb: gb,
                deps,
                track,
            }
        };
    vec![
        d(
            "archive",
            Host::Local,
            Kind::Archive,
            "archive",
            "archives `sha256sum -c` OK",
            "`dist --check` and `sha256sum -c` exit 0, `--version` names the version",
            2,
            2,
            30,
            3,
            &[],
            'a',
        ),
        d(
            "merge-gate",
            Host::Local,
            Kind::Ci { args: &[], oracle19: false },
            "merge-gate",
            "`corpus (kiln)` passed, every corpus line's failed",
            NO_FAIL,
            6000,
            6000,
            180,
            25,
            &[],
            'a',
        ),
        d(
            "oracle19",
            Host::Local,
            Kind::Ci { args: &["--oracle-only", "--step", "kiln"], oracle19: true },
            "merge-gate",
            "`corpus (kiln)` passed at -fclang-compat=19",
            NO_FAIL,
            6000,
            6000,
            60,
            10,
            &[],
            'a',
        ),
        d(
            "pch-sweep",
            Host::Local,
            Kind::Pch,
            "pch",
            "the libc++ pair's tests",
            "the script's verdict `pch-release-gate: pass`",
            4000,
            4000,
            120,
            90,
            &["archive"],
            'a',
        ),
        d(
            "libcxx-aarch64",
            Host::Local,
            Kind::Libcxx,
            "libcxx",
            "outcomes of the results file",
            NOT_BELOW,
            9000,
            1,
            150,
            15,
            &["archive"],
            'a',
        ),
        d(
            "torture-gnu23",
            Host::Local,
            Kind::Torture { std: "gnu23" },
            "torture",
            "kiln's pass and run cases of the TSV",
            NOT_BELOW,
            7000,
            5,
            60,
            2,
            &["archive"],
            'b',
        ),
        d(
            "torture-gnu17",
            Host::Local,
            Kind::Torture { std: "gnu17" },
            "torture",
            "kiln's pass and run cases of the TSV",
            NOT_BELOW,
            7000,
            5,
            60,
            2,
            &["archive"],
            'b',
        ),
        d(
            "gxx-dg-aarch64",
            Host::Local,
            Kind::GxxDg,
            "gxx-dg",
            "kiln's classes of the TSV, by case",
            GXX_RULE,
            50000,
            5,
            120,
            10,
            &["archive"],
            'b',
        ),
        d(
            "libcxx-x86_64",
            Host::Hosted,
            Kind::Libcxx,
            "libcxx",
            "outcomes of the six shard results files",
            NOT_BELOW,
            9000,
            9000,
            120,
            0,
            &[],
            'h',
        ),
        d(
            "gxx-dg-x86_64",
            Host::Hosted,
            Kind::GxxDg,
            "gxx-dg",
            "kiln's classes of the merged TSV, by case",
            GXX_RULE,
            50000,
            50000,
            150,
            0,
            &[],
            'h',
        ),
        d(
            "oss-x86_64",
            Host::Hosted,
            Kind::Oss { project: None },
            "oss",
            "every `sweep = true` row's kiln report.json",
            "every sweep row reported, each at its \
           docs/status/oss.md baseline (TUs ok and tests passed not below), each \
           count capped at the same run's clang control",
            1,
            1,
            240,
            0,
            &[],
            'h',
        ),
    ]
}

/// The OSS steps' cost: the LLVM rows are the long ones.
fn oss_cost(project: &str) -> (u32, u32) {
    if project.starts_with("llvm_") { (240, 40) } else { (30, 5) }
}

/// Every step of a gate: [`defs`], one `oss-<project>` step per row of
/// scripts/oss/projects.toml (`projects`, in file order; no row may be
/// left out or carried over), and one `external-<id>` per external.
pub fn steps(projects: &[String], externals: &[(String, String)]) -> Vec<Step> {
    let mut out: Vec<Step> = defs()
        .into_iter()
        .map(|d| Step {
            id: d.id.to_owned(),
            host: d.host,
            kind: d.kind,
            section: d.section,
            counts: d.counts,
            rule: d.rule,
            min_units: d.min_units,
            smoke_min_units: d.smoke_min_units,
            minutes: d.minutes,
            scratch_gb: d.scratch_gb,
            deps: d.deps.iter().map(|s| (*s).to_owned()).collect(),
            track: d.track,
        })
        .collect();
    for p in projects {
        let (minutes, scratch_gb) = oss_cost(p);
        out.push(Step {
            id: format!("oss-{p}"),
            host: Host::Local,
            kind: Kind::Oss { project: Some(p.clone()) },
            section: "oss",
            counts: "report.json: TUs ok/total, tests passed/total",
            rule: "TUs ok and tests passed not below the row's docs/status/oss.md baseline (kiln column)",
            min_units: 1,
            smoke_min_units: 1,
            minutes,
            scratch_gb,
            deps: vec!["archive".to_owned()],
            track: 'b',
        });
    }
    for (id, command) in externals {
        out.push(Step {
            id: format!("external-{id}"),
            host: Host::Local,
            kind: Kind::External { command: command.clone() },
            section: "external",
            counts: "`units=<n>` in the last line the command prints",
            rule: "exit 0",
            min_units: 1,
            smoke_min_units: 1,
            minutes: 15,
            scratch_gb: 0,
            deps: Vec::new(),
            track: 'a',
        });
    }
    out
}

/// The project names of scripts/oss/projects.toml: its `[name]` tables.
pub fn projects_of(toml: &str) -> Vec<String> {
    toml.lines()
        .filter_map(|l| l.trim().strip_prefix('[')?.strip_suffix(']').map(str::to_owned))
        .filter(|n| !n.contains('.') && !n.is_empty())
        .collect()
}

/// A step's measured result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Measured {
    /// `name=value` pairs, in order.
    pub counts: Vec<(String, u64)>,
    pub units: u64,
    pub pass: bool,
    pub detail: String,
}

impl Measured {
    pub fn count(&self, k: &str) -> Option<u64> {
        self.counts.iter().find(|(n, _)| n == k).map(|(_, v)| *v)
    }

    pub fn counts_line(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.counts {
            let _ = write!(s, "{}{k}={v}", if s.is_empty() { "" } else { " " });
        }
        s
    }
}

/// Applies the minimum unit count (lesson (a) of the hand-driven gates).
pub fn enforce_min(m: &mut Measured, min: u64) {
    if m.units < min {
        m.pass = false;
        m.detail = format!("ran {} units, the manifest's minimum is {min}; {}", m.units, m.detail);
    }
}

/// `cargo xtask ci` output: the `corpus (...)` result lines.
pub fn read_ci(log: &str, exit_ok: bool) -> Measured {
    let mut m = Measured { pass: exit_ok, ..Measured::default() };
    let (mut lines, mut failed) = (0, 0);
    for l in log.lines() {
        let Some(rest) = l.strip_prefix("xtask ci: corpus (") else { continue };
        let Some((name, result)) = rest.split_once("): test result: ") else { continue };
        lines += 1;
        let num = |what: &str| {
            result.split("; ").find_map(|p| {
                p.trim_start_matches("ok. ")
                    .trim_start_matches("FAILED. ")
                    .strip_suffix(what)?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
        };
        failed += num(" failed").unwrap_or(0);
        if !result.starts_with("ok.") {
            m.pass = false;
        }
        if name == "kiln" {
            m.units = num(" passed").unwrap_or(0);
        }
    }
    m.counts = vec![("passed".into(), m.units), ("failed".into(), failed), ("lines".into(), lines)];
    m.pass &= failed == 0 && lines > 0;
    m.detail = format!("{lines} corpus lines, {failed} failed");
    m
}

/// `pch-release-gate.sh` output: its verdict and the libc++ pair's size.
pub fn read_pch(log: &str, exit_ok: bool) -> Measured {
    let tests = log
        .lines()
        .find_map(|l| l.strip_prefix("libc++ pair: ")?.split(' ').next()?.parse::<u64>().ok());
    let verdict = log.lines().any(|l| l.trim() == "pch-release-gate: pass");
    Measured {
        counts: vec![("libcxx_pair_tests".into(), tests.unwrap_or(0))],
        units: tests.unwrap_or(0),
        pass: exit_ok && verdict,
        detail: if verdict {
            "pch-release-gate: pass".into()
        } else {
            "no `pch-release-gate: pass` line".into()
        },
    }
}

/// A gcc-torture or gxx-dg TSV: (cases run, kiln passes, kiln's
/// expect-unknown). Skip rows are not run.
pub fn read_case_tsv(tsv: &str) -> (u64, u64, u64) {
    let mut lines = tsv.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let Some(k) = head.iter().position(|h| *h == "kiln") else { return (0, 0, 0) };
    let (mut run, mut pass, mut unknown) = (0, 0, 0);
    for l in lines {
        match l.split('\t').nth(k) {
            Some("skip") | None => {}
            Some(c) => {
                run += 1;
                pass += u64::from(c == "pass");
                unknown += u64::from(c == "expect-unknown");
            }
        }
    }
    (run, pass, unknown)
}

/// The outcome counts of `cargo xtask libcxx --results` files.
pub fn read_libcxx(files: &[String]) -> Measured {
    const ORDER: [&str; 8] =
        ["PASS", "FAIL", "XFAIL", "XPASS", "UNSUPPORTED", "KILLED", "TIMEOUT", "ERROR"];
    let mut c = [0u64; 8];
    for f in files {
        for l in f.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if let Some(i) = ORDER.iter().position(|o| l.split('\t').next() == Some(o)) {
                c[i] += 1;
            }
        }
    }
    let total: u64 = c.iter().sum();
    let mut counts: Vec<(String, u64)> =
        ORDER.iter().map(|o| (o.to_ascii_lowercase(), 0)).collect();
    for (slot, n) in counts.iter_mut().zip(c) {
        slot.1 = n;
    }
    counts.push(("total".into(), total));
    Measured { counts, units: total, pass: true, detail: String::new() }
}

/// An OSS `report.json`: TUs and tests.
pub fn read_oss_report(json: &str) -> Result<Measured, String> {
    let j = Json::parse(json)?;
    let n = |a: &str, b: &str| j.get(a).and_then(|o| o.get(b)).and_then(Json::as_u64);
    let counts = vec![
        ("tus_ok".to_owned(), n("tus", "ok").unwrap_or(0)),
        ("tus_total".to_owned(), n("tus", "total").unwrap_or(0)),
        ("tests_passed".to_owned(), n("tests", "passed").unwrap_or(0)),
        ("tests_total".to_owned(), n("tests", "total").unwrap_or(0)),
    ];
    let units = counts[1].1;
    Ok(Measured { counts, units, pass: true, detail: String::new() })
}

/// The kiln column of a project's section of docs/status/oss.md:
/// (TUs ok, tests passed), each `None` when the section has no number.
pub fn oss_baseline(oss_md: &str, project: &str) -> Option<(Option<u64>, Option<u64>)> {
    let begin = format!("<!-- oss:{project} -->");
    let start = oss_md.find(&begin)? + begin.len();
    let end = oss_md[start..].find(&format!("<!-- /oss:{project} -->"))? + start;
    let mut tus = None;
    let mut tests = None;
    for l in oss_md[start..end].lines() {
        let cells: Vec<&str> = l.split('|').map(str::trim).collect();
        let first =
            |c: Option<&&str>| c.and_then(|v| v.split(" / ").next()?.trim().parse::<u64>().ok());
        if cells.get(1) == Some(&"TUs ok / failed") {
            tus = first(cells.get(2));
        } else if cells.get(1) == Some(&"tests passed / total") {
            tests = first(cells.get(2));
        }
    }
    Some((tus, tests))
}

/// A hosted (x86-64) row's baseline: the oss.md kiln column, each count
/// capped at what the same sweep's clang control reached on that host.
/// The oss.md numbers are the development host's (aarch64), and a project
/// can build fewer translation units on another architecture (opencv's
/// CPU-dispatch sources: 635 on aarch64, 537 on x86-64 under both
/// compilers); a count above clang's on the same host is no regression.
pub fn oss_baseline_capped(
    base: Option<(Option<u64>, Option<u64>)>,
    control: Option<&Measured>,
) -> Option<(Option<u64>, Option<u64>)> {
    let (tus, tests) = base?;
    let Some(c) = control else { return Some((tus, tests)) };
    let cap = |want: Option<u64>, key: &str| match (want, c.count(key)) {
        (Some(w), Some(got)) => Some(w.min(got)),
        (w, _) => w,
    };
    Some((cap(tus, "tus_ok"), cap(tests, "tests_passed")))
}

/// The OSS pass rule against a baseline.
pub fn oss_verdict(m: &mut Measured, base: Option<(Option<u64>, Option<u64>)>) {
    let Some((tus, tests)) = base else {
        m.detail = "no docs/status/oss.md section: counts recorded, nothing to compare".into();
        return;
    };
    let mut bad = Vec::new();
    for (key, want) in [("tus_ok", tus), ("tests_passed", tests)] {
        if let (Some(want), Some(got)) = (want, m.count(key))
            && got < want
        {
            bad.push(format!("{key} {got} < baseline {want}"));
        }
    }
    m.pass &= bad.is_empty();
    m.detail =
        if bad.is_empty() { "at or above the oss.md baseline".into() } else { bad.join(", ") };
}

/// The stored count baselines (docs/status/release-gate-baseline.tsv):
/// `step <TAB> count <TAB> value <TAB> source`.
pub fn read_baseline(tsv: &str) -> Vec<(String, String, u64)> {
    tsv.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            Some((c.first()?.to_string(), c.get(1)?.to_string(), c.get(2)?.parse().ok()?))
        })
        .collect()
}

/// The not-below rule: every stored baseline count of `step`.
pub fn not_below(m: &mut Measured, step: &str, baseline: &[(String, String, u64)]) {
    let mut bad = Vec::new();
    let mut seen = 0;
    for (s, k, want) in baseline.iter().filter(|(s, ..)| s == step) {
        seen += 1;
        match m.count(k) {
            Some(got) if got >= *want => {}
            got => bad.push(format!("{k} {} < baseline {want} ({s})", got.unwrap_or(0))),
        }
    }
    m.pass &= bad.is_empty();
    m.detail = if seen == 0 {
        "no stored baseline for this step".into()
    } else if bad.is_empty() {
        format!("not below the stored baseline ({seen} counts)")
    } else {
        bad.join(", ")
    };
}
