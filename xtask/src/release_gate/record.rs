//! The release gate's state records (one per step, in the state
//! directory) and the release record it writes (`docs/status/
//! release-X.Y.Z.md`), with `check` (spec/harness.md "Release gate").

use std::fmt::Write as _;

use super::manifest::{Host, Measured, Step};

/// One step's state record: `key: value` lines in `<state>/<id>.rec`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepRecord {
    pub step: String,
    pub commit: String,
    pub version: String,
    /// `release` or `smoke`.
    pub mode: String,
    /// FNV-1a 64 of the `kiln++` the step ran, `-` when it ran none.
    pub kiln: String,
    pub start: u64,
    pub end: u64,
    pub load: String,
    pub arch: String,
    pub counts: Vec<(String, u64)>,
    pub units: Option<u64>,
    pub pass: bool,
    pub detail: String,
    pub log: String,
    pub log_hash: String,
}

impl StepRecord {
    pub fn new(step: &str, ctx: &Ctx, m: &Measured) -> Self {
        Self {
            step: step.to_owned(),
            commit: ctx.commit.clone(),
            version: ctx.version.clone(),
            mode: ctx.mode().to_owned(),
            counts: m.counts.clone(),
            units: Some(m.units),
            pass: m.pass,
            detail: m.detail.replace('\n', " "),
            ..Self::default()
        }
    }

    pub fn to_text(&self) -> String {
        let counts: Vec<String> = self.counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let units = self.units.map_or_else(String::new, |u| u.to_string());
        format!(
            "step: {}\ncommit: {}\nversion: {}\nmode: {}\nkiln: {}\nstart: {}\nend: {}\nload: {}\n\
             arch: {}\ncounts: {}\nunits: {units}\nverdict: {}\ndetail: {}\nlog: {}\nlog_hash: {}\n",
            self.step,
            self.commit,
            self.version,
            self.mode,
            self.kiln,
            self.start,
            self.end,
            self.load,
            self.arch,
            counts.join(" "),
            if self.pass { "pass" } else { "FAIL" },
            self.detail,
            self.log,
            self.log_hash
        )
    }

    #[allow(clippy::many_single_char_names, reason = "key, value, record, line")]
    pub fn parse(text: &str) -> Self {
        let mut r = Self::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once(": ").or_else(|| l.strip_suffix(':').map(|k| (k, "")))
            else {
                continue;
            };
            let v = v.to_owned();
            match k {
                "step" => r.step = v,
                "commit" => r.commit = v,
                "version" => r.version = v,
                "mode" => r.mode = v,
                "kiln" => r.kiln = v,
                "start" => r.start = v.parse().unwrap_or(0),
                "end" => r.end = v.parse().unwrap_or(0),
                "load" => r.load = v,
                "arch" => r.arch = v,
                "counts" => {
                    r.counts = v
                        .split(' ')
                        .filter_map(|kv| {
                            let (a, b) = kv.split_once('=')?;
                            Some((a.to_owned(), b.parse().ok()?))
                        })
                        .collect();
                }
                "units" => r.units = v.parse().ok(),
                "verdict" => r.pass = v == "pass",
                "detail" => r.detail = v,
                "log" => r.log = v,
                "log_hash" => r.log_hash = v,
                _ => {}
            }
        }
        r
    }

    pub fn counts_line(&self) -> String {
        self.counts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
    }
}

/// What a record is checked against: this gate's commit, version, mode
/// and `kiln++`.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub commit: String,
    pub version: String,
    pub smoke: bool,
    /// The hash of the `kiln++` the gate runs, when known.
    pub kiln: Option<String>,
}

impl Ctx {
    pub fn mode(&self) -> &'static str {
        if self.smoke { "smoke" } else { "release" }
    }
}

/// A step's state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant, reason = "one per step, compared rarely")]
pub enum Status {
    Missing,
    /// A record that does not count for this gate, and why. There is no
    /// flag that accepts one (the carried-over-rows rule, WP-1475).
    Stale(String),
    Done(StepRecord),
}

/// The status of `step` given its record (if any).
pub fn status(
    step: &Step,
    rec: Option<StepRecord>,
    ctx: &Ctx,
    log_hash_now: Option<&str>,
) -> Status {
    let Some(r) = rec else { return Status::Missing };
    let stale = if r.commit != ctx.commit {
        Some(format!("made at commit {}, the gate is at {}", short(&r.commit), short(&ctx.commit)))
    } else if r.mode != ctx.mode() {
        Some(format!("a {} record in a {} gate", r.mode, ctx.mode()))
    } else if r.version != ctx.version {
        Some(format!("made for version {}, the gate is for {}", r.version, ctx.version))
    } else if r.units.is_none() || r.counts.is_empty() {
        Some("the record has no counts".to_owned())
    } else if step.uses_kiln() && ctx.kiln.as_deref().is_some_and(|k| k != r.kiln) {
        Some(format!("ran kiln++ {}, the gate's is {}", r.kiln, ctx.kiln.as_deref().unwrap_or("")))
    } else if step.host == Host::Local && log_hash_now != Some(r.log_hash.as_str()) {
        Some("its log is missing or changed".to_owned())
    } else {
        None
    };
    match stale {
        Some(why) => Status::Stale(why),
        None => Status::Done(r),
    }
}

pub fn short(c: &str) -> &str {
    c.get(..9).unwrap_or(c)
}

/// FNV-1a 64, as 16 hex digits: the log, `kiln++` and region hashes.
pub fn fnv64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// `YYYY-MM-DD HH:MM UTC` of a Unix time.
pub fn utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

const BEGIN: &str = "<!-- release-gate:begin ";
const END: &str = "<!-- release-gate:end ";

/// The record's sections, in order, with their headings.
pub const SECTIONS: &[(&str, &str)] = &[
    ("summary", "Summary"),
    ("archive", "RC gate: the release archive"),
    ("merge-gate", "The merge gate, both oracle lanes"),
    ("pch", "Precompiled headers: the release sweep"),
    ("torture", "C mode: gcc.c-torture"),
    ("libcxx", "libc++ 22.1.8 test suite"),
    ("gxx-dg", "GCC C++ testsuite (g++.dg)"),
    ("oss", "Open-source projects"),
    ("external", "Downstream build"),
];

/// The generated text of every section, from the records of `steps`.
/// Fails, naming them, when a step has no completed record.
pub fn render(
    steps: &[Step],
    statuses: &[Status],
    ctx: &Ctx,
    explained: &str,
) -> Result<Vec<(&'static str, String)>, String> {
    let missing: Vec<String> = steps
        .iter()
        .zip(statuses)
        .filter_map(|(s, st)| match st {
            Status::Done(_) => None,
            Status::Missing => Some(format!("{} (no record)", s.id)),
            Status::Stale(why) => Some(format!("{} (stale: {why})", s.id)),
        })
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "record: {} manifest steps lack a completed record for {}:\n  {}",
            missing.len(),
            short(&ctx.commit),
            missing.join("\n  ")
        ));
    }
    let recs: Vec<(&Step, &StepRecord)> = steps
        .iter()
        .zip(statuses)
        .filter_map(|(s, st)| if let Status::Done(r) = st { Some((s, r)) } else { None })
        .collect();
    let failed: Vec<&str> =
        recs.iter().filter(|(_, r)| !r.pass).map(|(s, _)| s.id.as_str()).collect();
    let mut out = Vec::new();
    let mut sum = String::new();
    if ctx.smoke {
        sum.push_str(
            "**SMOKE GATE: NOT A RELEASE RECORD.** Its steps ran restricted (`--smoke`).\n\n",
        );
    }
    let (start, end) = (
        recs.iter().map(|(_, r)| r.start).min().unwrap_or(0),
        recs.iter().map(|(_, r)| r.end).max().unwrap_or(0),
    );
    let loads: Vec<&str> =
        recs.iter().map(|(_, r)| r.load.as_str()).filter(|l| !l.is_empty()).collect();
    let _ = write!(
        sum,
        "- Release candidate: `{}`, version `{}`, mode `{}`.\n- {} steps, run {} to {}; \
         load (1-minute average at each step's start and end): {}.\n\n**Status: {}**\n",
        short(&ctx.commit),
        ctx.version,
        ctx.mode(),
        recs.len(),
        utc(start),
        utc(end),
        if loads.is_empty() { "-".to_owned() } else { loads.join(", ") },
        if failed.is_empty() {
            "every step passed.".to_owned()
        } else {
            format!("{} steps FAILED: {}.", failed.len(), failed.join(", "))
        }
    );
    // The binary under test (WP-1862): the archive row's detail names its
    // version line, hash and build (PGO and the profile, or plain).
    if let Some((_, r)) = recs.iter().find(|(s, _)| s.id == "archive") {
        let _ = write!(
            sum,
            "\nThe `kiln++` the steps ran is the release archive's: {}.\n",
            r.detail.trim_end_matches('.')
        );
    }
    out.push(("summary", sum));
    for &(sec, _) in &SECTIONS[1..] {
        let mut t = String::from(
            "| step | host | commit | units | counts | verdict | detail |\n|---|---|---|---:|---|---|---|\n",
        );
        for (s, r) in recs.iter().filter(|(s, _)| s.section == sec) {
            let host = match s.host {
                Host::Local => r.arch.clone(),
                Host::Hosted => "x86_64 (hosted)".to_owned(),
            };
            let _ = writeln!(
                t,
                "| `{}` | {host} | `{}` | {} | {} | {} | {} |",
                s.id,
                short(&r.commit),
                r.units.unwrap_or(0),
                r.counts_line(),
                if r.pass { "pass" } else { "FAIL" },
                r.detail.replace('|', "/")
            );
        }
        if sec == "gxx-dg" {
            let list: Vec<&str> =
                explained.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).collect();
            let _ = write!(
                t,
                "\nExplained pass-to-failing cases (docs/status/gxx-dg-explained.txt): {}\n",
                list.len()
            );
            for l in list {
                let _ = writeln!(t, "- {l}");
            }
        }
        out.push((sec, t));
    }
    Ok(out)
}

fn begin_line(sec: &str, ctx: &Ctx, body: &str) -> String {
    format!(
        "{BEGIN}{sec} commit={} version={} mode={} fnv64={} -->",
        short(&ctx.commit),
        ctx.version,
        ctx.mode(),
        fnv64(body.as_bytes())
    )
}

/// Splices the generated sections into `old` (the record's current text,
/// empty for a new one): text outside the markers is kept; a section
/// without markers is appended under its heading.
pub fn splice(old: &str, title: &str, sections: &[(&'static str, String)], ctx: &Ctx) -> String {
    let mut text = if old.is_empty() {
        format!(
            "# {title}\n\nWritten by `cargo xtask release-gate record` (docs/RELEASING.md). \
             The tables between the `release-gate` markers are generated from the gate's \
             state records; text outside them is written by hand and kept.\n"
        )
    } else {
        old.to_owned()
    };
    for (sec, body) in sections {
        let block = format!("{}\n{body}{END}{sec} -->", begin_line(sec, ctx, body));
        let open = format!("{BEGIN}{sec} ");
        let close = format!("{END}{sec} -->");
        match (text.find(&open), text.find(&close)) {
            (Some(a), Some(b)) if b > a => text.replace_range(a..b + close.len(), &block),
            _ => {
                let heading = SECTIONS.iter().find(|(s, _)| s == sec).map_or(*sec, |(_, h)| *h);
                let _ = write!(text, "\n## {heading}\n\n{block}\n");
            }
        }
    }
    text
}

/// `check`: every manifest step has a row; one commit, version and mode
/// (`release`) throughout; no generated region edited by hand.
pub fn check(text: &str, steps: &[Step]) -> Result<String, Vec<String>> {
    let mut errs = Vec::new();
    let mut ids = Vec::new();
    let mut stamp: Option<(String, String, String)> = None;
    let mut sections = 0;
    let mut rest = text;
    while let Some(a) = rest.find(BEGIN) {
        let after = &rest[a + BEGIN.len()..];
        let Some(eol) = after.find(" -->\n") else {
            errs.push("a begin marker without its end".to_owned());
            break;
        };
        let fields: Vec<&str> = after[..eol].split(' ').collect();
        let sec = fields.first().copied().unwrap_or("");
        let get = |k: &str| fields.iter().find_map(|f| f.strip_prefix(k)).unwrap_or("").to_owned();
        let body_start = &after[eol + 5..];
        let close = format!("{END}{sec} -->");
        let Some(b) = body_start.find(&close) else {
            errs.push(format!("section {sec}: no end marker"));
            break;
        };
        let body = &body_start[..b];
        sections += 1;
        if fnv64(body.as_bytes()) != get("fnv64=") {
            errs.push(format!(
                "section {sec}: edited by hand (its hash does not match the marker)"
            ));
        }
        let this = (get("commit="), get("version="), get("mode="));
        if this.2 != "release" {
            errs.push(format!(
                "section {sec}: mode {} (a smoke gate is never a release record)",
                this.2
            ));
        }
        match &stamp {
            None => stamp = Some(this.clone()),
            Some(s) if *s != this => errs.push(format!(
                "section {sec}: commit {} version {}, other sections {} {}",
                this.0, this.1, s.0, s.1
            )),
            Some(_) => {}
        }
        for l in body.lines() {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            if let (Some(id), Some(commit)) = (cells.get(1), cells.get(3))
                && let Some(id) = id.strip_prefix('`').and_then(|i| i.strip_suffix('`'))
            {
                let commit = commit.trim_matches('`');
                if commit != this.0 {
                    errs.push(format!(
                        "step {id}: row at commit {commit}, the record is at {}",
                        this.0
                    ));
                }
                if cells.get(6) != Some(&"pass") {
                    errs.push(format!("step {id}: verdict {}", cells.get(6).unwrap_or(&"")));
                }
                ids.push(id.to_owned());
            }
        }
        rest = &body_start[b + close.len()..];
    }
    if sections == 0 {
        errs.push("no release-gate sections".to_owned());
    }
    for s in steps {
        if !ids.contains(&s.id) {
            errs.push(format!("step {}: no row", s.id));
        }
    }
    if errs.is_empty() {
        let (c, v, _) = stamp.unwrap_or_default();
        Ok(format!("{} steps at {c}, version {v}: complete", ids.len()))
    } else {
        Err(errs)
    }
}
