//! The `pch` group (`spec/bench.md` "The pch group"): kiln's precompiled
//! headers against clang's, on fixed header sets.
//!
//! Each case is a header set `bench/pch/<case>.h` and a translation unit
//! `bench/pch/<case>.{cpp,c}` that uses it without including it. Every
//! variant of a case (no PCH, PCH build, PCH load, PCH replay) is a bench
//! entry `pch/<case>/<variant>`, measured like any other entry
//! (`super::sample`), judged against `bench/baselines/pch.json` on memory
//! only by default, and, with `--record`, written as a generated region of
//! `docs/status/pch.md`. The run also reports which path kiln's `load`
//! consumer took (the declaration image or replay) and, when it replayed,
//! the producer's reason.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};

use super::compare::{self, ThresholdOverrides, Thresholds, Verdict};
use super::entry::{self, BenchSpec, Entry, Mode};
use super::record::{self, CompilerRun, Outcome, RunRecord};
use super::stats::round_to;
use super::{Ctx, Layout, Options, Report, Reports, RUN_TIMEOUT, Subject};
use crate::util::json::Json;
use crate::util::process::{self, RssProbe, Shown};
use crate::util::CompilerKind;

/// The group directory under the bench root, the entry id prefix and the
/// baseline file's name.
pub const GROUP: &str = "pch";
/// The comparison compiler when `--compare-with` is not given.
pub const DEFAULT_ORACLE: &str = "clang++-19";
/// The region of `docs/status/pch.md` a run without `--label` writes.
pub const DEFAULT_LABEL: &str = "main";

/// A header set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    /// `bench/pch/<name>.h` and the entry id component.
    pub name: &'static str,
    /// The heading of the case in `docs/status/pch.md`.
    pub title: &'static str,
    /// `-stdlib=` for both compilers, if any.
    pub stdlib: Option<&'static str>,
    /// A C case: `-x c-header`/`-x c`, `-std=c17`, a `.c` translation unit.
    pub c: bool,
}

/// The cases, in run and page order.
pub const CASES: &[Case] = &[
    Case { name: "gnu", title: "`<bits/stdc++.h>` (libstdc++ 14)", stdlib: None, c: false },
    Case { name: "lcx", title: "All libc++ 19 headers", stdlib: Some("libc++"), c: false },
    Case {
        name: "mid",
        title: "`<algorithm>`, `<map>`, `<string>`, `<vector>` (libstdc++ 14)",
        stdlib: None,
        c: false,
    },
    Case {
        name: "c",
        title: "C: `<stdio.h>`, `<stdlib.h>`, `<string.h>`, `<zlib.h>`",
        stdlib: None,
        c: true,
    },
];

impl Case {
    pub fn header(&self) -> String {
        format!("bench/{GROUP}/{}.h", self.name)
    }

    pub fn tu(&self) -> String {
        format!("bench/{GROUP}/{}.{}", self.name, if self.c { "c" } else { "cpp" })
    }
}

/// One measured compile of a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// `-include <case>.h`, `-fsyntax-only`.
    NoneSyntax,
    /// `-include <case>.h`, `-O0 -c`.
    NoneObj,
    /// The PCH build (kiln with `-fkiln-pch-image`).
    Build,
    /// `-include-pch`, `-fsyntax-only` (kiln with `-fkiln-pch-image=load`).
    LoadSyntax,
    /// `-include-pch`, `-O0 -c` (kiln with `-fkiln-pch-image=load`).
    LoadObj,
    /// kiln only: `-fno-kiln-pch-image -include-pch`, `-fsyntax-only`.
    ReplaySyntax,
    /// kiln only: `-fno-kiln-pch-image -include-pch`, `-O0 -c`.
    ReplayObj,
}

impl Variant {
    /// Run order: the build first, since the consumers read its PCH.
    pub const ALL: [Self; 7] = [
        Self::Build,
        Self::NoneSyntax,
        Self::NoneObj,
        Self::LoadSyntax,
        Self::LoadObj,
        Self::ReplaySyntax,
        Self::ReplayObj,
    ];

    /// Page order.
    pub const SHOWN: [Self; 7] = [
        Self::NoneSyntax,
        Self::NoneObj,
        Self::Build,
        Self::LoadSyntax,
        Self::LoadObj,
        Self::ReplaySyntax,
        Self::ReplayObj,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NoneSyntax => "none-syntax",
            Self::NoneObj => "none-obj",
            Self::Build => "build",
            Self::LoadSyntax => "load-syntax",
            Self::LoadObj => "load-obj",
            Self::ReplaySyntax => "replay-syntax",
            Self::ReplayObj => "replay-obj",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NoneSyntax => "no PCH, `-fsyntax-only`",
            Self::NoneObj => "no PCH, `-O0 -c`",
            Self::Build => "PCH build",
            Self::LoadSyntax => "PCH load, `-fsyntax-only`",
            Self::LoadObj => "PCH load, `-O0 -c`",
            Self::ReplaySyntax => "PCH replay (kiln), `-fsyntax-only`",
            Self::ReplayObj => "PCH replay (kiln), `-O0 -c`",
        }
    }

    pub fn mode(self) -> Mode {
        match self {
            Self::Build => Mode::PchBuild,
            Self::NoneObj | Self::LoadObj | Self::ReplayObj => Mode::Obj,
            Self::NoneSyntax | Self::LoadSyntax | Self::ReplaySyntax => Mode::Syntax,
        }
    }

    /// Whether only kiln has this variant (clang has no replay switch).
    pub fn kiln_only(self) -> bool {
        matches!(self, Self::ReplaySyntax | Self::ReplayObj)
    }

    fn consumes(self) -> bool {
        matches!(self, Self::LoadSyntax | Self::LoadObj | Self::ReplaySyntax | Self::ReplayObj)
    }
}

/// Stands for the compiler's PCH file in [`args`] and the recorded flags.
pub const PCH: &str = "<pch>";
/// Stands for the compiler's object file.
pub const OBJ: &str = "<obj>";

/// The arguments of one compile of `case`, the input last, with [`PCH`] and
/// [`OBJ`] for the run's files (under the target directory, never
/// `/dev/null`). `kiln` selects kiln's image flags.
pub fn args(case: &Case, v: Variant, kiln: bool) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    a.push(if case.c { "-std=c17" } else { "-std=c++26" }.into());
    if let Some(lib) = case.stdlib {
        a.push(format!("-stdlib={lib}"));
    }
    if kiln {
        match v {
            Variant::Build => a.push("-fkiln-pch-image".into()),
            Variant::LoadSyntax | Variant::LoadObj => a.push("-fkiln-pch-image=load".into()),
            Variant::ReplaySyntax | Variant::ReplayObj => a.push("-fno-kiln-pch-image".into()),
            Variant::NoneSyntax | Variant::NoneObj => {}
        }
    }
    match v {
        Variant::Build => {
            let x = if case.c { "c-header" } else { "c++-header" };
            a.extend(["-x".into(), x.into(), case.header(), "-o".into(), PCH.into()]);
            return a;
        }
        Variant::NoneSyntax | Variant::NoneObj => {
            a.extend(["-include".into(), case.header()]);
        }
        _ => a.extend(["-include-pch".into(), PCH.into()]),
    }
    if v.mode() == Mode::Obj {
        a.extend(["-O0".into(), "-c".into(), "-o".into(), OBJ.into()]);
    } else {
        a.push("-fsyntax-only".into());
    }
    if case.c {
        a.extend(["-x".into(), "c".into()]);
    }
    a.push(case.tu());
    a
}

/// The entry of one variant.
pub fn entry(root: &Path, case: &Case, v: Variant) -> Entry {
    let arg = if v == Variant::Build { case.header() } else { case.tu() };
    Entry {
        id: format!("{GROUP}/{}/{}", case.name, v.name()),
        group: GROUP.into(),
        name: format!("{}/{}", case.name, v.name()),
        path: root.join(&arg),
        arg,
    }
}

/// Splits the command-line inputs into whether they select the group (its
/// directory, named explicitly; the bench root alone does not) and the rest.
/// A file under the directory is an error: the group runs whole, and
/// `--filter` narrows it.
pub fn split_inputs(bench_root: &Path, inputs: &[PathBuf]) -> Result<(bool, Vec<PathBuf>), String> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let dir = canon(&bench_root.join(GROUP));
    let (mut selected, mut rest) = (false, Vec::new());
    for i in inputs {
        let c = canon(i);
        if c == dir {
            selected = true;
        } else if c.starts_with(&dir) {
            return Err(format!(
                "{}: bench/{GROUP} runs as a whole: name bench/{GROUP} and narrow it with --filter",
                i.display()
            ));
        } else {
            rest.push(i.clone());
        }
    }
    Ok((selected, rest))
}

/// The text after `precompiled header '<file>'` of the first `-v` note about
/// a PCH in `stderr`: `: consumers use its declaration image (N records)`
/// and `: consumers replay the header: <why>` from a producer, `:
/// declaration image (N records)` and ` is replayed: <why>` from a
/// consumer. The leading `: ` or space is dropped.
pub fn image_note(stderr: &str) -> Option<String> {
    const AT: &str = "note: precompiled header '";
    stderr.lines().find_map(|l| {
        let rest = &l[l.find(AT)? + AT.len()..];
        let rest = &rest[rest.find('\'')? + 1..];
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();
        Some(rest.to_owned())
    })
}

/// The path a `load` consumer took, from its note.
pub fn path_of(consumer_note: Option<&str>) -> &'static str {
    match consumer_note {
        Some(n) if n.starts_with("declaration image") => "image",
        Some(n) if n.starts_with("is replayed") => "replay",
        _ => "unknown",
    }
}

/// What a case found besides its measurements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaseInfo {
    pub load_avg_1m: Option<f64>,
    /// PCH file sizes in bytes: kiln, clang.
    pub pch_bytes: [Option<u64>; 2],
    /// kiln's producer and consumer `-v` notes ([`image_note`]).
    pub producer_note: Option<String>,
    pub consumer_note: Option<String>,
    /// `-fkiln-pch-image=check` at `-fsyntax-only` and `-O0 -c`, run only
    /// when the image was taken: `Ok` when both agreed with replay, else
    /// the first error.
    pub check: Option<Result<(), String>>,
}

impl CaseInfo {
    pub fn path(&self) -> &'static str {
        path_of(self.consumer_note.as_deref())
    }

    fn to_json(&self) -> Json {
        #[allow(clippy::cast_precision_loss)]
        let size = |b: Option<u64>| b.map_or(Json::Null, |b| Json::Num(b as f64));
        let note = |n: &Option<String>| n.as_ref().map_or(Json::Null, Json::str);
        Json::obj([
            ("load_avg_1m", Json::opt_num(self.load_avg_1m.map(|l| round_to(l, 2)))),
            ("pch_bytes_kiln", size(self.pch_bytes[0])),
            ("pch_bytes_clang", size(self.pch_bytes[1])),
            ("path", Json::str(self.path())),
            ("producer_note", note(&self.producer_note)),
            ("consumer_note", note(&self.consumer_note)),
            (
                "check",
                match &self.check {
                    None => Json::Null,
                    Some(Ok(())) => Json::str("equal"),
                    Some(Err(e)) => Json::str(e),
                },
            ),
        ])
    }
}

/// The run's files of one compiler for one case.
struct Files {
    pch: PathBuf,
    obj: PathBuf,
}

impl Files {
    fn of(scratch: &Path, case: &Case, s: &Subject) -> Self {
        let k = s.kind();
        Self {
            pch: scratch.join(format!("{}.{k}.pch", case.name)),
            obj: scratch.join(format!("{}.{k}.o", case.name)),
        }
    }

    fn command(&self, s: &Subject, args: &[String], root: &Path) -> Command {
        let mut cmd = Command::new(&s.path);
        for a in args {
            match a.as_str() {
                PCH => cmd.arg(&self.pch),
                OBJ => cmd.arg(&self.obj),
                _ => cmd.arg(a),
            };
        }
        cmd.current_dir(root);
        cmd
    }
}

/// `cargo xtask bench bench/pch`: measures every case with kiln and the
/// oracle, judges kiln against `pch.json`, writes the results file and,
/// with `--record`, the baselines and the page region.
pub fn run(opts: &Options, layout: &Layout, compiler: &Path) -> Result<Report, String> {
    let start = Instant::now();
    let test = Subject::new(compiler)?;
    if test.id.kind != CompilerKind::Kiln {
        return Err(format!(
            "bench/{GROUP} measures kiln's PCH: --compiler `{}` is {} (`{}`)",
            compiler.display(),
            test.kind(),
            test.id.version
        ));
    }
    let oracle_path = opts.compare_with.clone().unwrap_or_else(|| PathBuf::from(DEFAULT_ORACLE));
    let oracle = Subject::new(&oracle_path)?;
    if oracle.id.kind != CompilerKind::Clang {
        return Err(format!(
            "bench/{GROUP} compares with clang's PCH: --compare-with `{}` is {}",
            oracle_path.display(),
            oracle.kind()
        ));
    }
    std::fs::create_dir_all(&layout.scratch)
        .map_err(|e| format!("cannot create {}: {e}", layout.scratch.display()))?;
    let cx = Ctx {
        layout,
        probe: RssProbe::detect(&layout.scratch),
        host: record::Host::current(),
        runs: opts.runs,
        mem_limit: process::MemLimit::from_mib(opts.mem_limit_mib),
        verbose: opts.verbose,
        slice: super::SliceCompiles::new(),
        slice_listed: 0,
    };
    let result = run_in(opts, &cx, &test, &oracle, start);
    let _ = std::fs::remove_dir_all(&layout.scratch);
    result
}

fn selected(opts: &Options, case: &Case, v: Variant) -> bool {
    let id = format!("{GROUP}/{}/{}", case.name, v.name());
    opts.filter.as_deref().is_none_or(|f| id.contains(f))
}

fn source_hash(root: &Path, case: &Case) -> Result<String, String> {
    let mut bytes = Vec::new();
    for p in [case.header(), case.tu()] {
        let full = root.join(&p);
        bytes.extend(std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?);
    }
    Ok(entry::source_hash(&bytes))
}

fn cap_for(cx: &Ctx<'_>, s: &Subject, args: &[String]) -> Option<process::MemLimit> {
    cx.mem_limit.map(|l| {
        if s.id.kind == CompilerKind::Kiln {
            l.plus(process::kiln_stack_reservation_mib(args))
        } else {
            l
        }
    })
}

/// One unmeasured compile: `Ok(stderr)` on success, `Err(why)` otherwise.
fn run_once(cx: &Ctx<'_>, s: &Subject, cmd: &Command, args: &[String]) -> Result<String, String> {
    if cx.verbose {
        eprintln!("  $ {}", Shown(cmd));
    }
    let mut c = process::limited(cmd, cap_for(cx, s, args));
    let out = process::run(&mut c, RUN_TIMEOUT)
        .map_err(|e| format!("cannot run {}: {e}", s.path.display()))?;
    let stderr = out.stderr_str();
    if out.success() {
        Ok(stderr)
    } else {
        let head: Vec<&str> = stderr.lines().take(5).collect();
        Err(format!("{}\n{}", out.describe_end(), head.join("\n")).trim_end().to_owned())
    }
}

#[allow(clippy::too_many_lines)]
fn run_in(
    opts: &Options,
    cx: &Ctx<'_>,
    test: &Subject,
    oracle: &Subject,
    start: Instant,
) -> Result<Report, String> {
    let root = &cx.layout.root;
    let now = SystemTime::now();
    let stamp = record::timestamp(now);
    let load = super::load_avg_1m();
    println!(
        "bench: {GROUP} group, {} cases x {} runs with {} ({}), oracle {} ({})",
        CASES.iter().filter(|c| Variant::ALL.iter().any(|v| selected(opts, c, *v))).count(),
        cx.runs,
        test.path.display(),
        test.id.version,
        oracle.path.display(),
        oracle.id.version
    );
    println!(
        "host: {}, {} cpus, load {}; {}",
        cx.host.arch,
        cx.host.cpus,
        load.map_or_else(|| "unknown".to_owned(), |l| format!("{l:.1}")),
        cx.probe.describe()
    );
    let mut test_run = test.run_record();
    let mut oracle_run = oracle.run_record();
    let mut entries = Vec::new();
    let mut infos: BTreeMap<&'static str, CaseInfo> = BTreeMap::new();
    let mut check_failures = Vec::new();
    let total = CASES.iter().map(|c| Variant::ALL.iter().filter(|v| selected(opts, c, **v)).count());
    let total: usize = total.sum();
    let mut n = 0;
    for case in CASES {
        let chosen: Vec<Variant> =
            Variant::ALL.into_iter().filter(|v| selected(opts, case, *v)).collect();
        if chosen.is_empty() {
            continue;
        }
        let hash = source_hash(root, case)?;
        let mut info = CaseInfo { load_avg_1m: super::load_avg_1m(), ..CaseInfo::default() };
        let files = [Files::of(&cx.layout.scratch, case, test), Files::of(&cx.layout.scratch, case, oracle)];
        let mut built = [false, false];
        for v in &chosen {
            n += 1;
            let e = entry(root, case, *v);
            let spec = spec_of(*v);
            let subjects: Vec<(usize, &Subject)> =
                if v.kiln_only() { vec![(0, test)] } else { vec![(0, test), (1, oracle)] };
            for (i, s) in subjects {
                let kiln = i == 0;
                let a = args(case, *v, kiln);
                // A consumer measured without its build (`--filter`) builds the
                // PCH once, unmeasured.
                if v.consumes() && !built[i] {
                    let b = args(case, Variant::Build, kiln);
                    let cmd = files[i].command(s, &b, root);
                    built[i] = run_once(cx, s, &cmd, &b).is_ok();
                }
                let cmd = files[i].command(s, &a, root);
                if cx.verbose {
                    eprintln!("  $ {}", Shown(&cmd));
                }
                let flags = a[..a.len() - 1].join(" ");
                let none = Reports { constexpr: None, inst: false };
                let cap = cap_for(cx, s, &a);
                let outcome = super::sample(cx, &e, &spec, s, &cmd, cap, none, (flags, &hash));
                eprintln!(
                    "[{n:>w$}/{total}] {} {}: {}",
                    e.id,
                    s.kind(),
                    super::describe_outcome(&outcome),
                    w = total.to_string().len()
                );
                if *v == Variant::Build && matches!(outcome, Outcome::Measured(_)) {
                    built[i] = true;
                    info.pch_bytes[i] = std::fs::metadata(&files[i].pch).ok().map(|m| m.len());
                }
                let run = if kiln { &mut test_run } else { &mut oracle_run };
                run.entries.insert(e.id.clone(), outcome);
            }
            entries.push(e);
        }
        // kiln's notes: the producer's reason (its own PCH, so the measured
        // one is untouched) and the path the `load` consumer takes.
        let vpch = cx.layout.scratch.join(format!("{}.kiln-v.pch", case.name));
        let mut b = args(case, Variant::Build, true);
        b.insert(b.len() - 5, "-v".into());
        let mut cmd = Files { pch: vpch, obj: files[0].obj.clone() }.command(test, &b, root);
        info.producer_note = run_once(cx, test, &cmd, &b).ok().and_then(|s| image_note(&s));
        if built[0] {
            let mut l = args(case, Variant::LoadSyntax, true);
            l.insert(l.len() - 1 - usize::from(case.c) * 2, "-v".into());
            cmd = files[0].command(test, &l, root);
            info.consumer_note = run_once(cx, test, &cmd, &l).ok().and_then(|s| image_note(&s));
            if info.path() == "image" {
                info.check = Some(check(cx, test, case, &files[0]));
                if let Some(Err(why)) = &info.check {
                    check_failures.push(format!("{GROUP}/{} (kiln): {why}", case.name));
                }
            }
        }
        eprintln!(
            "  {GROUP}/{}: kiln's load path: {}{}",
            case.name,
            info.path(),
            info.producer_note.as_deref().map_or_else(String::new, |n| format!(" ({n})"))
        );
        infos.insert(case.name, info);
    }
    if entries.is_empty() {
        return Err("no bench entries selected".into());
    }

    let (mut report, files) = judge(opts, cx, &entries, test, &test_run, &oracle_run)?;
    print_ratios(&entries, &test_run, &oracle_run);
    for f in &check_failures {
        report.failed += 1;
        println!("\nimage/replay difference: {f}");
    }

    let rec = RunRecord {
        timestamp: stamp.clone(),
        host: cx.host.clone(),
        load_avg_1m: load.map(|l| round_to(l, 2)),
        rss_source: cx.probe.source_id().to_owned(),
        runs: cx.runs,
        test: test_run,
        oracle: Some(oracle_run),
    };
    let label = opts.label.clone().unwrap_or_else(|| DEFAULT_LABEL.to_owned());
    let commit = git_head(root);
    let Json::Obj(mut fields) = rec.to_json() else { unreachable!("an object") };
    fields.push((
        "pch".into(),
        Json::obj([
            ("label", Json::str(&label)),
            ("commit", commit.as_ref().map_or(Json::Null, Json::str)),
            ("cases", Json::obj(infos.iter().map(|(k, v)| (*k, v.to_json())))),
        ]),
    ));
    report.results_file = super::write_results_json(&cx.layout.results, &stamp, &Json::Obj(fields))?;
    println!("\nresults: {}", super::shown_path(root, &report.results_file));

    if opts.record {
        if opts.label.is_none() {
            super::record_baselines(cx, &entries, &rec.test, &stamp, files)?;
        } else {
            println!("--label: the baselines are not recorded (a labelled run is not main's)");
        }
        if opts.filter.is_some() {
            println!(
                "--filter: docs/status/pch.md is not rewritten (its region holds every case)"
            );
        } else {
            let page = cx.layout.perf_md.with_file_name("pch.md");
            let shown = super::shown_path(root, &page);
            let old = std::fs::read_to_string(&page).unwrap_or_default();
            let region = render_region(&RegionContext {
                label: &label,
                when: &record::human_time(now),
                stamp: &stamp,
                commit: commit.as_deref(),
                test: (&test.path.display().to_string(), &test.id.version),
                oracle: (&oracle.path.display().to_string(), &oracle.id.version),
                host: format!("{}, {} CPUs", cx.host.arch, cx.host.cpus),
                runs: cx.runs,
                rss: cx.probe.source_id(),
                infos: &infos,
                test_run: &rec.test,
                oracle_run: rec.oracle.as_ref().expect("measured above"),
            });
            super::write_file(&page, &splice(&old, &label, &region))?;
            println!("wrote {shown}: region `{label}`");
        }
    }

    report.failing = report.failed > 0 || (report.regressed > 0 && !opts.record);
    println!(
        "\nbench result: {}. {} entries: {} failed, {} regressed{}, {} improved, {} new, {} \
         advisory ({:.1}s)",
        if report.failing { "FAILED" } else { "ok" },
        report.entries,
        report.failed,
        report.regressed,
        if opts.record && report.regressed > 0 { " (re-recorded)" } else { "" },
        report.improved,
        report.new,
        report.advisory,
        start.elapsed().as_secs_f64()
    );
    Ok(report)
}

/// The directives every entry of the group has: the variant's mode.
fn spec_of(v: Variant) -> BenchSpec {
    BenchSpec {
        mode: v.mode(),
        std: "c++26".into(),
        args: Vec::new(),
        oven: Vec::new(),
        thresholds: ThresholdOverrides::default(),
        xfail_kiln: None,
        budget_time_ms: None,
        generate: None,
    }
}

/// The group's thresholds: memory as everywhere, wall time not judged
/// unless `--threshold-time` asks for it (the shared host moves it ~1.4x).
pub fn thresholds(opts: &ThresholdOverrides) -> Thresholds {
    let advisory_time = ThresholdOverrides { time_pct: Some(f64::INFINITY), ..Default::default() };
    Thresholds::default().with(&advisory_time).with(opts)
}

/// Judges kiln's measurements against `pch.json` and prints the table.
fn judge(
    opts: &Options,
    cx: &Ctx<'_>,
    entries: &[Entry],
    test: &Subject,
    test_run: &CompilerRun,
    oracle_run: &CompilerRun,
) -> Result<(Report, BTreeMap<String, record::BaselineFile>), String> {
    let files = super::load_baselines(cx, entries)?;
    let mut report = Report { entries: entries.len(), ..Report::default() };
    let mut rows = Vec::new();
    let mut failures = String::new();
    let t = thresholds(&opts.thresholds);
    for e in entries {
        let base = files
            .get(&e.group)
            .and_then(|f| f.entries.get(&e.name))
            .and_then(|k| k.get(test.kind()));
        let spec = spec_of(variant_of(&e.id));
        match &test_run.entries[&e.id] {
            Outcome::Failed(why) => {
                report.failed += 1;
                let _ = writeln!(failures, "{} ({}): {why}", e.id, test.kind());
                rows.push(super::row(e, Some(&spec), base, None, &Verdict::Ok));
            }
            Outcome::Measured(m) => {
                let v = compare::judge(base, m, t);
                match &v {
                    Verdict::New => report.new += 1,
                    Verdict::Ok => {}
                    Verdict::Improved => report.improved += 1,
                    Verdict::Regressed { .. } => report.regressed += 1,
                    Verdict::Advisory(_) => report.advisory += 1,
                }
                rows.push(super::row(e, Some(&spec), base, Some(m), &v));
            }
        }
        if let Some(Outcome::Failed(why)) = oracle_run.entries.get(&e.id) {
            report.failed += 1;
            let _ = writeln!(failures, "{} (oracle {}): {why}", e.id, oracle_run.kind);
        }
    }
    for r in &mut rows {
        r.drain(9..17);
    }
    let header =
        ["entry", "mode", "base ms", "ms", "time", "spread", "base MB", "MB", "mem", "status"];
    println!();
    print!("{}", compare::render_table(&header, &rows));
    if !t.time_pct.is_finite() {
        println!("(wall time is not judged in the {GROUP} group; --threshold-time judges it)");
    }
    if !failures.is_empty() {
        println!("\nfailures:\n{}", super::indent(&failures));
    }
    Ok((report, files))
}

fn variant_of(id: &str) -> Variant {
    let name = id.rsplit('/').next().unwrap_or_default();
    Variant::ALL.into_iter().find(|v| v.name() == name).unwrap_or(Variant::NoneSyntax)
}

fn print_ratios(entries: &[Entry], test_run: &CompilerRun, oracle_run: &CompilerRun) {
    let rows: Vec<compare::RatioRow> = entries
        .iter()
        .filter(|e| oracle_run.entries.contains_key(&e.id))
        .map(|e| compare::RatioRow {
            id: e.id.clone(),
            mode: variant_of(&e.id).mode().as_str().to_owned(),
            test: test_run.entries[&e.id].measurement().cloned(),
            oracle: oracle_run.entries[&e.id].measurement().cloned(),
        })
        .collect();
    if !rows.is_empty() {
        println!();
        print!("{}", compare::render_ratio_table(&rows, "kiln", "clang"));
    }
}

/// `-fkiln-pch-image=check` at `-fsyntax-only` and at `-O0 -c`: both
/// paths in one process, compared (spec/driver.md "The declaration image").
fn check(cx: &Ctx<'_>, s: &Subject, case: &Case, files: &Files) -> Result<(), String> {
    for v in [Variant::LoadSyntax, Variant::LoadObj] {
        let a: Vec<String> = args(case, v, true)
            .into_iter()
            .map(|a| if a == "-fkiln-pch-image=load" { "-fkiln-pch-image=check".into() } else { a })
            .collect();
        let cmd = files.command(s, &a, &cx.layout.root);
        run_once(cx, s, &cmd, &a).map_err(|why| format!("{}: {why}", v.name()))?;
    }
    Ok(())
}

/// `git rev-parse --short=9 HEAD` of `root`, when it is a checkout.
fn git_head(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !sha.is_empty()).then_some(sha)
}

/// What a page region says besides the measurements.
pub struct RegionContext<'a> {
    pub label: &'a str,
    pub when: &'a str,
    pub stamp: &'a str,
    pub commit: Option<&'a str>,
    /// Path and `--version` line.
    pub test: (&'a str, &'a str),
    pub oracle: (&'a str, &'a str),
    pub host: String,
    pub runs: usize,
    pub rss: &'a str,
    pub infos: &'a BTreeMap<&'static str, CaseInfo>,
    pub test_run: &'a CompilerRun,
    pub oracle_run: &'a CompilerRun,
}

/// The begin and end markers of region `label`.
pub fn markers(label: &str) -> (String, String) {
    (
        format!(
            "<!-- xtask bench pch {label}: begin (generated by `cargo xtask bench bench/pch \
             --record`; do not edit) -->"
        ),
        format!("<!-- xtask bench pch {label}: end -->"),
    )
}

/// `page` with region `label` replaced by `region` (which carries its
/// markers). A missing region goes after the last region of the page, or at
/// its end when it has none.
pub fn splice(page: &str, label: &str, region: &str) -> String {
    let (begin, end) = markers(label);
    let begin_key = begin.split(':').next().unwrap_or(&begin).to_owned() + ":";
    if let Some(b) = page.find(&begin_key)
        && let Some(e) = page[b..].find(&end)
    {
        let e = b + e + end.len();
        let tail = page[e..].strip_prefix('\n').unwrap_or(&page[e..]);
        return format!("{}{region}{tail}", &page[..b]);
    }
    const ANY_END: &str = ": end -->";
    let last = page.match_indices("<!-- xtask bench pch ").filter_map(|(at, _)| {
        let rest = &page[at..];
        let line_end = rest.find('\n').unwrap_or(rest.len());
        rest[..line_end].ends_with(ANY_END).then_some(at + line_end)
    });
    match last.last() {
        Some(at) => {
            let tail = page[at..].strip_prefix('\n').unwrap_or(&page[at..]);
            format!("{}\n\n{region}{tail}", &page[..at])
        }
        None if page.is_empty() => region.to_owned(),
        None => format!("{}\n\n{region}", page.trim_end()),
    }
}

#[allow(clippy::cast_precision_loss)]
fn secs(m: Option<&record::Measurement>) -> String {
    m.map_or_else(|| "FAIL".to_owned(), |m| format!("{:.2}", m.wall_ms_median / 1000.0))
}

#[allow(clippy::cast_precision_loss)]
fn mib(m: Option<&record::Measurement>) -> String {
    m.and_then(|m| m.max_rss_kb).map_or_else(|| "-".to_owned(), |k| format!("{:.0}", k as f64 / 1024.0))
}

#[allow(clippy::cast_precision_loss)]
fn size(b: Option<u64>) -> String {
    b.map_or_else(|| "-".to_owned(), |b| format!("{:.2} MB", b as f64 / 1_048_576.0))
}

fn ratio(t: Option<f64>, o: Option<f64>) -> String {
    t.zip(o).and_then(|(t, o)| compare::ratio(t, o)).map_or_else(|| "-".to_owned(), |r| format!("{r:.2}x"))
}

#[allow(clippy::cast_precision_loss)]
fn rss(m: Option<&record::Measurement>) -> Option<f64> {
    m.and_then(|m| m.max_rss_kb).map(|k| k as f64)
}

fn wall(m: Option<&record::Measurement>) -> Option<f64> {
    m.map(|m| m.wall_ms_median)
}

fn measured<'a>(run: &'a CompilerRun, case: &Case, v: Variant) -> Option<&'a record::Measurement> {
    run.entries.get(&format!("{GROUP}/{}/{}", case.name, v.name()))?.measurement()
}

/// The shell form of one compile, as the spec's "reproduce by hand" gives
/// it: run from the repo root, with `$T` a scratch directory for the PCH and
/// object files.
pub fn command_line(compiler: &str, case: &Case, v: Variant, kiln: bool) -> String {
    let a = args(case, v, kiln);
    let pch = format!("$T/{}.{}.pch", case.name, if kiln { "kiln" } else { "clang" });
    let obj = format!("$T/{}.o", case.name);
    let a: Vec<String> = a
        .into_iter()
        .map(|x| match x.as_str() {
            PCH => pch.clone(),
            OBJ => obj.clone(),
            _ => x,
        })
        .collect();
    format!("{compiler} {}", a.join(" "))
}

/// The generated region: one section per case.
#[allow(clippy::too_many_lines)]
pub fn render_region(cx: &RegionContext<'_>) -> String {
    let (begin, end) = markers(cx.label);
    let oracle_name = Path::new(cx.oracle.0).file_name().map_or(cx.oracle.0, |n| n.to_str().unwrap_or(cx.oracle.0));
    let mut s = String::new();
    let _ = writeln!(s, "{begin}");
    let _ = writeln!(s, "## Measured: {}\n", cx.label);
    let _ = writeln!(
        s,
        "Generated by `cargo xtask bench bench/pch --record{}` on {} (results `{}`).",
        if cx.label == DEFAULT_LABEL { String::new() } else { format!(" --label '{}'", cx.label) },
        cx.when,
        cx.stamp
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- kiln: `{}` ({}), at commit {}.",
        cx.test.1,
        cx.test.0,
        cx.commit.map_or_else(|| "unknown".to_owned(), |c| format!("`{c}`"))
    );
    let _ = writeln!(s, "- clang: `{}` ({}).", cx.oracle.1, cx.oracle.0);
    let _ = writeln!(
        s,
        "- Host {}. Each number is the median of {} runs: wall time in seconds, peak RSS in MiB \
         (`{}`). The host is shared: wall times move about 1.4x with its load, so the \
         kiln-to-clang ratios, taken back to back, are the steadier figure; memory does not \
         move.",
        cx.host, cx.runs, cx.rss
    );
    let loads: Vec<String> = CASES
        .iter()
        .filter_map(|c| Some((c.name, cx.infos.get(c.name)?.load_avg_1m?)))
        .map(|(n, l)| format!("{n} {l:.1}"))
        .collect();
    if !loads.is_empty() {
        let _ = writeln!(s, "- Load average (1 min) at the start of each case: {}.", loads.join(", "));
    }
    for case in CASES {
        let Some(info) = cx.infos.get(case.name) else { continue };
        let _ = writeln!(s, "\n### {}\n", case.title);
        let path = info.path();
        let _ = write!(s, "Path: {path}.");
        if let Some(n) = &info.producer_note {
            let _ = write!(s, " The producer's `-v` says `{n}`.");
        }
        if let Some(n) = &info.consumer_note {
            let _ = write!(s, " The consumer's says `{n}`.");
        }
        match &info.check {
            Some(Ok(())) => {
                let _ = write!(s, " `-fkiln-pch-image=check` finds the image equal to replay.");
            }
            Some(Err(why)) => {
                let first = why.lines().find(|l| l.contains("error")).unwrap_or(why);
                let _ = write!(s, " **`-fkiln-pch-image=check` differs**: `{}`.", first.trim());
            }
            None => {}
        }
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "PCH file: kiln {}, clang {}.\n",
            size(info.pch_bytes[0]),
            size(info.pch_bytes[1])
        );
        let _ = writeln!(s, "| | kiln s | kiln MiB | clang s | clang MiB | time kiln/clang | memory kiln/clang |");
        let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|---:|");
        let get = |run, v| measured(run, case, v);
        for v in Variant::SHOWN {
            let id = format!("{GROUP}/{}/{}", case.name, v.name());
            if !cx.test_run.entries.contains_key(&id) {
                continue;
            }
            let t = get(cx.test_run, v);
            let label = if v == Variant::LoadSyntax || v == Variant::LoadObj {
                format!("{} ({path} taken)", v.label())
            } else {
                v.label().to_owned()
            };
            if v.kiln_only() {
                let _ = writeln!(s, "| {label} | {} | {} | - | - | - | - |", secs(t), mib(t));
            } else {
                let o = get(cx.oracle_run, v);
                let _ = writeln!(
                    s,
                    "| {label} | {} | {} | {} | {} | {} | {} |",
                    secs(t),
                    mib(t),
                    secs(o),
                    mib(o),
                    ratio(wall(t), wall(o)),
                    ratio(rss(t), rss(o))
                );
            }
        }
        // What the PCH buys each compiler: load over no PCH.
        let buys = |run: &CompilerRun| -> String {
            let one = |l, n| ratio(wall(measured(run, case, l)), wall(measured(run, case, n)));
            format!(
                "{} (`-fsyntax-only`), {} (`-O0 -c`)",
                one(Variant::LoadSyntax, Variant::NoneSyntax),
                one(Variant::LoadObj, Variant::NoneObj)
            )
        };
        let _ = writeln!(
            s,
            "\nPCH load over no PCH, wall time: kiln {}; clang {}.",
            buys(cx.test_run),
            buys(cx.oracle_run)
        );
        let _ = writeln!(s, "\n<details><summary>Command lines (from the repo root, `T=$(mktemp -d)`)</summary>\n");
        let _ = writeln!(s, "```sh");
        for v in Variant::SHOWN {
            let _ = writeln!(s, "{}", command_line("kiln++", case, v, true));
            if !v.kiln_only() {
                let _ = writeln!(s, "{}", command_line(oracle_name, case, v, false));
            }
        }
        let _ = writeln!(s, "```\n\n</details>");
    }
    let _ = writeln!(s, "{end}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_name_the_pch_and_never_dev_null() {
        let gnu = &CASES[0];
        let build = args(gnu, Variant::Build, true);
        assert_eq!(
            build.join(" "),
            "-std=c++26 -fkiln-pch-image -x c++-header bench/pch/gnu.h -o <pch>"
        );
        assert_eq!(
            args(gnu, Variant::Build, false).join(" "),
            "-std=c++26 -x c++-header bench/pch/gnu.h -o <pch>"
        );
        assert_eq!(
            args(gnu, Variant::LoadObj, true).join(" "),
            "-std=c++26 -fkiln-pch-image=load -include-pch <pch> -O0 -c -o <obj> bench/pch/gnu.cpp"
        );
        assert_eq!(
            args(&CASES[1], Variant::ReplaySyntax, true).join(" "),
            "-std=c++26 -stdlib=libc++ -fno-kiln-pch-image -include-pch <pch> -fsyntax-only \
             bench/pch/lcx.cpp"
        );
        assert_eq!(
            args(&CASES[3], Variant::NoneSyntax, false).join(" "),
            "-std=c17 -include bench/pch/c.h -fsyntax-only -x c bench/pch/c.c"
        );
        assert_eq!(
            args(&CASES[3], Variant::Build, true).join(" "),
            "-std=c17 -fkiln-pch-image -x c-header bench/pch/c.h -o <pch>"
        );
        for c in CASES {
            for v in Variant::ALL {
                for k in [true, false] {
                    let a = args(c, v, k);
                    assert!(!a.iter().any(|x| x.contains("/dev/null")), "{a:?}");
                    if v.mode() != Mode::Syntax {
                        let o = a.iter().position(|x| x == "-o").expect("an output");
                        assert!(a[o + 1] == PCH || a[o + 1] == OBJ, "{a:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn every_case_has_its_files() {
        let root = crate::util::repo_root();
        for c in CASES {
            for p in [c.header(), c.tu()] {
                assert!(root.join(&p).is_file(), "{p}");
            }
        }
    }

    #[test]
    fn notes_give_the_path() {
        let producer = "kiln -cc1 version 1.0.1\nnote: precompiled header '/t/x.pch': consumers \
                        replay the header: a type the image does not carry\n";
        assert_eq!(
            image_note(producer).as_deref(),
            Some("consumers replay the header: a type the image does not carry")
        );
        let replayed = "note: precompiled header 'p.pch' is replayed: it has no declaration image";
        assert_eq!(image_note(replayed).as_deref(), Some("is replayed: it has no declaration image"));
        assert_eq!(path_of(image_note(replayed).as_deref()), "replay");
        let image = "note: precompiled header 'p.pch': declaration image (162 records)";
        assert_eq!(path_of(image_note(image).as_deref()), "image");
        assert_eq!(path_of(None), "unknown");
        assert_eq!(image_note("warning: nothing\n"), None);
    }

    #[test]
    fn time_is_advisory_unless_asked() {
        let t = thresholds(&ThresholdOverrides::default());
        assert!(t.time_pct.is_infinite());
        assert!((t.mem_pct - 2.0).abs() < f64::EPSILON);
        let o = ThresholdOverrides { time_pct: Some(10.0), ..Default::default() };
        assert!((thresholds(&o).time_pct - 10.0).abs() < f64::EPSILON);

        // A measurement 3x slower, same memory: not a regression.
        let base = record::Baseline {
            measurement: record::tests::sample("kiln", 100.0, Some(1000)),
            recorded: "r".into(),
        };
        let slow = record::tests::sample("kiln", 300.0, Some(1000));
        assert_eq!(compare::judge(Some(&base), &slow, t), Verdict::Ok);
        let big = record::tests::sample("kiln", 100.0, Some(1100));
        assert!(matches!(
            compare::judge(Some(&base), &big, t),
            Verdict::Regressed { mem: true, time: false, .. }
        ));
    }

    #[test]
    fn command_lines_reproduce_by_hand() {
        assert_eq!(
            command_line("clang++-19", &CASES[1], Variant::LoadObj, false),
            "clang++-19 -std=c++26 -stdlib=libc++ -include-pch $T/lcx.clang.pch -O0 -c -o $T/lcx.o \
             bench/pch/lcx.cpp"
        );
        assert_eq!(
            command_line("kiln++", &CASES[2], Variant::Build, true),
            "kiln++ -std=c++26 -fkiln-pch-image -x c++-header bench/pch/mid.h -o $T/mid.kiln.pch"
        );
    }

    #[test]
    fn regions_are_replaced_or_added() {
        let r = |l: &str, body: &str| {
            let (b, e) = markers(l);
            format!("{b}\n{body}\n{e}\n")
        };
        let page = format!("# T\n\nintro\n\n{}\n## Later\n\nkept\n", r("main", "old"));
        let got = splice(&page, "main", &r("main", "new"));
        assert_eq!(got, format!("# T\n\nintro\n\n{}\n## Later\n\nkept\n", r("main", "new")));
        // A new label goes after the last region.
        let got = splice(&got, "wp-1", &r("wp-1", "x"));
        assert_eq!(
            got,
            format!("# T\n\nintro\n\n{}\n{}\n## Later\n\nkept\n", r("main", "new"), r("wp-1", "x"))
        );
        // Replacing the second keeps the first.
        let again = splice(&got, "wp-1", &r("wp-1", "y"));
        assert!(again.contains("new") && again.contains("\ny\n") && !again.contains("\nx\n"));
        // A page without regions gets one at its end.
        assert_eq!(splice("# T\n", "main", &r("main", "a")), format!("# T\n\n{}", r("main", "a")));
        assert_eq!(splice("", "main", &r("main", "a")), r("main", "a"));
    }

    #[test]
    fn selects_only_the_named_directory() {
        let root = crate::util::repo_root().join("bench");
        let (sel, rest) = split_inputs(&root, &[root.join("pch"), root.join("oven")]).unwrap();
        assert!(sel);
        assert_eq!(rest, [root.join("oven")]);
        let (sel, rest) = split_inputs(&root, std::slice::from_ref(&root)).unwrap();
        assert!(!sel);
        assert_eq!(rest.len(), 1);
        let err = split_inputs(&root, &[root.join("pch/gnu.cpp")]).unwrap_err();
        assert!(err.contains("runs as a whole"), "{err}");
    }

    #[test]
    fn the_whole_bench_root_skips_the_group() {
        let root = crate::util::repo_root();
        let bench = root.join("bench");
        let all = entry::discover(&root, &bench, std::slice::from_ref(&bench), None).unwrap();
        assert!(!all.iter().any(|e| e.group == GROUP), "bench/pch is not an ordinary group");
    }
}
