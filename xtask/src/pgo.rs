//! The profile-guided build of kiln itself (WP-1308, spec/harness.md "PGO
//! build"): `cargo xtask build [--pgo]`, `cargo xtask pgo-profile`,
//! `cargo xtask pgo-train`, and the `--pgo` switch of `cargo xtask dist`.
//!
//! A PGO build is four steps, and every entry point runs the same code for
//! each of them, so the all-in-one switch and the step-by-step path cannot
//! drift apart:
//!
//! 1. an instrumented build (`-Cprofile-generate`) in its own target
//!    directory, `<target-dir>/pgo/<flavour>/instrumented`;
//! 2. the training: the instrumented `kiln++` compiles [`TRAINING`] once
//!    each, through the bench harness's own command construction
//!    (`bench::train`), writing raw profiles to `<target-dir>/pgo/<flavour>/profiles`;
//! 3. the merge: `llvm-profdata merge` (the one of rustc's LLVM major,
//!    found by [`ToolSearch`]) into one `.profdata` file;
//! 4. the final build with `-Cprofile-use=<absolute path>`.
//!
//! `pgo-profile` is steps 1-3 (the pre-step a packager runs before its own,
//! unmodified `cargo build --release`), `pgo-train` is step 2 alone, and
//! `build --pgo` / `dist --pgo` are steps 1-4.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::util::{self, process::Shown};

/// The training corpus, relative to `bench/`: every bench entry that is not
/// in [`HELD_OUT`], less the groups that are runs of their own
/// (`bench/pch`, `bench/libcxx-slice`, which needs a libc++ test checkout).
/// Every case must compile: one that fails fails the command.
pub const TRAINING: &[&str] = &[
    "frontend/accessor-wide-emit.cpp",
    "frontend/constant-table-emit.cpp",
    "frontend/expansion-giant-obj.cpp",
    "frontend/expansion-outline-obj.cpp",
    "frontend/nested-template-ids-dup.cpp",
    "frontend/nested-template-ids.cpp",
    "frontend/reflection-mangle.cpp",
    "frontend/simd-generate.cpp",
    "frontend/trial-folds.cpp",
    "frontend/virtual-dtor-chain.cpp",
    "frontend/virtual-lattice-emit.cpp",
    "frontend/virtual-lattice.cpp",
    "oven/builtins-memcpy.cpp",
    "oven/builtins-popcount.cpp",
    "oven/builtins-strlen.cpp",
    "oven/churn-list.cpp",
    "oven/churn-mixed.cpp",
    "oven/churn-string.cpp",
    "oven/churn-tree.cpp",
    "oven/churn-vector.cpp",
    "oven/class-churn.cpp",
    "oven/fib.cpp",
    "oven/inplace-alloc.cpp",
    "oven/inplace-union.cpp",
    "oven/sieve-nohdr.cpp",
    "oven/sieve.cpp",
    "oven/soa.cpp",
    "oven/switch-dense.cpp",
    "oven/time-trace.cpp",
    "oven/trial-rollback.cpp",
    "oven/vector-alloc.cpp",
    "oven/virtual-dispatch.cpp",
    "oven/wide-struct-read.cpp",
    "oven/wide-struct-store.cpp",
    "pp/headers.cpp",
    "pp/repetition.cpp",
    "pp/xmacro.cpp",
    "reflect/s1.cpp",
    "reflect/s2.cpp",
    "reflect/s3.cpp",
    "reflect/s4.cpp",
    "reflect/s5.cpp",
    "reflect/s9.cpp",
    "runtime/generator-iterate.cpp",
];

/// The held-out set, relative to `bench/`: never trained on, so that a
/// bench run of a PGO binary against a plain one on these measures the gain
/// honestly. The library-heavy frontend cases, all of `overload/`, the
/// oven's sort/string/table cases (with their `-nohdr` twins) and four of
/// the reflection shapes.
pub const HELD_OUT: &[&str] = &[
    "frontend/format-like-g.cpp",
    "frontend/registration-wide-obj.cpp",
    "frontend/templates-wide-g.cpp",
    "frontend/templates.cpp",
    "frontend/templates_deep.cpp",
    "frontend/templates_wide.cpp",
    "oven/sort-nohdr.cpp",
    "oven/sort.cpp",
    "oven/string.cpp",
    "oven/table-nohdr.cpp",
    "oven/table.cpp",
    "overload/arrow-chain.cpp",
    "overload/calls.cpp",
    "overload/ctors.cpp",
    "overload/less-10k.cpp",
    "reflect/s10.cpp",
    "reflect/s6.cpp",
    "reflect/s7.cpp",
    "reflect/s8.cpp",
];

/// The raw-profile file name pattern of the training (`LLVM_PROFILE_FILE`):
/// `%m` merges every process of one binary into one file as they exit.
pub const PROFRAW_PATTERN: &str = "kiln-%m.profraw";

/// The LLVM option that makes rustc warn, per function, when the profile
/// has no data for it: how a build shows that a profile applied.
pub const WARN_MISSING: &str = "-Cllvm-args=-pgo-warn-missing-function";

/// What rustc prints for each such function.
const MISSING_MARKER: &str = "no profile data available for function";

const BUILD_USAGE: &str = "\
usage: cargo xtask build [--pgo] [--target <triple>] [--features <list>] [--llvm-dir <dir>]

Without --pgo: exactly `cargo build --release -p kiln_driver` (plus --target and
--features as given); the binary is <target-dir>/release/kiln++.

With --pgo (spec/harness.md \"PGO build\"): `cargo xtask pgo-profile` (an
instrumented build in <target-dir>/pgo/release/instrumented, the training corpus
through the bench harness, `llvm-profdata merge`), then the same release build with
-Cprofile-use=<target-dir>/pgo/release/kiln.profdata, so the binary lands where
the plain build puts it. About three times the build time.

  --pgo              build profile-guided
  --target <triple>  cargo's --target; with --pgo it must be the host (the training
                     runs the instrumented binary here)
  --features <list>  cargo's --features for kiln_driver (both builds)
  --llvm-dir <dir>   the LLVM installation whose bin/llvm-profdata merges the profile
                     (--pgo only; default: KILN_LLVM_DIR, then kiln's LLVM search for
                     rustc's LLVM major, then rustup's llvm-tools component)

The caller's CARGO_ENCODED_RUSTFLAGS, else RUSTFLAGS, else CARGO_BUILD_RUSTFLAGS, go to
both PGO builds (any -Cprofile-use/-Cprofile-generate in them is dropped).
";

const PROFILE_USAGE: &str = "\
usage: cargo xtask pgo-profile --out <file> [--target <triple>] [--features <list>]
                               [--llvm-dir <dir>] [-v]

The PGO pre-step (spec/harness.md \"PGO build\"): an instrumented release build of
kiln_driver in <target-dir>/pgo/release/instrumented, the training corpus compiled
once each by it (as `cargo xtask pgo-train`), and `llvm-profdata merge` into <file>.
Then any `cargo build --release` of the same source, target and features applies it
with RUSTFLAGS=\"<the same flags> -Cprofile-use=<absolute path of file>\".

Run it with the RUSTFLAGS (or CARGO_ENCODED_RUSTFLAGS / CARGO_BUILD_RUSTFLAGS) the
final build will have: they go to the instrumented build too, less any
-Cprofile-use/-Cprofile-generate. A different -C debuginfo, --remap-path-prefix,
--target or feature set between the two builds makes some or all functions miss
their profile data.

  --out <file>       where the merged profile goes
  --target <triple>  cargo's --target, as the final build will pass it (must be
                     the host)
  --features <list>  cargo's --features for kiln_driver, as the final build
  --llvm-dir <dir>   the LLVM installation whose bin/llvm-profdata merges
  -v, --verbose      print the training commands
";

const TRAIN_USAGE: &str = "\
usage: cargo xtask pgo-train --compiler <kiln++> --profile-dir <dir> [-v]
       cargo xtask pgo-train --list

The PGO training step alone (spec/harness.md \"PGO build\"): compiles each entry of
the training corpus once with <kiln++>, an instrumented build (-Cprofile-generate),
through the bench harness's command construction, with
LLVM_PROFILE_FILE=<dir>/kiln-%m.profraw. Fails when any case fails or when no raw
profile was written. Raw profiles already in <dir> are kept (and merged). Then prints
the `llvm-profdata merge` command that makes the profile.

  --compiler <path>  the instrumented kiln++
  --profile-dir <d>  where the raw profiles go
  --list             print the training corpus and the held-out set, one path per line
  -v, --verbose      print the commands
";

// ---------------------------------------------------------------------------
// Command lines.

/// Options shared by `build`, `pgo-profile` and `dist --pgo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PgoOpts {
    pub pgo: bool,
    pub target: Option<String>,
    pub features: Option<String>,
    pub llvm_dir: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub verbose: bool,
}

/// Which command a [`PgoOpts`] is parsed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cmd {
    Build,
    Profile,
}

fn split_flag(a: &str) -> (String, Option<String>) {
    match a.split_once('=') {
        Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
        _ => (a.to_owned(), None),
    }
}

fn parse_opts(cmd: Cmd, args: impl Iterator<Item = String>) -> Result<Option<PgoOpts>, String> {
    let mut o = PgoOpts { pgo: cmd == Cmd::Profile, ..PgoOpts::default() };
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let (flag, inline) = split_flag(&a);
        let mut value = |name: &str| {
            inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--pgo" if inline.is_none() && cmd == Cmd::Build => o.pgo = true,
            "--target" => o.target = Some(value("--target")?),
            "--features" => o.features = Some(value("--features")?),
            "--llvm-dir" => o.llvm_dir = Some(PathBuf::from(value("--llvm-dir")?)),
            "--out" if cmd == Cmd::Profile => o.out = Some(PathBuf::from(value("--out")?)),
            "-v" | "--verbose" if inline.is_none() && cmd == Cmd::Profile => o.verbose = true,
            _ => return Err(format!("unknown argument `{a}`")),
        }
    }
    if !o.pgo && o.llvm_dir.is_some() {
        return Err("--llvm-dir needs --pgo".to_owned());
    }
    if cmd == Cmd::Profile && o.out.is_none() {
        return Err("--out <file> is required".to_owned());
    }
    Ok(Some(o))
}

/// `pgo-train`'s options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TrainOpts {
    compiler: Option<PathBuf>,
    profile_dir: Option<PathBuf>,
    list: bool,
    verbose: bool,
}

fn parse_train(args: impl Iterator<Item = String>) -> Result<Option<TrainOpts>, String> {
    let mut o = TrainOpts::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let (flag, inline) = split_flag(&a);
        let mut value = |name: &str| {
            inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--profile-dir" => o.profile_dir = Some(PathBuf::from(value("--profile-dir")?)),
            "--list" if inline.is_none() => o.list = true,
            "-v" | "--verbose" if inline.is_none() => o.verbose = true,
            _ => return Err(format!("unknown argument `{a}`")),
        }
    }
    if !o.list && (o.compiler.is_none() || o.profile_dir.is_none()) {
        return Err("--compiler <kiln++> and --profile-dir <dir> are required".to_owned());
    }
    Ok(Some(o))
}

fn report(who: &str, usage: &str, r: Result<Option<Result<(), String>>, String>) -> ExitCode {
    match r {
        Err(e) => {
            eprintln!("xtask {who}: {e}\n\n{usage}");
            ExitCode::from(2)
        }
        Ok(None) => {
            print!("{usage}");
            ExitCode::SUCCESS
        }
        Ok(Some(Ok(()))) => ExitCode::SUCCESS,
        Ok(Some(Err(e))) => {
            eprintln!("xtask {who}: error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask build`.
pub fn build_main(args: impl Iterator<Item = String>) -> ExitCode {
    let r = parse_opts(Cmd::Build, args).map(|o| o.map(|o| run_build(&o)));
    report("build", BUILD_USAGE, r)
}

/// `cargo xtask pgo-profile`.
pub fn profile_main(args: impl Iterator<Item = String>) -> ExitCode {
    let r = parse_opts(Cmd::Profile, args).map(|o| o.map(|o| run_profile(&o)));
    report("pgo-profile", PROFILE_USAGE, r)
}

/// `cargo xtask pgo-train`.
pub fn train_main(args: impl Iterator<Item = String>) -> ExitCode {
    let r = parse_train(args).map(|o| o.map(|o| run_train(&o)));
    report("pgo-train", TRAIN_USAGE, r)
}

// ---------------------------------------------------------------------------
// The builds.

/// One `cargo build` of `kiln_driver`: what stays the same between the
/// instrumented and the final build of a PGO build.
#[derive(Clone, Debug)]
pub struct Plan {
    /// `release` or `dist`: the cargo profile, also the profile directory
    /// under the target directory and the `pgo/<flavour>` directory name.
    pub profile: &'static str,
    /// The cargo arguments after `build` (`--profile`, `-p`, features,
    /// `--target`).
    pub cargo_args: Vec<String>,
    /// The rustflags both builds get (the caller's, plus dist's remaps).
    pub flags: Vec<String>,
    /// `--target`, if given.
    pub target: Option<String>,
}

impl Plan {
    /// `cargo build --release -p kiln_driver [--features f] [--target t]`
    /// with the caller's rustflags.
    pub fn release(target: Option<&str>, features: Option<&str>) -> Plan {
        let mut cargo_args: Vec<String> =
            ["--release", "-p", "kiln_driver"].iter().map(|s| (*s).to_owned()).collect();
        if let Some(f) = features {
            cargo_args.extend(["--features".to_owned(), f.to_owned()]);
        }
        if let Some(t) = target {
            cargo_args.extend(["--target".to_owned(), t.to_owned()]);
        }
        Plan {
            profile: "release",
            cargo_args,
            flags: caller_rustflags(&EnvFlags::from_env()),
            target: target.map(str::to_owned),
        }
    }

    /// The directory the binary lands in under `target_dir`.
    pub fn exe_dir(&self, target_dir: &Path) -> PathBuf {
        let mut d = target_dir.to_path_buf();
        if let Some(t) = &self.target {
            d.push(t);
        }
        d.join(self.profile)
    }

    /// Runs the build in `target_dir` with `extra` rustflags; returns the
    /// `kiln++` path. `tee` captures cargo's stderr, showing all of it but
    /// rustc's per-function "no profile data" warnings.
    pub fn run(
        &self,
        root: &Path,
        target_dir: &Path,
        extra: &[String],
        what: &str,
        tee: bool,
    ) -> Result<(PathBuf, String), String> {
        let mut flags = self.flags.clone();
        flags.extend(extra.iter().cloned());
        let mut cmd = Command::new(env!("CARGO"));
        cmd.arg("build")
            .args(&self.cargo_args)
            .env("CARGO_TARGET_DIR", target_dir)
            .env_remove("RUSTFLAGS")
            .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
            .current_dir(root);
        eprintln!("xtask: {what}: {} (rustflags: {})", Shown(&cmd), flags.join(" "));
        let hide = tee.then_some(MISSING_MARKER);
        let (status, captured) = util::process::run_step_hiding(&mut cmd, tee, hide)
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        if !status.success() {
            return Err(format!("{what} failed ({status})"));
        }
        Ok((util::kilnpp_beside(&self.exe_dir(target_dir))?, captured))
    }
}

/// The rustflags variables cargo reads, in its order of precedence.
#[derive(Clone, Debug, Default)]
pub struct EnvFlags {
    pub encoded: Option<String>,
    pub rustflags: Option<String>,
    pub build_rustflags: Option<String>,
}

impl EnvFlags {
    pub fn from_env() -> EnvFlags {
        EnvFlags {
            encoded: std::env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
            rustflags: std::env::var("RUSTFLAGS").ok(),
            build_rustflags: std::env::var("CARGO_BUILD_RUSTFLAGS").ok(),
        }
    }
}

/// The caller's rustflags as cargo would take them (`CARGO_ENCODED_RUSTFLAGS`,
/// else `RUSTFLAGS`, else `CARGO_BUILD_RUSTFLAGS`), less any
/// `-Cprofile-use` / `-Cprofile-generate` (also as `-C <opt>`): the
/// packager's environment may already carry the final build's
/// `-Cprofile-use`.
pub fn caller_rustflags(env: &EnvFlags) -> Vec<String> {
    let flags: Vec<String> = if let Some(e) = &env.encoded {
        if e.is_empty() { Vec::new() } else { e.split('\x1f').map(str::to_owned).collect() }
    } else {
        env.rustflags
            .as_ref()
            .or(env.build_rustflags.as_ref())
            .map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    strip_profile_flags(flags)
}

/// `flags` without `-Cprofile-use=`/`-Cprofile-generate[=]` in either
/// spelling.
pub fn strip_profile_flags(flags: Vec<String>) -> Vec<String> {
    let is_profile = |s: &str| {
        let s = s.strip_prefix("-C").unwrap_or(s);
        s.starts_with("profile-use") || s.starts_with("profile-generate")
    };
    let mut out = Vec::with_capacity(flags.len());
    let mut it = flags.into_iter().peekable();
    while let Some(f) = it.next() {
        if f == "-C" && it.peek().is_some_and(|n| is_profile(n)) {
            it.next();
        } else if !(f.starts_with("-C") && f.len() > 2 && is_profile(&f)) {
            out.push(f);
        }
    }
    out
}

/// Where a PGO build of `plan` keeps its state under the target directory:
/// `<target-dir>/pgo/<flavour>/`.
pub struct PgoDirs {
    /// The instrumented build's own target directory.
    pub instrumented: PathBuf,
    /// The raw profiles.
    pub profiles: PathBuf,
    /// The merged profile of the all-in-one switch.
    pub profdata: PathBuf,
}

impl PgoDirs {
    pub fn new(target_dir: &Path, plan: &Plan) -> PgoDirs {
        let base = target_dir.join("pgo").join(plan.profile);
        PgoDirs {
            instrumented: base.join("instrumented"),
            profiles: base.join("profiles"),
            profdata: base.join("kiln.profdata"),
        }
    }
}

/// Fails unless `target` (if given) is the host: the training runs the
/// instrumented binary on this machine.
fn check_host_target(target: Option<&str>) -> Result<(), String> {
    let Some(t) = target else { return Ok(()) };
    let host = rustc_vv(&util::repo_root())?.host;
    if t == host {
        Ok(())
    } else {
        Err(format!(
            "--pgo trains the instrumented kiln on this machine: --target must be the host \
             ({host}), not {t}"
        ))
    }
}

/// Steps 1-3: the instrumented build, the training and the merge into
/// `out`. The tool is found (and its version checked) before anything is
/// built.
pub fn make_profile(
    root: &Path,
    target_dir: &Path,
    plan: &Plan,
    llvm_dir: Option<&Path>,
    out: &Path,
    verbose: bool,
) -> Result<(), String> {
    check_host_target(plan.target.as_deref())?;
    let tool = find_profdata(root, llvm_dir)?;
    eprintln!(
        "xtask: PGO: llvm-profdata {} ({}, from {}); rustc's LLVM is {}",
        tool.version,
        tool.path.display(),
        tool.origin,
        tool.rustc_llvm
    );
    let dirs = PgoDirs::new(target_dir, plan);
    let generate = format!("-Cprofile-generate={}", dirs.profiles.display());
    let (instrumented, _) =
        plan.run(root, &dirs.instrumented, &[generate], "PGO step 1/3, instrumented build", false)?;
    // The instrumented build scripts and proc macros wrote profiles of
    // their own while the build ran: start the training from nothing.
    if dirs.profiles.exists() {
        std::fs::remove_dir_all(&dirs.profiles)
            .map_err(|e| format!("cannot clear {}: {e}", dirs.profiles.display()))?;
    }
    eprintln!("xtask: PGO step 2/3, training {} cases", TRAINING.len());
    train(root, target_dir, &instrumented, &dirs.profiles, verbose)?;
    eprintln!("xtask: PGO step 3/3, merging");
    merge(&tool, &dirs.profiles, out)?;
    Ok(())
}

/// Step 4: the final build of `plan` with `profdata`; prints how many
/// functions the profile did not cover (rustc's warning per function,
/// [`WARN_MISSING`]).
pub fn final_build(
    root: &Path,
    target_dir: &Path,
    plan: &Plan,
    profdata: &Path,
) -> Result<PathBuf, String> {
    let use_flag = format!("-Cprofile-use={}", profdata.display());
    let (exe, captured) =
        plan.run(root, target_dir, &[use_flag, WARN_MISSING.to_owned()], "PGO final build", true)?;
    let (kiln, scripts) = count_missing(&captured);
    eprintln!(
        "xtask: PGO: {kiln} function(s) of kiln's crates had no profile data, and {scripts} \
         of build scripts (which the training never runs); a profile that does not match \
         the build leaves nearly every function without"
    );
    Ok(exe)
}

/// The number of functions rustc found no profile data for: in the
/// crates' own code units, and in build scripts' (`build_script_build.*`).
pub fn count_missing(build_output: &str) -> (usize, usize) {
    let lines = build_output.lines().filter(|l| l.contains(MISSING_MARKER));
    lines.fold((0, 0), |(k, b), l| {
        if l.starts_with("warning: build_script_build.") { (k, b + 1) } else { (k + 1, b) }
    })
}

fn absolute(p: &Path) -> Result<PathBuf, String> {
    std::path::absolute(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn run_build(o: &PgoOpts) -> Result<(), String> {
    let root = util::repo_root();
    if !o.pgo {
        // Exactly `cargo build --release -p kiln_driver`: no environment
        // change, so the output is what that command produces.
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args(["build", "--release", "-p", "kiln_driver"]).current_dir(&root);
        if let Some(f) = &o.features {
            cmd.args(["--features", f]);
        }
        if let Some(t) = &o.target {
            cmd.args(["--target", t]);
        }
        let (status, _) = util::process::run_step(&mut cmd, false)
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        return if status.success() {
            Ok(())
        } else {
            Err(format!("cargo build --release failed ({status})"))
        };
    }
    let target_dir = absolute(&util::target_dir(&root)?)?;
    let plan = Plan::release(o.target.as_deref(), o.features.as_deref());
    let dirs = PgoDirs::new(&target_dir, &plan);
    make_profile(&root, &target_dir, &plan, o.llvm_dir.as_deref(), &dirs.profdata, o.verbose)?;
    let exe = final_build(&root, &target_dir, &plan, &dirs.profdata)?;
    eprintln!("xtask build: PGO kiln++ at {}", exe.display());
    Ok(())
}

fn run_profile(o: &PgoOpts) -> Result<(), String> {
    let root = util::repo_root();
    let target_dir = absolute(&util::target_dir(&root)?)?;
    let out = absolute(o.out.as_deref().unwrap_or(Path::new("kiln.profdata")))?;
    let plan = Plan::release(o.target.as_deref(), o.features.as_deref());
    make_profile(&root, &target_dir, &plan, o.llvm_dir.as_deref(), &out, o.verbose)?;
    let mut flags = plan.flags.clone();
    flags.push(format!("-Cprofile-use={}", out.display()));
    eprintln!(
        "xtask pgo-profile: wrote {}\nthe final build: RUSTFLAGS='{}' cargo build {}",
        out.display(),
        flags.join(" "),
        plan.cargo_args.join(" ")
    );
    Ok(())
}

fn run_train(o: &TrainOpts) -> Result<(), String> {
    if o.list {
        for p in TRAINING {
            println!("train    bench/{p}");
        }
        for p in HELD_OUT {
            println!("held-out bench/{p}");
        }
        return Ok(());
    }
    let root = util::repo_root();
    let target_dir = absolute(&util::target_dir(&root)?)?;
    let (Some(compiler), Some(dir)) = (&o.compiler, &o.profile_dir) else {
        return Err("--compiler and --profile-dir are required".to_owned());
    };
    let dir = absolute(dir)?;
    train(&root, &target_dir, &absolute(compiler)?, &dir, o.verbose)?;
    match find_profdata(&root, None) {
        Ok(tool) => eprintln!(
            "xtask pgo-train: merge with: {} merge -o <file>.profdata {}/*.profraw",
            tool.path.display(),
            dir.display()
        ),
        Err(e) => eprintln!("xtask pgo-train: no llvm-profdata to merge with: {e}"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Step 2: training.

/// The training corpus as bench paths under `root`, and the cases left
/// out: the reflection shapes that need GCC 16.1's libstdc++
/// (`gen_reflect::Shape::libstdcxx`) when `has_gcc16` is false. That is the
/// one part of the corpus that depends on the machine; every other case
/// needs only the checkout, the LLVM installation and a libstdc++.
pub fn training_paths(root: &Path, has_gcc16: bool) -> (Vec<PathBuf>, Vec<&'static str>) {
    let (mut keep, mut left_out) = (Vec::new(), Vec::new());
    for p in TRAINING {
        let needs_gcc16 = p
            .strip_prefix("reflect/")
            .and_then(|f| crate::gen_reflect::shape(f.trim_end_matches(".cpp")))
            .is_some_and(|s| s.libstdcxx);
        if needs_gcc16 && !has_gcc16 {
            left_out.push(*p);
        } else {
            keep.push(root.join("bench").join(p));
        }
    }
    (keep, left_out)
}

/// Whether the GCC oracle (`KILN_GCC`, else `g++-16`) gives the reflection
/// shapes a `--gcc-toolchain` (`gen_reflect::kiln_flags`).
fn has_gcc16() -> bool {
    crate::gen_reflect::shape("s1").is_some_and(|s| !crate::gen_reflect::kiln_flags(s).is_empty())
}

/// Step 2: `compiler` (instrumented) compiles [`TRAINING`] once each,
/// writing raw profiles into `dir`. Every case must compile.
pub fn train(
    root: &Path,
    target_dir: &Path,
    compiler: &Path,
    dir: &Path,
    verbose: bool,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut env: Vec<(OsString, OsString)> =
        vec![("LLVM_PROFILE_FILE".into(), dir.join(PROFRAW_PATTERN).into_os_string())];
    // A `dist` build has no source-tree resource fallback and its
    // instrumented binary is outside any install tree: name the checkout's.
    if std::env::var_os("KILN_RESOURCE_DIR").is_none_or(|v| v.is_empty()) {
        env.push(("KILN_RESOURCE_DIR".into(), root.join("resource").into_os_string()));
    }
    let (paths, left_out) = training_paths(root, has_gcc16());
    if !left_out.is_empty() {
        eprintln!(
            "xtask: PGO: warning: no GCC 16.1 (KILN_GCC, else g++-16 on PATH): training without \
             the reflection shapes that need its libstdc++: {}",
            left_out.join(", ")
        );
    }
    let start = std::time::Instant::now();
    let results = crate::bench::train(root, target_dir, compiler, &paths, &env, verbose)?;
    let failed: Vec<String> = results
        .iter()
        .filter_map(|t| t.result.as_ref().err().map(|why| format!("  {}: {why}", t.id)))
        .collect();
    if !failed.is_empty() {
        return Err(format!(
            "{} of {} training cases failed (a case that fails trains nothing):\n{}",
            failed.len(),
            results.len(),
            failed.join("\n")
        ));
    }
    if profraws(dir)?.is_empty() {
        return Err(format!(
            "{} wrote no raw profile into {}: is it an instrumented (-Cprofile-generate) build?",
            compiler.display(),
            dir.display()
        ));
    }
    eprintln!(
        "xtask: PGO: trained {} cases in {:.0}s into {}",
        results.len(),
        start.elapsed().as_secs_f64(),
        dir.display()
    );
    Ok(())
}

/// The `.profraw` files in `dir`, sorted.
pub fn profraws(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "profraw"))
        .collect();
    v.sort();
    Ok(v)
}

// ---------------------------------------------------------------------------
// Step 3: the merge, and finding the tool.

/// `llvm-profdata merge -o <out> <dir>/*.profraw`, printed before it runs.
pub fn merge(tool: &Profdata, dir: &Path, out: &Path) -> Result<(), String> {
    let raws = profraws(dir)?;
    if raws.is_empty() {
        return Err(format!("no .profraw files in {}", dir.display()));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let mut cmd = Command::new(&tool.path);
    cmd.arg("merge").arg("-o").arg(out).args(&raws);
    eprintln!("xtask: PGO: {}", Shown(&cmd));
    let o = cmd.output().map_err(|e| format!("cannot run {}: {e}", tool.path.display()))?;
    if !o.status.success() {
        return Err(format!(
            "llvm-profdata merge failed ({}): {}",
            o.status,
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(())
}

/// A usable `llvm-profdata`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profdata {
    pub path: PathBuf,
    /// Its LLVM version (`22.1.8`).
    pub version: String,
    /// How it was found.
    pub origin: String,
    /// rustc's LLVM version, which it matches in major.
    pub rustc_llvm: String,
}

/// What `rustc -vV` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustcVv {
    pub host: String,
    /// `22.1.2`.
    pub llvm: String,
}

/// Parses `rustc -vV`: `host:` and `LLVM version:`.
pub fn parse_rustc_vv(out: &str) -> Option<RustcVv> {
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_owned());
    Some(RustcVv { host: field("host:")?, llvm: field("LLVM version:")? })
}

/// `rustc -vV` of the toolchain the checkout pins (run in `root`).
fn rustc_vv(root: &Path) -> Result<RustcVv, String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let o = Command::new(&rustc)
        .arg("-vV")
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run rustc: {e}"))?;
    parse_rustc_vv(&String::from_utf8_lossy(&o.stdout))
        .ok_or_else(|| "`rustc -vV` printed no host or LLVM version".to_owned())
}

/// The leading integer of a version (`22.1.2-rust-1.95.0-stable` -> 22).
pub fn major_of(version: &str) -> Option<u32> {
    let d: String = version.chars().take_while(char::is_ascii_digit).collect();
    d.parse().ok()
}

/// The version `llvm-profdata --version` prints: the token after
/// `LLVM version ` (`Ubuntu LLVM version 22.1.8`, rustup's
/// `LLVM version 22.1.2-rust-1.95.0-stable`).
pub fn parse_tool_version(out: &str) -> Option<String> {
    let rest = &out[out.find("LLVM version ")? + "LLVM version ".len()..];
    let v = rest.split_whitespace().next()?;
    major_of(v).map(|_| v.to_owned())
}

/// The inputs of the `llvm-profdata` search, separated from the process
/// environment so it can be tested on fake trees.
#[derive(Clone, Debug, Default)]
pub struct ToolSearch {
    /// `--llvm-dir`.
    pub flag: Option<PathBuf>,
    /// `$KILN_LLVM_DIR`, if set and not empty.
    pub env: Option<PathBuf>,
    /// `$PATH`.
    pub path: Option<OsString>,
    /// Prepended to the fixed layout paths (empty, or a scratch tree).
    pub root: String,
    /// rustup's `llvm-tools` directory, `<sysroot>/lib/rustlib/<host>/bin`.
    pub rustup_bin: Option<PathBuf>,
}

/// Homebrew on Linux's prefix (as the driver's search).
const LINUXBREW: &str = "/home/linuxbrew/.linuxbrew/opt";

impl ToolSearch {
    /// The candidates in order, each with its origin; explicit ones first
    /// (`true`: an invalid one is an error, not skipped). Mirrors kiln's own
    /// LLVM discovery (`crates/kiln_driver/src/llvm.rs`, spec/driver.md
    /// "LLVM discovery") for the one major `want`, then adds an
    /// `llvm-profdata-<N>` / `llvm-profdata` on `PATH` and rustup's
    /// `llvm-tools` component.
    pub fn candidates(&self, want: u32) -> Vec<(PathBuf, String, bool)> {
        let tool = |prefix: &Path| prefix.join("bin").join("llvm-profdata");
        if let Some(d) = &self.flag {
            return vec![(tool(d), "--llvm-dir".to_owned(), true)];
        }
        if let Some(d) = &self.env {
            return vec![(tool(d), "KILN_LLVM_DIR".to_owned(), true)];
        }
        let on_path = |name: &str| {
            self.path
                .as_ref()
                .and_then(|p| std::env::split_paths(p).map(|d| d.join(name)).find(|p| p.is_file()))
        };
        let prefix_of = |t: &Path| {
            std::fs::canonicalize(t).ok().and_then(|r| Some(r.parent()?.parent()?.to_path_buf()))
        };
        let mut v = Vec::new();
        for name in ["llvm-config".to_owned(), format!("llvm-config-{want}")] {
            if let Some(t) = on_path(&name)
                && let Some(p) = prefix_of(&t)
            {
                v.push((tool(&p), t.display().to_string(), false));
            }
        }
        for layout in [
            format!("/usr/lib/llvm-{want}"),
            format!("/usr/lib64/llvm{want}"),
            format!("{LINUXBREW}/llvm@{want}"),
            format!("{LINUXBREW}/llvm"),
            "/usr".to_owned(),
        ] {
            v.push((tool(Path::new(&format!("{}{layout}", self.root))), layout, false));
        }
        for name in [format!("opt-{want}"), format!("clang++-{want}")] {
            if let Some(t) = on_path(&name)
                && let Some(p) = prefix_of(&t)
            {
                v.push((tool(&p), t.display().to_string(), false));
            }
        }
        for name in [format!("llvm-profdata-{want}"), "llvm-profdata".to_owned()] {
            if let Some(t) = on_path(&name) {
                v.push((t.clone(), "PATH".to_owned(), false));
            }
        }
        if let Some(b) = &self.rustup_bin {
            v.push((b.join("llvm-profdata"), "rustup's llvm-tools".to_owned(), false));
        }
        v
    }

    /// The first candidate whose `llvm-profdata` exists and has major
    /// `want`, `version_of` telling a tool's version (`--version`). An
    /// explicit candidate that fails is an error; implicit ones are skipped,
    /// and when none is usable the error lists each with why.
    pub fn find(
        &self,
        rustc_llvm: &str,
        version_of: &dyn Fn(&Path) -> Option<String>,
    ) -> Result<Profdata, String> {
        let want = major_of(rustc_llvm)
            .ok_or_else(|| format!("cannot tell the major of rustc's LLVM `{rustc_llvm}`"))?;
        let mut tried: Vec<String> = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        for (path, origin, explicit) in self.candidates(want) {
            let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let why = if path.is_file() {
                match version_of(&path) {
                    None => "cannot tell its version (`--version`)".to_owned(),
                    Some(v) if major_of(&v) == Some(want) => {
                        return Ok(Profdata {
                            path,
                            version: v,
                            origin,
                            rustc_llvm: rustc_llvm.to_owned(),
                        });
                    }
                    Some(v) => format!("LLVM {v}"),
                }
            } else {
                "not found".to_owned()
            };
            if explicit {
                return Err(format!(
                    "{} (from {origin}): {why}; rustc's LLVM is {rustc_llvm}, and a profile is \
                     only read by the LLVM major that wrote its format: name an LLVM {want} \
                     installation with --llvm-dir=<dir>, or install rustup's llvm-tools \
                     component (`rustup component add llvm-tools`) and unset KILN_LLVM_DIR",
                    path.display()
                ));
            }
            tried.push(format!("  {} ({origin}): {why}", path.display()));
        }
        Err(format!(
            "no llvm-profdata of LLVM {want} (rustc's LLVM is {rustc_llvm}); install LLVM {want} \
             (it is one of the LLVMs kiln drives), name it with --llvm-dir=<dir>, or run \
             `rustup component add llvm-tools`. Tried:\n{}",
            tried.join("\n")
        ))
    }
}

/// `<tool> --version`'s LLVM version.
fn tool_version(tool: &Path) -> Option<String> {
    let o = Command::new(tool).arg("--version").output().ok()?;
    o.status.success().then_some(())?;
    parse_tool_version(&String::from_utf8_lossy(&o.stdout))
}

/// The `llvm-profdata` for this toolchain: [`ToolSearch`] over the process
/// environment, `llvm_dir` as `--llvm-dir`.
pub fn find_profdata(root: &Path, llvm_dir: Option<&Path>) -> Result<Profdata, String> {
    let vv = rustc_vv(root)?;
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    let search = ToolSearch {
        flag: llvm_dir.map(Path::to_path_buf),
        env: std::env::var_os("KILN_LLVM_DIR").filter(|v| !v.is_empty()).map(PathBuf::from),
        path: std::env::var_os("PATH"),
        root: String::new(),
        rustup_bin: sysroot.map(|s| s.join("lib").join("rustlib").join(&vv.host).join("bin")),
    };
    search.find(&vv.llvm, &tool_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> impl Iterator<Item = String> {
        v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn build_and_profile_options() {
        let o = parse_opts(Cmd::Build, args(&[])).unwrap().unwrap();
        assert_eq!(o, PgoOpts::default());
        let o = parse_opts(Cmd::Build, args(&["--pgo", "--llvm-dir=/l", "--target", "t"]))
            .unwrap()
            .unwrap();
        assert!(o.pgo);
        assert_eq!(o.llvm_dir.as_deref(), Some(Path::new("/l")));
        assert_eq!(o.target.as_deref(), Some("t"));
        assert!(parse_opts(Cmd::Build, args(&["--llvm-dir=/l"])).is_err(), "needs --pgo");
        assert!(parse_opts(Cmd::Build, args(&["--pgo=1"])).is_err());
        assert!(parse_opts(Cmd::Build, args(&["--out", "x"])).is_err(), "pgo-profile only");
        assert!(parse_opts(Cmd::Build, args(&["--help"])).unwrap().is_none());
        let o = parse_opts(Cmd::Profile, args(&["--out", "p.profdata", "--features", "dist"]))
            .unwrap()
            .unwrap();
        assert!(o.pgo);
        assert_eq!(o.out.as_deref(), Some(Path::new("p.profdata")));
        assert_eq!(o.features.as_deref(), Some("dist"));
        assert!(parse_opts(Cmd::Profile, args(&[])).is_err(), "--out is required");
        assert!(parse_opts(Cmd::Profile, args(&["--pgo", "--out=x"])).is_err());
    }

    #[test]
    fn train_options() {
        let o = parse_train(args(&["--compiler", "k", "--profile-dir=d", "-v"])).unwrap().unwrap();
        assert_eq!(o.compiler.as_deref(), Some(Path::new("k")));
        assert_eq!(o.profile_dir.as_deref(), Some(Path::new("d")));
        assert!(o.verbose && !o.list);
        assert!(parse_train(args(&["--list"])).unwrap().unwrap().list);
        assert!(parse_train(args(&["--compiler", "k"])).is_err());
        assert!(parse_train(args(&["--profile-dir"])).is_err());
        assert!(parse_train(args(&["--bogus"])).is_err());
    }

    #[test]
    fn rustflags_from_the_environment() {
        let s = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let env = EnvFlags {
            encoded: Some("-Cdebuginfo=2\x1f-Cprofile-use=/p.profdata".to_owned()),
            rustflags: Some("-Cignored".to_owned()),
            build_rustflags: None,
        };
        assert_eq!(caller_rustflags(&env), s(&["-Cdebuginfo=2"]));
        // Set but empty: no flags, as cargo has it.
        let env = EnvFlags { encoded: Some(String::new()), ..env };
        assert!(caller_rustflags(&env).is_empty());
        let env = EnvFlags {
            encoded: None,
            rustflags: Some(" -C profile-use=/p  --remap-path-prefix=/a=/b ".to_owned()),
            build_rustflags: Some("-Cignored".to_owned()),
        };
        assert_eq!(caller_rustflags(&env), s(&["--remap-path-prefix=/a=/b"]));
        let env = EnvFlags { rustflags: None, ..env };
        assert_eq!(caller_rustflags(&env), s(&["-Cignored"]));
        assert_eq!(
            strip_profile_flags(s(&["-Cprofile-generate", "-C", "opt-level=3", "-C"])),
            s(&["-C", "opt-level=3", "-C"])
        );
    }

    #[test]
    fn versions() {
        let vv = "rustc 1.95.0 (abc 2026-01-01)\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\n\
                  release: 1.95.0\nLLVM version: 22.1.2\n";
        assert_eq!(
            parse_rustc_vv(vv),
            Some(RustcVv { host: "x86_64-unknown-linux-gnu".into(), llvm: "22.1.2".into() })
        );
        assert_eq!(parse_rustc_vv("host: a\n"), None);
        assert_eq!(
            parse_tool_version("Ubuntu LLVM version 22.1.8\n  Optimized build.\n").as_deref(),
            Some("22.1.8")
        );
        assert_eq!(
            parse_tool_version(
                "LLVM (http://llvm.org/):\n  LLVM version 22.1.2-rust-1.95.0-stable\n"
            )
            .as_deref(),
            Some("22.1.2-rust-1.95.0-stable")
        );
        assert_eq!(parse_tool_version("LLVM version git"), None);
        assert_eq!(major_of("19.1.7git"), Some(19));
        assert_eq!(major_of("x"), None);
    }

    /// A scratch tree of fake installations: `<dir>/bin/llvm-profdata`
    /// files whose "version" the test's `version_of` reads from the file.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Tree {
            let d = std::env::temp_dir().join(format!("kiln-pgo-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Tree(d)
        }
        fn tool(&self, rel: &str, version: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, version).unwrap();
            p
        }
        fn search(&self) -> ToolSearch {
            ToolSearch { root: self.0.display().to_string(), ..ToolSearch::default() }
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn read_version(p: &Path) -> Option<String> {
        std::fs::read_to_string(p).ok().filter(|s| !s.is_empty())
    }

    #[test]
    fn tool_search_order_and_version_check() {
        let t = Tree::new("order");
        // Nothing installed: an error naming what was tried.
        let e = t.search().find("22.1.2", &read_version).unwrap_err();
        assert!(e.contains("no llvm-profdata of LLVM 22") && e.contains("/usr/lib/llvm-22"), "{e}");
        // The wrong major is skipped; the right one in a later layout wins.
        t.tool("usr/lib/llvm-22/bin/llvm-profdata", "19.1.7");
        t.tool("usr/bin/llvm-profdata", "22.1.8");
        let found = t.search().find("22.1.2", &read_version).unwrap();
        assert_eq!(found.path, t.0.join("usr/bin/llvm-profdata"));
        assert_eq!((found.version.as_str(), found.origin.as_str()), ("22.1.8", "/usr"));
        // The first layout of the right major comes first.
        t.tool("usr/lib/llvm-22/bin/llvm-profdata", "22.1.8");
        let found = t.search().find("22.1.2", &read_version).unwrap();
        assert_eq!(found.path, t.0.join("usr/lib/llvm-22/bin/llvm-profdata"));
        // rustc on LLVM 19 looks for 19 only.
        let e = t.search().find("19.1.5", &read_version).unwrap_err();
        assert!(e.contains("LLVM 22.1.8") && e.contains("/usr/lib/llvm-19"), "{e}");
        // rustup's llvm-tools is the last resort.
        let rustup = t.tool("sysroot/lib/rustlib/h/bin/llvm-profdata", "19.1.5-rust-1.80.0");
        let s = ToolSearch { rustup_bin: rustup.parent().map(Path::to_path_buf), ..t.search() };
        let found = s.find("19.1.5", &read_version).unwrap();
        assert_eq!((found.path, found.origin.as_str()), (rustup, "rustup's llvm-tools"));
    }

    #[test]
    fn explicit_choice_does_not_fall_through() {
        let t = Tree::new("explicit");
        t.tool("usr/lib/llvm-22/bin/llvm-profdata", "22.1.8");
        let l19 = t.0.join("llvm19");
        t.tool("llvm19/bin/llvm-profdata", "19.1.7");
        let s = ToolSearch { env: Some(l19.clone()), ..t.search() };
        let e = s.find("22.1.2", &read_version).unwrap_err();
        assert!(e.contains("from KILN_LLVM_DIR") && e.contains("LLVM 19.1.7"), "{e}");
        // --llvm-dir wins over KILN_LLVM_DIR.
        let s = ToolSearch { flag: Some(t.0.join("usr/lib/llvm-22")), ..s };
        assert_eq!(s.find("22.1.2", &read_version).unwrap().origin, "--llvm-dir");
        let s = ToolSearch { flag: Some(t.0.join("missing")), ..s };
        let e = s.find("22.1.2", &read_version).unwrap_err();
        assert!(e.contains("from --llvm-dir") && e.contains("not found"), "{e}");
        // A tool whose version cannot be told is not used.
        let s = ToolSearch { flag: Some(l19), ..t.search() };
        let blank = |_: &Path| None;
        assert!(s.find("22.1.2", &blank).unwrap_err().contains("cannot tell its version"));
    }

    #[test]
    fn tool_search_follows_path_tools() {
        let t = Tree::new("path");
        t.tool("opt/llvm/bin/llvm-profdata", "22.1.0");
        let opt = t.tool("opt/llvm/bin/opt-22", "");
        let s =
            ToolSearch { path: Some(opt.parent().unwrap().as_os_str().to_owned()), ..t.search() };
        let found = s.find("22.1.2", &read_version).unwrap();
        assert_eq!(
            found.path,
            std::fs::canonicalize(t.0.join("opt/llvm")).unwrap().join("bin/llvm-profdata")
        );
        assert_eq!(found.origin, opt.display().to_string());
    }

    /// Every listed file exists, neither list repeats a file, and no file is
    /// both trained on and held out.
    #[test]
    fn corpus_lists() {
        let root = util::repo_root();
        for p in TRAINING.iter().chain(HELD_OUT) {
            assert!(root.join("bench").join(p).is_file(), "bench/{p} does not exist");
            assert!(!p.starts_with("pch/") && !p.starts_with("libcxx-slice/"), "{p}");
        }
        for list in [TRAINING, HELD_OUT] {
            let mut v = list.to_vec();
            v.sort_unstable();
            assert_eq!(v, list, "lists stay sorted");
            v.dedup();
            assert_eq!(v.len(), list.len(), "a list repeats a file");
        }
        for p in TRAINING {
            assert!(!HELD_OUT.contains(p), "bench/{p} is both trained on and held out");
        }
        // The held-out cases the measurement is reported on (spec/harness.md).
        for p in ["frontend/format-like-g.cpp", "frontend/templates.cpp", "oven/sort.cpp"] {
            assert!(HELD_OUT.contains(&p), "{p}");
        }
        assert!(HELD_OUT.iter().filter(|p| p.starts_with("overload/")).count() >= 4);
    }

    #[test]
    fn gcc16_shapes_are_the_only_optional_cases() {
        let root = util::repo_root();
        let (all, none) = training_paths(&root, true);
        assert_eq!((all.len(), none.len()), (TRAINING.len(), 0));
        let (some, left_out) = training_paths(&root, false);
        assert_eq!(some.len() + left_out.len(), TRAINING.len());
        assert!(left_out.contains(&"reflect/s1.cpp") && !left_out.contains(&"reflect/s9.cpp"));
        assert!(left_out.iter().all(|p| p.starts_with("reflect/")));
    }

    #[test]
    fn missing_function_count() {
        let out = "   Compiling a\nwarning: a.cgu.0: no profile data available for function f \
                   Hash = 1 up to 0 count discarded\n\nwarning: `a` generated 1 warning\n";
        assert_eq!(count_missing(out), (1, 0));
        let script = "warning: build_script_build.f1-cgu.1: no profile data available for \
                      function g Hash = 2 up to 0 count discarded\n";
        assert_eq!(count_missing(&format!("{out}{script}{script}")), (1, 2));
        assert_eq!(count_missing(""), (0, 0));
    }

    #[test]
    fn plan_paths() {
        let p = Plan::release(Some("x86_64-unknown-linux-gnu"), Some("dist"));
        assert_eq!(p.exe_dir(Path::new("/t")), Path::new("/t/x86_64-unknown-linux-gnu/release"));
        assert!(p.cargo_args.windows(2).any(|w| w == ["--features", "dist"]));
        let d = PgoDirs::new(Path::new("/t"), &p);
        assert_eq!(d.instrumented, Path::new("/t/pgo/release/instrumented"));
        assert_eq!(d.profdata, Path::new("/t/pgo/release/kiln.profdata"));
    }
}
