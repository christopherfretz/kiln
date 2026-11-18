//! `cargo xtask kiln-hod <build|test|sweep>`: the `kiln_hod` workspace
//! (a `kiln++` with hod in process; spec/driver.md "In-process back end").
//! Not part of `cargo xtask ci`: the workspace fetches hod by git revision,
//! which the root workspace never does. CI runs it in a job of its own.
//!
//! - `build [--profile <p>]`: `cargo build --locked` of the workspace into
//!   `<target>/kiln_hod`.
//! - `test`: builds the stock `kiln++` (root workspace, debug) and hod's
//!   executable, then runs the workspace's `cargo test --locked` with
//!   `KILN_HOD_STOCK` and `KILN_HOD_EXTERNAL` naming them.
//! - `sweep [options] <dir|file>...`: the equality sweep. For every test
//!   file (its `//@` directives' arguments, `-c -O0` as given), each text
//!   dialect and with and without `-g`: the stock `kiln++` with the
//!   external `hod` executable built from the same revision, then the
//!   wrapper in process, in the same directory with the same output name.
//!   Where the external route took or declined the job, the wrapper must
//!   give the same answer (same reason) and the same object bytes, except
//!   for the jobs the in-process route does not take by rule (`opt`
//!   triggers: counted apart). Exit status 1 on any difference. The
//!   wrapper's route per job (bridge, or text and why) is counted.
//!   `--dual`: the wrapper also runs with `KILN_HOD_DUAL=1` (the bridged
//!   object compared with the text path's in process).
//! - `bridge-eq [options] <dir|file>...`: the bridge's IR-equality test.
//!   The wrapper compiles every test file (as `sweep`, without `-g`) with
//!   `KILN_HOD_BRIDGE_EQ=1`: each module the bridge takes is compared with
//!   what hod's text reader builds from the module's text, both printed by
//!   `hod_ir`. Counts modules compared, equal, refused equally (by
//!   reason), not bridged (by reason) and different; exit status 1 on a
//!   difference not listed in `kiln_hod/bridge-eq-expected.txt`.
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::test::directives::{self, Expect, Lang};
use crate::util::json::Json;
use crate::util::{self, process};

const USAGE: &str = "\
usage: cargo xtask kiln-hod build [--profile <dev|release|harness>]
       cargo xtask kiln-hod test
       cargo xtask kiln-hod sweep [-j N] [--llvm-dir <dir>]... [--no-g] [--dual]
                                  [--report <file>] <dir|file>...
       cargo xtask kiln-hod bridge-eq [-j N] [--llvm-dir <dir>]... [--no-g] [--report <file>]
                                      <dir|file>...

The kiln_hod workspace (kiln_hod/, its own Cargo.lock, hod by git revision):
a kiln++ with hod in process (spec/driver.md \"In-process back end\").
build   cargo build --locked of the workspace into <target>/kiln_hod
test    the stock kiln++ (root, debug) and hod's executable, then the workspace's
        cargo test --locked
sweep   object equality of the in-process route against the stock kiln++ driving
        the external hod built from the same revision, over the test files given;
        dialects: each --llvm-dir (default /usr/lib/llvm-19 and /usr/lib/llvm-22,
        those that exist), each with and without -g (--no-g: without only);
        --dual: the wrapper also compares the bridged object with the text path's
bridge-eq
        IR equality of the bridge (kiln_llvm::Module into hod_ir) with hod's text
        reader on every module of the test files given, printed by hod_ir; each
        dialect, with and without -g (--no-g: without only)
";

/// The expected differences of `bridge-eq`: one test file per line (from
/// the repository root), a tab, and why. Empty while there is none.
const EXPECTED: &str = "kiln_hod/bridge-eq-expected.txt";

/// The text of a failure to fetch hod: the network or the revision.
const FETCH_HINT: &str = "cannot fetch hod at the revision kiln_hod/Cargo.toml pins \
     (https://github.com/christopherfretz/hod): the network is unavailable or the revision is \
     gone; the root workspace does not need it";

pub fn main(mut args: impl Iterator<Item = String>) -> ExitCode {
    let r = match args.next().as_deref() {
        Some("build") => {
            let rest: Vec<String> = args.collect();
            let profile = match rest.as_slice() {
                [] => Ok("dev".to_owned()),
                [f, p] if f == "--profile" => Ok(p.clone()),
                _ => Err(format!("unexpected arguments {rest:?}\n{USAGE}")),
            };
            profile
                .and_then(|p| build(&p).map(|exe| eprintln!("xtask kiln-hod: {}", exe.display())))
        }
        Some("test") => test(),
        Some("sweep") => Sweep::parse(args, Mode::Objects).and_then(|s| s.run()),
        Some("bridge-eq") => Sweep::parse(args, Mode::BridgeEq).and_then(|s| s.run()),
        Some("-h" | "--help") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => Err(USAGE.to_owned()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask kiln-hod: error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The root target directory.
fn root_target() -> Result<PathBuf, String> {
    util::target_dir(&util::repo_root())
}

/// The workspace's target directory: `<target>/kiln_hod`, never one the
/// root workspace builds into (their locks differ).
fn hod_target() -> Result<PathBuf, String> {
    Ok(root_target()?.join("kiln_hod"))
}

/// `cargo <args>` for the workspace (`--manifest-path kiln_hod/Cargo.toml`,
/// `--locked`, its target directory). A failure to fetch hod is said so.
fn cargo_ws(sub: &str, extra: &[&str], env: &[(&str, &Path)]) -> Result<(), String> {
    let manifest = util::repo_root().join("kiln_hod/Cargo.toml");
    let mut cmd = Command::new(env!("CARGO"));
    cmd.arg(sub).arg("--locked").arg("--manifest-path").arg(&manifest).args(extra);
    cmd.env("CARGO_TARGET_DIR", hod_target()?);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let (status, err) =
        process::run_step(&mut cmd, true).map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        return Ok(());
    }
    if err.contains("christopherfretz/hod")
        || err.contains("failed to load source for dependency `hod`")
    {
        return Err(FETCH_HINT.to_owned());
    }
    Err(format!("cargo {sub} --manifest-path kiln_hod/Cargo.toml failed ({status})"))
}

/// Builds the workspace in `profile`; the wrapper's `kiln++`.
fn build(profile: &str) -> Result<PathBuf, String> {
    eprintln!("xtask kiln-hod: cargo build --locked --profile {profile} (kiln_hod workspace)");
    cargo_ws("build", &["--profile", profile], &[])?;
    let dir = if profile == "dev" { "debug" } else { profile };
    util::kilnpp_beside(&hod_target()?.join(dir))
}

fn test() -> Result<(), String> {
    let root = util::repo_root();
    let stock = util::build_kiln(&root, &root_target()?, "kiln-hod", false)?;
    let hod = build_hod_cli("dev")?;
    eprintln!("xtask kiln-hod: cargo test --locked (kiln_hod workspace)");
    cargo_ws("test", &[], &[("KILN_HOD_STOCK", &stock), ("KILN_HOD_EXTERNAL", &hod)])
}

/// hod's workspace as cargo checked it out for the lock (the `hod`
/// package's manifest is `<workspace>/crates/hod/Cargo.toml`).
fn hod_checkout() -> Result<PathBuf, String> {
    let manifest = util::repo_root().join("kiln_hod/Cargo.toml");
    let o = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--manifest-path"])
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !o.status.success() {
        return Err(FETCH_HINT.to_owned());
    }
    let json = Json::parse(&String::from_utf8_lossy(&o.stdout))?;
    let packages = json.get("packages").and_then(Json::as_arr).unwrap_or_default();
    let hod = packages
        .iter()
        .find(|p| p.get("name").and_then(Json::as_str) == Some("hod"))
        .and_then(|p| p.get("manifest_path").and_then(Json::as_str))
        .ok_or("cargo metadata names no package `hod`")?;
    Path::new(hod)
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("unexpected manifest path {hod}"))
}

/// The `hod` executable built from the locked revision (hod's own
/// workspace and lock), in `profile` (`dev` or `release`).
fn build_hod_cli(profile: &str) -> Result<PathBuf, String> {
    let ws = hod_checkout()?;
    let target = root_target()?.join("kiln_hod_hod_cli");
    eprintln!("xtask kiln-hod: building hod's executable ({profile}) from {}", ws.display());
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["build", "--locked", "-p", "hod_cli", "--profile", profile, "--manifest-path"])
        .arg(ws.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target);
    let (status, _) =
        process::run_step(&mut cmd, true).map_err(|e| format!("cannot run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("building hod's executable failed ({status})"));
    }
    Ok(target.join(if profile == "dev" { "debug" } else { profile }).join("hod"))
}

/// What a sweep compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The objects of the external route and the wrapper.
    Objects,
    /// The same, the wrapper with `KILN_HOD_DUAL=1`.
    Dual,
    /// The bridge's IR with the text reader's (`bridge-eq`).
    BridgeEq,
}

/// `sweep`'s options.
#[derive(Debug)]
struct Sweep {
    mode: Mode,
    jobs: usize,
    llvm_dirs: Vec<PathBuf>,
    with_g: bool,
    report: Option<PathBuf>,
    inputs: Vec<PathBuf>,
}

impl Sweep {
    fn parse(args: impl Iterator<Item = String>, mode: Mode) -> Result<Sweep, String> {
        let mut s = Sweep {
            mode,
            jobs: std::thread::available_parallelism().map_or(4, std::num::NonZero::get),
            llvm_dirs: Vec::new(),
            with_g: true,
            report: None,
            inputs: Vec::new(),
        };
        let mut args = args.peekable();
        while let Some(a) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match a.as_str() {
                "-j" => s.jobs = value("-j")?.parse().map_err(|_| "-j needs a number")?,
                "--llvm-dir" => s.llvm_dirs.push(PathBuf::from(value("--llvm-dir")?)),
                "--no-g" => s.with_g = false,
                "--dual" if s.mode == Mode::Objects => s.mode = Mode::Dual,
                "--report" => s.report = Some(PathBuf::from(value("--report")?)),
                o if o.starts_with('-') => return Err(format!("unknown option {o}\n{USAGE}")),
                p => s.inputs.push(PathBuf::from(p)),
            }
        }
        if s.inputs.is_empty() {
            return Err(format!("no test directory or file\n{USAGE}"));
        }
        if s.llvm_dirs.is_empty() {
            s.llvm_dirs = ["/usr/lib/llvm-19", "/usr/lib/llvm-22"]
                .iter()
                .map(PathBuf::from)
                .filter(|d| d.join("bin/llc").is_file())
                .collect();
        }
        if s.llvm_dirs.is_empty() {
            return Err("no LLVM directory (--llvm-dir)".into());
        }
        s.jobs = s.jobs.max(1);
        Ok(s)
    }

    fn run(&self) -> Result<(), String> {
        let root = util::repo_root();
        let (stock, hod) = if self.mode == Mode::BridgeEq {
            (PathBuf::new(), PathBuf::new())
        } else {
            (util::build_kiln(&root, &root_target()?, "kiln-hod", false)?, build_hod_cli("dev")?)
        };
        let wrapper = build("dev")?;
        let files = self.files(&root)?;
        let mut jobs = Vec::new();
        for f in &files {
            for llvm in &self.llvm_dirs {
                for g in [false, true] {
                    if g && (!self.with_g || f.args.iter().any(|a| a == "-g")) {
                        continue;
                    }
                    jobs.push(Job { file: f, llvm, g });
                }
            }
        }
        eprintln!(
            "xtask kiln-hod sweep: {} cases (file x target), {} jobs, {} workers",
            files.len(),
            jobs.len(),
            self.jobs
        );
        let scratch = hod_target()?.join("sweep");
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let tools = Tools {
            mode: self.mode,
            stock: &stock,
            wrapper: &wrapper,
            hod: &hod,
            scratch: &scratch,
        };
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<(usize, Verdict)>> = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..self.jobs {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(job) = jobs.get(i) else { break };
                        // Each child process takes its own token
                        // (`process::run`); a worker holds none.
                        let v = job.run(&tools, i);
                        results
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push((i, v));
                    }
                });
            }
        });
        let mut results = results.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
        results.sort_by_key(|(i, _)| *i);
        let _ = std::fs::remove_dir_all(&scratch);
        if self.mode == Mode::BridgeEq {
            return self.summarize_eq(&root, &jobs, &results);
        }
        self.summarize(&jobs, &results)
    }

    /// The test files under the inputs: those whose directives say they
    /// compile (no `compile-fail`, `pp`, `no-crash`, `ice`), without
    /// modules, header units, extra units or a PCH, and whose arguments
    /// need no harness-only placeholder.
    fn files(&self, root: &Path) -> Result<Vec<File>, String> {
        let mut paths = Vec::new();
        for i in &self.inputs {
            let i = if i.is_absolute() { i.clone() } else { root.join(i) };
            walk(&i, &mut paths).map_err(|e| format!("{}: {e}", i.display()))?;
        }
        paths.sort();
        paths.dedup();
        let mut out = Vec::new();
        for p in paths {
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let lang = if p.extension().is_some_and(|e| e == "c") { Lang::C } else { Lang::Cxx };
            if let Some(f) = codegen_case(&p, &text, lang) {
                out.extend(f);
                continue;
            }
            if !text.contains("//@") {
                continue;
            }
            let Ok(spec) = directives::parse(&text, lang) else { continue };
            if matches!(
                spec.expect,
                Expect::CompileFail | Expect::Pp | Expect::NoCrash | Expect::Ice
            ) || !spec.modules.is_empty()
                || !spec.units.is_empty()
                || !spec.header_units.is_empty()
                || spec.pch.is_some()
            {
                continue;
            }
            let dir = p.parent().unwrap_or(root).to_path_buf();
            let unresolved = std::cell::Cell::new(false);
            let args: Vec<String> = spec
                .args
                .iter()
                .map(|a| {
                    directives::substitute(a, |n| match n {
                        "repo_root" => Some(root.display().to_string()),
                        "resource_dir" => Some(root.join("resource").display().to_string()),
                        "test_dir" => Some(dir.display().to_string()),
                        _ => {
                            unresolved.set(true);
                            None
                        }
                    })
                })
                .collect();
            if unresolved.get() {
                continue;
            }
            let mut args = object_args(&args);
            args.insert(0, format!("-std={}", spec.std));
            out.push(File { path: p, lang, args });
        }
        Ok(out)
    }

    fn summarize(&self, jobs: &[Job<'_>], results: &[(usize, Verdict)]) -> Result<(), String> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        let mut routes: BTreeMap<String, usize> = BTreeMap::new();
        let mut bad = Vec::new();
        let mut tsv = String::from("file\tllvm\tg\texternal\tin_process\tresult\n");
        for (i, v) in results {
            let job = &jobs[*i];
            *counts.entry(v.kind.to_owned()).or_default() += 1;
            if let Some(r) = &v.reason {
                *reasons.entry(r.clone()).or_default() += 1;
            }
            if let Some(r) = &v.route {
                *routes.entry(r.clone()).or_default() += 1;
            }
            let line = format!(
                "{} {}\t{}\t{}\t{}\t{}\t{}",
                job.file.path.display(),
                job.file
                    .args
                    .iter()
                    .find(|a| a.starts_with("--target="))
                    .map_or("", String::as_str),
                job.llvm.display(),
                u8::from(job.g),
                v.external,
                v.in_process,
                v.kind
            );
            let _ = writeln!(tsv, "{line}");
            if v.bad {
                bad.push(line);
            }
        }
        if let Some(r) = &self.report {
            std::fs::write(r, &tsv).map_err(|e| format!("{}: {e}", r.display()))?;
        }
        eprintln!("xtask kiln-hod sweep: {} jobs", results.len());
        for (k, n) in &counts {
            eprintln!("  {k}: {n}");
        }
        eprintln!("  declines by reason (both routes):");
        for (r, n) in &reasons {
            eprintln!("    {r}: {n}");
        }
        eprintln!("  the wrapper's route (modules):");
        for (r, n) in &routes {
            eprintln!("    {r}: {n}");
        }
        for b in &bad {
            eprintln!("DIFF {b}");
        }
        if bad.is_empty() { Ok(()) } else { Err(format!("{} differences", bad.len())) }
    }
}

/// The files of `p` (itself if a file), recursively, skipping `Inputs/`
/// directories (their files are included by tests, not tests).
fn walk(p: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if p.is_file() {
        out.push(p.to_path_buf());
        return Ok(());
    }
    for e in std::fs::read_dir(p)? {
        let path = e?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "Inputs") {
                walk(&path, out)?;
            }
        } else if path.extension().is_some_and(|x| x == "cpp" || x == "c" || x == "cc") {
            out.push(path);
        }
    }
    Ok(())
}

/// A test's arguments for an object compile: without the options that
/// pick another output (`-S`, `-emit-llvm`, `-E`, `-fsyntax-only`, `-c`,
/// `-o <f>`, the dumps); the sweep adds `-c -o out.o`.
fn object_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => {
                it.next();
            }
            "-S" | "-emit-llvm" | "-E" | "-P" | "-fsyntax-only" | "-c" => {}
            a if a.starts_with("-fdump-") || a.starts_with("-fkiln-dump") => {}
            _ => out.push(a.clone()),
        }
    }
    out
}

/// A file of the IR-diff corpus (`tests/codegen`, no `expect:`
/// directive; `crates/kiln_codegen/tests/ir_vs_clang.rs`): one job per
/// triple it is compared on, with its pinned flags and `//@ args:` lines;
/// `None` for any other file, and no job for a case that imports modules.
fn codegen_case(p: &Path, text: &str, lang: Lang) -> Option<Vec<File>> {
    if !p.parent().is_some_and(|d| d.ends_with("tests/codegen")) || text.contains("//@ expect:") {
        return None;
    }
    if text.contains("//@ module:") {
        return Some(Vec::new());
    }
    let extra: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("//@ args:"))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect();
    let files = [
        (
            "aarch64-unknown-linux-gnu",
            &["-moutline-atomics", "-ftrivial-auto-var-init=uninitialized"][..],
        ),
        ("x86_64-unknown-linux-gnu", &["-ftrivial-auto-var-init=uninitialized"][..]),
    ]
    .iter()
    .map(|(triple, pinned)| {
        let mut args = vec!["-std=c++26".to_owned(), format!("--target={triple}")];
        args.extend(pinned.iter().map(|a| (*a).to_owned()));
        args.extend(object_args(&extra));
        File { path: p.to_path_buf(), lang, args }
    })
    .collect();
    Some(files)
}

#[derive(Debug)]
struct File {
    path: PathBuf,
    lang: Lang,
    /// The arguments before `-c <file> -o out.o`, `-std=` and any
    /// `--target=` included.
    args: Vec<String>,
}

#[derive(Debug)]
struct Job<'a> {
    file: &'a File,
    llvm: &'a PathBuf,
    g: bool,
}

struct Tools<'a> {
    mode: Mode,
    stock: &'a Path,
    wrapper: &'a Path,
    hod: &'a Path,
    scratch: &'a Path,
}

/// One compile's statistics line, the fields the sweep reads.
#[derive(Debug, Default)]
struct Stats {
    backend: String,
    reason: Option<String>,
    skip: Option<String>,
    in_process: bool,
}

impl Stats {
    fn read(log: &Path) -> Option<Stats> {
        let text = std::fs::read_to_string(log).ok()?;
        let json = Json::parse(text.lines().last()?).ok()?;
        let s = |k: &str| json.get(k).and_then(Json::as_str).map(str::to_owned);
        Some(Stats {
            backend: s("backend").unwrap_or_default(),
            reason: s("hod_reason"),
            skip: s("hod_skip"),
            in_process: matches!(json.get("in_process"), Some(Json::Bool(true))),
        })
    }

    fn describe(&self) -> String {
        let mut d = self.backend.clone();
        if let Some(r) = self.reason.as_ref().or(self.skip.as_ref()) {
            let _ = write!(d, "({r})");
        }
        d
    }
}

/// What one compile left: its status, statistics line and object.
struct Ran {
    ok: bool,
    stats: Option<Stats>,
    object: Option<Vec<u8>>,
}

/// One job's result.
#[derive(Debug)]
struct Verdict {
    kind: &'static str,
    /// The wrapper's route (`bridge`, `text(<why>)`); for `bridge-eq`,
    /// the equality check's result.
    route: Option<String>,
    reason: Option<String>,
    external: String,
    in_process: String,
    bad: bool,
}

impl Job<'_> {
    fn compile(&self, t: &Tools<'_>, dir: &Path, exe: &Path, name: &str) -> Ran {
        let log = dir.join(format!("{name}.jsonl"));
        let program =
            if self.file.lang == Lang::C { exe.with_file_name("kiln") } else { exe.into() };
        let mut cmd = Command::new(program);
        cmd.current_dir(dir)
            .args(&self.file.args)
            .args(if self.g { &["-g"][..] } else { &[] })
            .args(["-fkiln-backend=hod", "-c"])
            .arg(&self.file.path)
            .args(["-o", "out.o"])
            .env("KILN_STATS_LOG", &log)
            .env("KILN_LLVM_DIR", self.llvm)
            .env("KILN_HOD", t.hod)
            .env_remove("KILN_BACKEND")
            .env_remove("KILN_HOD_STRICT")
            .env_remove("HOD_COVERAGE_LOG")
            .env_remove("KILN_HOD_BRIDGE")
            .env_remove("KILN_HOD_DUAL")
            .env_remove("KILN_HOD_BRIDGE_EQ");
        if exe == t.wrapper {
            cmd.env("KILN_HOD_BRIDGE_LOG", dir.join("bridge.jsonl"));
            match t.mode {
                Mode::Dual => {
                    cmd.env("KILN_HOD_DUAL", "1");
                }
                Mode::BridgeEq => {
                    cmd.env("KILN_HOD_BRIDGE_EQ", "1");
                }
                Mode::Objects => {}
            }
        }
        util::isolate_caches(&mut cmd);
        let ok = process::run(&mut cmd, Duration::from_secs(600)).is_ok_and(|o| o.success());
        let object = std::fs::read(dir.join("out.o")).ok();
        let _ = std::fs::remove_file(dir.join("out.o"));
        Ran { ok, stats: Stats::read(&log), object }
    }

    fn run(&self, t: &Tools<'_>, n: usize) -> Verdict {
        let dir = t.scratch.join(n.to_string());
        let _ = std::fs::create_dir_all(&dir);
        let mut v =
            if t.mode == Mode::BridgeEq { self.equality(t, &dir) } else { self.compare(t, &dir) };
        if v.route.is_none() {
            v.route = route(&dir.join("bridge.jsonl"), "route");
        }
        let _ = std::fs::remove_dir_all(&dir);
        v
    }

    fn compare(&self, t: &Tools<'_>, dir: &Path) -> Verdict {
        let verdict = |kind, reason: Option<&String>, a: &Ran, b: Option<&Ran>, bad| Verdict {
            kind,
            route: None,
            reason: reason.cloned(),
            external: a.stats.as_ref().map_or_else(|| "-".to_owned(), Stats::describe),
            in_process: b
                .and_then(|b| b.stats.as_ref())
                .map_or_else(|| "-".to_owned(), Stats::describe),
            bad,
        };
        let a = self.compile(t, dir, t.stock, "external");
        let Some(sa) = a.stats.as_ref().filter(|_| a.ok) else {
            return verdict("compile failed (stock)", None, &a, None, false);
        };
        if sa.skip.is_some() || !sa.backend.starts_with("hod") {
            return verdict("not offered to hod", None, &a, None, false);
        }
        let b = self.compile(t, dir, t.wrapper, "in-process");
        let Some(sb) = b.stats.as_ref().filter(|_| b.ok) else {
            return verdict("DIFF: the wrapper failed", None, &a, Some(&b), true);
        };
        if !sb.in_process {
            // Rule 4: the in-process route takes no module `opt` would
            // change (its triggers are reported as `hod_skip`).
            let kind = if sb.skip.is_some() && sb.backend == "llvm" {
                "external only (opt triggers)"
            } else {
                "DIFF: not offered in process"
            };
            return verdict(kind, None, &a, Some(&b), kind.starts_with("DIFF"));
        }
        if sa.backend != sb.backend || sa.reason != sb.reason {
            return verdict("DIFF: answers differ", None, &a, Some(&b), true);
        }
        let same = a.object.is_some() && a.object == b.object;
        let kind = match (sa.backend.as_str(), same) {
            ("hod", true) => "taken in process, same bytes",
            ("hod-declined", true) => "declined, same bytes",
            ("hod-failed", true) => "failed, same bytes",
            _ => "DIFF: object bytes",
        };
        let reason = (sa.backend != "hod").then_some(sa.reason.as_ref()).flatten();
        verdict(kind, reason, &a, Some(&b), !same)
    }

    /// `bridge-eq`: the wrapper alone, with the equality check.
    fn equality(&self, t: &Tools<'_>, dir: &Path) -> Verdict {
        let b = self.compile(t, dir, t.wrapper, "in-process");
        let eq = route(&dir.join("bridge.jsonl"), "eq");
        let (kind, bad) = match eq.as_deref() {
            None if !b.ok => ("not in process, compile failed", false),
            None => ("not in process", false),
            Some("equal") => ("equal", false),
            Some(e) if e.starts_with("refused:") => ("refused equally", false),
            Some(e) if e.starts_with("fallback:") || e == "debug" => ("not bridged", false),
            Some(_) => ("DIFF", true),
        };
        Verdict {
            kind,
            route: eq,
            reason: None,
            external: String::new(),
            in_process: b.stats.as_ref().map_or_else(|| "-".to_owned(), Stats::describe),
            bad,
        }
    }
}

/// The value of `key` in the bridge log's lines (one per module), joined
/// by `;` when a job compiled several modules.
fn route(log: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    let values: Vec<String> = text
        .lines()
        .filter_map(|l| Json::parse(l).ok())
        .filter_map(|j| {
            let value = j.get(key).and_then(Json::as_str)?.to_owned();
            if key != "route" {
                return Some(value);
            }
            Some(match j.get("reason").and_then(Json::as_str) {
                Some(why) => format!("{value}({why})"),
                None => value,
            })
        })
        .collect();
    (!values.is_empty()).then(|| values.join(";"))
}

impl Sweep {
    fn summarize_eq(
        &self,
        root: &Path,
        jobs: &[Job<'_>],
        results: &[(usize, Verdict)],
    ) -> Result<(), String> {
        let expected_text = std::fs::read_to_string(root.join(EXPECTED)).unwrap_or_default();
        let expected: Vec<&str> = expected_text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split('\t').next())
            .collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut refused: BTreeMap<String, usize> = BTreeMap::new();
        let mut not_bridged: BTreeMap<String, usize> = BTreeMap::new();
        let mut bad = Vec::new();
        let mut tsv = String::from("file\tllvm\tresult\teq\n");
        for (i, v) in results {
            let job = &jobs[*i];
            let eq = v.route.clone().unwrap_or_default();
            let file = job.file.path.strip_prefix(root).unwrap_or(&job.file.path);
            let listed = expected.iter().any(|e| Path::new(e) == file);
            let kind = if v.bad && listed { "expected difference" } else { v.kind };
            *counts.entry(kind.to_owned()).or_default() += 1;
            for one in eq.split(';') {
                if let Some(r) = one.strip_prefix("refused:") {
                    *refused.entry(r.to_owned()).or_default() += 1;
                } else if let Some(r) = one.strip_prefix("fallback:") {
                    *not_bridged.entry(r.to_owned()).or_default() += 1;
                } else if one == "debug" {
                    *not_bridged.entry("debug".to_owned()).or_default() += 1;
                }
            }
            let target = job.file.args.iter().find(|a| a.starts_with("--target="));
            let line = format!(
                "{} {}{}\t{}\t{kind}\t{eq}",
                file.display(),
                target.map_or("", String::as_str),
                if job.g { " -g" } else { "" },
                job.llvm.display(),
            );
            let _ = writeln!(tsv, "{line}");
            if v.bad && !listed {
                bad.push(line);
            }
        }
        if let Some(r) = &self.report {
            std::fs::write(r, &tsv).map_err(|e| format!("{}: {e}", r.display()))?;
        }
        let compared: usize = ["equal", "refused equally", "DIFF", "expected difference"]
            .iter()
            .map(|k| counts.get(*k).copied().unwrap_or(0))
            .sum();
        eprintln!("xtask kiln-hod bridge-eq: {} jobs, {compared} modules compared", results.len());
        for (k, n) in &counts {
            eprintln!("  {k}: {n}");
        }
        eprintln!("  refused equally, by reason:");
        for (r, n) in &refused {
            eprintln!("    {r}: {n}");
        }
        eprintln!("  not bridged (text path), by reason:");
        for (r, n) in &not_bridged {
            eprintln!("    {r}: {n}");
        }
        for b in &bad {
            eprintln!("DIFF {b}");
        }
        if bad.is_empty() { Ok(()) } else { Err(format!("{} differences", bad.len())) }
    }
}
