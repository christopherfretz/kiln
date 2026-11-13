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
use super::{Ctx, Layout, Options, RUN_TIMEOUT, Report, Reports, Subject};
use crate::util::CompilerKind;
use crate::util::json::Json;
use crate::util::process::{self, RssProbe, Shown};

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
    /// The header set needs GCC 16's libstdc++ (`<meta>`): both kiln
    /// commands get `--gcc-toolchain=` naming the GCC 16 installation
    /// ([`gcc16`]); the case is left out without one. clang 19 has no
    /// `<meta>`, so the case has kiln's rows only.
    pub gcc16: bool,
}

/// The cases, in run and page order.
pub const CASES: &[Case] = &[
    Case {
        name: "gnu",
        title: "`<bits/stdc++.h>` (libstdc++ 14)",
        stdlib: None,
        c: false,
        gcc16: false,
    },
    Case {
        name: "lcx",
        title: "All libc++ 19 headers",
        stdlib: Some("libc++"),
        c: false,
        gcc16: false,
    },
    Case {
        name: "mid",
        title: "`<algorithm>`, `<map>`, `<string>`, `<vector>` (libstdc++ 14)",
        stdlib: None,
        c: false,
        gcc16: false,
    },
    Case {
        name: "meta",
        title: "`<meta>` and six containers (GCC 16 libstdc++)",
        stdlib: None,
        c: false,
        gcc16: true,
    },
    Case {
        name: "c",
        title: "C: `<stdio.h>`, `<stdlib.h>`, `<string.h>`, `<zlib.h>`",
        stdlib: None,
        c: true,
        gcc16: false,
    },
];

impl Case {
    pub fn header(&self) -> String {
        format!("bench/{GROUP}/{}.h", self.name)
    }

    pub fn tu(&self) -> String {
        format!("bench/{GROUP}/{}.{}", self.name, if self.c { "c" } else { "cpp" })
    }

    /// The `-empty` variants' main file, shared by every case.
    pub fn empty_tu(&self) -> String {
        format!("bench/{GROUP}/empty.{}", if self.c { "c" } else { "cpp" })
    }

    /// Whether the comparison compiler measures the case (clang 19 has no
    /// `<meta>`).
    pub fn compared(&self) -> bool {
        !self.gcc16
    }

    /// Whether `v` is measured for this case: the snapshot is C++ only.
    pub fn has(&self, v: Variant) -> bool {
        !(self.c && v.snapshot())
    }
}

/// How one compile of a case gets its header set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// `-include <case>.h` (textual inclusion, no PCH).
    Textual,
    /// The PCH build (kiln with `-fkiln-pch-image`).
    Build,
    /// `-include-pch` (kiln with `-fkiln-pch-image=load`; clang's PCH).
    Load,
    /// kiln only: `-fno-kiln-pch-image -include-pch`, the replay.
    Replay,
    /// kiln only: the PCH build with `-fkiln-pch-snapshot=write`.
    SnapBuild,
    /// kiln only: `-fkiln-pch-snapshot=load -include-pch` of the
    /// [`Via::SnapBuild`] PCH: the restored snapshot.
    Snap,
}

/// One measured compile of a case: how it gets the header set, whether it
/// writes an object file (`-O0 -c`) or only checks (`-fsyntax-only`), and
/// whether its main file is the case's unit or the empty one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub via: Via,
    pub obj: bool,
    pub empty: bool,
}

const fn variant(via: Via, obj: bool, empty: bool) -> Variant {
    Variant { via, obj, empty }
}

impl Variant {
    pub const BUILD: Self = variant(Via::Build, false, false);
    pub const SNAP_BUILD: Self = variant(Via::SnapBuild, false, false);
    pub const NONE_SYNTAX: Self = variant(Via::Textual, false, false);
    pub const NONE_OBJ: Self = variant(Via::Textual, true, false);
    pub const LOAD_SYNTAX: Self = variant(Via::Load, false, false);
    pub const LOAD_OBJ: Self = variant(Via::Load, true, false);
    pub const REPLAY_SYNTAX: Self = variant(Via::Replay, false, false);
    pub const REPLAY_OBJ: Self = variant(Via::Replay, true, false);
    pub const SNAP_SYNTAX: Self = variant(Via::Snap, false, false);
    pub const SNAP_OBJ: Self = variant(Via::Snap, true, false);

    /// The consumers, in page order within each main file.
    const CONSUMERS: [Via; 4] = [Via::Textual, Via::Load, Via::Replay, Via::Snap];

    /// Run order: the builds first, since the consumers read their PCHs.
    pub const ALL: [Self; 18] = Self::all();

    const fn all() -> [Self; 18] {
        let mut out = [Self::BUILD; 18];
        out[1] = Self::SNAP_BUILD;
        let mut n = 2;
        let mut e = 0;
        while e < 2 {
            let mut c = 0;
            while c < Self::CONSUMERS.len() {
                out[n] = variant(Self::CONSUMERS[c], false, e == 1);
                out[n + 1] = variant(Self::CONSUMERS[c], true, e == 1);
                n += 2;
                c += 1;
            }
            e += 1;
        }
        out
    }

    /// Page order: the case's unit (no PCH, the builds, the consumers),
    /// then the empty main file's consumers.
    pub const SHOWN: [Self; 18] = [
        Self::NONE_SYNTAX,
        Self::NONE_OBJ,
        Self::BUILD,
        Self::LOAD_SYNTAX,
        Self::LOAD_OBJ,
        Self::REPLAY_SYNTAX,
        Self::REPLAY_OBJ,
        Self::SNAP_BUILD,
        Self::SNAP_SYNTAX,
        Self::SNAP_OBJ,
        variant(Via::Textual, false, true),
        variant(Via::Textual, true, true),
        variant(Via::Load, false, true),
        variant(Via::Load, true, true),
        variant(Via::Replay, false, true),
        variant(Via::Replay, true, true),
        variant(Via::Snap, false, true),
        variant(Via::Snap, true, true),
    ];

    /// The entry id component: `build`, `snap-build`, else
    /// `<none|load|replay|snap>-<syntax|obj>`, with `-empty` for the empty
    /// main file.
    pub fn name(self) -> String {
        let via = match self.via {
            Via::Build => return "build".to_owned(),
            Via::SnapBuild => return "snap-build".to_owned(),
            Via::Textual => "none",
            Via::Load => "load",
            Via::Replay => "replay",
            Via::Snap => "snap",
        };
        let what = if self.obj { "obj" } else { "syntax" };
        format!("{via}-{what}{}", if self.empty { "-empty" } else { "" })
    }

    fn label(self) -> String {
        let via = match self.via {
            Via::Build => return "PCH build".to_owned(),
            Via::SnapBuild => return "PCH build with snapshot (kiln)".to_owned(),
            Via::Textual => "no PCH",
            Via::Load => "PCH load",
            Via::Replay => "PCH replay (kiln)",
            Via::Snap => "PCH snapshot (kiln)",
        };
        let what = if self.obj { "`-O0 -c`" } else { "`-fsyntax-only`" };
        format!("{via}, {what}{}", if self.empty { ", empty main" } else { "" })
    }

    pub fn mode(self) -> Mode {
        if self.builds() {
            Mode::PchBuild
        } else if self.obj {
            Mode::Obj
        } else {
            Mode::Syntax
        }
    }

    fn builds(self) -> bool {
        matches!(self.via, Via::Build | Via::SnapBuild)
    }

    /// Whether only kiln has this variant (clang has no replay switch and
    /// no snapshot).
    pub fn kiln_only(self) -> bool {
        matches!(self.via, Via::Replay | Via::SnapBuild | Via::Snap)
    }

    /// Whether the variant writes or reads the snapshot PCH.
    pub fn snapshot(self) -> bool {
        matches!(self.via, Via::SnapBuild | Via::Snap)
    }

    /// Whether kiln's accounted memory is recorded and judged: the case's
    /// unit built without a PCH (spec/bench.md "The pch group").
    pub fn accounted(self) -> bool {
        self.via == Via::Textual && !self.empty
    }

    fn consumes(self) -> bool {
        matches!(self.via, Via::Load | Via::Replay | Via::Snap)
    }

    /// The build whose PCH the variant reads.
    fn build(self) -> Self {
        if self.snapshot() { Self::SNAP_BUILD } else { Self::BUILD }
    }
}

/// Stands for the compiler's PCH file in [`args`] and the recorded flags.
pub const PCH: &str = "<pch>";
/// Stands for kiln's snapshot PCH file (the [`Via::SnapBuild`] output).
pub const SNAP_PCH: &str = "<snap-pch>";
/// Stands for the compiler's object file.
pub const OBJ: &str = "<obj>";
/// Stands for the GCC 16 installation prefix of a [`Case::gcc16`] case.
pub const GCC16: &str = "<gcc16>";

/// The GCC 16 installation prefix the [`Case::gcc16`] cases pass to
/// `--gcc-toolchain=`: the one of the GCC oracle (`KILN_GCC`, else
/// `g++-16`; scripts/env.sh sets it from `.toolchain/gcc-16.1`), `None`
/// when there is none.
pub fn gcc16() -> Option<&'static Path> {
    static PREFIX: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    PREFIX
        .get_or_init(|| {
            let env = std::env::var(crate::oracle::GCC_ENV).ok();
            let (gcc, _) = crate::oracle::gcc_from(env.as_deref());
            crate::oracle::gcc_toolchain_of(&gcc)
        })
        .as_deref()
}

/// The arguments of one compile of `case`, the input last, with [`PCH`],
/// [`SNAP_PCH`], [`OBJ`] and [`GCC16`] for the run's files (under the
/// target directory, never `/dev/null`). `kiln` selects kiln's image and
/// snapshot flags. The snapshot variants add `-O0` to the build and to
/// `-fsyntax-only`, so that every snapshot command line is the producer's
/// (the snapshot's identity keeps `-O0`).
pub fn args(case: &Case, v: Variant, kiln: bool) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    a.push(if case.c { "-std=c17" } else { "-std=c++26" }.into());
    if let Some(lib) = case.stdlib {
        a.push(format!("-stdlib={lib}"));
    }
    if case.gcc16 {
        a.push(format!("--gcc-toolchain={GCC16}"));
    }
    if kiln {
        match v.via {
            Via::Build => a.push("-fkiln-pch-image".into()),
            Via::Load => a.push("-fkiln-pch-image=load".into()),
            Via::Replay => a.push("-fno-kiln-pch-image".into()),
            Via::SnapBuild => a.push("-fkiln-pch-snapshot=write".into()),
            Via::Snap => a.push("-fkiln-pch-snapshot=load".into()),
            Via::Textual => {}
        }
    }
    let pch = if v.snapshot() { SNAP_PCH } else { PCH };
    match v.via {
        Via::Build | Via::SnapBuild => {
            let x = if case.c { "c-header" } else { "c++-header" };
            if v.snapshot() {
                a.push("-O0".into());
            }
            a.extend(["-x".into(), x.into(), case.header(), "-o".into(), pch.into()]);
            return a;
        }
        Via::Textual => {
            a.extend(["-include".into(), case.header()]);
        }
        _ => a.extend(["-include-pch".into(), pch.into()]),
    }
    if v.mode() == Mode::Obj {
        a.extend(["-O0".into(), "-c".into(), "-o".into(), OBJ.into()]);
    } else {
        if v.snapshot() {
            a.push("-O0".into());
        }
        a.push("-fsyntax-only".into());
    }
    if case.c {
        a.extend(["-x".into(), "c".into()]);
    }
    a.push(if v.empty { case.empty_tu() } else { case.tu() });
    a
}

/// The entry of one variant.
pub fn entry(root: &Path, case: &Case, v: Variant) -> Entry {
    let arg = if v.builds() {
        case.header()
    } else if v.empty {
        case.empty_tu()
    } else {
        case.tu()
    };
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

/// The text after `snapshot ` of the last `-v` note about a PCH's snapshot
/// in `stderr`: `written` or `not written: <why>` from a producer,
/// `matches; restoring it` or why it replays from a consumer (a restore
/// that fails late notes again, so the last note decides).
pub fn snapshot_note(stderr: &str) -> Option<String> {
    const AT: &str = "': snapshot ";
    stderr
        .lines()
        .rev()
        .filter(|l| l.contains("note: precompiled header '"))
        .find_map(|l| Some(l[l.find(AT)? + AT.len()..].to_owned()))
}

/// The path a snapshot consumer took, from its note: `snapshot` when it
/// restored, `replay` otherwise.
pub fn snapshot_path_of(consumer_note: Option<&str>) -> &'static str {
    match consumer_note {
        Some(n) if n.starts_with("matches") => "snapshot",
        Some(_) => "replay",
        None => "unknown",
    }
}

/// What a case found besides its measurements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaseInfo {
    pub load_avg_1m: Option<f64>,
    /// PCH file sizes in bytes: kiln, clang, kiln with the snapshot.
    pub pch_bytes: [Option<u64>; 3],
    /// kiln's producer and consumer `-v` notes ([`image_note`]).
    pub producer_note: Option<String>,
    pub consumer_note: Option<String>,
    /// kiln's snapshot producer and consumer `-v` notes
    /// ([`snapshot_note`]); `None` for a case without the snapshot.
    pub snap_producer_note: Option<String>,
    pub snap_consumer_note: Option<String>,
    /// `-fkiln-pch-image=check` at `-fsyntax-only` and `-O0 -c`, run only
    /// when the image was taken: `Ok` when both agreed with replay, else
    /// the first error.
    pub check: Option<Result<(), String>>,
}

impl CaseInfo {
    pub fn path(&self) -> &'static str {
        path_of(self.consumer_note.as_deref())
    }

    pub fn snapshot_path(&self) -> &'static str {
        snapshot_path_of(self.snap_consumer_note.as_deref())
    }

    fn to_json(&self) -> Json {
        #[allow(clippy::cast_precision_loss)]
        let size = |b: Option<u64>| b.map_or(Json::Null, |b| Json::Num(b as f64));
        let note = |n: &Option<String>| n.as_ref().map_or(Json::Null, Json::str);
        Json::obj([
            ("load_avg_1m", Json::opt_num(self.load_avg_1m.map(|l| round_to(l, 2)))),
            ("pch_bytes_kiln", size(self.pch_bytes[0])),
            ("pch_bytes_clang", size(self.pch_bytes[1])),
            ("pch_bytes_kiln_snapshot", size(self.pch_bytes[2])),
            ("path", Json::str(self.path())),
            ("producer_note", note(&self.producer_note)),
            ("consumer_note", note(&self.consumer_note)),
            ("snapshot_path", Json::str(self.snapshot_path())),
            ("snapshot_producer_note", note(&self.snap_producer_note)),
            ("snapshot_consumer_note", note(&self.snap_consumer_note)),
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
    snap: PathBuf,
    obj: PathBuf,
}

impl Files {
    fn of(scratch: &Path, case: &Case, s: &Subject) -> Self {
        let k = s.kind();
        Self {
            pch: scratch.join(format!("{}.{k}.pch", case.name)),
            snap: scratch.join(format!("{}.{k}-snap.pch", case.name)),
            obj: scratch.join(format!("{}.{k}.o", case.name)),
        }
    }

    fn command(&self, s: &Subject, args: &[String], root: &Path) -> Command {
        let mut cmd = Command::new(&s.path);
        let gcc16 = gcc16().map(|p| p.display().to_string()).unwrap_or_default();
        for a in args {
            match a.as_str() {
                PCH => cmd.arg(&self.pch),
                SNAP_PCH => cmd.arg(&self.snap),
                OBJ => cmd.arg(&self.obj),
                _ if a.contains(GCC16) => cmd.arg(a.replace(GCC16, &gcc16)),
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
    case.has(v) && opts.filter.as_deref().is_none_or(|f| id.contains(f))
}

/// The hash of the variant's sources: the header set and the main file
/// (the case's unit for the builds, as before the empty variants).
fn source_hash(root: &Path, case: &Case, v: Variant) -> Result<String, String> {
    let mut bytes = Vec::new();
    for p in [case.header(), if v.empty { case.empty_tu() } else { case.tu() }] {
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
    let total =
        CASES.iter().map(|c| Variant::ALL.iter().filter(|v| selected(opts, c, **v)).count());
    let total: usize = total.sum();
    let mut n = 0;
    for case in CASES {
        let chosen: Vec<Variant> =
            Variant::ALL.into_iter().filter(|v| selected(opts, case, *v)).collect();
        if chosen.is_empty() {
            continue;
        }
        if case.gcc16 && gcc16().is_none() {
            println!(
                "  {GROUP}/{}: left out: no GCC 16 installation (set KILN_GCC to a g++-16)",
                case.name
            );
            continue;
        }
        let mut info = CaseInfo { load_avg_1m: super::load_avg_1m(), ..CaseInfo::default() };
        let files = [
            Files::of(&cx.layout.scratch, case, test),
            Files::of(&cx.layout.scratch, case, oracle),
        ];
        // Whether each PCH exists: kiln's, clang's, kiln's snapshot PCH.
        let mut built = [false, false, false];
        let slot = |i: usize, v: Variant| if v.snapshot() { 2 } else { i };
        let subjects_of = |v: Variant| -> Vec<(usize, &Subject)> {
            if v.kiln_only() || !case.compared() {
                vec![(0, test)]
            } else {
                vec![(0, test), (1, oracle)]
            }
        };
        // Interleaved: each round runs every variant once with each
        // compiler, so the host's load drifts over all of them alike; the
        // rounds' samples are merged per entry.
        let one = Ctx {
            layout: cx.layout,
            probe: cx.probe.clone(),
            host: cx.host.clone(),
            runs: 1,
            mem_limit: cx.mem_limit,
            verbose: cx.verbose,
            slice: super::SliceCompiles::new(),
            slice_listed: 0,
        };
        let mut parts: BTreeMap<(String, usize), Vec<Outcome>> = BTreeMap::new();
        for round in 1..=cx.runs {
            for v in &chosen {
                let e = entry(root, case, *v);
                let spec = spec_of(*v);
                let hash = source_hash(root, case, *v)?;
                for (i, s) in subjects_of(*v) {
                    let got = parts.entry((e.id.clone(), i)).or_default();
                    if matches!(got.last(), Some(Outcome::Failed(_))) {
                        continue;
                    }
                    let kiln = i == 0;
                    let a = args(case, *v, kiln);
                    // A consumer measured without its build (`--filter`)
                    // builds the PCH once, unmeasured.
                    if v.consumes() && !built[slot(i, *v)] {
                        let b = args(case, v.build(), kiln);
                        let cmd = files[i].command(s, &b, root);
                        built[slot(i, *v)] = run_once(cx, s, &cmd, &b).is_ok();
                    }
                    let cmd = files[i].command(s, &a, root);
                    if cx.verbose {
                        eprintln!("  $ {}", Shown(&cmd));
                    }
                    let flags = a[..a.len() - 1].join(" ");
                    let none = Reports { constexpr: None, inst: false };
                    let cap = cap_for(cx, s, &a);
                    let outcome =
                        super::sample(&one, &e, &spec, s, &cmd, cap, none, (flags, &hash));
                    if v.builds() && matches!(outcome, Outcome::Measured(_)) {
                        built[slot(i, *v)] = true;
                        let out = if v.snapshot() { &files[i].snap } else { &files[i].pch };
                        info.pch_bytes[slot(i, *v)] = std::fs::metadata(out).ok().map(|m| m.len());
                    }
                    got.push(outcome);
                }
            }
            eprintln!("  {GROUP}/{}: round {round}/{} done", case.name, cx.runs);
        }
        for v in &chosen {
            n += 1;
            let e = entry(root, case, *v);
            for (i, s) in subjects_of(*v) {
                let kiln = i == 0;
                let mut outcome = merge(parts.remove(&(e.id.clone(), i)).unwrap_or_default());
                if kiln && v.accounted() {
                    outcome = with_accounted(cx, s, &files[i], case, *v, outcome);
                }
                eprintln!(
                    "[{n:>w$}/{total}] {} {}: {}",
                    e.id,
                    s.kind(),
                    super::describe_outcome(&outcome),
                    w = total.to_string().len()
                );
                let run = if kiln { &mut test_run } else { &mut oracle_run };
                run.entries.insert(e.id.clone(), outcome);
            }
            entries.push(e);
        }
        if built[2] {
            // The snapshot's notes: the producer's (its own PCH) and the
            // path the measured snapshot PCH's consumer takes.
            let spch = cx.layout.scratch.join(format!("{}.kiln-v-snap.pch", case.name));
            let mut b = args(case, Variant::SNAP_BUILD, true);
            b.insert(b.len() - 5, "-v".into());
            let cmd = Files { pch: spch.clone(), snap: spch, obj: files[0].obj.clone() }
                .command(test, &b, root);
            info.snap_producer_note =
                run_once(cx, test, &cmd, &b).ok().and_then(|s| snapshot_note(&s));
            let mut l = args(case, Variant::SNAP_SYNTAX, true);
            l.insert(l.len() - 1, "-v".into());
            let cmd = files[0].command(test, &l, root);
            info.snap_consumer_note =
                run_once(cx, test, &cmd, &l).ok().and_then(|s| snapshot_note(&s));
            eprintln!(
                "  {GROUP}/{}: kiln's snapshot path: {} ({})",
                case.name,
                info.snapshot_path(),
                info.snap_consumer_note.as_deref().unwrap_or("no note")
            );
        }
        // kiln's notes: the producer's reason (its own PCH, so the measured
        // one is untouched) and the path the `load` consumer takes.
        let vpch = cx.layout.scratch.join(format!("{}.kiln-v.pch", case.name));
        let mut b = args(case, Variant::BUILD, true);
        b.insert(b.len() - 5, "-v".into());
        let mut cmd = Files { pch: vpch.clone(), snap: vpch, obj: files[0].obj.clone() }
            .command(test, &b, root);
        info.producer_note = run_once(cx, test, &cmd, &b).ok().and_then(|s| image_note(&s));
        if built[0] {
            let mut l = args(case, Variant::LOAD_SYNTAX, true);
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
            ("kiln_built_here", Json::Bool(opts.compiler.is_none())),
            ("cases", Json::obj(infos.iter().map(|(k, v)| (*k, v.to_json())))),
        ]),
    ));
    report.results_file =
        super::write_results_json(&cx.layout.results, &stamp, &Json::Obj(fields))?;
    println!("\nresults: {}", super::shown_path(root, &report.results_file));

    if opts.record {
        if opts.label.is_none() {
            super::record_baselines(cx, &entries, &rec.test, &stamp, opts.keep_timing, files)?;
        } else {
            println!("--label: the baselines are not recorded (a labelled run is not main's)");
        }
        if opts.filter.is_some() {
            println!("--filter: docs/status/pch.md is not rewritten (its region holds every case)");
        } else {
            let page = cx.layout.perf_md.with_file_name("pch.md");
            let shown = super::shown_path(root, &page);
            let old = std::fs::read_to_string(&page).unwrap_or_default();
            let region = render_region(&RegionContext {
                label: &label,
                when: &record::human_time(now),
                stamp: &stamp,
                commit: commit.as_deref(),
                built_here: opts.compiler.is_none(),
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

/// `outcome` with kiln's accounted memory, from one more, unmeasured
/// compile with `-fkiln-memory-report`, so the measured runs stay as they
/// are. A missing or malformed report fails the entry.
fn with_accounted(
    cx: &Ctx<'_>,
    s: &Subject,
    files: &Files,
    case: &Case,
    v: Variant,
    outcome: Outcome,
) -> Outcome {
    let Outcome::Measured(mut m) = outcome else { return outcome };
    let a = memory_report_args(case, v);
    let cmd = files.command(s, &a, &cx.layout.root);
    match run_once(cx, s, &cmd, &a).and_then(|e| record::Accounted::parse_report(&e)) {
        Ok(acct) => {
            m.accounted = Some(acct);
            Outcome::Measured(m)
        }
        Err(why) => Outcome::Failed(format!("-fkiln-memory-report: {why}")),
    }
}

/// One entry's interleaved rounds (one run each) as one measurement: the
/// samples in round order and their statistics, as `super::sample` gives
/// for its runs. The first failure, if any, is the outcome.
fn merge(rounds: Vec<Outcome>) -> Outcome {
    let mut ms = Vec::with_capacity(rounds.len());
    for o in rounds {
        match o {
            Outcome::Measured(m) => ms.push(m),
            failed @ Outcome::Failed(_) => return failed,
        }
    }
    let Some(mut out) = ms.first().cloned() else { return Outcome::Failed("no runs".into()) };
    let walls: Vec<f64> = ms.iter().flat_map(|m| m.samples_wall_ms.iter().copied()).collect();
    let rss: Vec<u64> = ms.iter().flat_map(|m| m.samples_rss_kb.iter().copied()).collect();
    let Some(w) = super::stats::Summary::of(&walls) else {
        return Outcome::Failed("no runs".into());
    };
    out.runs = walls.len();
    out.wall_ms_median = round_to(w.median, 3);
    out.wall_ms_min = round_to(w.min, 3);
    out.wall_ms_max = round_to(w.max, 3);
    out.spread_pct = round_to(w.spread_pct(), 2);
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max_rss_kb = (rss.len() == walls.len())
        .then(|| super::stats::Summary::of(&rss.iter().map(|&k| k as f64).collect::<Vec<_>>()))
        .flatten()
        .map(|s| s.median.round() as u64);
    out.max_rss_kb = max_rss_kb;
    out.samples_wall_ms = walls;
    out.samples_rss_kb = rss;
    Outcome::Measured(out)
}

/// kiln's arguments of `v` with `-fkiln-memory-report`, before the input
/// (and its `-x c`).
pub fn memory_report_args(case: &Case, v: Variant) -> Vec<String> {
    let mut a = args(case, v, true);
    a.insert(a.len() - 1 - usize::from(case.c) * 2, "-fkiln-memory-report".into());
    a
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

/// The group's default peak-RSS threshold, percent, for every variant.
/// Identical recordings differ by up to 2.5% (`gnu/build`, 86 MB) and the
/// medians of the `none` variants by up to 6.4% (`c/none-syntax`, 13 MB),
/// so the corpus's 2% would flag noise. The `none` variants are also
/// judged on kiln's accounted memory, at [`compare::ACCT_PCT`].
pub const MEM_PCT: f64 = 5.0;

/// The group's thresholds: memory at [`MEM_PCT`], wall time not judged
/// unless `--threshold-time` asks for it (the shared host moves it ~1.4x).
pub fn thresholds(opts: &ThresholdOverrides) -> Thresholds {
    let group = ThresholdOverrides {
        time_pct: Some(f64::INFINITY),
        mem_pct: Some(MEM_PCT),
        ..Default::default()
    };
    Thresholds::default().with(&group).with(opts)
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
    for (r, e) in rows.iter_mut().zip(entries) {
        r.drain(9..19);
        let (base_acct, acct) = accounted_cells(&files, e, test, &test_run.entries[&e.id]);
        let v = variant_of(&e.id);
        let judged = if v.accounted() {
            format!("rss {}%, acct {}%", t.mem_pct, compare::ACCT_PCT)
        } else {
            format!("rss {}%", t.mem_pct)
        };
        let status = r.pop().unwrap_or_default();
        r.extend([base_acct, acct, judged, status]);
    }
    let header = [
        "entry",
        "mode",
        "base ms",
        "ms",
        "time",
        "spread",
        "base MB",
        "MB",
        "mem",
        "base acct",
        "acct",
        "judged",
        "status",
    ];
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

/// The baseline's and the measurement's accounted memory (reserved, MB, and
/// the change), or `-`.
fn accounted_cells(
    files: &BTreeMap<String, record::BaselineFile>,
    e: &Entry,
    test: &Subject,
    outcome: &Outcome,
) -> (String, String) {
    let base = files
        .get(&e.group)
        .and_then(|f| f.entries.get(&e.name))
        .and_then(|k| k.get(test.kind()))
        .and_then(|b| b.measurement.accounted);
    let cur = outcome.measurement().and_then(|m| m.accounted);
    #[allow(clippy::cast_precision_loss)]
    let mb = |a: record::Accounted| format!("{:.2}", a.reserved as f64 / 1e6);
    let cur_cell = match (base, cur) {
        #[allow(clippy::cast_precision_loss)]
        (Some(b), Some(c)) => match compare::delta_pct(b.reserved as f64, c.reserved as f64) {
            Some(d) => format!("{} ({d:+.2}%)", mb(c)),
            None => mb(c),
        },
        (_, Some(c)) => mb(c),
        _ => "-".to_owned(),
    };
    (base.map_or_else(|| "-".to_owned(), mb), cur_cell)
}

fn variant_of(id: &str) -> Variant {
    let name = id.rsplit('/').next().unwrap_or_default();
    Variant::ALL.into_iter().find(|v| v.name() == name).unwrap_or(Variant::NONE_SYNTAX)
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
    for v in [Variant::LOAD_SYNTAX, Variant::LOAD_OBJ] {
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
    /// `git rev-parse` of the harness's tree.
    pub commit: Option<&'a str>,
    /// Whether the run built kiln from that tree (no `--compiler`).
    pub built_here: bool,
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
    const ANY_END: &str = ": end -->";
    let (begin, end) = markers(label);
    let begin_key = begin.split(':').next().unwrap_or(&begin).to_owned() + ":";
    if let Some(b) = page.find(&begin_key)
        && let Some(e) = page[b..].find(&end)
    {
        let e = b + e + end.len();
        let tail = page[e..].strip_prefix('\n').unwrap_or(&page[e..]);
        return format!("{}{region}{tail}", &page[..b]);
    }
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
    m.and_then(|m| m.max_rss_kb)
        .map_or_else(|| "-".to_owned(), |k| format!("{:.0}", k as f64 / 1024.0))
}

#[allow(clippy::cast_precision_loss)]
fn acct_mib(a: record::Accounted) -> String {
    format!("{:.2} MiB", a.reserved as f64 / f64::from(1u32 << 20))
}

#[allow(clippy::cast_precision_loss)]
fn size(b: Option<u64>) -> String {
    b.map_or_else(|| "-".to_owned(), |b| format!("{:.2} MB", b as f64 / 1_048_576.0))
}

fn ratio(t: Option<f64>, o: Option<f64>) -> String {
    t.zip(o)
        .and_then(|(t, o)| compare::ratio(t, o))
        .map_or_else(|| "-".to_owned(), |r| format!("{r:.2}x"))
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
    let snap = format!("$T/{}.kiln-snap.pch", case.name);
    let obj = format!("$T/{}.o", case.name);
    let a: Vec<String> = a
        .into_iter()
        .map(|x| match x.as_str() {
            PCH => pch.clone(),
            SNAP_PCH => snap.clone(),
            OBJ => obj.clone(),
            _ => x.replace(GCC16, "$GCC16"),
        })
        .collect();
    format!("{compiler} {}", a.join(" "))
}

/// The generated region: one section per case.
#[allow(clippy::too_many_lines)]
pub fn render_region(cx: &RegionContext<'_>) -> String {
    let (begin, end) = markers(cx.label);
    let oracle_name = Path::new(cx.oracle.0)
        .file_name()
        .map_or(cx.oracle.0, |n| n.to_str().unwrap_or(cx.oracle.0));
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
    let commit = cx.commit.map_or_else(|| "unknown".to_owned(), |c| format!("`{c}`"));
    if cx.built_here {
        let _ = writeln!(s, "- kiln: `{}`, the release build of commit {commit}.", cx.test.1);
    } else {
        let _ = writeln!(
            s,
            "- kiln: `{}`, given by `--compiler` (the label names its tree); the bench ran \
             from commit {commit}.",
            cx.test.1
        );
    }
    let _ = writeln!(s, "- clang: `{}` (`{oracle_name}`).", cx.oracle.1);
    let _ = writeln!(
        s,
        "- Host {}. Each number is the median of {} runs, interleaved (each round runs every \
         variant of a case once with each compiler): wall time in seconds, peak RSS in MiB \
         (RSS source `{}`). The host is shared: wall times move about 1.4x with its load, so the \
         kiln-to-clang ratios, taken back to back, are the steadier figure. Peak RSS moves a \
         few percent between identical runs; kiln's accounted memory (`-fkiln-memory-report`) \
         does not.",
        cx.host, cx.runs, cx.rss
    );
    let loads: Vec<String> = CASES
        .iter()
        .filter_map(|c| Some((c.name, cx.infos.get(c.name)?.load_avg_1m?)))
        .map(|(n, l)| format!("{n} {l:.1}"))
        .collect();
    if !loads.is_empty() {
        let _ =
            writeln!(s, "- Load average (1 min) at the start of each case: {}.", loads.join(", "));
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
        if case.has(Variant::SNAP_BUILD) {
            let _ = write!(s, " Snapshot path: {}.", info.snapshot_path());
            if let Some(n) = &info.snap_producer_note {
                let _ = write!(s, " The snapshot producer's `-v` says `snapshot {n}`.");
            }
            if let Some(n) = &info.snap_consumer_note {
                let _ = write!(s, " Its consumer's says `snapshot {n}`.");
            }
        }
        let _ = writeln!(s, "\n");
        let _ = write!(
            s,
            "PCH file: kiln {}, clang {}",
            size(info.pch_bytes[0]),
            size(info.pch_bytes[1])
        );
        if case.has(Variant::SNAP_BUILD) {
            let _ = write!(s, ", kiln with the snapshot {}", size(info.pch_bytes[2]));
        }
        let _ = writeln!(s, ".\n");
        if !case.compared() {
            let _ = writeln!(s, "clang 19 has no `<meta>`: kiln's rows only.\n");
        }
        let _ = writeln!(
            s,
            "| | kiln s | kiln MiB | clang s | clang MiB | time kiln/clang | memory kiln/clang |"
        );
        let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|---:|");
        let get = |run, v| measured(run, case, v);
        for v in Variant::SHOWN {
            let id = format!("{GROUP}/{}/{}", case.name, v.name());
            if !cx.test_run.entries.contains_key(&id) {
                continue;
            }
            let t = get(cx.test_run, v);
            let label = if v.via == Via::Load {
                format!("{} ({path} taken)", v.label())
            } else {
                v.label()
            };
            if v.kiln_only() || !case.compared() {
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
        let acct = |v| measured(cx.test_run, case, v).and_then(|m| m.accounted);
        if let (Some(a), Some(b)) = (acct(Variant::NONE_SYNTAX), acct(Variant::NONE_OBJ)) {
            let _ = writeln!(
                s,
                "\nkiln's accounted memory without a PCH (`-fkiln-memory-report`, reserved \
                 bytes): {} (`-fsyntax-only`), {} (`-O0 -c`).",
                acct_mib(a),
                acct_mib(b)
            );
        }
        // What the PCH buys each compiler: load over no PCH.
        let buys = |run: &CompilerRun| -> String {
            let one = |l, n| ratio(wall(measured(run, case, l)), wall(measured(run, case, n)));
            format!(
                "{} (`-fsyntax-only`), {} (`-O0 -c`)",
                one(Variant::LOAD_SYNTAX, Variant::NONE_SYNTAX),
                one(Variant::LOAD_OBJ, Variant::NONE_OBJ)
            )
        };
        if case.compared() {
            let _ = writeln!(
                s,
                "\nWall time of PCH load relative to no PCH: kiln {}; clang {}.",
                buys(cx.test_run),
                buys(cx.oracle_run)
            );
        }
        if case.has(Variant::SNAP_BUILD) {
            // The snapshot against replay and against clang's PCH, for the
            // unit and the empty main file.
            let k = |v| wall(measured(cx.test_run, case, v));
            let o = |v| wall(measured(cx.oracle_run, case, v));
            let row = |obj: bool, empty: bool| {
                let snap = variant(Via::Snap, obj, empty);
                format!(
                    "{} vs replay, {} vs clang's PCH",
                    ratio(k(snap), k(variant(Via::Replay, obj, empty))),
                    ratio(k(snap), o(variant(Via::Load, obj, empty)))
                )
            };
            let _ = writeln!(
                s,
                "\nWall time of the kiln snapshot: `-fsyntax-only` {}; `-O0 -c` {}; empty main, \
                 `-fsyntax-only` {}; empty main, `-O0 -c` {}.",
                row(false, false),
                row(true, false),
                row(false, true),
                row(true, true)
            );
        }
        let _ = writeln!(
            s,
            "\n<details><summary>Command lines (from the repo root, `T=$(mktemp -d)`)</summary>\n"
        );
        let _ = writeln!(s, "```sh");
        for v in Variant::SHOWN.into_iter().filter(|v| case.has(*v)) {
            let _ = writeln!(s, "{}", command_line("kiln++", case, v, true));
            if !v.kiln_only() && case.compared() {
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
        let build = args(gnu, Variant::BUILD, true);
        assert_eq!(
            build.join(" "),
            "-std=c++26 -fkiln-pch-image -x c++-header bench/pch/gnu.h -o <pch>"
        );
        assert_eq!(
            args(gnu, Variant::BUILD, false).join(" "),
            "-std=c++26 -x c++-header bench/pch/gnu.h -o <pch>"
        );
        assert_eq!(
            args(gnu, Variant::LOAD_OBJ, true).join(" "),
            "-std=c++26 -fkiln-pch-image=load -include-pch <pch> -O0 -c -o <obj> bench/pch/gnu.cpp"
        );
        assert_eq!(
            args(&CASES[1], Variant::REPLAY_SYNTAX, true).join(" "),
            "-std=c++26 -stdlib=libc++ -fno-kiln-pch-image -include-pch <pch> -fsyntax-only \
             bench/pch/lcx.cpp"
        );
        assert_eq!(
            args(&CASES[4], Variant::NONE_SYNTAX, false).join(" "),
            "-std=c17 -include bench/pch/c.h -fsyntax-only -x c bench/pch/c.c"
        );
        assert_eq!(
            args(&CASES[4], Variant::BUILD, true).join(" "),
            "-std=c17 -fkiln-pch-image -x c-header bench/pch/c.h -o <pch>"
        );
        for c in CASES {
            for v in Variant::ALL {
                for k in [true, false] {
                    let a = args(c, v, k);
                    assert!(!a.iter().any(|x| x.contains("/dev/null")), "{a:?}");
                    if v.mode() != Mode::Syntax {
                        let o = a.iter().position(|x| x == "-o").expect("an output");
                        assert!([PCH, SNAP_PCH, OBJ].contains(&a[o + 1].as_str()), "{a:?}");
                    }
                }
            }
        }
    }

    /// Only the `none` variants are judged on accounted memory, from a
    /// compile of the same kind with the report flag before the input.
    #[test]
    fn accounted_memory_of_the_none_variants() {
        let judged: Vec<String> =
            Variant::ALL.into_iter().filter(|v| v.accounted()).map(Variant::name).collect();
        assert_eq!(judged, ["none-syntax", "none-obj"]);
        assert_eq!(
            memory_report_args(&CASES[0], Variant::NONE_OBJ).join(" "),
            "-std=c++26 -include bench/pch/gnu.h -O0 -c -o <obj> -fkiln-memory-report \
             bench/pch/gnu.cpp"
        );
        assert_eq!(
            memory_report_args(&CASES[4], Variant::NONE_SYNTAX).join(" "),
            "-std=c17 -include bench/pch/c.h -fsyntax-only -fkiln-memory-report -x c bench/pch/c.c"
        );
        // `--threshold-mem` moves RSS, never the accounted threshold.
        let o = ThresholdOverrides { mem_pct: Some(1.0), ..Default::default() };
        assert!((thresholds(&o).mem_pct - 1.0).abs() < f64::EPSILON);
        assert!((compare::ACCT_PCT - 2.0).abs() < f64::EPSILON);
    }

    /// The snapshot variants carry the producer's flags (`-O0` on every
    /// one), the empty ones compile the shared empty unit, and the `meta`
    /// case names GCC 16 by placeholder.
    #[test]
    fn snapshot_and_empty_variants() {
        let gnu = &CASES[0];
        assert_eq!(
            args(gnu, Variant::SNAP_BUILD, true).join(" "),
            "-std=c++26 -fkiln-pch-snapshot=write -O0 -x c++-header bench/pch/gnu.h -o <snap-pch>"
        );
        assert_eq!(
            args(gnu, Variant::SNAP_SYNTAX, true).join(" "),
            "-std=c++26 -fkiln-pch-snapshot=load -include-pch <snap-pch> -O0 -fsyntax-only \
             bench/pch/gnu.cpp"
        );
        assert_eq!(
            args(gnu, variant(Via::Snap, true, true), true).join(" "),
            "-std=c++26 -fkiln-pch-snapshot=load -include-pch <snap-pch> -O0 -c -o <obj> \
             bench/pch/empty.cpp"
        );
        assert_eq!(
            args(gnu, variant(Via::Load, false, true), false).join(" "),
            "-std=c++26 -include-pch <pch> -fsyntax-only bench/pch/empty.cpp"
        );
        let meta = CASES.iter().find(|c| c.gcc16).expect("a meta case");
        assert_eq!(
            command_line("kiln++", meta, Variant::NONE_SYNTAX, true),
            "kiln++ -std=c++26 --gcc-toolchain=$GCC16 -include bench/pch/meta.h -fsyntax-only \
             bench/pch/meta.cpp"
        );
        assert!(!meta.compared());
        let c = &CASES[4];
        assert!(c.c && !c.has(Variant::SNAP_SYNTAX) && c.has(Variant::REPLAY_OBJ));
        // Every variant has its own name, and the names map back.
        let names: std::collections::BTreeSet<String> =
            Variant::ALL.iter().map(|v| v.name()).collect();
        assert_eq!(names.len(), Variant::ALL.len());
        for v in Variant::SHOWN {
            assert_eq!(variant_of(&format!("pch/gnu/{}", v.name())), v);
        }
        assert_eq!(
            snapshot_note("note: precompiled header 'p.pch': snapshot matches; restoring it\n")
                .as_deref(),
            Some("matches; restoring it")
        );
        let late = "note: precompiled header 'p.pch': snapshot matches; restoring it\nnote: \
                    precompiled header 'p.pch': snapshot Sema: bad\n";
        assert_eq!(snapshot_path_of(snapshot_note(late).as_deref()), "replay");
    }

    #[test]
    fn interleaved_rounds_merge() {
        let one = |ms: f64, kb: u64| {
            let mut m = record::tests::sample("kiln", ms, Some(kb));
            m.samples_wall_ms = vec![ms];
            m.samples_rss_kb = vec![kb];
            Outcome::Measured(m)
        };
        let Outcome::Measured(m) =
            merge(vec![one(30.0, 3), one(10.0, 1), one(20.0, 2), one(50.0, 5), one(40.0, 4)])
        else {
            panic!("measured")
        };
        assert_eq!(m.runs, 5);
        assert_eq!(m.samples_wall_ms, [30.0, 10.0, 20.0, 50.0, 40.0]);
        assert!((m.wall_ms_median - 30.0).abs() < 1e-9 && (m.wall_ms_min - 10.0).abs() < 1e-9);
        assert_eq!(m.max_rss_kb, Some(3));
        assert!(matches!(
            merge(vec![one(1.0, 1), Outcome::Failed("x".into())]),
            Outcome::Failed(_)
        ));
    }

    #[test]
    fn every_case_has_its_files() {
        let root = crate::util::repo_root();
        for c in CASES {
            for p in [c.header(), c.tu(), c.empty_tu()] {
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
        assert_eq!(
            image_note(replayed).as_deref(),
            Some("is replayed: it has no declaration image")
        );
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
        assert!((t.mem_pct - MEM_PCT).abs() < f64::EPSILON);
        let o = ThresholdOverrides { time_pct: Some(10.0), ..Default::default() };
        assert!((thresholds(&o).time_pct - 10.0).abs() < f64::EPSILON);

        // A measurement 3x slower, same memory: not a regression.
        let base = record::Baseline {
            measurement: record::tests::sample("kiln", 100.0, Some(1000)),
            recorded: "r".into(),
        };
        let slow = record::tests::sample("kiln", 300.0, Some(1000));
        assert_eq!(compare::judge(Some(&base), &slow, t), Verdict::Ok);
        let noise = record::tests::sample("kiln", 100.0, Some(1030));
        assert_eq!(compare::judge(Some(&base), &noise, t), Verdict::Ok);
        let big = record::tests::sample("kiln", 100.0, Some(1100));
        assert!(matches!(
            compare::judge(Some(&base), &big, t),
            Verdict::Regressed { mem: true, time: false, .. }
        ));
    }

    #[test]
    fn command_lines_reproduce_by_hand() {
        assert_eq!(
            command_line("clang++-19", &CASES[1], Variant::LOAD_OBJ, false),
            "clang++-19 -std=c++26 -stdlib=libc++ -include-pch $T/lcx.clang.pch -O0 -c -o $T/lcx.o \
             bench/pch/lcx.cpp"
        );
        assert_eq!(
            command_line("kiln++", &CASES[2], Variant::BUILD, true),
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
