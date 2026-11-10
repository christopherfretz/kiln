//! `cargo xtask test --external <dir>`: a never-crash run over a corpus that
//! lives outside the repository (`spec/harness.md` "External corpora").
//!
//! Every `.c`/`.cc`/`.cpp`/`.cxx` file under `<dir>` is compiled once with
//! `-fsyntax-only -ferror-limit=0` (plus its `//@ std:`/`//@ args:` when its
//! header parses as test directives). It must compile or be rejected
//! cleanly (exit 0 or 1): a signal, a panic, an internal compiler error, a
//! stack overflow, any other exit code, a timeout or a memory kill fails the
//! run. Only counts are printed (file names too with `-v`), and nothing is
//! written anywhere: the corpus may be private, so no part of it may reach
//! the repository.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::Options;
use super::directives::{self, Lang};
use crate::util::process::{self, MemLimit, Output};
use crate::util::{CompilerKind, build_kiln, repo_root, target_dir};

/// How one external case ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Accepted,
    Rejected,
    Crashed,
    TimedOut,
    Killed,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected cleanly",
            Self::Crashed => "crashed",
            Self::TimedOut => "timed out",
            Self::Killed => "killed",
        }
    }

    pub fn is_failure(self) -> bool {
        matches!(self, Self::Crashed | Self::TimedOut | Self::Killed)
    }

    pub const ALL: [Self; 5] =
        [Self::Accepted, Self::Rejected, Self::Crashed, Self::TimedOut, Self::Killed];
}

/// Classifies a finished compile under the never-crash rule.
pub fn classify(o: &Output) -> Verdict {
    if o.timed_out() {
        return Verdict::TimedOut;
    }
    if o.out_of_memory() {
        return Verdict::Killed;
    }
    let stderr = o.stderr_str();
    let crashed =
        ["panicked at", "PLEASE submit a bug report", "internal compiler error", "stack overflow"]
            .iter()
            .any(|m| stderr.contains(m));
    match o.code() {
        Some(0) if !crashed => Verdict::Accepted,
        Some(1) if !crashed => Verdict::Rejected,
        _ => Verdict::Crashed,
    }
}

/// `dir` must exist and lie outside the repository `root` (the in-repo
/// corpus has `cargo xtask test`; this mode never writes, so a private
/// corpus stays where it is).
pub fn check_outside(dir: &Path, root: &Path) -> Result<PathBuf, String> {
    let d = std::fs::canonicalize(dir).map_err(|e| format!("--external {}: {e}", dir.display()))?;
    if !d.is_dir() {
        return Err(format!("--external {}: not a directory", dir.display()));
    }
    let r = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if d.starts_with(&r) {
        return Err(format!(
            "--external {} is inside the repository; run in-repo tests with `cargo xtask test`",
            dir.display()
        ));
    }
    Ok(d)
}

/// The sources under `dir`, sorted, recursively.
pub fn sources(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries =
            std::fs::read_dir(&d).map_err(|e| format!("cannot read {}: {e}", d.display()))?;
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x, "c" | "cc" | "cpp" | "cxx"))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The compile of one external case: `-fsyntax-only -ferror-limit=0`, its
/// `std:`/`args:` when its header parses, GCC's spelling for GCC.
pub fn command(compiler: &Path, kind: CompilerKind, path: &Path, mode: &[&str]) -> Command {
    let lang = if path.extension().is_some_and(|x| x == "c") { Lang::C } else { Lang::Cxx };
    let source = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned());
    let (std, args) = match source.as_deref().map(|s| directives::parse(s, lang)) {
        Ok(Ok(spec)) => (spec.std, spec.args),
        _ => ((if lang == Lang::C { "c17" } else { "c++26" }).to_owned(), Vec::new()),
    };
    let mut c = Command::new(compiler);
    crate::util::isolate_caches(&mut c);
    if let Some(d) = path.parent() {
        c.current_dir(d);
    }
    c.arg(format!("-std={std}")).args(mode);
    if kind == CompilerKind::Gcc {
        let warnings = crate::oracle::GccWarnings::of(compiler, lang == Lang::C);
        c.arg("-fmax-errors=0").args(crate::oracle::gcc_args_with(&args, warnings.as_deref()));
    } else {
        c.arg("-ferror-limit=0").args(args);
    }
    if kind == CompilerKind::Kiln {
        c.env("KILN_LLVM_DIR", crate::oracle::Oracle::from_env().unwrap_or_default().llvm_dir());
    }
    c.arg(path);
    c
}

/// Per-verdict counts and the failing cases' paths relative to the corpus.
#[derive(Debug, Default)]
pub struct Summary {
    pub counts: [usize; 5],
    pub failures: Vec<(Verdict, String)>,
    pub elapsed: Duration,
    /// Measured runs only: the summed compile wall time and the largest
    /// peak RSS (KiB) of any case.
    pub compile_time: Duration,
    pub max_rss_kb: Option<u64>,
}

impl Summary {
    pub fn failed(&self) -> bool {
        !self.failures.is_empty()
    }

    pub fn line(&self, total: usize) -> String {
        let parts: Vec<String> = Verdict::ALL
            .iter()
            .zip(self.counts)
            .map(|(v, n)| format!("{n} {}", v.label()))
            .collect();
        format!(
            "external: {} {total} cases: {} in {:.2}s",
            if self.failed() { "FAILED." } else { "ok." },
            parts.join(", "),
            self.elapsed.as_secs_f64()
        )
    }

    /// The extra line of a measured run.
    #[allow(clippy::cast_precision_loss)]
    pub fn measured_line(&self) -> String {
        let rss = self
            .max_rss_kb
            .map_or_else(|| "unavailable".to_owned(), |k| format!("{:.1} MiB", k as f64 / 1024.0));
        format!(
            "external: compile time {:.2}s in total, largest peak RSS {rss}",
            self.compile_time.as_secs_f64()
        )
    }
}

/// One external compile may take this long.
const TIMEOUT: Duration = Duration::from_secs(120);

/// Wall time and peak RSS of a measured external run (`xtask bench
/// --external`): the probe and a scratch directory for its reports.
pub struct Measure<'a> {
    pub probe: &'a process::RssProbe,
    pub scratch: &'a Path,
}

/// Runs every case under `dir` with `compiler` on `jobs` threads.
pub fn run(
    dir: &Path,
    compiler: &Path,
    kind: CompilerKind,
    jobs: usize,
    cap: Option<MemLimit>,
    measure: Option<&Measure<'_>>,
) -> Result<(usize, Summary), String> {
    let start = Instant::now();
    let cases = sources(dir)?;
    let next = AtomicUsize::new(0);
    let summary = Mutex::new(Summary::default());
    let cap = cap.map(|c| {
        if kind == CompilerKind::Kiln {
            c.plus(process::kiln_stack_reservation_mib::<&str>(&[]))
        } else {
            c
        }
    });
    std::thread::scope(|s| {
        for worker in 0..jobs.clamp(1, cases.len().max(1)) {
            let (next, summary, cases) = (&next, &summary, &cases);
            s.spawn(move || {
                while let Some(path) = cases.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let cmd = command(compiler, kind, path, &["-fsyntax-only"]);
                    let (v, wall, rss) = match measure {
                        None => match process::run(&mut process::limited(&cmd, cap), TIMEOUT) {
                            Ok(o) => (classify(&o), Duration::ZERO, None),
                            Err(_) => (Verdict::Crashed, Duration::ZERO, None),
                        },
                        Some(m) => {
                            let rss_file = m.scratch.join(format!("rss-{worker}.txt"));
                            match process::run_measured(&cmd, m.probe, &rss_file, TIMEOUT, cap) {
                                Ok(r) => (classify(&r.as_output()), r.wall, r.max_rss_kb),
                                Err(_) => (Verdict::Crashed, Duration::ZERO, None),
                            }
                        }
                    };
                    let mut s = summary.lock().expect("no worker panics while holding the lock");
                    s.counts[v as usize] += 1;
                    s.compile_time += wall;
                    s.max_rss_kb = s.max_rss_kb.max(rss);
                    if v.is_failure() {
                        let rel = path.strip_prefix(dir).unwrap_or(path);
                        s.failures.push((v, rel.display().to_string()));
                    }
                }
            });
        }
    });
    let mut summary = summary.into_inner().expect("workers have finished");
    summary.failures.sort();
    summary.elapsed = start.elapsed();
    Ok((cases.len(), summary))
}

/// `cargo xtask test --external <dir>`.
pub fn main(opts: &Options) -> ExitCode {
    match execute(opts, true) {
        Ok(s) if s.failed() => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask test --external: error: {e}");
            ExitCode::from(2)
        }
    }
}

/// Runs [`Options::external`]; `outside_only` is off only for the
/// self-tests, whose fixture corpus lives in the repo.
pub fn execute(opts: &Options, outside_only: bool) -> Result<Summary, String> {
    let root = repo_root();
    let given = opts.external.as_deref().ok_or("no --external directory")?;
    let dir = if outside_only {
        check_outside(given, &root)?
    } else {
        std::fs::canonicalize(given).map_err(|e| format!("{}: {e}", given.display()))?
    };
    let compiler = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln(&root, &target_dir(&root)?, "test", false)?,
    };
    let kind = crate::util::identify(&compiler)?.kind;
    let jobs = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)));
    let (total, summary) =
        run(&dir, &compiler, kind, jobs, MemLimit::from_mib(opts.mem_limit_mib), None)?;
    if opts.verbose {
        for (v, name) in &summary.failures {
            println!("{:<9} {name}", v.label().to_uppercase());
        }
    }
    println!("{}", summary.line(total));
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_corpus_counts_only() {
        if crate::util::identify(Path::new("clang++-19")).is_err() {
            eprintln!("clang++-19 not installed; skipping the --external fixture run");
            return;
        }
        let root = repo_root();
        let dir = root.join("xtask/fixtures/external");
        let opts = Options {
            compiler: Some(root.join("xtask/fixtures/fake-gcc/g++")),
            external: Some(dir.clone()),
            jobs: Some(2),
            ..Options::default()
        };
        let s = execute(&opts, false).unwrap();
        assert_eq!(s.counts, [1, 1, 2, 0, 0], "{s:?}");
        assert_eq!(
            s.failures,
            [
                (Verdict::Crashed, "ice.cpp".to_owned()),
                (Verdict::Crashed, "nested/signal.cpp".to_owned())
            ]
        );
        assert!(s.line(4).starts_with(
            "external: FAILED. 4 cases: 1 accepted, 1 rejected cleanly, 2 crashed, 0 timed out, 0 killed in "
        ));
        // The real entry point refuses a directory inside the repository.
        let e = execute(&opts, true).unwrap_err();
        assert!(e.contains("inside the repository"), "{e}");
        assert!(check_outside(Path::new("/no/such/dir"), &root).is_err());
    }
}
