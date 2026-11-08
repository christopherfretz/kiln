//! Internal compiler errors: the panic hook, the report and the crash
//! reproducer (spec/driver.md "Internal compiler errors").
//!
//! The `kiln` binary installs [`install_hook`] first thing. A panic on
//! any thread then records itself (only the first one: a panic on a
//! helper thread that the compile thread re-raises stays one report) and
//! prints nothing; the panic unwinds to `main`, which calls [`report`]:
//! the banner, then the reproducer (`<stem>-<random>.ii` and `.sh`),
//! then the exit code (1, as clang 19 after writing its reproducer; 70,
//! clang's crashed-`-cc1` code, with `-fno-crash-diagnostics`).
//!
//! Nothing here runs on the non-crashing path except [`catching`]: a
//! `catch_unwind` frame per input (free unless a panic unwinds through
//! it) and, in the binary, recording the input's path.
use std::any::Any;
use std::backtrace::Backtrace;
use std::cell::Cell;
use std::fmt::Write as _;
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread::ThreadId;

use kiln_diag::DiagOptions;
use kiln_source::activity::{self, Phase};
use kiln_source::{Loc, SourceManager};

use crate::args::{Action, Args, Lang};
use crate::pp_cmd::Job;
use crate::{SharedWriter, input_std, make_engine, rsp};

/// Where bug reports go.
pub const BUG_REPORT_URL: &str = "https://github.com/christopherfretz/kiln/issues";

/// `kiln <version> (<git sha>)`: the compiler's identity in a report.
pub fn version_line() -> String {
    format!("kiln {} ({})", env!("CARGO_PKG_VERSION"), env!("KILN_GIT_SHA"))
}

/// The exit code after a crash with the reproducer written (or
/// attempted): clang 19's driver exits 1 once it has printed its
/// `diagnostic msg:` lines.
pub const EXIT_CRASH: i32 = 1;
/// The exit code after a crash under `-fno-crash-diagnostics`: clang's
/// driver then passes on the crashed `-cc1`'s status; 70 (`EX_SOFTWARE`)
/// is the status clang's driver itself treats as a `-cc1` crash, and the
/// code kiln exited with before it had a report.
pub const EXIT_CRASH_NO_DIAGNOSTICS: i32 = 70;

/// The first panic of the process.
struct Record {
    message: String,
    /// `file:line:col` of the panic in the compiler's source.
    rust_loc: Option<String>,
    thread: ThreadId,
    thread_name: String,
    phase: Phase,
    loc: Loc,
    /// `file:line:col` of [`Record::loc`], once a [`catching`] frame with
    /// the right source manager resolved it.
    source_loc: Option<String>,
    backtrace: Option<String>,
}

/// The input a compile thread is working on.
#[derive(Clone)]
struct CrashInput {
    path: String,
    stdin: Option<String>,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static RECORD: Mutex<Option<Record>> = Mutex::new(None);
static INPUT: Mutex<Option<CrashInput>> = Mutex::new(None);
static COUNTER: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Panics on this thread are expected (the reproducer's own
    /// preprocessing): the hook records and prints nothing.
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// Installs the panic hook. The `kiln` binary calls it before anything
/// else; library users (the in-process tests) never do, and then every
/// function here is a no-op.
pub fn install_hook() {
    INSTALLED.store(true, Ordering::Relaxed);
    std::panic::set_hook(Box::new(hook));
}

fn installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

/// `RUST_BACKTRACE` (not `0`) or `KILN_BACKTRACE` (not `0`).
fn backtrace_enabled() -> bool {
    let on = |v: &str| std::env::var(v).is_ok_and(|s| !s.is_empty() && s != "0");
    on("KILN_BACKTRACE") || on("RUST_BACKTRACE")
}

/// The text of a panic payload (`panic!` with a literal or a format).
fn payload_text(p: &(dyn Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

fn hook(info: &PanicHookInfo<'_>) {
    if QUIET.with(Cell::get) {
        return;
    }
    let a = activity::current();
    let current = std::thread::current();
    let record = Record {
        message: payload_text(info.payload()),
        rust_loc: info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())),
        thread: current.id(),
        thread_name: current.name().unwrap_or("<unnamed>").to_owned(),
        phase: a.phase,
        loc: a.loc,
        source_loc: None,
        backtrace: backtrace_enabled().then(|| Backtrace::force_capture().to_string()),
    };
    let mut slot = RECORD.lock().unwrap_or_else(PoisonError::into_inner);
    match &*slot {
        None => *slot = Some(record),
        // A second panic on the thread of the first, before `main`
        // reported it, is a panic while unwinding: the runtime aborts
        // right after this hook, so report now (without a reproducer).
        Some(first) if first.thread == record.thread => {
            let prog = std::env::args().next().unwrap_or_else(|| "kiln".to_owned());
            let mut text = banner(&program_name(&prog), first);
            let _ = writeln!(
                text,
                "{}: note: a second panic while unwinding aborts the compiler: {}",
                program_name(&prog),
                record.message.lines().next().unwrap_or("")
            );
            let _ = std::io::stderr().lock().write_all(text.as_bytes());
        }
        // A panic on another thread while the first unwinds (the compile
        // thread re-raising a helper thread's panic): the same crash.
        Some(_) => {}
    }
}

/// Runs `f`, one input's job over `sm`, for the crash report: records
/// the input (`path`, and standard input's text for `-`) for the
/// reproducer and, if a panic unwinds through, resolves the panicking
/// location with `sm` before letting it go on.
pub(crate) fn catching<R>(
    path: &str,
    stdin: Option<&str>,
    sm: &SourceManager,
    f: impl FnOnce() -> R,
) -> R {
    if !installed() || QUIET.with(Cell::get) {
        return f();
    }
    let input = CrashInput { path: path.to_owned(), stdin: stdin.map(str::to_owned) };
    let previous = INPUT.lock().unwrap_or_else(PoisonError::into_inner).replace(input);
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => {
            *INPUT.lock().unwrap_or_else(PoisonError::into_inner) = previous;
            r
        }
        Err(payload) => {
            resolve(sm);
            std::panic::resume_unwind(payload)
        }
    }
}

/// Gives the recorded panic its source location in `sm` (once: the
/// innermost frame's manager is the one the location belongs to).
fn resolve(sm: &SourceManager) {
    let mut slot = RECORD.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(r) = slot.as_mut() else { return };
    if r.source_loc.is_some() || !r.loc.is_valid() || r.thread != std::thread::current().id() {
        return;
    }
    let loc = r.loc;
    drop(slot);
    // The manager is shared by `&` and append-only, so reading it while
    // the panic unwinds is sound; a panic here is swallowed.
    QUIET.with(|q| q.set(true));
    let resolved = std::panic::catch_unwind(AssertUnwindSafe(|| {
        sm.presumed_line_col(loc).map(|p| format!("{}:{}:{}", p.file_name, p.line, p.col))
    }));
    QUIET.with(|q| q.set(false));
    if let Ok(Some(text)) = resolved
        && let Some(r) = RECORD.lock().unwrap_or_else(PoisonError::into_inner).as_mut()
    {
        r.source_loc = Some(text);
    }
}

/// The basename of `argv[0]`, the prefix of the report's lines.
fn program_name(argv0: &str) -> String {
    Path::new(argv0).file_name().map_or_else(|| "kiln".to_owned(), |n| n.to_string_lossy().into())
}

/// The report's lines up to the bug-report request.
fn banner(prog: &str, r: &Record) -> String {
    let mut s = String::new();
    let mut lines = r.message.lines();
    let _ = writeln!(s, "{prog}: error: internal compiler error: {}", lines.next().unwrap_or(""));
    for l in lines {
        let _ = writeln!(s, "  {l}");
    }
    let input = INPUT.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let what = match &input {
        Some(i) if i.path == "-" => "while compiling '<stdin>'".to_owned(),
        Some(i) => format!("while compiling '{}'", i.path),
        None => "while running the driver".to_owned(),
    };
    let at = r.source_loc.as_ref().map_or_else(String::new, |l| format!(", at {l}"));
    let _ = writeln!(s, "{prog}: note: {what} (phase: {}{at})", r.phase.name());
    if let Some(l) = &r.rust_loc {
        let _ = writeln!(s, "{prog}: note: in compiler source {l} (thread '{}')", r.thread_name);
    }
    let _ = writeln!(s, "{}", version_line());
    let _ = writeln!(
        s,
        "{prog}: note: please submit a bug report to {BUG_REPORT_URL} and include the crash \
         reproducer"
    );
    match &r.backtrace {
        Some(bt) => {
            let _ = writeln!(s, "{prog}: note: compiler backtrace:\n{}", bt.trim_end());
        }
        None => {
            let _ =
                writeln!(s, "{prog}: note: set KILN_BACKTRACE=1 to include the compiler backtrace");
        }
    }
    s
}

/// Reports the recorded crash on standard error and writes the
/// reproducer; returns the exit code. `argv0` and `command_line` are the
/// process's (before response-file expansion).
pub fn report(argv0: &str, command_line: &[String]) -> i32 {
    let prog = program_name(argv0);
    let record = RECORD.lock().unwrap_or_else(PoisonError::into_inner).take();
    let record = record.unwrap_or_else(|| Record {
        message: "unknown panic".to_owned(),
        rust_loc: None,
        thread: std::thread::current().id(),
        thread_name: String::new(),
        phase: Phase::Driver,
        loc: Loc::INVALID,
        source_loc: None,
        backtrace: None,
    });
    let mut text = banner(&prog, &record);
    // The command line again, quietly: if parsing it is what crashed,
    // there is nothing more to say.
    QUIET.with(|q| q.set(true));
    let parsed = std::panic::catch_unwind(|| {
        let expanded = rsp::expand(command_line);
        let args = Args::parse_as(argv0, &expanded);
        (expanded, args)
    });
    QUIET.with(|q| q.set(false));
    let Ok((expanded, args)) = parsed else {
        let _ = std::io::stderr().lock().write_all(text.as_bytes());
        return EXIT_CRASH;
    };
    if args.no_crash_diagnostics {
        let _ = std::io::stderr().lock().write_all(text.as_bytes());
        return EXIT_CRASH_NO_DIAGNOSTICS;
    }
    let input = INPUT.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let files = write_reproducer(argv0, &expanded, &args, input.as_ref());
    let _ = writeln!(
        text,
        "{prog}: note: preprocessed source and associated run script are located at:"
    );
    for line in files {
        let _ = writeln!(text, "{prog}: note: diagnostic msg: {line}");
    }
    let _ = std::io::stderr().lock().write_all(text.as_bytes());
    EXIT_CRASH
}

/// `-fcrash-diagnostics-dir=`, else `$TMPDIR`, else `/tmp`.
fn reproducer_dir(args: &Args) -> PathBuf {
    if let Some(d) = &args.crash_diagnostics_dir {
        return PathBuf::from(d);
    }
    match std::env::var_os("TMPDIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from("/tmp"),
    }
}

/// Six hex digits, as clang's: the clock, the process and a counter.
fn random_suffix(attempt: u32) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() ^ (d.as_secs() as u32));
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut x = nanos ^ std::process::id().rotate_left(13) ^ n.rotate_left(7) ^ attempt;
    // A 32-bit finalizer (murmur3's) spreads the inputs over the digits.
    x ^= x >> 16;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 13;
    x = x.wrapping_mul(0xc2b2_ae35);
    x ^= x >> 16;
    format!("{:06x}", x & 0x00ff_ffff)
}

/// A word for a POSIX shell.
fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./=+,:@%^".contains(&b))
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// The reproducer's run script: a header, then the command line with
/// the input replaced by the preprocessed file (when there is one).
fn run_script(argv0: &str, expanded: &[String], input: Option<&str>, ii: Option<&Path>) -> String {
    let cwd = std::env::current_dir().map_or_else(|_| "?".to_owned(), |d| d.display().to_string());
    // The program as invoked (its name picks the driver mode), absolute
    // when it was given as a relative path.
    let program = if argv0.contains('/') && !argv0.starts_with('/') {
        Path::new(&cwd).join(argv0).display().to_string()
    } else {
        argv0.to_owned()
    };
    let original: Vec<String> = std::iter::once(argv0)
        .chain(expanded.iter().map(String::as_str))
        .map(shell_quote)
        .collect();
    let mut words = vec![shell_quote(&program)];
    let mut replaced = false;
    let mut prev = "";
    for a in expanded {
        let is_input = !replaced && Some(a.as_str()) == input && prev != "-o";
        match (is_input, ii) {
            (true, Some(ii)) => {
                words.push(shell_quote(&ii.display().to_string()));
                replaced = true;
            }
            _ => words.push(shell_quote(a)),
        }
        prev = a;
    }
    format!(
        "# Crash reproducer for {}\n# Original command: {}\n# Working directory: {}\ncd {}\n{}\n",
        version_line(),
        original.join(" "),
        cwd,
        shell_quote(&cwd),
        words.join(" ")
    )
}

/// Writes the reproducer; returns the `diagnostic msg:` lines (the
/// files written, or why not).
fn write_reproducer(
    argv0: &str,
    expanded: &[String],
    args: &Args,
    input: Option<&CrashInput>,
) -> Vec<String> {
    let dir = reproducer_dir(args);
    let stem = match input {
        Some(i) if i.path == "-" => "stdin".to_owned(),
        Some(i) => Path::new(&i.path)
            .file_stem()
            .map_or_else(|| "crash".to_owned(), |s| s.to_string_lossy().into_owned()),
        None => args.inputs.first().map_or_else(
            || "kiln".to_owned(),
            |i| {
                Path::new(&i.path)
                    .file_stem()
                    .map_or_else(|| "crash".to_owned(), |s| s.to_string_lossy().into_owned())
            },
        ),
    };
    let lang = input
        .and_then(|i| args.inputs.iter().find(|a| a.path == i.path))
        .map_or(Lang::Cxx, |i| args.input_lang(i));
    let ext = if lang == Lang::C { "i" } else { "ii" };
    // Made if missing, as clang does; a fresh name: the run script is
    // created exclusively.
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return vec![format!("cannot write the crash reproducer to '{}': {e}", dir.display())];
    }
    let mut script = None;
    for attempt in 0..16 {
        let base = format!("{stem}-{}", random_suffix(attempt));
        let sh = dir.join(format!("{base}.sh"));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&sh) {
            Ok(f) => {
                script = Some((f, sh, dir.join(format!("{base}.{ext}"))));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return vec![format!(
                    "cannot write the crash reproducer to '{}': {e}",
                    dir.display()
                )];
            }
        }
    }
    let Some((mut sh_file, sh, ii)) = script else {
        return vec![format!("cannot write the crash reproducer to '{}'", dir.display())];
    };
    let mut lines = Vec::new();
    let mut ii_written = None;
    match input {
        None => lines.push(
            "the crash happened outside any input; only the run script was written".to_owned(),
        ),
        Some(i) => match preprocess(args, i) {
            Some(text) => match std::fs::write(&ii, text) {
                Ok(()) => {
                    lines.push(ii.display().to_string());
                    ii_written = Some(ii.as_path());
                }
                Err(e) => lines.push(format!("cannot write '{}': {e}", ii.display())),
            },
            None => lines.push(
                "preprocessing the input also crashed; only the run script was written".to_owned(),
            ),
        },
    }
    let text = run_script(argv0, expanded, input.map(|i| i.path.as_str()), ii_written);
    match sh_file.write_all(text.as_bytes()) {
        Ok(()) => lines.push(sh.display().to_string()),
        Err(e) => lines.push(format!("cannot write '{}': {e}", sh.display())),
    }
    lines
}

/// `-E` of `input` with the crashing command line's flags, on a fresh
/// thread with the compile thread's stack (and its
/// `-fkiln-crash-for-testing=` request); `None` if it panics too.
fn preprocess(args: &Args, input: &CrashInput) -> Option<String> {
    let mut a = args.clone();
    a.action = Action::Preprocess;
    a.output = None;
    a.deps = crate::args::DepArgs::default();
    a.print = crate::args::PrintArgs::default();
    a.verbose = false;
    let input = input.clone();
    let size = crate::templ_opts::stack_size_of(&args.argv);
    let worker = std::thread::Builder::new().name("kiln-reproducer".into()).stack_size(size);
    let handle = worker
        .spawn(move || {
            QUIET.with(|q| q.set(true));
            kiln_sema::stack_guard::StackGuard::install(size);
            activity::set_crash_for_testing(a.crash_for_testing);
            let sink = SharedWriter::new(std::io::sink());
            let mut diag_opts = DiagOptions::from_args(&a.diag_args);
            diag_opts.clang_compat = a.predef.clang_compat.major();
            let dsm = SourceManager::new();
            let tc = crate::toolchain_of(&a, &make_engine(&dsm, &diag_opts, sink.clone(), None));
            let lang = a
                .inputs
                .iter()
                .find(|i| i.path == input.path)
                .map_or(Lang::Cxx, |i| a.input_lang(i));
            let std = input_std(&a, lang).ok()?;
            let job = Job {
                args: &a,
                toolchain: &tc,
                std,
                path: &input.path,
                stdin_text: input.stdin.clone(),
                diag_opts: &diag_opts,
                stderr: sink,
            };
            Some(job.run().output)
        })
        .ok()?;
    handle.join().ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("-std=c++26"), "-std=c++26");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("-DX=\"y\""), "'-DX=\"y\"'");
    }

    #[test]
    fn script_replaces_the_input_only() {
        let args: Vec<String> =
            ["-o", "t.cpp", "-c", "t.cpp", "-DA=t.cpp"].iter().map(|s| (*s).to_owned()).collect();
        let s = run_script("kiln++", &args, Some("t.cpp"), Some(Path::new("/d/t-abc.ii")));
        let last = s.lines().last().unwrap();
        assert_eq!(last, "kiln++ -o t.cpp -c /d/t-abc.ii -DA=t.cpp");
        assert!(s.starts_with("# Crash reproducer for kiln "));
        assert!(s.contains("\n# Original command: kiln++ -o t.cpp -c t.cpp -DA=t.cpp\n"));
        let s = run_script("kiln++", &args, Some("t.cpp"), None);
        assert_eq!(s.lines().last().unwrap(), "kiln++ -o t.cpp -c t.cpp -DA=t.cpp");
    }

    #[test]
    fn suffixes_are_six_hex_digits() {
        let a = random_suffix(0);
        assert_eq!(a.len(), 6);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(a, random_suffix(0));
    }

    #[test]
    fn banner_shape() {
        let r = Record {
            message: "boom\nsecond line".to_owned(),
            rust_loc: Some("crates/x/src/y.rs:1:2".to_owned()),
            thread: std::thread::current().id(),
            thread_name: "kiln".to_owned(),
            phase: Phase::Oven,
            loc: Loc::INVALID,
            source_loc: Some("t.cpp:3:4".to_owned()),
            backtrace: None,
        };
        let b = banner("kiln++", &r);
        let lines: Vec<&str> = b.lines().collect();
        assert_eq!(lines[0], "kiln++: error: internal compiler error: boom");
        assert_eq!(lines[1], "  second line");
        // No input recorded: the library never installs the hook.
        assert_eq!(lines[2], "kiln++: note: while running the driver (phase: oven, at t.cpp:3:4)");
        assert_eq!(
            lines[3],
            "kiln++: note: in compiler source crates/x/src/y.rs:1:2 (thread 'kiln')"
        );
        assert_eq!(lines[4], version_line());
        assert!(lines[5].contains(BUG_REPORT_URL));
    }
}
