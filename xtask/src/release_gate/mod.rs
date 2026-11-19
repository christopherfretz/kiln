//! `cargo xtask release-gate` (WP-1219): the release gate as one
//! resumable command with a checked record. Contract: spec/harness.md
//! "Release gate"; procedure: docs/RELEASING.md. The steps are the table
//! in [`manifest`]; each step's result is a state record ([`record`]) tied
//! to the release candidate's commit, version and `kiln++`.

mod manifest;
mod record;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use manifest::{Host, Kind, Measured, Step};
use record::{Ctx, Status, StepRecord};

use crate::util;

const USAGE: &str = "\
usage: cargo xtask release-gate <command> [options]

The release gate as one resumable command (docs/RELEASING.md; contract:
spec/harness.md \"Release gate\"). Every step's result is a state record
tied to this commit, version and kiln++; a record made elsewhere is stale
and is re-run, never accepted.

  plan                     the steps for this commit: done, stale, missing
  run [--step <id>[,..]] [--from <id>] [--redo] [--dry-run]
                           run the missing and stale local steps, in
                           dependency order, two tracks side by side
  import-x86 <dir>         record the hosted steps from the downloaded
                           sweep artifacts of this commit (gh run download)
  control <step> <unit> --previous <kiln++> [--times <n>]
                           run one unit n times (default 3) with the RC's and
                           the previous release's kiln++; prints, decides nothing
  record [--out <file>] [--refresh-baselines]
                           write docs/status/release-<version>.md; fails when a
                           step lacks a completed record for this commit
  check <record.md>        verify a committed record

options for every command:
  --state <dir>            the state directory (default <target>/release-gate)
  --external <id>=<cmd>    an external step (remembered in the state directory)
  --smoke                  a restricted gate that proves the machinery; its
                           records and its record file say so and never pass check
  --kiln <kiln++>          with --smoke: this kiln++ instead of the archive's
                           (named kiln++, or kiln for the kiln++ beside it)
";

/// The free space a step leaves on the state directory's file system.
const DISK_MARGIN_GB: u64 = 50;
/// The externals a release gate requires (the 1.2.1 record's downstream build).
const REQUIRED_EXTERNALS: &[&str] = &["downstream"];
/// `--smoke` restrictions: one small directory of each suite.
const SMOKE_TORTURE: &[&str] =
    &["--suite", "execute", "--opt", "-O0", "--filter", "execute/20000*"];
const SMOKE_GXX: &[&str] = &["--filter", "g++.dg/cpp26/*"];
const SMOKE_LIBCXX: &str = "std/utilities/any";

#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent flags")]
struct Opts {
    cmd: String,
    pos: Vec<String>,
    state: Option<PathBuf>,
    externals: Vec<(String, String)>,
    smoke: bool,
    kiln: Option<PathBuf>,
    steps: Vec<String>,
    from: Option<String>,
    redo: bool,
    dry_run: bool,
    previous: Option<PathBuf>,
    times: u32,
    out: Option<PathBuf>,
    refresh: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Opts, String> {
    let mut o = Opts { times: 3, ..Opts::default() };
    let mut it = args.peekable();
    o.cmd = it.next().ok_or("no command")?;
    while let Some(a) = it.next() {
        let mut val = |n: &str| it.next().ok_or(format!("{n} needs a value"));
        match a.as_str() {
            "--state" => o.state = Some(val("--state")?.into()),
            "--external" => {
                let v = val("--external")?;
                let (id, cmd) = v.split_once('=').ok_or("--external takes <id>=<command>")?;
                o.externals.push((id.to_owned(), cmd.to_owned()));
            }
            "--smoke" => o.smoke = true,
            "--kiln" => o.kiln = Some(val("--kiln")?.into()),
            "--step" => o.steps.extend(val("--step")?.split(',').map(str::to_owned)),
            "--from" => o.from = Some(val("--from")?),
            "--redo" => o.redo = true,
            "--dry-run" => o.dry_run = true,
            "--previous" => o.previous = Some(val("--previous")?.into()),
            "--times" => o.times = val("--times")?.parse().map_err(|e| format!("--times: {e}"))?,
            "--out" => o.out = Some(val("--out")?.into()),
            "--refresh-baselines" => o.refresh = true,
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with("--") => return Err(format!("unknown option `{s}`")),
            _ => o.pos.push(a),
        }
    }
    if o.kiln.is_some() && !o.smoke {
        return Err(
            "--kiln is for --smoke gates only: a release gate runs the archive's kiln++".into()
        );
    }
    Ok(o)
}

/// Everything one invocation knows about the gate.
#[derive(Debug)]
struct Gate {
    root: PathBuf,
    state: PathBuf,
    ctx: Ctx,
    steps: Vec<Step>,
    kiln: Option<PathBuf>,
    baseline: Vec<(String, String, u64)>,
}

pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("xtask release-gate: {e}\n");
            }
            eprint!("{USAGE}");
            return if e.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
        }
    };
    let r = load(&o).and_then(|g| match o.cmd.as_str() {
        "plan" => Ok(plan(&g)),
        "run" => run(&g, &o),
        "import-x86" => import_x86(&g, o.pos.first().ok_or("import-x86 <artifact dir>")?),
        "control" => control(&g, &o),
        "record" => write_record(&g, &o),
        "check" => check(&g, o.pos.first().ok_or("check <record.md>")?),
        c => Err(format!("unknown command `{c}`")),
    });
    match r {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("xtask release-gate: {e}");
            ExitCode::FAILURE
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// The workspace version (`[workspace.package] version`).
fn version_of(cargo_toml: &str) -> Option<String> {
    cargo_toml
        .lines()
        .find_map(|l| l.strip_prefix("version = \"")?.strip_suffix('"').map(str::to_owned))
}

fn load(o: &Opts) -> Result<Gate, String> {
    let root = util::repo_root();
    let state = match &o.state {
        Some(s) => s.clone(),
        None => util::target_dir(&root)?.join("release-gate"),
    };
    fs::create_dir_all(state.join("logs")).map_err(|e| format!("{}: {e}", state.display()))?;
    let read = |p: &Path| fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    let version = version_of(&read(&root.join("Cargo.toml"))?).ok_or("no workspace version")?;
    let commit = git(&root, &["rev-parse", "HEAD"])?;
    // The externals: remembered from earlier invocations, plus this one's.
    let ext_file = state.join("externals.tsv");
    let mut externals: Vec<(String, String)> = fs::read_to_string(&ext_file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_owned(), b.to_owned())))
        .collect();
    for (id, cmd) in &o.externals {
        externals.retain(|(i, _)| i != id);
        externals.push((id.clone(), cmd.clone()));
    }
    if !o.externals.is_empty() {
        let text: String =
            externals.iter().fold(String::new(), |t, (a, b)| t + a + "\t" + b + "\n");
        fs::write(&ext_file, text).map_err(|e| format!("{}: {e}", ext_file.display()))?;
    }
    if !o.smoke {
        for id in REQUIRED_EXTERNALS {
            if !externals.iter().any(|(i, _)| i == id) {
                externals.push(((*id).to_owned(), String::new()));
            }
        }
    }
    let projects = manifest::projects_of(&read(&root.join("scripts/oss/projects.toml"))?);
    let steps = manifest::steps(&projects, &externals);
    let kiln = match &o.kiln {
        Some(k) => Some(cxx_driver(k)?),
        None => fs::read_to_string(state.join("kiln++.path")).ok().map(|p| PathBuf::from(p.trim())),
    };
    let kiln_hash = kiln.as_deref().and_then(|k| fs::read(k).ok()).map(|b| record::fnv64(&b));
    let baseline = manifest::read_baseline(
        &fs::read_to_string(root.join("docs/status/release-gate-baseline.tsv")).unwrap_or_default(),
    );
    let ctx = Ctx { commit, version, smoke: o.smoke, kiln: kiln_hash };
    Ok(Gate { root, state, ctx, steps, kiln, baseline })
}

fn rec_path(g: &Gate, id: &str) -> PathBuf {
    g.state.join(format!("{id}.rec"))
}

fn statuses(g: &Gate) -> Vec<Status> {
    g.steps
        .iter()
        .map(|s| {
            let rec = fs::read_to_string(rec_path(g, &s.id)).ok().map(|t| StepRecord::parse(&t));
            let log_hash =
                rec.as_ref().and_then(|r| fs::read(&r.log).ok()).map(|b| record::fnv64(&b));
            record::status(s, rec, &g.ctx, log_hash.as_deref())
        })
        .collect()
}

fn plan(g: &Gate) -> bool {
    let st = statuses(g);
    println!(
        "release gate for {} (version {}, {} mode), state {}\n",
        record::short(&g.ctx.commit),
        g.ctx.version,
        g.ctx.mode(),
        g.state.display()
    );
    let (mut todo, mut minutes) = (0, 0);
    for (s, st) in g.steps.iter().zip(&st) {
        let what = match st {
            Status::Done(r) => format!("done: {}", if r.pass { "pass" } else { "FAIL" }),
            Status::Missing => "missing".to_owned(),
            Status::Stale(w) => format!("stale: {w}"),
        };
        if !matches!(st, Status::Done(_)) {
            todo += 1;
            minutes += s.minutes;
        }
        let host = if s.host == Host::Local { "local" } else { "hosted" };
        let min = if g.ctx.smoke { s.smoke_min_units } else { s.min_units };
        println!(
            "{:<24} {host:<6} {what}\n{:<24} min {min} units ({}), ~{} min, {} GB; pass: {}",
            s.id, "", s.counts, s.minutes, s.scratch_gb, s.rule
        );
    }
    println!("\n{todo} of {} steps to do, about {minutes} min of steps", g.steps.len());
    if g.kiln.is_none() {
        println!("kiln++: none yet (the archive step unpacks it)");
    }
    true
}

/// The steps `run` considers, in manifest order.
fn selected<'a>(g: &'a Gate, o: &Opts) -> Result<Vec<&'a Step>, String> {
    for id in o.steps.iter().chain(&o.from) {
        if !g.steps.iter().any(|s| &s.id == id) {
            return Err(format!("no step `{id}` (see `plan`)"));
        }
    }
    let from = o.from.as_ref().map_or(0, |f| g.steps.iter().position(|s| &s.id == f).unwrap_or(0));
    Ok(g.steps[from..].iter().filter(|s| o.steps.is_empty() || o.steps.contains(&s.id)).collect())
}

fn run(g: &Gate, o: &Opts) -> Result<bool, String> {
    if !o.dry_run && !g.ctx.smoke && !git(&g.root, &["status", "--porcelain"])?.is_empty() {
        return Err("the tree has uncommitted changes: a gate runs on a commit".into());
    }
    let st = statuses(g);
    let mut todo: Vec<&Step> = Vec::new();
    for s in selected(g, o)? {
        let i = g.steps.iter().position(|x| x.id == s.id).unwrap_or(0);
        let why = match &st[i] {
            Status::Done(_) if o.redo && o.steps.contains(&s.id) => "redo".to_owned(),
            Status::Done(_) => {
                println!("skip  {} (done for this commit)", s.id);
                continue;
            }
            Status::Missing => "missing".to_owned(),
            Status::Stale(w) => format!("stale: {w}"),
        };
        if s.host == Host::Hosted {
            println!(
                "hosted {} ({why}): push the RC and import the sweep (docs/RELEASING.md):",
                s.id
            );
            println!("       git push origin HEAD:refs/heads/sweep/rc-{}", g.ctx.version);
            continue;
        }
        println!("{} {} ({why})", if o.dry_run { "would run" } else { "run  " }, s.id);
        todo.push(s);
    }
    if o.dry_run || todo.is_empty() {
        return Ok(true);
    }
    if o.redo {
        // A redo runs the step again: no reused g++.dg run either.
        for s in &todo {
            let _ = fs::remove_file(out_path(g, &format!("{}.stamp", s.id)));
        }
    }
    let ok = std::sync::atomic::AtomicBool::new(true);
    let go = |s: &Step| {
        if !run_step(g, s) {
            ok.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    };
    // The archive first: every later step uses its kiln++.
    let (first, rest): (Vec<&Step>, Vec<&Step>) =
        todo.iter().partition(|s| s.kind == Kind::Archive);
    for s in &first {
        go(s);
    }
    let g2 = if first.is_empty() { None } else { Some(load_again(g)) };
    let g = g2.as_ref().unwrap_or(g);
    let go = |s: &Step| {
        if !run_step(g, s) {
            ok.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    };
    std::thread::scope(|sc| {
        for track in ['a', 'b'] {
            let steps: Vec<&&Step> = rest.iter().filter(|s| s.track == track).collect();
            sc.spawn(move || steps.into_iter().for_each(|s| go(s)));
        }
    });
    Ok(ok.load(std::sync::atomic::Ordering::Relaxed))
}

/// The C++ driver a step gets: an absolute path whose file name is
/// `kiln++`. kiln picks its language from the name it was invoked by, so
/// the same binary as `kiln` links as C and leaves out the C++ runtime.
/// The path is made absolute without resolving symlinks (`kiln++` is a
/// symlink to `kiln` in a build and in the archive); a `kiln` resolves to
/// the `kiln++` beside it, any other name is refused.
pub(super) fn cxx_driver(k: &Path) -> Result<PathBuf, String> {
    let abs = std::path::absolute(k).map_err(|e| format!("{}: {e}", k.display()))?;
    let pp = match abs.file_name().and_then(|n| n.to_str()) {
        Some("kiln++") => abs,
        Some("kiln") => abs.with_file_name("kiln++"),
        _ => {
            return Err(format!(
                "{}: the compiler must be named `kiln++` (or `kiln`, whose sibling `kiln++` is \
                 used): kiln picks C or C++ by the name it is invoked as",
                k.display()
            ));
        }
    };
    if !pp.is_file() {
        return Err(format!("{}: no such kiln++", pp.display()));
    }
    Ok(pp)
}

/// After the archive step: the gate again, with its kiln++.
fn load_again(g: &Gate) -> Gate {
    let mut g2 = Gate {
        root: g.root.clone(),
        state: g.state.clone(),
        ctx: g.ctx.clone(),
        steps: g.steps.clone(),
        kiln: g.kiln.clone(),
        baseline: g.baseline.clone(),
    };
    if let Ok(p) = fs::read_to_string(g.state.join("kiln++.path")) {
        let k = PathBuf::from(p.trim());
        g2.ctx.kiln = fs::read(&k).ok().map(|b| record::fnv64(&b));
        g2.kiln = Some(k);
    }
    g2
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn load1() -> String {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|l| l.split(' ').next().map(str::to_owned))
        .unwrap_or_default()
}

fn free_gb(dir: &Path) -> Option<u64> {
    let o = Command::new("df").arg("-Pk").arg(dir).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    t.lines().nth(1)?.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k / 1_048_576)
}

/// Runs one local step and writes its record. `false` when it did not
/// pass (or could not start).
fn run_step(g: &Gate, s: &Step) -> bool {
    if s.deps.iter().any(|d| d == "archive") && g.kiln.is_none() {
        println!("FAIL  {}: no kiln++ (the archive step has not passed)", s.id);
        return false;
    }
    if let Some(free) = free_gb(&g.state)
        && u64::from(s.scratch_gb) + DISK_MARGIN_GB > free
    {
        println!(
            "REFUSE {}: needs {} GB of scratch, {free} GB free leaves less than {DISK_MARGIN_GB} GB",
            s.id, s.scratch_gb
        );
        return false;
    }
    let log = g.state.join("logs").join(format!("{}.log", s.id));
    let (start, load_start) = (now(), load1());
    println!("start {} ({}; log {})", s.id, record::utc(start), log.display());
    let mut m = match execute(g, s, &log) {
        Ok(m) => m,
        Err(e) => Measured { detail: e, ..Measured::default() },
    };
    manifest::enforce_min(&mut m, if g.ctx.smoke { s.smoke_min_units } else { s.min_units });
    let mut r = StepRecord::new(&s.id, &g.ctx, &m);
    r.start = start;
    r.end = now();
    r.load = format!("{load_start}-{}", load1());
    std::env::consts::ARCH.clone_into(&mut r.arch);
    r.kiln = if s.uses_kiln() { g.ctx.kiln.clone().unwrap_or_default() } else { "-".to_owned() };
    r.log = log.display().to_string();
    r.log_hash = fs::read(&log).map(|b| record::fnv64(&b)).unwrap_or_default();
    let path = rec_path(g, &s.id);
    let tmp = path.with_extension("rec.tmp");
    if fs::write(&tmp, r.to_text()).and_then(|()| fs::rename(&tmp, &path)).is_err() {
        println!("FAIL  {}: cannot write {}", s.id, path.display());
        return false;
    }
    println!(
        "{} {} in {} s: {} units; {}",
        if m.pass { "pass " } else { "FAIL " },
        s.id,
        r.end - start,
        m.units,
        m.detail
    );
    m.pass
}

/// Runs `c` with stdout and stderr appended to `log`; its exit status.
fn sh(c: &mut Command, log: &Path) -> Result<bool, String> {
    let f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| format!("{}: {e}", log.display()))?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let st = c
        .stdout(Stdio::from(f))
        .stderr(Stdio::from(f2))
        .status()
        .map_err(|e| format!("{c:?}: {e}"))?;
    Ok(st.success())
}

fn cargo_xtask(g: &Gate, args: &[&str]) -> Command {
    let mut c = Command::new("cargo");
    c.arg("xtask").args(args).current_dir(&g.root);
    c
}

fn out_path(g: &Gate, name: &str) -> PathBuf {
    let d = g.state.join("out");
    let _ = fs::create_dir_all(&d);
    d.join(name)
}

/// What one local step does: the existing commands, never re-implemented.
#[allow(clippy::too_many_lines, clippy::many_single_char_names, reason = "one arm per kind")]
fn execute(g: &Gate, s: &Step, log: &Path) -> Result<Measured, String> {
    let _ = fs::remove_file(log);
    let kiln = g.kiln.clone().unwrap_or_default();
    let ks = kiln.display().to_string();
    let read = |p: &Path| fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    match &s.kind {
        Kind::Archive => archive(g, log),
        Kind::Ci { args, oracle19 } => {
            let mut c = cargo_xtask(g, &[&["ci"], *args].concat());
            if *oracle19 {
                c.env("KILN_ORACLE", "19");
            }
            let ok = sh(&mut c, log)?;
            Ok(manifest::read_ci(&read(log)?, ok))
        }
        Kind::Pch => {
            let dir = g.state.join("pch");
            let ok = sh(
                Command::new("bash")
                    .arg("scripts/pch-release-gate.sh")
                    .arg(&kiln)
                    .arg(&dir)
                    .arg("all")
                    .current_dir(&g.root),
                log,
            )?;
            Ok(manifest::read_pch(&read(log)?, ok))
        }
        Kind::Torture { std } => {
            let tsv = out_path(g, &format!("{}.tsv", s.id));
            let md = out_path(g, &format!("{}.md", s.id));
            let c_driver = kiln.with_file_name("kiln").display().to_string();
            let (t, m) = (tsv.display().to_string(), md.display().to_string());
            let mut a = vec![
                "gcc-torture",
                "-j",
                "4",
                "--std",
                std,
                "--compiler",
                &c_driver,
                "--tsv",
                &t,
                "--summary",
                &m,
            ];
            a.extend(if g.ctx.smoke {
                SMOKE_TORTURE
            } else {
                &["--suite", "execute,ieee,builtins,compile", "--opt", "-O0,-O2"]
            });
            sh(Command::new("bash").arg("scripts/fetch-gcc-tests.sh").current_dir(&g.root), log)?;
            let ok = sh(&mut cargo_xtask(g, &a), log)?;
            let (run, pass, unknown) = manifest::read_case_tsv(&read(&tsv).unwrap_or_default());
            let mut me = Measured {
                counts: vec![
                    ("pass".into(), pass),
                    ("run".into(), run),
                    ("expect_unknown".into(), unknown),
                ],
                units: run,
                pass: ok,
                detail: String::new(),
            };
            if g.ctx.smoke {
                me.detail = "smoke: a restricted run, the baseline does not apply".into();
            } else {
                manifest::not_below(&mut me, &s.id, &g.baseline);
            }
            Ok(me)
        }
        Kind::Libcxx => {
            let (t, md) =
                (out_path(g, &format!("{}.tsv", s.id)), out_path(g, &format!("{}.md", s.id)));
            let (ts, mds) = (t.display().to_string(), md.display().to_string());
            let sel: &[&str] = if g.ctx.smoke { &[SMOKE_LIBCXX] } else { &["std", "libcxx"] };
            let mut a = vec!["libcxx"];
            a.extend(sel);
            a.extend([
                "--tag",
                "llvmorg-22.1.8",
                "--compiler",
                &ks,
                "--no-fail",
                "-j",
                "3",
                "--timeout",
                "20",
                "--mem-limit",
                "3072",
                "--report",
                &mds,
                "--results",
                &ts,
            ]);
            let ok = sh(&mut cargo_xtask(g, &a), log)?;
            let mut me = manifest::read_libcxx(&[read(&t).unwrap_or_default()]);
            me.pass = ok;
            if g.ctx.smoke {
                me.detail = "smoke: a restricted run, the baseline does not apply".into();
            } else {
                manifest::not_below(&mut me, &s.id, &g.baseline);
            }
            Ok(me)
        }
        Kind::GxxDg => {
            let tsv = out_path(g, &format!("{}.tsv", s.id));
            let stamp = out_path(g, &format!("{}.stamp", s.id));
            let want = format!("{} {} {ks}", g.ctx.commit, g.ctx.kiln.clone().unwrap_or_default());
            // The run's TSV is reused when it is this commit's and kiln++'s
            // (its hash and the path it was invoked by):
            // a killed compare, or an edited explained list, does not cost
            // the two-hour run again.
            if fs::read_to_string(&stamp).ok().as_deref() != Some(want.as_str()) {
                sh(
                    Command::new("bash").arg("scripts/fetch-gcc-tests.sh").current_dir(&g.root),
                    log,
                )?;
                let (t, md) = (
                    tsv.display().to_string(),
                    out_path(g, &format!("{}.md", s.id)).display().to_string(),
                );
                let mut a =
                    vec!["gxx-dg", "-j", "4", "--compiler", &ks, "--tsv", &t, "--summary", &md];
                a.extend(if g.ctx.smoke {
                    SMOKE_GXX
                } else {
                    &["--suite", "dg,torture,common,old-deja", "--opt", "-O0,-O2"]
                });
                if !sh(&mut cargo_xtask(g, &a), log)? {
                    return Err("cargo xtask gxx-dg failed".into());
                }
                fs::write(&stamp, &want).map_err(|e| e.to_string())?;
            }
            gxx_verdict(g, &tsv, "aarch64", log)
        }
        Kind::Oss { project: Some(p) } => oss_local(g, p, &kiln, log),
        Kind::Oss { project: None } => Err("a hosted step: import-x86".into()),
        Kind::External { command } => {
            if command.is_empty() {
                return Err(format!(
                    "no command: pass --external {}=<command>",
                    s.id.trim_start_matches("external-")
                ));
            }
            let ok = sh(Command::new("sh").arg("-c").arg(command).current_dir(&g.root), log)?;
            let text = read(log)?;
            let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_owned();
            let units = last
                .split_whitespace()
                .find_map(|w| w.strip_prefix("units=")?.parse().ok())
                .unwrap_or(0);
            Ok(Measured { counts: vec![("units".into(), units)], units, pass: ok, detail: last })
        }
    }
}

fn gxx_verdict(g: &Gate, tsv: &Path, host: &str, log: &Path) -> Result<Measured, String> {
    let ok = sh(
        Command::new("python3")
            .args([
                "-I",
                "scripts/gxx-dg-compare.py",
                "compare",
                "--gate",
                "--explained",
                "docs/status/gxx-dg-explained.txt",
            ])
            .arg(format!("docs/status/gxx-dg-baseline.txt@{host}"))
            .arg(tsv)
            .current_dir(&g.root),
        log,
    )?;
    let text = fs::read_to_string(log).unwrap_or_default();
    let (run, pass, unknown) =
        manifest::read_case_tsv(&fs::read_to_string(tsv).unwrap_or_default());
    let gate = text
        .lines()
        .rev()
        .find(|l| l.starts_with("gate: "))
        .unwrap_or("no `gate:` line")
        .to_owned();
    Ok(Measured {
        counts: vec![("pass".into(), pass), ("decided".into(), run - unknown), ("run".into(), run)],
        units: run,
        pass: ok,
        detail: gate,
    })
}

fn archive(g: &Gate, log: &Path) -> Result<Measured, String> {
    if !sh(&mut cargo_xtask(g, &["dist", "--check"]), log)? {
        return Err("cargo xtask dist --check failed".into());
    }
    let dist = util::target_dir(&g.root)?.join("kiln-dist");
    let o = Command::new("sha256sum")
        .args(["-c", "SHA256SUMS"])
        .current_dir(&dist)
        .output()
        .map_err(|e| e.to_string())?;
    let sums = String::from_utf8_lossy(&o.stdout).into_owned();
    fs::write(log.with_extension("sha256.log"), &sums).map_err(|e| e.to_string())?;
    let ok_n = sums.lines().filter(|l| l.ends_with(": OK")).count() as u64;
    let prefix = format!("kiln-{}-", g.ctx.version);
    let tarball = fs::read_dir(&dist)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with(&prefix) && n.ends_with("-linux-gnu.tar.xz"))
        .ok_or("no release archive in the dist directory")?;
    let into = g.state.join("kiln");
    fs::create_dir_all(&into).map_err(|e| e.to_string())?;
    if !sh(Command::new("tar").arg("-xJf").arg(dist.join(&tarball)).arg("-C").arg(&into), log)? {
        return Err("cannot unpack the archive".into());
    }
    let kiln = into.join(tarball.trim_end_matches(".tar.xz")).join("bin/kiln++");
    let v = Command::new(&kiln)
        .arg("--version")
        .output()
        .map_err(|e| format!("{}: {e}", kiln.display()))?;
    let first = String::from_utf8_lossy(&v.stdout).lines().next().unwrap_or("").to_owned();
    fs::write(g.state.join("kiln++.path"), kiln.display().to_string())
        .map_err(|e| e.to_string())?;
    let named = first.contains(&format!("kiln version {}", g.ctx.version));
    Ok(Measured {
        counts: vec![("archives_ok".into(), ok_n)],
        units: ok_n,
        pass: o.status.success() && named,
        detail: format!("{first}; kiln++ {}", record::fnv64(&fs::read(&kiln).unwrap_or_default())),
    })
}

fn oss_local(g: &Gate, p: &str, kiln: &Path, log: &Path) -> Result<Measured, String> {
    let root = g.state.join("oss");
    let venv = root.join("venv");
    if !venv.join("bin/meson").is_file() {
        sh(Command::new("python3").arg("-m").arg("venv").arg(&venv), log)?;
        sh(
            Command::new(venv.join("bin/pip")).args([
                "install",
                "-q",
                "meson==1.12.1",
                "ninja==1.13.2",
            ]),
            log,
        )?;
    }
    let mut c = Command::new("timeout");
    c.args(["14400", "scripts/oss/oss-build.sh", p])
        .current_dir(&g.root)
        .env("KILN_OSS_ROOT", &root)
        .env("KILN_OSS_CXX", kiln);
    // Clone failures on a loaded network file system (lesson (e)).
    c.envs([
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "transfer.unpackLimit"),
        ("GIT_CONFIG_VALUE_0", "1"),
        ("GIT_CONFIG_KEY_1", "fetch.unpackLimit"),
        ("GIT_CONFIG_VALUE_1", "1"),
    ]);
    let ok = sh(&mut c, log)?;
    let build = root.join("build").join(p).join("kiln");
    let report = fs::read_to_string(build.join("report.json"))
        .map_err(|_| format!("oss-build.sh exited {ok} without a report.json"))?;
    fs::write(out_path(g, &format!("oss-{p}.json")), &report).map_err(|e| e.to_string())?;
    // The build tree is the state directory's own; the report is kept.
    let _ = fs::remove_dir_all(&build);
    let mut m = manifest::read_oss_report(&report)?;
    let md = fs::read_to_string(g.root.join("docs/status/oss.md")).unwrap_or_default();
    manifest::oss_verdict(&mut m, manifest::oss_baseline(&md, p));
    Ok(m)
}

/// `import-x86`: the hosted steps from the downloaded sweep artifacts.
#[allow(clippy::too_many_lines, reason = "three imports in order")]
fn import_x86(g: &Gate, dir: &str) -> Result<bool, String> {
    let dir = Path::new(dir);
    let mut commits = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    // The artifacts import-x86 reads (not kiln-dist, not the clang
    // control's reports) must each say they are of this commit.
    let read_by_import = |e: &&PathBuf| {
        let n = e.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        e.is_dir()
            && (n.starts_with("libcxx-") && n != "libcxx-summary"
                || n == "gxx-dg-summary"
                || n.starts_with("oss-") && n.ends_with("-kiln"))
    };
    for e in entries.iter().filter(read_by_import) {
        let c = fs::read_to_string(e.join("commit")).map_err(|_| {
            format!("{}: no `commit` file (an artifact of an older sweep.yml?)", e.display())
        })?;
        commits.push((e.clone(), c.trim().to_owned()));
    }
    let wrong: Vec<String> = commits
        .iter()
        .filter(|(_, c)| *c != g.ctx.commit)
        .map(|(e, c)| format!("{} is from {}", e.display(), record::short(c)))
        .collect();
    if !wrong.is_empty() || commits.is_empty() {
        return Err(format!(
            "the artifacts are not all of {}: {}",
            record::short(&g.ctx.commit),
            if commits.is_empty() { "none found".to_owned() } else { wrong.join("; ") }
        ));
    }
    let names = |pre: &str| {
        entries
            .iter()
            .filter(|e| e.file_name().is_some_and(|n| n.to_string_lossy().starts_with(pre)))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut all = true;
    let mut put = |id: &str, mut m: Measured, note: String| -> Result<(), String> {
        let s = g.steps.iter().find(|s| s.id == id).ok_or("no such step")?;
        manifest::enforce_min(&mut m, if g.ctx.smoke { s.smoke_min_units } else { s.min_units });
        let log = g.state.join("logs").join(format!("{id}.log"));
        fs::write(&log, format!("import-x86 {}\n{note}\n", dir.display()))
            .map_err(|e| e.to_string())?;
        let mut r = StepRecord::new(id, &g.ctx, &m);
        (r.start, r.end, r.kiln, r.arch) = (now(), now(), "-".into(), "x86_64".into());
        r.log = log.display().to_string();
        r.log_hash = record::fnv64(&fs::read(&log).unwrap_or_default());
        fs::write(rec_path(g, id), r.to_text()).map_err(|e| e.to_string())?;
        println!("{} {id}: {} units; {}", if m.pass { "pass" } else { "FAIL" }, m.units, m.detail);
        all &= m.pass;
        Ok(())
    };
    // libc++: the shards' results files.
    let shards: Vec<PathBuf> =
        names("libcxx-").into_iter().filter(|e| !e.ends_with("libcxx-summary")).collect();
    let files: Vec<String> = shards
        .iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "tsv"))
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect();
    let mut m = manifest::read_libcxx(&files);
    manifest::not_below(&mut m, "libcxx-x86_64", &g.baseline);
    put("libcxx-x86_64", m, format!("{} shards", shards.len()))?;
    // g++.dg: the merged TSV.
    let tsv = dir.join("gxx-dg-summary/gxx-dg-x86_64.tsv");
    let kept = out_path(g, "gxx-dg-x86_64.tsv");
    fs::copy(&tsv, &kept).map_err(|e| format!("{}: {e}", tsv.display()))?;
    let m = gxx_verdict(g, &kept, "x86_64", &g.state.join("logs/gxx-dg-x86_64.compare.log"))?;
    put("gxx-dg-x86_64", m, tsv.display().to_string())?;
    // OSS: every sweep row's kiln report.
    let toml =
        fs::read_to_string(g.root.join("scripts/oss/projects.toml")).map_err(|e| e.to_string())?;
    let want = sweep_projects(&toml);
    let md = fs::read_to_string(g.root.join("docs/status/oss.md")).unwrap_or_default();
    let (mut got, mut bad) = (0, Vec::new());
    for p in &want {
        match fs::read_to_string(dir.join(format!("oss-{p}-kiln/report.json")))
            .ok()
            .map(|t| manifest::read_oss_report(&t))
        {
            Some(Ok(mut r)) => {
                got += 1;
                let control = fs::read_to_string(dir.join(format!("oss-{p}-clang/report.json")))
                    .ok()
                    .and_then(|t| manifest::read_oss_report(&t).ok());
                let base =
                    manifest::oss_baseline_capped(manifest::oss_baseline(&md, p), control.as_ref());
                manifest::oss_verdict(&mut r, base);
                if !r.pass {
                    bad.push(format!("{p}: {}", r.detail));
                }
            }
            _ => bad.push(format!("{p}: no report")),
        }
    }
    let m = Measured {
        counts: vec![
            ("projects".into(), want.len() as u64),
            ("reported".into(), got),
            ("below_baseline".into(), bad.len() as u64),
        ],
        units: got,
        pass: bad.is_empty() && got == want.len() as u64,
        detail: if bad.is_empty() {
            format!("all {got} sweep rows at their baseline")
        } else {
            bad.join("; ")
        },
    };
    put("oss-x86_64", m, want.join(" "))?;
    Ok(all)
}

/// The `sweep = true` rows of scripts/oss/projects.toml.
fn sweep_projects(toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = None;
    for l in toml.lines().map(str::trim) {
        if let Some(n) = l.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            cur = Some(n.to_owned());
        } else if l.replace(' ', "") == "sweep=true"
            && let Some(c) = cur.take()
        {
            out.push(c);
        }
    }
    out
}

/// `control`: one unit, n times, with both binaries (lesson (c)).
#[allow(clippy::many_single_char_names, reason = "short-lived paths")]
fn control(g: &Gate, o: &Opts) -> Result<bool, String> {
    let [step, unit] = &o.pos[..] else {
        return Err("control <step> <unit> --previous <kiln++>".into());
    };
    let prev = cxx_driver(o.previous.as_deref().ok_or("control needs --previous <kiln++>")?)?;
    let rc = g.kiln.clone().ok_or("no kiln++ for this gate yet")?;
    let s = g.steps.iter().find(|s| &s.id == step).ok_or(format!("no step `{step}`"))?;
    let dir = g.state.join("control");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    println!("| run | {unit} with the RC | with {} |\n|---|---|---|", prev.display());
    for i in 1..=o.times {
        let mut row = vec![i.to_string()];
        for (who, k) in [("rc", &rc), ("previous", &prev)] {
            let log = dir.join(format!("{step}-{who}-{i}.log"));
            let tsv = dir.join(format!("{step}-{who}-{i}.tsv"));
            let (ks, ts) = (k.display().to_string(), tsv.display().to_string());
            let cell = match &s.kind {
                Kind::Oss { .. } => {
                    let root = dir.join(format!("oss-{who}"));
                    let mut c = Command::new("scripts/oss/oss-build.sh");
                    c.arg(unit)
                        .current_dir(&g.root)
                        .env("KILN_OSS_ROOT", &root)
                        .env("KILN_OSS_CXX", k);
                    let rc = sh(&mut c, &log)?;
                    let r =
                        fs::read_to_string(root.join("build").join(unit).join("kiln/report.json"))
                            .ok()
                            .and_then(|t| manifest::read_oss_report(&t).ok());
                    r.map_or(format!("no report (exit ok: {rc})"), |m| m.counts_line())
                }
                Kind::Torture { std } => {
                    let c_driver = k.with_file_name("kiln").display().to_string();
                    sh(
                        &mut cargo_xtask(
                            g,
                            &[
                                "gcc-torture",
                                "--std",
                                std,
                                "--compiler",
                                &c_driver,
                                "--filter",
                                unit,
                                "--tsv",
                                &ts,
                            ],
                        ),
                        &log,
                    )?;
                    classes(&tsv)
                }
                Kind::GxxDg => {
                    sh(
                        &mut cargo_xtask(
                            g,
                            &["gxx-dg", "--compiler", &ks, "--filter", unit, "--tsv", &ts],
                        ),
                        &log,
                    )?;
                    classes(&tsv)
                }
                Kind::Libcxx => {
                    sh(
                        &mut cargo_xtask(
                            g,
                            &["libcxx", "--no-fail", "--compiler", &ks, "--results", &ts, unit],
                        ),
                        &log,
                    )?;
                    fs::read_to_string(&tsv)
                        .unwrap_or_default()
                        .lines()
                        .filter(|l| !l.starts_with('#'))
                        .map(|l| l.split('\t').next().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join(" ")
                }
                _ => return Err(format!("no control for step {step}: run it twice by hand")),
            };
            row.push(cell);
        }
        println!("| {} |", row.join(" | "));
    }
    println!(
        "\nlogs in {}; a human decides flake or regression (docs/RELEASING.md)",
        dir.display()
    );
    Ok(true)
}

/// The kiln classes of a case TSV, as `opt=class` words.
fn classes(tsv: &Path) -> String {
    let text = fs::read_to_string(tsv).unwrap_or_default();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let k = head.iter().position(|h| *h == "kiln").unwrap_or(2);
    lines
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            format!("{}={}", c.get(1).unwrap_or(&""), c.get(k).unwrap_or(&""))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_record(g: &Gate, o: &Opts) -> Result<bool, String> {
    let st = statuses(g);
    let explained =
        fs::read_to_string(g.root.join("docs/status/gxx-dg-explained.txt")).unwrap_or_default();
    let sections = record::render(&g.steps, &st, &g.ctx, &explained)?;
    let path = o
        .out
        .clone()
        .unwrap_or_else(|| g.root.join(format!("docs/status/release-{}.md", g.ctx.version)));
    let old = fs::read_to_string(&path).unwrap_or_default();
    let title = format!(
        "kiln {} release measurements{}",
        g.ctx.version,
        if g.ctx.smoke { " (smoke gate)" } else { "" }
    );
    fs::write(&path, record::splice(&old, &title, &sections, &g.ctx))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("record: wrote {}", path.display());
    if o.refresh {
        refresh(g, &st)?;
    }
    Ok(true)
}

/// `record --refresh-baselines`: this gate becomes the next one's baseline.
fn refresh(g: &Gate, st: &[Status]) -> Result<(), String> {
    if g.ctx.smoke || st.iter().any(|s| !matches!(s, Status::Done(r) if r.pass)) {
        return Err("--refresh-baselines needs a passed release gate".into());
    }
    let mut t = String::from(
        "# The release gate's stored count baselines (WP-1219; spec/harness.md\n# \"Release gate\"): step, count, the value the step must not fall below,\n# where the value comes from. `cargo xtask release-gate record\n# --refresh-baselines` rewrites it from a passed gate.\n",
    );
    for (s, x) in g.steps.iter().zip(st) {
        if let (Status::Done(r), Kind::Torture { .. } | Kind::Libcxx) = (x, &s.kind) {
            let n = r.counts.iter().find(|(k, _)| k == "pass").map_or(0, |(_, v)| *v);
            let _ = writeln!(t, "{}\tpass\t{n}\trelease-{}.md", s.id, g.ctx.version);
        }
    }
    fs::write(g.root.join("docs/status/release-gate-baseline.tsv"), t)
        .map_err(|e| e.to_string())?;
    let ok = Command::new("python3")
        .args([
            "-I",
            "scripts/gxx-dg-compare.py",
            "baseline",
            "--out",
            "docs/status/gxx-dg-baseline.txt",
            "--commit",
            record::short(&g.ctx.commit),
        ])
        .arg(format!("aarch64={}", out_path(g, "gxx-dg-aarch64.tsv").display()))
        .arg(format!("x86_64={}", out_path(g, "gxx-dg-x86_64.tsv").display()))
        .current_dir(&g.root)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return Err("gxx-dg-compare.py baseline failed".into());
    }
    let ex = g.root.join("docs/status/gxx-dg-explained.txt");
    let head = fs::read_to_string(&ex)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with('#'))
        .fold(String::new(), |t, l| t + l + "\n");
    fs::write(&ex, head).map_err(|e| e.to_string())?;
    println!(
        "record: refreshed release-gate-baseline.tsv, gxx-dg-baseline.txt, gxx-dg-explained.txt"
    );
    Ok(())
}

fn check(g: &Gate, file: &str) -> Result<bool, String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    // The steps of a release gate: the externals it requires, whatever
    // this state directory holds.
    let steps: Vec<Step> = g
        .steps
        .iter()
        .filter(|s| {
            !matches!(s.kind, Kind::External { .. })
                || REQUIRED_EXTERNALS.iter().any(|e| s.id == format!("external-{e}"))
        })
        .cloned()
        .collect();
    match record::check(&text, &steps) {
        Ok(msg) => {
            println!("check: {file}: {msg}");
            Ok(true)
        }
        Err(errs) => {
            println!("check: {file}: {} problems:\n  {}", errs.len(), errs.join("\n  "));
            Ok(false)
        }
    }
}
