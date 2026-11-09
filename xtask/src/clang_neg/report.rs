//! The JSON report and the generated region of
//! `docs/status/accepts-invalid.md` (spec/clang-neg.md "Report").

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::time::Duration;

use super::{CaseResult, Status, dir_key};
use crate::libcxx::cluster::normalize;
use crate::util::json::Json;

/// What the page header records about the run.
#[derive(Clone, Debug)]
pub struct RunInfo {
    pub kiln_version: String,
    pub kiln_commit: String,
    pub clang_tag: String,
    pub clang_commit: String,
    pub command: String,
    pub limits: String,
    pub rss_source: String,
    pub files: usize,
    pub verify_files: usize,
    pub wall: Duration,
}

/// Skipped configurations by reason.
#[derive(Clone, Debug, Default)]
pub struct Skips(BTreeMap<String, SkipCount>);

#[derive(Clone, Debug, Default)]
pub struct SkipCount {
    pub configs: usize,
    pub files: BTreeSet<String>,
}

impl Skips {
    pub fn add(&mut self, reason: &str, file: &str, configs: usize) {
        let e = self.0.entry(reason.to_owned()).or_default();
        e.configs += configs;
        e.files.insert(file.to_owned());
    }

    /// Reasons by configurations skipped, most first.
    pub fn ranked(&self) -> Vec<(&String, &SkipCount)> {
        let mut v: Vec<_> = self.0.iter().collect();
        v.sort_by(|a, b| b.1.configs.cmp(&a.1.configs).then(a.0.cmp(b.0)));
        v
    }

    pub fn total(&self) -> usize {
        self.0.values().map(|s| s.configs).sum()
    }
}

/// Counts over a set of cases (the whole run, or one directory).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub cases: usize,
    pub ran: usize,
    pub crashes: usize,
    pub timeouts: usize,
    pub ooms: usize,
    pub flag_rejected: usize,
    pub tool_errors: usize,
    /// Ran cases with at least one active `expected-error`.
    pub negative_cases: usize,
    pub accepts_invalid_cases: usize,
    /// Files with at least one negative case.
    pub negative_files: usize,
    /// Files with at least one accepts-invalid case.
    pub accepts_invalid_files: usize,
    pub expected: usize,
    pub missed: usize,
    pub no_diag_cases: usize,
    pub rejects_valid_cases: usize,
    pub rejects_valid_files: usize,
    /// Cases where kiln's verifier rejected a directive.
    pub verify_complaints: usize,
    /// Ran cases with a kiln "not supported yet" / "not implemented yet"
    /// error: kiln rejected for a reason other than the program's fault.
    pub unsupported_cases: usize,
    pub peak_rss_kb: Option<u64>,
}

/// A kiln error about a feature kiln lacks, not about the program.
pub fn is_unsupported(text: &str) -> bool {
    text.contains("not supported yet") || text.contains("not implemented yet")
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        return "-".to_owned();
    }
    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    let p = n as f64 * 100.0 / d as f64;
    format!("{p:.1}%")
}

impl Summary {
    pub fn of<'a>(results: impl IntoIterator<Item = &'a CaseResult>) -> Self {
        let mut s = Self::default();
        let mut neg = BTreeSet::new();
        let mut ai = BTreeSet::new();
        let mut rv = BTreeSet::new();
        for r in results {
            s.cases += 1;
            s.peak_rss_kb = s.peak_rss_kb.max(r.max_rss_kb);
            match &r.status {
                Status::Ran(c) => {
                    s.ran += 1;
                    if !c.expected.is_empty() {
                        s.negative_cases += 1;
                        neg.insert(r.file.as_str());
                    }
                    if c.accepts_invalid() {
                        s.accepts_invalid_cases += 1;
                        ai.insert(r.file.as_str());
                    }
                    s.expected += c.expected.len();
                    s.missed += c.missed.len();
                    s.verify_complaints += usize::from(c.verify_errors > 0);
                    s.unsupported_cases +=
                        usize::from(c.kiln_errors.iter().any(|e| is_unsupported(&e.text)));
                    if r.no_diagnostics && c.expected.is_empty() {
                        s.no_diag_cases += 1;
                    }
                    if r.rejects_valid() {
                        s.rejects_valid_cases += 1;
                        rv.insert(r.file.as_str());
                    }
                }
                Status::Crash(_) => s.crashes += 1,
                Status::Timeout => s.timeouts += 1,
                Status::Oom => s.ooms += 1,
                Status::FlagRejected(_) => s.flag_rejected += 1,
                Status::ToolError(_) => s.tool_errors += 1,
            }
        }
        s.negative_files = neg.len();
        s.accepts_invalid_files = ai.len();
        s.rejects_valid_files = rv.len();
        s
    }

    pub fn line(&self) -> String {
        format!(
            "accepts-invalid: {} of {} files ({}), {} of {} cases ({}); missed errors: {} of {} expected-error lines ({}); rejects-valid: {} of {} no-diagnostics cases; {} crashes, {} timeouts, {} OOM; {} cases ran, {} skipped by kiln's driver",
            self.accepts_invalid_files,
            self.negative_files,
            pct(self.accepts_invalid_files, self.negative_files),
            self.accepts_invalid_cases,
            self.negative_cases,
            pct(self.accepts_invalid_cases, self.negative_cases),
            self.missed,
            self.expected,
            pct(self.missed, self.expected),
            self.rejects_valid_cases,
            self.no_diag_cases,
            self.crashes,
            self.timeouts,
            self.ooms,
            self.ran,
            self.flag_rejected,
        )
    }

    fn to_json(&self) -> Json {
        #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
        let n = |v: usize| Json::Num(v as f64);
        #[allow(clippy::cast_precision_loss)]
        let rss = self.peak_rss_kb.map(|k| k as f64);
        Json::obj([
            ("cases", n(self.cases)),
            ("ran", n(self.ran)),
            ("crashes", n(self.crashes)),
            ("timeouts", n(self.timeouts)),
            ("ooms", n(self.ooms)),
            ("flag_rejected", n(self.flag_rejected)),
            ("tool_errors", n(self.tool_errors)),
            ("negative_cases", n(self.negative_cases)),
            ("accepts_invalid_cases", n(self.accepts_invalid_cases)),
            ("negative_files", n(self.negative_files)),
            ("accepts_invalid_files", n(self.accepts_invalid_files)),
            ("expected_errors", n(self.expected)),
            ("missed_errors", n(self.missed)),
            ("no_diagnostics_cases", n(self.no_diag_cases)),
            ("rejects_valid_cases", n(self.rejects_valid_cases)),
            ("rejects_valid_files", n(self.rejects_valid_files)),
            ("verify_complaints", n(self.verify_complaints)),
            ("unsupported_cases", n(self.unsupported_cases)),
            ("peak_rss_kb", Json::opt_num(rss)),
        ])
    }
}

/// Per-directory summaries, keyed by [`dir_key`].
pub fn by_dir(results: &[CaseResult]) -> BTreeMap<String, Summary> {
    let mut groups: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for r in results {
        groups.entry(dir_key(&r.file)).or_default().push(r);
    }
    groups.into_iter().map(|(k, v)| (k, Summary::of(v))).collect()
}

/// A missed-error cluster: the normalized clang text of the expected
/// error, with occurrence and file counts and one example.
#[derive(Clone, Debug, Default)]
pub struct Cluster {
    pub count: usize,
    pub files: BTreeSet<String>,
    pub example: String,
}

/// Missed expected-errors clustered by their normalized clang text, most
/// frequent first.
pub fn missed_clusters(results: &[CaseResult]) -> Vec<(String, Cluster)> {
    let mut m: BTreeMap<String, Cluster> = BTreeMap::new();
    for r in results {
        let Some(c) = r.classified() else { continue };
        for e in &c.missed {
            let key = normalize(&e.text);
            let cl = m.entry(key).or_default();
            cl.count += 1;
            if cl.files.insert(r.file.clone()) && cl.example.is_empty() {
                cl.example = format!("{}:{}", r.file, e.line.map_or("*".into(), |l| l.to_string()));
            }
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| {
        b.1.files.len().cmp(&a.1.files.len()).then(b.1.count.cmp(&a.1.count)).then(a.0.cmp(&b.0))
    });
    v
}

/// kiln's "not supported yet" errors by normalized text: cases, files.
pub fn unsupported_clusters(results: &[CaseResult]) -> Vec<(String, usize, usize)> {
    let mut m: BTreeMap<String, (usize, BTreeSet<&str>)> = BTreeMap::new();
    for r in results {
        let Some(c) = r.classified() else { continue };
        let keys: BTreeSet<String> = c
            .kiln_errors
            .iter()
            .filter(|e| is_unsupported(&e.text))
            .map(|e| normalize(&e.text))
            .collect();
        for k in keys {
            let e = m.entry(k).or_default();
            e.0 += 1;
            e.1.insert(r.file.as_str());
        }
    }
    let mut v: Vec<_> = m.into_iter().map(|(k, (n, f))| (k, n, f.len())).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

/// One accepts-invalid file: its accepts-invalid and negative case counts
/// and the most expected errors any accepts-invalid case had.
pub fn accepts_invalid_files(results: &[CaseResult]) -> Vec<(String, usize, usize, usize)> {
    let mut m: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    for r in results {
        let Some(c) = r.classified() else { continue };
        if c.expected.is_empty() {
            continue;
        }
        let e = m.entry(r.file.as_str()).or_default();
        e.1 += 1;
        if c.accepts_invalid() {
            e.0 += 1;
            e.2 = e.2.max(c.expected.len());
        }
    }
    let mut v: Vec<_> = m
        .into_iter()
        .filter(|(_, c)| c.0 > 0)
        .map(|(f, c)| (f.to_owned(), c.0, c.1, c.2))
        .collect();
    v.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
    v
}

fn entry_json(e: &super::outcome::Entry) -> Json {
    Json::obj([("line", Json::opt_num(e.line.map(f64::from))), ("text", Json::str(e.text.clone()))])
}

/// The whole report as JSON: run info, totals, per-directory totals,
/// skips and every case.
pub fn json(info: &RunInfo, results: &[CaseResult], skips: &Skips) -> Json {
    #[allow(clippy::cast_precision_loss)] // counts are far below 2^52
    let n = |v: usize| Json::Num(v as f64);
    let cases = results
        .iter()
        .map(|r| {
            let (status, extra): (&str, Vec<(&str, Json)>) = match &r.status {
                Status::Ran(c) => (
                    if c.accepts_invalid() {
                        "accepts-invalid"
                    } else if r.rejects_valid() {
                        "rejects-valid"
                    } else {
                        "ran"
                    },
                    vec![
                        ("expected_errors", n(c.expected.len())),
                        ("kiln_errors", n(c.kiln_errors.len())),
                        ("verify_complaints", Json::Num(f64::from(c.verify_errors))),
                        ("missed", Json::Arr(c.missed.iter().map(entry_json).collect())),
                        (
                            "first_kiln_error",
                            c.kiln_errors.first().map_or(Json::Null, |e| Json::str(e.text.clone())),
                        ),
                    ],
                ),
                Status::Crash(k) => ("crash", vec![("key", Json::str(k.clone()))]),
                Status::Timeout => ("timeout", Vec::new()),
                Status::Oom => ("oom", Vec::new()),
                Status::FlagRejected(m) => ("skipped", vec![("reason", Json::str(m.clone()))]),
                Status::ToolError(m) => ("tool-error", vec![("error", Json::str(m.clone()))]),
            };
            let mut fields = vec![
                ("file", Json::str(r.file.clone())),
                ("run", n(r.cfg.run)),
                ("std", Json::str(r.cfg.std.clone())),
                ("verify", Json::str(r.cfg.prefixes.join(","))),
                ("args", Json::Arr(r.cfg.args.iter().map(|a| Json::str(a.clone())).collect())),
                ("status", Json::str(status)),
                ("wall_ms", Json::Num(r.wall.as_secs_f64() * 1000.0)),
            ];
            fields.extend(extra);
            Json::obj(fields)
        })
        .collect();
    Json::obj([
        ("clang_tag", Json::str(info.clang_tag.clone())),
        ("clang_commit", Json::str(info.clang_commit.clone())),
        ("kiln_commit", Json::str(info.kiln_commit.clone())),
        ("command", Json::str(info.command.clone())),
        ("wall_s", Json::Num(info.wall.as_secs_f64())),
        ("totals", Summary::of(results).to_json()),
        ("by_dir", Json::obj(by_dir(results).into_iter().map(|(k, s)| (k, s.to_json())))),
        (
            "skips",
            Json::obj(skips.ranked().into_iter().map(|(r, s)| {
                (r.clone(), Json::obj([("configs", n(s.configs)), ("files", n(s.files.len()))]))
            })),
        ),
        ("cases", Json::Arr(cases)),
    ])
}

pub const BEGIN: &str = "<!-- clang-neg:begin (generated by cargo xtask clang-neg --report) -->";
pub const END: &str = "<!-- clang-neg:end -->";

fn cell(s: &str) -> String {
    let s = s.replace('|', "\\|").replace('\n', " ");
    if s.chars().count() > 110 {
        format!("{}…", s.chars().take(110).collect::<String>())
    } else {
        s
    }
}

/// The generated region of the status page.
pub fn render(info: &RunInfo, results: &[CaseResult], skips: &Skips) -> String {
    let s = Summary::of(results);
    let mut o = String::new();
    let _ = writeln!(o, "{BEGIN}\n");
    let _ = writeln!(o, "## Run\n");
    let _ =
        writeln!(o, "- clang tests: `{}` (`{}`), `clang/test`", info.clang_tag, info.clang_commit);
    let _ = writeln!(o, "- kiln: {} at `{}`", info.kiln_version, info.kiln_commit);
    let _ = writeln!(o, "- command: `{}`", info.command);
    let _ = writeln!(o, "- limits: {}", info.limits);
    let _ = writeln!(
        o,
        "- {} test files, {} with `%clang_cc1 -verify` RUN lines; {} cases run, {} configurations skipped",
        info.files,
        info.verify_files,
        s.cases - s.flag_rejected,
        skips.total()
    );
    let peak =
        s.peak_rss_kb.map_or_else(|| "unavailable".to_owned(), |k| format!("{} MiB", k / 1024));
    let _ = writeln!(
        o,
        "- wall time {:.0} s; peak RSS of one case {peak} ({})\n",
        info.wall.as_secs_f64(),
        info.rss_source
    );
    let _ = writeln!(o, "## Totals\n");
    let _ = writeln!(o, "| measure | count | of | rate |\n|---|---:|---:|---:|");
    let rows = [
        (
            "ACCEPTS-INVALID files (clang expects an error, kiln reports none in some configuration)",
            s.accepts_invalid_files,
            s.negative_files,
        ),
        (
            "ACCEPTS-INVALID cases (per RUN configuration)",
            s.accepts_invalid_cases,
            s.negative_cases,
        ),
        (
            "MISSED-ERROR lines (expected-error with no kiln error on its line)",
            s.missed,
            s.expected,
        ),
        (
            "REJECTS-VALID cases (kiln errors in an `expected-no-diagnostics` file)",
            s.rejects_valid_cases,
            s.no_diag_cases,
        ),
        ("crashes", s.crashes, s.cases),
        ("timeouts", s.timeouts, s.cases),
        ("out of memory", s.ooms, s.cases),
    ];
    for (what, n, d) in rows {
        let _ = writeln!(o, "| {what} | {n} | {d} | {} |", pct(n, d));
    }
    let _ = writeln!(
        o,
        "\n{} cases had a directive kiln's verifier could not parse (counted as they ran). {} cases had a kiln \"not supported yet\" error, which counts as an error here although it is no verdict on the program (see the last table).\n",
        s.verify_complaints, s.unsupported_cases
    );
    render_tables(&mut o, results, skips);
    let _ = writeln!(o, "\n{END}");
    o
}

/// The per-directory, per-file, per-cluster and per-outcome tables of
/// [`render`].
fn render_tables(o: &mut String, results: &[CaseResult], skips: &Skips) {
    let _ = writeln!(o, "## By directory\n");
    let _ = writeln!(
        o,
        "| directory | accepts-invalid files | rate | missed lines | rate | rejects-valid | crash / timeout |\n|---|---:|---:|---:|---:|---:|---:|"
    );
    for (k, d) in by_dir(results) {
        let _ = writeln!(
            o,
            "| {k} | {} / {} | {} | {} / {} | {} | {} | {} / {} |",
            d.accepts_invalid_files,
            d.negative_files,
            pct(d.accepts_invalid_files, d.negative_files),
            d.missed,
            d.expected,
            pct(d.missed, d.expected),
            d.rejects_valid_cases,
            d.crashes,
            d.timeouts
        );
    }
    let ai = accepts_invalid_files(results);
    let _ = writeln!(o, "\n## Top 50 accepts-invalid files\n");
    let _ = writeln!(o, "Ranked by the expected errors of their worst configuration.\n");
    let _ = writeln!(o, "| file | accepts-invalid configs | expected errors |\n|---|---:|---:|");
    for (f, a, n, e) in ai.iter().take(50) {
        let _ = writeln!(o, "| `{f}` | {a} / {n} | {e} |");
    }
    let _ = writeln!(o, "\n## Top 40 missed-error clusters\n");
    let _ = writeln!(o, "clang's expected text, normalized (quoted spans `'…'`, digits `N`).\n");
    let _ = writeln!(o, "| clang expects | files | lines | example |\n|---|---:|---:|---|");
    for (k, c) in missed_clusters(results).iter().take(40) {
        let _ = writeln!(o, "| {} | {} | {} | `{}` |", cell(k), c.files.len(), c.count, c.example);
    }
    let rv: Vec<&CaseResult> = results.iter().filter(|r| r.rejects_valid()).collect();
    let _ = writeln!(o, "\n## Rejects-valid cases\n");
    if rv.is_empty() {
        let _ = writeln!(o, "None.");
    } else {
        let _ = writeln!(o, "| file | configuration | first kiln error |\n|---|---|---|");
        for r in rv {
            let first =
                r.classified().and_then(|c| c.kiln_errors.first()).map_or("", |e| e.text.as_str());
            let _ = writeln!(o, "| `{}` | {} | {} |", r.file, r.cfg.label(), cell(first));
        }
    }
    let bad: Vec<&CaseResult> = results
        .iter()
        .filter(|r| matches!(r.status, Status::Crash(_) | Status::Timeout | Status::Oom))
        .collect();
    let _ = writeln!(o, "\n## Crashes, timeouts and out-of-memory\n");
    if bad.is_empty() {
        let _ = writeln!(o, "None.");
    } else {
        let _ = writeln!(o, "| file | configuration | outcome |\n|---|---|---|");
        for r in bad {
            let what = match &r.status {
                Status::Crash(k) => cell(k),
                Status::Timeout => "timeout".to_owned(),
                _ => "out of memory".to_owned(),
            };
            let _ = writeln!(o, "| `{}` | {} | {what} |", r.file, r.cfg.label());
        }
    }
    let _ = writeln!(o, "\n## kiln \"not supported yet\" errors\n");
    let _ = writeln!(o, "| kiln error | cases | files |\n|---|---:|---:|");
    for (k, n, f) in unsupported_clusters(results).iter().take(30) {
        let _ = writeln!(o, "| {} | {n} | {f} |", cell(k));
    }
    let _ = writeln!(o, "\n## Skipped configurations\n");
    let _ = writeln!(o, "| reason | configurations | files |\n|---|---:|---:|");
    for (r, c) in skips.ranked() {
        let _ = writeln!(o, "| {} | {} | {} |", cell(r), c.configs, c.files.len());
    }
}

/// `existing` with its generated region replaced by `region` (appended
/// when it has none).
pub fn splice(existing: &str, region: &str) -> String {
    match (existing.find(BEGIN), existing.find(END)) {
        (Some(b), Some(e)) if b < e => {
            format!("{}{}{}", &existing[..b], region.trim_end(), &existing[e + END.len()..])
        }
        _ if existing.trim().is_empty() => region.to_owned(),
        _ => format!("{}\n\n{region}", existing.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::outcome::{Classified, Entry};
    use super::super::runline::CaseCfg;
    use super::*;

    fn case(file: &str, status: Status, no_diag: bool) -> CaseResult {
        CaseResult {
            file: file.into(),
            cfg: CaseCfg {
                run: 1,
                std: "c++20".into(),
                prefixes: vec!["expected".into()],
                args: vec![],
            },
            status,
            no_diagnostics: no_diag,
            wall: Duration::from_millis(5),
            max_rss_kb: Some(1024),
        }
    }

    fn e(line: u32, text: &str) -> Entry {
        Entry { file: Some("t.cpp".into()), line: Some(line), text: text.into() }
    }

    fn ran(expected: &[Entry], missed: &[Entry], kiln: &[Entry]) -> Status {
        Status::Ran(Classified {
            expected: expected.to_vec(),
            missed: missed.to_vec(),
            kiln_errors: kiln.to_vec(),
            ..Classified::default()
        })
    }

    /// spec/clang-neg.md "Report": file- and line-level counts.
    #[test]
    fn summaries_count_files_and_lines() {
        let x = e(3, "no member named 'a' in 'S'");
        let y = e(5, "no member named 'b' in 'T'");
        let results = vec![
            case("CXX/a/p1.cpp", ran(&[x.clone(), y.clone()], &[x.clone(), y.clone()], &[]), false),
            case(
                "CXX/a/p1.cpp",
                ran(std::slice::from_ref(&x), &[], std::slice::from_ref(&x)),
                false,
            ),
            case(
                "CXX/b/p2.cpp",
                ran(&[x.clone(), y.clone()], std::slice::from_ref(&y), std::slice::from_ref(&x)),
                false,
            ),
            case("SemaCXX/v.cpp", ran(&[], &[], &[e(1, "bad")]), true),
            case("SemaCXX/w.cpp", Status::Timeout, false),
        ];
        let s = Summary::of(&results);
        assert_eq!((s.negative_files, s.accepts_invalid_files), (2, 1));
        assert_eq!((s.negative_cases, s.accepts_invalid_cases), (3, 1));
        assert_eq!((s.expected, s.missed), (5, 3));
        assert_eq!((s.rejects_valid_cases, s.no_diag_cases, s.timeouts), (1, 1, 1));
        let d = by_dir(&results);
        assert_eq!(d.keys().collect::<Vec<_>>(), ["CXX/a", "CXX/b", "SemaCXX"]);
        let cl = missed_clusters(&results);
        assert_eq!(cl[0].0, "no member named '…' in '…'");
        assert_eq!((cl[0].1.count, cl[0].1.files.len()), (3, 2));
        let ai = accepts_invalid_files(&results);
        assert_eq!(ai, [("CXX/a/p1.cpp".to_owned(), 1, 2, 2)]);
        let page = render(
            &RunInfo {
                kiln_version: "k".into(),
                kiln_commit: "c".into(),
                clang_tag: "llvmorg-22.1.8".into(),
                clang_commit: "h".into(),
                command: "cargo xtask clang-neg".into(),
                limits: "l".into(),
                rss_source: "r".into(),
                files: 4,
                verify_files: 4,
                wall: Duration::from_secs(1),
            },
            &results,
            &Skips::default(),
        );
        assert!(page.contains("| `CXX/a/p1.cpp` | 1 / 2 | 2 |"));
        assert!(page.contains(
            "| MISSED-ERROR lines (expected-error with no kiln error on its line) | 3 | 5 | 60.0% |"
        ));
        let spliced = splice(&format!("# T\n\n{BEGIN}\nold\n{END}\n\ntail\n"), &page);
        assert!(spliced.starts_with("# T\n\n") && spliced.ends_with("\n\ntail\n"));
        assert!(!spliced.contains("old"));
    }
}
