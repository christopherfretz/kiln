//! GCC's own testsuites run through kiln, GCC 16.1 and clang
//! (spec/harness.md "GCC testsuites"). The tests are GPL and never
//! vendored: `scripts/fetch-gcc-tests.sh` makes a sparse checkout of a
//! pinned GCC release under `.oss/gcc-tests` (gitignored), and the runners
//! read it there.
//!
//! The pieces are suite-independent so that a later g++.dg / g++.old-deja
//! runner reuses them: [`locate`] (the checkout, fetched or refreshed by each run),
//! [`tcl`] (the list syntax and `string match` globs), [`dg`] (directive
//! extraction and per-option resolution), [`target`] (selectors and
//! effective targets), [`xfile`] (the `.x` files of the c-torture suites),
//! [`Tool`] (a compiler with per-flag probing) and [`outcome`] (the result
//! classes and cluster keys). `cargo xtask gcc-torture` ([`torture`]) is
//! the first runner.

pub mod dg;
pub mod expect;
pub mod gxx;
pub mod outcome;
pub mod target;
pub mod tcl;
pub mod torture;
pub mod xfile;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use crate::util::process::{self, MemLimit};
use dg::Action;
use outcome::{Class, Verdict};

/// The GCC release tag the tests are read at (the GCC oracle's release).
pub const GCC_TAG: &str = "releases/gcc-16.1.0";

/// `<checkout>/gcc/testsuite` for `--tests <checkout>` or, by default,
/// `<root>/.oss/gcc-tests`, which `scripts/fetch-gcc-tests.sh` makes or
/// refreshes on every run (idempotent: an existing checkout at the tag only
/// gets its sparse set updated, so files added to it arrive). An explicit
/// checkout must exist; a refresh failure over an existing default
/// checkout is a warning.
pub fn locate(root: &Path, tests: Option<&Path>) -> Result<PathBuf, String> {
    let suite = |c: &Path| c.join("gcc").join("testsuite");
    if let Some(t) = tests {
        let s = suite(t);
        return if s.is_dir() {
            Ok(s)
        } else {
            Err(format!(
                "no GCC testsuite at {} (scripts/fetch-gcc-tests.sh makes one)",
                s.display()
            ))
        };
    }
    let dir = root.join(".oss").join("gcc-tests");
    let present = suite(&dir).join("gcc.c-torture").is_dir();
    if !present {
        eprintln!("xtask: fetching GCC's tests ({GCC_TAG}) into {}", dir.display());
    }
    let st = Command::new(root.join("scripts").join("fetch-gcc-tests.sh"))
        .arg(GCC_TAG)
        .arg(&dir)
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("cannot run scripts/fetch-gcc-tests.sh: {e}"));
    match st {
        Ok(st) if st.success() => {}
        Ok(st) if !present => return Err(format!("scripts/fetch-gcc-tests.sh failed ({st})")),
        Err(e) if !present => return Err(e),
        Ok(st) => {
            eprintln!("xtask: warning: refreshing {} failed ({st}); using it as is", dir.display());
        }
        Err(e) => eprintln!("xtask: warning: {e}; using {} as is", dir.display()),
    }
    Ok(suite(&dir))
}

/// The three compilers a case runs through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Kiln,
    Gcc,
    Clang,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::Kiln => "kiln",
            Self::Gcc => "gcc",
            Self::Clang => "clang",
        }
    }
}

/// A C compiler driver and the flags it was found to reject.
pub struct Tool {
    pub role: Role,
    pub path: PathBuf,
    pub env: Vec<(String, String)>,
    /// `--version`'s first line.
    pub version: String,
    /// Arguments put before every command line (kiln's
    /// `-fclang-compat=<clang oracle major>`, [`Tool::lead_with`]).
    lead: Vec<String>,
    /// Flag unit -> accepted, filled by [`Tool::accepts`].
    probed: Mutex<BTreeMap<String, bool>>,
    /// Whether the bare probe compiles at all (when it does not, no flag
    /// is dropped: there is nothing to compare against).
    baseline: bool,
    scratch: PathBuf,
    /// The probe TU's file name.
    probe: String,
}

/// The major version in a `clang --version` line (`Debian clang version
/// 19.1.7 (3+b1)` -> 19).
pub fn clang_major(version: &str) -> Option<u32> {
    let rest = version.split("clang version ").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

const PROBE_TU: &str = "int main(void) { return 0; }\n";

impl Tool {
    /// Checks `path` runs (`--version`) and compiles the probe TU.
    pub fn new(
        role: Role,
        path: PathBuf,
        env: Vec<(String, String)>,
        scratch: &Path,
    ) -> Result<Self, String> {
        Self::new_for(role, path, env, scratch, "probe.c")
    }

    /// [`Tool::new`] probing with `probe` (`probe.c`, or `probe.cc` for a
    /// C++ driver), the name the probe TU is written under.
    pub fn new_for(
        role: Role,
        path: PathBuf,
        env: Vec<(String, String)>,
        scratch: &Path,
        probe: &str,
    ) -> Result<Self, String> {
        let mut v = Command::new(&path);
        v.arg("--version").envs(env.iter().map(|(k, x)| (k, x)));
        let o = process::run(&mut v, Duration::from_secs(30))
            .map_err(|e| format!("cannot run {} `{}`: {e}", role.name(), path.display()))?;
        if !o.success() {
            return Err(format!("`{} --version` failed ({})", path.display(), o.describe_end()));
        }
        let version = o.stdout_str().lines().next().unwrap_or_default().trim().to_owned();
        let scratch = scratch.join(format!("probe-{}", role.name()));
        std::fs::create_dir_all(&scratch)
            .and_then(|()| std::fs::write(scratch.join(probe), PROBE_TU))
            .map_err(|e| format!("cannot write {}: {e}", scratch.display()))?;
        let mut t = Self {
            role,
            path,
            env,
            version,
            lead: Vec::new(),
            probed: Mutex::default(),
            baseline: true,
            scratch,
            probe: probe.to_owned(),
        };
        t.baseline = t.probe(&[]);
        Ok(t)
    }

    /// A command running this compiler with its environment.
    pub fn command(&self) -> Command {
        let mut c = Command::new(&self.path);
        c.envs(self.env.iter().map(|(k, x)| (k, x))).args(&self.lead);
        c
    }

    /// Puts `arg` before every later command line and notes it after the
    /// version in the report header.
    pub fn lead_with(&mut self, arg: String) {
        self.version = format!("{} ({arg})", self.version);
        self.lead.push(arg);
    }

    /// [`Tool::lead_with`] noting `note` in place of `arg` (a flag whose
    /// value is a local path).
    pub fn lead_noted(&mut self, arg: String, note: &str) {
        self.version = format!("{} ({note})", self.version);
        self.lead.push(arg);
    }

    /// The major version of a clang tool ([`clang_major`]).
    pub fn clang_major(&self) -> Option<u32> {
        clang_major(&self.version)
    }

    fn probe(&self, unit: &[String]) -> bool {
        let mut c = self.command();
        c.args(unit)
            .arg(&self.probe)
            .arg("-o")
            .arg(format!("probe-{}", std::process::id()))
            .current_dir(&self.scratch);
        let mut cap = MemLimit::from_mib(2048);
        if self.role == Role::Kiln {
            cap = cap.map(|m| m.plus(process::kiln_stack_reservation_mib(unit)));
        }
        let mut limited = process::limited(&c, cap);
        process::run(&mut limited, Duration::from_secs(60)).is_ok_and(|o| o.success())
    }

    /// Whether the compiler accepts a flag unit (`-fno-foo`, or `--param
    /// x=1` as two words), probed once with the probe TU.
    pub fn accepts(&self, unit: &[String]) -> bool {
        if !self.baseline {
            return true;
        }
        let key = unit.join(" ");
        if let Some(&v) = self.probed.lock().expect("no panics under the lock").get(&key) {
            return v;
        }
        let v = self.probe(unit);
        self.probed.lock().expect("no panics under the lock").insert(key, v);
        v
    }

    /// `flags` without the units this compiler rejects, and those units.
    ///
    /// A C++ tool (probe `probe.cc`) probes each unit after the case's
    /// last `-std=`: GCC accepts `-freflection` or `-fcontracts` only at
    /// the standards that have them.
    pub fn filter(&self, flags: &[String]) -> (Vec<String>, Vec<String>) {
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        let std = flags
            .iter()
            .rev()
            .find(|f| f.starts_with("-std="))
            .filter(|_| Path::new(&self.probe).extension().is_some_and(|e| e == "cc"));
        for unit in flag_units(flags) {
            let probe_unit = match std {
                Some(s) if !unit[0].starts_with("-std=") => {
                    let mut u = vec![s.clone()];
                    u.extend(unit.iter().cloned());
                    u
                }
                _ => unit.clone(),
            };
            if self.accepts(&probe_unit) {
                kept.extend(unit);
            } else {
                dropped.push(unit.join(" "));
            }
        }
        (kept, dropped)
    }

    /// Every unit probed so far that was rejected, sorted.
    pub fn rejected(&self) -> Vec<String> {
        let m = self.probed.lock().expect("no panics under the lock");
        m.iter().filter(|(_, ok)| !**ok).map(|(k, _)| k.clone()).collect()
    }
}

/// One compile-and-maybe-run of a test through one compiler: the suite
/// runner fills it from the test's resolved plan.
pub struct Case<'a> {
    /// Compiled with the same flags, one object each, linked together.
    pub sources: &'a [PathBuf],
    pub action: Action,
    /// Every compile flag in order (standard, option set, test flags).
    pub flags: &'a [String],
    /// Appended to the link (`-lm`).
    pub link_flags: &'a [String],
    pub negative: bool,
    /// Per compile or link step.
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
    pub mem: Option<MemLimit>,
    /// `-x <lang>` before each source (g++ compiles c-c++-common's `.c`
    /// files as C++); `None` lets the driver go by the extension.
    pub lang: Option<&'a str>,
    /// The diagnostics the test expects (g++.dg): when set, the compile
    /// steps' diagnostics are judged by line and severity
    /// ([`expect::judge`]) instead of by `negative` alone.
    pub expect: Option<&'a expect::Expected>,
}

/// A case's verdict and the flag units the compiler rejected (dropped).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub verdict: Verdict,
    pub dropped: Vec<String>,
}

impl Tool {
    /// Runs `case` in `dir` (created; left for the caller to remove):
    /// compile each source (`-E`, `-S` or `-c` by the action), link the
    /// objects for `link`/`run`, run the executable for `run`.
    pub fn run_case(&self, case: &Case<'_>, dir: &Path) -> Ran {
        let (flags, dropped) = self.filter(case.flags);
        let done = |verdict: Verdict| Ran { verdict, dropped: dropped.clone() };
        if let Err(e) = std::fs::create_dir_all(dir) {
            return done(Verdict::new(
                Class::NotRun,
                format!("cannot create {}: {e}", dir.display()),
            ));
        }
        let (mode, ext) = match case.action {
            Action::Preprocess => ("-E", "i"),
            Action::Compile => ("-S", "s"),
            _ => ("-c", "o"),
        };
        let mut objects = Vec::new();
        let mut stderr = String::new();
        let mut compiled = true;
        for (i, src) in case.sources.iter().enumerate() {
            let out = dir.join(format!("{i}.{ext}"));
            let mut args: Vec<std::ffi::OsString> = flags.iter().map(Into::into).collect();
            args.push(mode.into());
            if let Some(l) = case.lang {
                args.extend(["-x".into(), l.into()]);
            }
            args.extend([src.into(), "-o".into(), out.clone().into()]);
            let v = self.step(&args, dir, case, "compile");
            match v {
                Err(v) => return done(v),
                Ok((Some(v), err)) if case.expect.is_some() => {
                    if matches!(v.class, Class::Crash | Class::Timeout) {
                        return done(v);
                    }
                    stderr.push_str(&err);
                    compiled = false;
                }
                Ok((Some(v), _)) => {
                    return done(if case.negative { outcome::negative(Some(v)) } else { v });
                }
                Ok((None, err)) => {
                    stderr.push_str(&err);
                    objects.push(out);
                }
            }
        }
        // An expected link error (g++.dg, `Expected::link_error`): the
        // link must fail, and its failure is the pass.
        let mut link_error = None;
        if let Some(exp) = case.expect {
            let diags = expect::diagnostics(&stderr);
            if let Some(v) = expect::judge(exp, &diags, compiled) {
                return done(v);
            }
            link_error = exp.link_error(&diags).filter(|_| compiled);
            if link_error.is_none() && (exp.negative() || !compiled) {
                return done(Verdict::pass());
            }
        } else if case.negative {
            return done(outcome::negative(None));
        }
        if !case.action.links() {
            return done(Verdict::pass());
        }
        let exe = dir.join("t.exe");
        let mut args: Vec<std::ffi::OsString> = flags.iter().map(Into::into).collect();
        args.extend(objects.iter().map(Into::into));
        args.extend(case.link_flags.iter().map(Into::into));
        args.extend(["-o".into(), exe.clone().into()]);
        match (self.step(&args, dir, case, "link"), link_error) {
            (Ok((Some(v), _)), Some(_)) if v.class == Class::CompileReject => {
                return done(Verdict::pass());
            }
            (Err(v) | Ok((Some(v), _)), _) => return done(v),
            (Ok((None, _)), Some(d)) => {
                return done(Verdict::new(
                    Class::AcceptInvalid,
                    format!("linked (expected a link error: {d})"),
                ));
            }
            (Ok((None, _)), None) => {}
        }
        if case.action != Action::Run {
            return done(Verdict::pass());
        }
        let mut run = Command::new(&exe);
        run.current_dir(dir);
        let mut run = process::limited(&run, case.mem);
        done(match process::run(&mut run, case.run_timeout) {
            Ok(o) => outcome::run_step(o.status),
            Err(e) => Verdict::new(Class::NotRun, format!("cannot run the test: {e}")),
        })
    }

    /// One compiler invocation: `Ok((None, stderr))` on success,
    /// `Ok((Some, stderr))` with its failure, `Err` when it could not be
    /// started.
    fn step(
        &self,
        args: &[std::ffi::OsString],
        dir: &Path,
        case: &Case<'_>,
        step: &str,
    ) -> Result<(Option<Verdict>, String), Verdict> {
        let mut c = self.command();
        c.args(args).current_dir(dir);
        let limit = match self.role {
            Role::Kiln => case.mem.map(|m| m.plus(process::kiln_stack_reservation_mib(args))),
            _ => case.mem,
        };
        let mut c = process::limited(&c, limit);
        // A spawn failure is retried twice: on the dev host's shared
        // filesystem a burst of spawns once failed with ENOENT for a few
        // seconds (93 cases of a 7572-case run, every compiler alike) and
        // passed on a re-run.
        let mut tries = 0;
        loop {
            match process::run(&mut c, case.compile_timeout) {
                Ok(o) => {
                    let err = o.stderr_str();
                    return Ok((outcome::compile_step(o.status, &err, step), err));
                }
                Err(_) if tries < 2 => {
                    tries += 1;
                    std::thread::sleep(Duration::from_secs(2));
                    let _ = std::fs::create_dir_all(dir);
                }
                Err(e) => {
                    return Err(Verdict::new(
                        Class::NotRun,
                        format!(
                            "cannot run {} `{}`: {e} (working directory {})",
                            self.role.name(),
                            self.path.display(),
                            if dir.is_dir() { "present" } else { "missing" }
                        ),
                    ));
                }
            }
        }
    }
}

/// `existing` with its region between `begin` and `end` replaced by
/// `region` (which carries the markers), appended when it has none.
pub fn splice_region(existing: &str, region: &str, begin: &str, end: &str) -> String {
    let b = existing.find(begin);
    match (b, b.and_then(|b| existing[b..].find(end).map(|e| b + e))) {
        (Some(b), Some(e)) => {
            let after = &existing[e + end.len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            format!("{}{region}{after}", &existing[..b])
        }
        _ if existing.is_empty() => region.to_owned(),
        _ => format!("{}\n\n{region}", existing.trim_end()),
    }
}

/// Groups flags into units: a flag that takes its value as the next word
/// (`--param`, `-include`, `-Xlinker`, `-mllvm`) with that word.
pub fn flag_units(flags: &[String]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        let mut unit = vec![f.clone()];
        if matches!(
            f.as_str(),
            "--param"
                | "-include"
                | "-imacros"
                | "-Xlinker"
                | "-mllvm"
                | "-isystem"
                | "-idirafter"
                | "-iquote"
                | "-I"
                | "-D"
                | "-U"
                | "--embed-directory"
        ) && let Some(v) = it.next()
        {
            unit.push(v.clone());
        }
        out.push(unit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clang_major_versions() {
        assert_eq!(clang_major("Debian clang version 19.1.7 (3+b1)"), Some(19));
        assert_eq!(clang_major("clang version 22.0.0git"), Some(22));
        assert_eq!(clang_major("gcc (GCC) 16.1.0"), None);
    }

    #[test]
    fn units_pair_value_flags() {
        let f: Vec<String> = ["-O2", "--param", "max-unroll-times=4", "-w"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(flag_units(&f).len(), 3);
        assert_eq!(flag_units(&f)[1], ["--param", "max-unroll-times=4"]);
        let g: Vec<String> = ["-D", "_GNU_SOURCE", "--embed-directory", "/d", "-O2"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            flag_units(&g),
            [vec!["-D", "_GNU_SOURCE"], vec!["--embed-directory", "/d"], vec!["-O2"]]
        );
    }
}
