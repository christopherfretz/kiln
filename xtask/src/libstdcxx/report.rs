//! Console summary and the Markdown status page (`spec/libstdcxx.md`
//! "Report").

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use super::run::{Compile, HeaderResult, Status};
use crate::libcxx::cluster::Clusters;

/// The markers around one standard's generated region.
pub fn markers(std: &str) -> (String, String) {
    (format!("<!-- libstdcxx:{std}:begin -->"), format!("<!-- libstdcxx:{std}:end -->"))
}

/// Facts about the run for the region header.
pub struct RunInfo {
    pub std: String,
    pub kiln_version: String,
    pub kiln_commit: String,
    pub clang_version: String,
    pub libstdcxx: String,
    pub headers_dir: String,
    /// Which files of the include directory are compiled.
    pub selection: String,
    pub command: String,
    pub flags: String,
    pub limits: String,
    pub rss_source: String,
    pub wall: Duration,
}

/// The nearest-rank `p`-th percentile (0 < p ≤ 100) of `v`, `None` if empty.
pub fn percentile<T: Copy + Ord>(v: &[T], p: usize) -> Option<T> {
    let mut s = v.to_vec();
    s.sort_unstable();
    let n = s.len();
    if n == 0 {
        return None;
    }
    let rank = (p * n).div_ceil(100).clamp(1, n);
    Some(s[rank - 1])
}

/// Counts and distributions over a run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub ok: usize,
    pub fail: usize,
    pub crash: usize,
    pub timeout: usize,
    pub killed: usize,
    pub error: usize,
    /// Headers the reference compiler ran on, and of those, accepted.
    pub clang_ran: usize,
    pub clang_ok: usize,
    /// kiln non-ok results on headers the reference compiler rejects too.
    pub kiln_bad_on_clang_rejected: usize,
    pub kiln_wall: Vec<Duration>,
    pub kiln_rss_kb: Vec<u64>,
    pub clang_wall: Vec<Duration>,
    pub clang_rss_kb: Vec<u64>,
}

impl Summary {
    pub fn of(results: &[HeaderResult]) -> Self {
        let mut s = Self::default();
        for r in results {
            s.total += 1;
            match r.kiln.status {
                Status::Ok => s.ok += 1,
                Status::Fail { .. } => s.fail += 1,
                Status::Crash(_) => s.crash += 1,
                Status::Timeout => s.timeout += 1,
                Status::Killed => s.killed += 1,
                Status::Error(_) => s.error += 1,
            }
            if !r.kiln.status.is_ok() && r.clang_rejects() {
                s.kiln_bad_on_clang_rejected += 1;
            }
            s.kiln_wall.push(r.kiln.wall);
            s.kiln_rss_kb.extend(r.kiln.max_rss_kb);
            if let Some(c) = &r.clang {
                s.clang_ran += 1;
                s.clang_ok += usize::from(c.status.is_ok());
                s.clang_wall.push(c.wall);
                s.clang_rss_kb.extend(c.max_rss_kb);
            }
        }
        s
    }

    fn dist(wall: &[Duration], rss: &[u64]) -> String {
        let w = |p| percentile(wall, p).map_or_else(|| "—".to_owned(), secs);
        let r = |p| percentile(rss, p).map_or_else(|| "—".to_owned(), mib);
        format!("wall p50 {} / p95 {}, peak RSS p50 {} / p95 {}", w(50), w(95), r(50), r(95))
    }

    /// The one-line summary (console and page).
    pub fn line(&self, std: &str) -> String {
        let clang = if self.clang_ran == 0 {
            "reference compiler not run".to_owned()
        } else {
            format!(
                "clang++-19 accepts {} of {}; kiln {}; clang++-19 {}",
                self.clang_ok,
                self.clang_ran,
                Self::dist(&self.kiln_wall, &self.kiln_rss_kb),
                Self::dist(&self.clang_wall, &self.clang_rss_kb)
            )
        };
        let clang = if self.clang_ran == 0 {
            format!("{clang}; kiln {}", Self::dist(&self.kiln_wall, &self.kiln_rss_kb))
        } else {
            clang
        };
        format!(
            "{std}: {} of {} headers ok with kiln ({} fail, {} crash, {} timeout, {} killed{}; {} on headers clang++-19 rejects); {clang}",
            self.ok,
            self.total,
            self.fail,
            self.crash,
            self.timeout,
            self.killed,
            if self.error > 0 { format!(", {} error", self.error) } else { String::new() },
            self.kiln_bad_on_clang_rejected,
        )
    }
}

/// kiln's cluster keys for one header: a crash first, then its `error:`
/// clusters in order.
fn keys_of(c: &Compile) -> Vec<(String, String)> {
    let mut keys = Vec::new();
    match &c.status {
        Status::Crash(k) => keys.push((format!("crash: {k}"), k.clone())),
        Status::Timeout => keys.push(("timeout".to_owned(), String::new())),
        Status::Killed => keys.push(("killed (memory cap)".to_owned(), String::new())),
        _ => {}
    }
    keys.extend(c.keys.iter().cloned());
    keys
}

/// Clusters over kiln's non-ok headers that the reference compiler accepts
/// (or did not run on).
pub fn clusters_of(results: &[HeaderResult]) -> Clusters {
    let mut c = Clusters::default();
    for r in counted(results) {
        c.add(&r.header, &keys_of(&r.kiln));
    }
    c
}

fn counted(results: &[HeaderResult]) -> impl Iterator<Item = &HeaderResult> {
    results.iter().filter(|r| !r.kiln.status.is_ok() && !r.clang_rejects())
}

#[allow(clippy::cast_precision_loss)] // KiB counts fit f64
fn mib(kb: u64) -> String {
    format!("{:.0} MiB", kb as f64 / 1024.0)
}

fn secs(d: Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}

/// The generated region for `info.std`, markers included.
pub fn render(info: &RunInfo, results: &[HeaderResult], top_clusters: usize) -> String {
    let (begin, end) = markers(&info.std);
    let std = &info.std;
    let mut s = String::new();
    let _ = writeln!(s, "{begin}");
    let _ = writeln!(
        s,
        "<!-- generated by `cargo xtask libstdcxx --std {std} --report`; edits inside the markers are overwritten -->\n"
    );
    let summary = Summary::of(results);
    let _ = writeln!(s, "### `-std={std}`\n\n**{}**\n", summary.line(std));
    let _ = writeln!(s, "| | |\n|---|---|");
    for (k, v) in [
        ("kiln", format!("`{}` at commit `{}`", info.kiln_version, info.kiln_commit)),
        ("reference", format!("`{}`", info.clang_version)),
        ("libstdc++", format!("{} (`{}`)", info.libstdcxx, info.headers_dir)),
        ("headers", info.selection.clone()),
        ("command", format!("`{}`", info.command)),
        ("flags (both compilers)", format!("`{} t.cpp` (`t.cpp`: `#include <h>`)", info.flags)),
        ("limits", info.limits.clone()),
        ("RSS source", format!("`{}`", info.rss_source)),
        ("wall time", format!("{:.1} s", info.wall.as_secs_f64())),
    ] {
        let _ = writeln!(s, "| {k} | {v} |");
    }

    let _ = writeln!(
        s,
        "\n#### Headers (`-std={std}`)\n\n\"errors\" counts the `error:` lines with `-ferror-limit=0`. \
         \"first error\" is kiln's first (paths relative to the include directory); a header \
         clang++-19 rejects too is marked in its column and not counted against kiln.\n"
    );
    let _ = writeln!(
        s,
        "| header | kiln | errors | first error | clang++-19 | kiln wall | kiln RSS | clang wall | clang RSS |\n\
         |---|---|---:|---|---|---:|---:|---:|---:|"
    );
    let rss = |c: &Compile| c.max_rss_kb.map_or_else(|| "—".to_owned(), mib);
    for r in results {
        let first = match &r.kiln.status {
            Status::Fail { first, .. } => md_code(&cut(first, 160)),
            Status::Crash(k) | Status::Error(k) => md_code(&cut(k, 160)),
            _ => String::new(),
        };
        let (clang, cw, cr) = match &r.clang {
            None => ("not run".to_owned(), "—".to_owned(), "—".to_owned()),
            Some(c) => {
                let st = match &c.status {
                    Status::Ok => "ok".to_owned(),
                    Status::Fail { errors, first } => {
                        format!("**rejects** ({errors}): {}", md_code(&cut(first, 100)))
                    }
                    other => format!("**{}**", other.tag()),
                };
                (st, secs(c.wall), rss(c))
            }
        };
        let errors = r.kiln.status.errors();
        let _ = writeln!(
            s,
            "| `<{}>` | {} | {} | {first} | {clang} | {} | {} | {cw} | {cr} |",
            r.header,
            r.kiln.status.tag(),
            if errors > 0 { errors.to_string() } else { String::new() },
            secs(r.kiln.wall),
            rss(&r.kiln),
        );
    }

    let clusters = clusters_of(results);
    let mut first_headers: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for r in counted(results) {
        if let Some((k, _)) = keys_of(&r.kiln).into_iter().next() {
            first_headers.entry(k).or_default().push(&r.header);
        }
    }
    let _ = writeln!(
        s,
        "\n#### Error clusters (`-std={std}`, generated)\n\n{} clusters over kiln's non-ok headers that \
         clang++-19 accepts. A cluster is an `error:` message with the location dropped, quoted names \
         as `'…'` and numbers as `N` (`spec/libcxx.md` \"Clusters\"). \"first\" counts headers whose \
         first error it is, \"any\" headers that report it. Top {}; the hand-written triage is in \
         \"Clusters\" below.\n",
        clusters.len(),
        top_clusters.min(clusters.len())
    );
    let _ =
        writeln!(s, "| # | first | any | cluster | headers (first) |\n|---:|---:|---:|---|---|");
    for (i, (k, c)) in clusters.ranked().into_iter().take(top_clusters).enumerate() {
        let hs = first_headers.get(k).map_or_else(
            || format!("(e.g. `<{}>`)", c.example),
            |v| v.iter().map(|h| format!("`<{h}>`")).collect::<Vec<_>>().join(" "),
        );
        let _ = writeln!(s, "| {} | {} | {} | {} | {hs} |", i + 1, c.first, c.any, md_code(k));
    }
    let _ = writeln!(s, "\n{end}");
    s
}

/// `text` as inline code safe inside a table cell.
fn md_code(text: &str) -> String {
    let t = text.replace('|', "\\|");
    if t.contains('`') { format!("``{t}``") } else { format!("`{t}`") }
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_owned()
    }
}

/// Replaces `std`'s generated region of `existing`, or appends `region`
/// when the page has none yet (a new page is the region alone).
pub fn splice(existing: &str, std: &str, region: &str) -> String {
    let (begin, end) = markers(std);
    match (existing.find(&begin), existing.find(&end)) {
        (Some(b), Some(e)) if b < e => {
            format!("{}{}{}", &existing[..b], region.trim_end(), &existing[e + end.len()..])
        }
        _ if existing.trim().is_empty() => region.to_owned(),
        _ => format!("{}\n\n{region}", existing.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn compile(status: Status, keys: &[&str], ms: u64, kb: u64) -> Compile {
        Compile {
            status,
            keys: keys.iter().map(|k| ((*k).to_owned(), format!("raw {k}"))).collect(),
            wall: Duration::from_millis(ms),
            max_rss_kb: Some(kb),
        }
    }

    fn fail(first: &str, errors: usize) -> Status {
        Status::Fail { errors, first: first.to_owned() }
    }

    fn results() -> Vec<HeaderResult> {
        let ok = || Some(compile(Status::Ok, &[], 100, 1024));
        vec![
            HeaderResult {
                header: "cstdio".into(),
                kiln: compile(Status::Ok, &[], 200, 2048),
                clang: ok(),
            },
            HeaderResult {
                header: "vector".into(),
                kiln: compile(
                    fail("bits/a.h:1:1: error: unknown type name '__typeof'", 7),
                    &["unknown type name '…'", "x"],
                    400,
                    4096,
                ),
                clang: ok(),
            },
            HeaderResult {
                header: "list".into(),
                kiln: compile(fail("e", 1), &["unknown type name '…'"], 300, 3072),
                clang: ok(),
            },
            HeaderResult {
                header: "regex".into(),
                kiln: compile(Status::Crash("panicked at a.rs: boom".into()), &["x"], 500, 8192),
                clang: ok(),
            },
            HeaderResult {
                header: "stdfloat".into(),
                kiln: compile(fail("e|f", 2), &["y"], 100, 1024),
                clang: Some(compile(fail("t.cpp:1: error: no", 1), &["no"], 100, 1024)),
            },
        ]
    }

    fn info() -> RunInfo {
        RunInfo {
            std: "c++26".into(),
            kiln_version: "kiln version 0.1".into(),
            kiln_commit: "abc1234".into(),
            clang_version: "Debian clang version 19.1.7".into(),
            libstdcxx: "`_GLIBCXX_RELEASE` 14".into(),
            headers_dir: PathBuf::from("/usr/include/c++/14").display().to_string(),
            selection: "public".into(),
            command: "cargo xtask libstdcxx --std c++26 -j 2".into(),
            flags: "-std=c++26 -stdlib=libstdc++ -fsyntax-only -ferror-limit=0".into(),
            limits: "8192 MiB".into(),
            rss_source: "gnu-time".into(),
            wall: Duration::from_secs(9),
        }
    }

    #[test]
    fn percentiles() {
        assert_eq!(percentile::<u64>(&[], 50), None);
        assert_eq!(percentile(&[7], 95), Some(7));
        assert_eq!(percentile(&[4, 1, 3, 2], 50), Some(2));
        assert_eq!(percentile(&(1..=100).collect::<Vec<u64>>(), 95), Some(95));
        assert_eq!(percentile(&(1..=10).collect::<Vec<u64>>(), 95), Some(10));
    }

    #[test]
    fn summary() {
        let s = Summary::of(&results());
        assert_eq!((s.total, s.ok, s.fail, s.crash), (5, 1, 3, 1));
        assert_eq!((s.clang_ran, s.clang_ok, s.kiln_bad_on_clang_rejected), (5, 4, 1));
        let line = s.line("c++26");
        assert!(
            line.starts_with("c++26: 1 of 5 headers ok with kiln (3 fail, 1 crash, 0 timeout, 0 killed; 1 on headers clang++-19 rejects); clang++-19 accepts 4 of 5;"),
            "{line}"
        );
        assert!(
            line.contains("kiln wall p50 0.30 s / p95 0.50 s, peak RSS p50 3 MiB / p95 8 MiB"),
            "{line}"
        );
        let none = Summary::of(&[]).line("c++20");
        assert!(none.contains("reference compiler not run") && none.contains("p50 —"), "{none}");
    }

    #[test]
    fn page() {
        let page = render(&info(), &results(), 10);
        let (b, e) = markers("c++26");
        assert!(page.starts_with(&b) && page.trim_end().ends_with(&e));
        assert!(page.contains("**c++26: 1 of 5 headers ok with kiln"), "{page}");
        assert!(
            page.contains("| `<vector>` | fail | 7 | `bits/a.h:1:1: error: unknown type name '__typeof'` | ok | 0.40 s | 4 MiB | 0.10 s | 1 MiB |"),
            "{page}"
        );
        assert!(page.contains("| `<cstdio>` | ok |  |  | ok | 0.20 s | 2 MiB |"), "{page}");
        assert!(
            page.contains(
                "| `<stdfloat>` | fail | 2 | `e\\|f` | **rejects** (1): `t.cpp:1: error: no` |"
            ),
            "{page}"
        );
        // Clusters skip the header clang rejects; ranked by first count.
        assert!(page.contains("3 clusters over kiln's non-ok headers"), "{page}");
        assert!(
            page.contains("| 1 | 2 | 2 | `unknown type name '…'` | `<vector>` `<list>` |"),
            "{page}"
        );
        assert!(
            page.contains("| 2 | 1 | 1 | `crash: panicked at a.rs: boom` | `<regex>` |"),
            "{page}"
        );
        assert!(page.contains("| 3 | 0 | 2 | `x` | (e.g. `<vector>`) |"), "{page}");
    }

    #[test]
    fn splicing() {
        let region = render(&info(), &results()[..1], 5);
        assert_eq!(splice("", "c++26", &region), region);
        let page = format!("# T\n\n{region}\n## Clusters\nkept\n");
        let newer = render(&info(), &results()[..2], 5);
        let spliced = splice(&page, "c++26", &newer);
        assert!(spliced.starts_with("# T\n\n") && spliced.ends_with("\n## Clusters\nkept\n"));
        assert!(spliced.contains("`<vector>`") && !spliced.contains("1 of 1 headers"));
        // Another standard's region is appended, the first kept.
        let mut other = info();
        other.std = "c++20".into();
        let both = splice(&spliced, "c++20", &render(&other, &results()[..1], 5));
        assert!(both.contains(&markers("c++26").0) && both.contains(&markers("c++20").1));
        assert!(both.find("## Clusters").unwrap() < both.find(&markers("c++20").0).unwrap());
    }
}
