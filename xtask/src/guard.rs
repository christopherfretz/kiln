//! `cargo xtask guard`: CI's misbehaviour checks (WP-1901; spec/harness.md
//! "CI", "Misbehaviour checks"; docs/VALIDATION.md "GitHub Actions").
//!
//! Two invariants a compiler and its harness hold anyway, checked on a
//! traced run of a corpus slice ([`check_log`] over an `strace -f -y` log):
//!
//! - **no network**: no traced process creates a socket of any family but
//!   those of [`ALLOWED_FAMILIES`] (today none: kiln, xtask, clang, GCC and
//!   the linkers open no socket at all), and none connects or binds an
//!   `AF_INET`/`AF_INET6` address;
//! - **no stray writes**: every successful call that creates, truncates,
//!   opens for writing, renames, links or removes a file names a path under
//!   an allowed root (the target directory, which holds the test scratch
//!   directories, the module cache and the run's `$TMPDIR`; [`Allow`]).
//!
//! And one report on a pull request's diff ([`classify`]): the changes that
//! run code at build time or change what CI runs (a new or changed
//! dependency, a new build script or proc-macro crate, `.github/` or
//! `.cargo/`), printed as `::warning::` annotations and a job summary.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask guard <command>

  trace [--log <file>] [--allow <path>]... -- <command> [<arg>...]
        run <command> under `strace -f -y` with a fresh $TMPDIR under the target
        directory, then check the log (below); fails when the command fails or the
        log breaks a rule. Default log: <target>/guard/strace.log
  check-log <file> [--allow <path>]...
        check an existing strace log: no socket outside the allowed families, no
        AF_INET/AF_INET6 connect or bind, no write outside the target directory,
        /dev/null, /proc/self and the --allow paths
  risky --base <rev> [--head <rev>] [--strict]
        list the changes from <base> to <head> (default HEAD) that run code at
        build time or change CI: dependency entries of Cargo.toml and Cargo.lock,
        a new build.rs or proc-macro crate, .github/ and .cargo/; prints
        ::warning:: annotations and appends a summary to $GITHUB_STEP_SUMMARY.
        Exit 0 (1 with --strict when anything is flagged)

spec/harness.md \"Misbehaviour checks\".
";

/// The syscalls `guard trace` records: the socket calls and every call
/// that can create, write, rename or remove a path. A `?` makes strace
/// accept a name the architecture lacks (aarch64 has only the `*at` forms;
/// x86-64 has both).
pub const TRACED: &str = "socket,connect,bind,openat,?openat2,?open,?creat,?rename,renameat,\
?renameat2,?unlink,unlinkat,?rmdir,?mkdir,mkdirat,?link,linkat,?symlink,symlinkat,?mknod,\
mknodat,truncate";

/// Socket families a traced process may create. Empty: on a clean run of
/// the corpus slice (2026-11-21, aarch64 Debian 13, kiln, clang 19/22,
/// GCC 16.1, ld.lld and ld) no process opens a socket at all. A family
/// added here names the tool that needs it.
pub const ALLOWED_FAMILIES: &[&str] = &[];

/// Files and directories every run may write besides the target
/// directory: the null device, and the process's own `/proc` entries
/// (`/proc/self/...`, written by runtimes that tune themselves).
const ALWAYS_ALLOWED: &[&str] = &["/dev/null", "/proc/self", "/proc/thread-self"];

pub fn main(mut args: impl Iterator<Item = String>) -> ExitCode {
    let r = match args.next().as_deref() {
        Some("trace") => trace_main(args),
        Some("check-log") => check_log_main(args),
        Some("risky") => risky_main(args),
        Some("-h" | "--help") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => Err(Usage::Bad("a command: trace, check-log or risky".into())),
    };
    match r {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(Usage::Bad(m)) => {
            eprint!("xtask guard: expected {m}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Usage::Error(m)) => {
            eprintln!("xtask guard: {m}");
            ExitCode::from(2)
        }
    }
}

enum Usage {
    Bad(String),
    Error(String),
}

fn error(m: impl Into<String>) -> Usage {
    Usage::Error(m.into())
}

// ---------------------------------------------------------------- trace --

/// The allowlist of [`check_log`]: path prefixes (component-wise; an entry
/// allows itself and everything under it).
#[derive(Debug, Clone, Default)]
pub struct Allow {
    roots: Vec<PathBuf>,
}

impl Allow {
    /// [`ALWAYS_ALLOWED`] plus `extra`, each also in its canonical form when
    /// that differs (a symlinked checkout).
    pub fn new(extra: &[PathBuf]) -> Self {
        let mut a = Allow::default();
        for p in ALWAYS_ALLOWED.iter().map(PathBuf::from).chain(extra.iter().cloned()) {
            a.add(&p);
        }
        a
    }

    fn add(&mut self, p: &Path) {
        let p = normalize(p);
        if let Ok(c) = std::fs::canonicalize(&p)
            && c != p
        {
            self.roots.push(c);
        }
        self.roots.push(p);
    }

    pub fn allows(&self, p: &Path) -> bool {
        self.roots.iter().any(|r| p.starts_with(r))
    }
}

/// The jobserver FIFO a `--jobserver-auth=fifo:<path>` in `MAKEFLAGS` or
/// `CARGO_MAKEFLAGS` names: cargo, xtask and rustc open it read-write to
/// take tokens, which writes nothing to the file system (a development
/// machine's shared pool; GitHub CI has none).
fn jobserver_fifo() -> Option<PathBuf> {
    ["CARGO_MAKEFLAGS", "MAKEFLAGS"].iter().find_map(|v| {
        let flags = std::env::var(v).ok()?;
        let path = flags.split_whitespace().find_map(|w| w.strip_prefix("--jobserver-auth=fifo:"));
        path.map(PathBuf::from)
    })
}

fn allow_list(extra: Vec<PathBuf>) -> Result<Allow, Usage> {
    let root = crate::util::repo_root();
    let target = crate::util::target_dir(&root).map_err(error)?;
    let mut all = vec![target];
    all.extend(jobserver_fifo());
    all.extend(extra.into_iter().map(|p| if p.is_absolute() { p } else { root.join(p) }));
    Ok(Allow::new(&all))
}

fn trace_main(mut args: impl Iterator<Item = String>) -> Result<bool, Usage> {
    let mut log = None;
    let mut extra = Vec::new();
    let mut cmd = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--log" => {
                log = Some(PathBuf::from(args.next().ok_or(Usage::Bad("--log <file>".into()))?));
            }
            "--allow" => {
                extra.push(PathBuf::from(args.next().ok_or(Usage::Bad("--allow <path>".into()))?));
            }
            "--" => {
                cmd.extend(args.by_ref());
                break;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(true);
            }
            other => return Err(Usage::Bad(format!("`--` before the command, not `{other}`"))),
        }
    }
    if cmd.is_empty() {
        return Err(Usage::Bad("a command after `--`".into()));
    }
    let root = crate::util::repo_root();
    let target = crate::util::target_dir(&root).map_err(error)?;
    let guard = target.join("guard");
    let log = log.unwrap_or_else(|| guard.join("strace.log"));
    // A fresh $TMPDIR inside the target directory: the compilers' temporary
    // files (clang's and kiln's link objects, GCC's) land in an allowed
    // root, and a write to /tmp is then a violation like any other.
    let tmp = guard.join(format!("tmp-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)
        .map_err(|e| error(format!("cannot create {}: {e}", tmp.display())))?;
    if let Some(d) = log.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| error(format!("cannot create {}: {e}", d.display())))?;
    }
    println!(
        "xtask guard: tracing `{}` (log {}, TMPDIR {})",
        cmd.join(" "),
        log.display(),
        tmp.display()
    );
    let status = Command::new("strace")
        .args(["-f", "-y", "-qq", "-o"])
        .arg(&log)
        .args(["-e", &format!("trace={TRACED}"), "--"])
        .args(&cmd)
        .env("TMPDIR", &tmp)
        // The traced xtask then needs no `cargo metadata` to find it.
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .map_err(|e| error(format!("cannot run strace: {e} (apt-get install strace)")))?;
    let text = std::fs::read_to_string(&log)
        .map_err(|e| error(format!("cannot read {}: {e}", log.display())))?;
    let _ = std::fs::remove_dir_all(&tmp);
    let verdict = check_log(&text, &allow_list(extra)?);
    print!("{}", verdict.render());
    if !status.success() {
        println!("xtask guard: the traced command failed ({status})");
    }
    Ok(status.success() && verdict.passes())
}

fn check_log_main(mut args: impl Iterator<Item = String>) -> Result<bool, Usage> {
    let mut log = None;
    let mut extra = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--allow" => {
                extra.push(PathBuf::from(args.next().ok_or(Usage::Bad("--allow <path>".into()))?));
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(true);
            }
            _ if log.is_none() && !a.starts_with('-') => log = Some(PathBuf::from(a)),
            other => return Err(Usage::Bad(format!("one log file, not `{other}`"))),
        }
    }
    let log = log.ok_or(Usage::Bad("a log file".into()))?;
    let text = std::fs::read_to_string(&log)
        .map_err(|e| error(format!("cannot read {}: {e}", log.display())))?;
    let verdict = check_log(&text, &allow_list(extra)?);
    print!("{}", verdict.render());
    Ok(verdict.passes())
}

/// One completed syscall of an strace log: `<pid> <name>(<args>) = <ret>`,
/// with an `<unfinished ...>` line and its `<... name resumed>` line
/// joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub pid: u32,
    pub name: String,
    pub args: Vec<String>,
    /// The text after ` = `: `0`, `3</path>`, `-1 ENOENT (...)`, `?`.
    pub ret: String,
}

impl Call {
    /// The call failed (`= -1 ...`). A call whose process died inside it
    /// (`= ?`) counts as done.
    pub fn failed(&self) -> bool {
        self.ret.starts_with("-1")
    }

    fn line(&self) -> String {
        let mut s = format!("{} {}({}) = {}", self.pid, self.name, self.args.join(", "), self.ret);
        if s.len() > 300 {
            let mut cut = 300;
            while !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s.truncate(cut);
            s.push_str("...");
        }
        s
    }
}

/// Parses an `strace -f` log (`-o` file: every line starts with the pid).
/// Lines that are not syscalls (`--- SIGCHLD ...`, `+++ exited ...`) are
/// skipped; so is a line this parser cannot read, which [`Verdict`] counts.
pub fn parse_log(text: &str) -> (Vec<Call>, usize) {
    let mut pending: BTreeMap<u32, String> = BTreeMap::new();
    let mut calls = Vec::new();
    let mut unreadable = 0;
    for line in text.lines() {
        let Some((pid, rest)) = line.split_once(' ') else { continue };
        let Ok(pid) = pid.parse::<u32>() else {
            unreadable += usize::from(!line.trim().is_empty());
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with("---") || rest.starts_with("+++") {
            continue;
        }
        let mut whole = if let Some(r) = rest.strip_prefix("<... ") {
            // `<... openat resumed>, O_RDONLY) = 3`
            let Some((_, tail)) = r.split_once(" resumed>") else {
                unreadable += 1;
                continue;
            };
            let head = pending.remove(&pid).unwrap_or_default();
            format!("{head}{tail}")
        } else {
            rest.to_owned()
        };
        if let Some(head) = whole.strip_suffix("<unfinished ...>") {
            pending.insert(pid, head.trim_end().to_owned());
            continue;
        }
        // `name(args) <unfinished ...>) = ?` after a resume: drop the marker.
        whole = whole.replace(" <unfinished ...>", "");
        match parse_call(&whole) {
            Some((name, argv, result)) => calls.push(Call { pid, name, args: argv, ret: result }),
            None => unreadable += 1,
        }
    }
    // A call still unfinished when the log ends: its process was killed
    // inside it. Check it as if it had completed.
    for (pid, head) in pending {
        match parse_call(&format!("{head}) = ?")) {
            Some((name, argv, result)) => calls.push(Call { pid, name, args: argv, ret: result }),
            None => unreadable += 1,
        }
    }
    (calls, unreadable)
}

/// `name(a, b, ...) = ret` → its parts; strings (`"..."`, with escapes),
/// `-y` decorations (`3</path>`) and `{...}`/`[...]` structures are kept
/// whole in their argument.
fn parse_call(s: &str) -> Option<(String, Vec<String>, String)> {
    let open = s.find('(')?;
    let name = &s[..open];
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    let body = &s[open + 1..];
    let mut args = Vec::new();
    let mut cur = String::new();
    let (mut depth, mut in_str, mut in_deco, mut esc) = (0usize, false, false, false);
    let mut prev = '(';
    let mut end = None;
    for (i, c) in body.char_indices() {
        if in_str {
            cur.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if in_deco {
            cur.push(c);
            if c == '>' {
                in_deco = false;
            }
        } else {
            match c {
                '"' => {
                    in_str = true;
                    cur.push(c);
                }
                // A `-y` decoration follows an fd or `AT_FDCWD` directly.
                '<' if prev.is_ascii_alphanumeric() => {
                    in_deco = true;
                    cur.push(c);
                }
                '{' | '[' | '(' => {
                    depth += 1;
                    cur.push(c);
                }
                '}' | ']' => {
                    depth = depth.saturating_sub(1);
                    cur.push(c);
                }
                ')' if depth == 0 => {
                    end = Some(i);
                    break;
                }
                ')' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => args.push(std::mem::take(&mut cur).trim().to_owned()),
                _ => cur.push(c),
            }
        }
        prev = c;
    }
    let end = end?;
    if !cur.trim().is_empty() || !args.is_empty() {
        args.push(cur.trim().to_owned());
    }
    let after = body[end + 1..].trim_start();
    let ret = after.strip_prefix('=')?.trim().to_owned();
    Some((name.to_owned(), args, ret))
}

/// A quoted strace string argument, unescaped; `None` if `a` is not one
/// (a `NULL`, or a string strace cut short with `...`).
fn unquote(arg: &str) -> Option<String> {
    let inner = arg.strip_prefix('"')?.strip_suffix('"')?;
    let bytes = inner.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos] != b'\\' || pos + 1 == bytes.len() {
            out.push(bytes[pos]);
            pos += 1;
            continue;
        }
        let esc = bytes[pos + 1];
        pos += 2;
        match esc {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'v' => out.push(0x0b),
            b'f' => out.push(0x0c),
            b'x' => {
                let hex = inner.get(pos..pos + 2)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                pos += 2;
            }
            b'0'..=b'7' => {
                // Up to three octal digits.
                let mut value = u32::from(esc - b'0');
                let end = (pos + 2).min(bytes.len());
                while pos < end && (b'0'..=b'7').contains(&bytes[pos]) {
                    value = value * 8 + u32::from(bytes[pos] - b'0');
                    pos += 1;
                }
                out.push(u8::try_from(value).ok()?);
            }
            other => out.push(other),
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// The directory a dirfd argument names under `-y` (`AT_FDCWD</cwd>`,
/// `5</dir>`), if it carries one.
fn dirfd_path(a: &str) -> Option<PathBuf> {
    let open = a.find('<')?;
    let inner = a[open + 1..].strip_suffix('>')?;
    Some(PathBuf::from(inner))
}

/// `.` and `..` removed lexically (`/a/b/../c` → `/a/c`).
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Where a path argument points: absolute as written, else joined to its
/// dirfd's directory (`dir`: `None` for the plain `open`/`mkdir`/...
/// forms, which name the cwd only implicitly).
fn resolve(dir: Option<&str>, path: &str) -> Result<PathBuf, String> {
    let name = unquote(path).ok_or_else(|| format!("unreadable path argument {path}"))?;
    let p = Path::new(&name);
    if p.is_absolute() {
        return Ok(normalize(p));
    }
    match dir.and_then(dirfd_path) {
        Some(d) => Ok(normalize(&d.join(p))),
        None => Err(format!("relative path {path} with no directory (strace -y?)")),
    }
}

/// The flags of an `open`/`openat` that write: any access mode but
/// `O_RDONLY`, or a flag that creates or truncates.
fn opens_for_write(flags: &str) -> bool {
    flags.split('|').any(|f| {
        matches!(f.trim(), "O_WRONLY" | "O_RDWR" | "O_CREAT" | "O_TRUNC" | "O_APPEND" | "O_TMPFILE")
    })
}

/// The paths `c` writes, as `(dirfd argument, path argument)` pairs, or
/// `None` when it writes none (a read-only open, a non-file call).
fn written_paths(c: &Call) -> Option<Vec<(Option<&str>, &str)>> {
    let a = |i: usize| c.args.get(i).map(String::as_str);
    // (dirfd index, path index) of each written path; `None`: no dirfd.
    let slots: &[(Option<usize>, usize)] = match c.name.as_str() {
        "open" if opens_for_write(a(1)?) => &[(None, 0)],
        "openat" if opens_for_write(a(2)?) => &[(Some(0), 1)],
        // openat2(dirfd, path, {flags=O_..., ...}, size)
        "openat2" => {
            let flags = a(2)?.split("flags=").nth(1)?.split([',', '}']).next()?;
            if !opens_for_write(flags) {
                return None;
            }
            &[(Some(0), 1)]
        }
        "creat" | "mkdir" | "unlink" | "rmdir" | "truncate" | "mknod" => &[(None, 0)],
        "mkdirat" | "unlinkat" | "mknodat" => &[(Some(0), 1)],
        "rename" => &[(None, 0), (None, 1)],
        "renameat" | "renameat2" => &[(Some(0), 1), (Some(2), 3)],
        "link" | "symlink" => &[(None, 1)],
        "linkat" => &[(Some(2), 3)],
        "symlinkat" => &[(Some(1), 2)],
        _ => return None,
    };
    slots.iter().map(|&(d, p)| Some((d.and_then(a), a(p)?))).collect()
}

/// What [`check_log`] found.
#[derive(Debug, Default)]
pub struct Verdict {
    /// Calls read from the log.
    pub calls: usize,
    /// Lines the parser could not read (reported, not failing).
    pub unreadable: usize,
    /// `socket`, `connect`, `bind` calls that break the network rule.
    pub network: Vec<String>,
    /// Writes outside the allowlist, or whose path cannot be resolved.
    pub writes: Vec<String>,
    /// Distinct pids seen.
    pub processes: usize,
}

impl Verdict {
    pub fn passes(&self) -> bool {
        self.calls > 0 && self.network.is_empty() && self.writes.is_empty()
    }

    pub fn render(&self) -> String {
        const SHOWN: usize = 40;
        let mut s = String::new();
        for (what, list) in [("network", &self.network), ("write", &self.writes)] {
            for l in list.iter().take(SHOWN) {
                let _ = writeln!(s, "xtask guard: {what}: {l}");
            }
            if list.len() > SHOWN {
                let _ = writeln!(
                    s,
                    "xtask guard: ... and {} more {what} violations",
                    list.len() - SHOWN
                );
            }
        }
        if self.calls == 0 {
            let _ = writeln!(s, "xtask guard: the log holds no traced call (did strace run?)");
        }
        let _ = writeln!(
            s,
            "xtask guard: {}: {} calls of {} processes, {} network and {} write violations{}",
            if self.passes() { "ok" } else { "FAILED" },
            self.calls,
            self.processes,
            self.network.len(),
            self.writes.len(),
            if self.unreadable > 0 {
                format!(", {} unreadable lines", self.unreadable)
            } else {
                String::new()
            }
        );
        s
    }
}

/// The socket family of a `socket` call's first argument (`AF_INET`,
/// `AF_UNIX`, `AF_LOCAL` read as `AF_UNIX`), or of a `connect`/`bind`
/// address (`{sa_family=AF_INET, ...}`).
fn family(c: &Call) -> Option<String> {
    let f = match c.name.as_str() {
        "socket" => c.args.first()?.trim().to_owned(),
        "connect" | "bind" => {
            let addr = c.args.get(1)?;
            addr.split("sa_family=").nth(1)?.split([',', '}']).next()?.trim().to_owned()
        }
        _ => return None,
    };
    Some(if f == "AF_LOCAL" { "AF_UNIX".into() } else { f })
}

/// Checks a parsed log against the two rules (module docs). A socket call
/// counts whether or not it succeeded (an attempt to use the network is
/// the misbehaviour); a write counts only when it succeeded (a failed one
/// changed nothing).
pub fn check_log(text: &str, allow: &Allow) -> Verdict {
    let (calls, unreadable) = parse_log(text);
    let mut v = Verdict { calls: calls.len(), unreadable, ..Verdict::default() };
    v.processes = calls.iter().map(|c| c.pid).collect::<BTreeSet<_>>().len();
    for c in &calls {
        if let Some(f) = family(c) {
            let bad = match c.name.as_str() {
                "socket" => !ALLOWED_FAMILIES.contains(&f.as_str()),
                _ => f == "AF_INET" || f == "AF_INET6",
            };
            if bad {
                v.network.push(format!("{f}: {}", c.line()));
            }
            continue;
        }
        if c.failed() {
            continue;
        }
        let Some(paths) = written_paths(c) else { continue };
        for (dir, path) in paths {
            match resolve(dir, path) {
                Ok(p) if allow.allows(&p) => {}
                Ok(p) => v.writes.push(format!("{}: {}", p.display(), c.line())),
                Err(e) => v.writes.push(format!("{e}: {}", c.line())),
            }
        }
    }
    v
}

// ---------------------------------------------------------------- risky --

/// What a risky change is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// A new or changed dependency entry of a `Cargo.toml`.
    Dependency,
    /// A new or changed external package in `Cargo.lock`.
    LockedPackage,
    /// A new `build.rs`, or a `build =` key in a manifest's `[package]`.
    BuildScript,
    /// A crate that became a proc-macro (`[lib] proc-macro = true`).
    ProcMacro,
    /// Anything under `.github/` (workflows, actions).
    Workflow,
    /// Anything under `.cargo/` (linker, runner, rustflags, aliases).
    CargoConfig,
}

impl Risk {
    pub fn title(self) -> &'static str {
        match self {
            Risk::Dependency => "dependency",
            Risk::LockedPackage => "locked package",
            Risk::BuildScript => "build script",
            Risk::ProcMacro => "proc-macro crate",
            Risk::Workflow => "CI workflow",
            Risk::CargoConfig => "cargo config",
        }
    }
}

/// One flagged change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Flag {
    pub risk: Risk,
    pub file: String,
    pub detail: String,
}

/// One entry of `git diff --name-status --no-renames`: `A`, `M`, `D`, `T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub status: char,
    pub path: String,
}

/// The flags of `changes`; `base`/`head` read a file at that side (`None`:
/// absent there).
pub fn classify(
    changes: &[Change],
    base: &dyn Fn(&str) -> Option<String>,
    head: &dyn Fn(&str) -> Option<String>,
) -> Vec<Flag> {
    let mut out = Vec::new();
    let mut flag = |risk, file: &str, detail: String| {
        out.push(Flag { risk, file: file.to_owned(), detail });
    };
    for c in changes {
        let p = c.path.as_str();
        let file = p.rsplit('/').next().unwrap_or(p);
        if p.starts_with(".github/") {
            flag(Risk::Workflow, p, format!("{} {p}", verb(c.status)));
        }
        if p.starts_with(".cargo/") {
            flag(Risk::CargoConfig, p, format!("{} {p}", verb(c.status)));
        }
        if c.status == 'D' {
            continue;
        }
        if file == "build.rs" && c.status == 'A' {
            flag(Risk::BuildScript, p, format!("new build script {p}"));
        }
        if file == "Cargo.toml" {
            let old = base(p).map(|t| manifest_entries(&t)).unwrap_or_default();
            let new = head(p).map(|t| manifest_entries(&t)).unwrap_or_default();
            for (key, value) in &new {
                if old.get(key) == Some(value) {
                    continue;
                }
                let how = if old.contains_key(key) { "changed" } else { "new" };
                let (risk, what) = if key == "package.build" {
                    (Risk::BuildScript, format!("{how} build script key `build = {value}`"))
                } else if key == "lib.proc-macro" || key == "lib.proc_macro" {
                    if value != "true" {
                        continue;
                    }
                    (
                        Risk::ProcMacro,
                        "the crate is now a proc-macro (`[lib] proc-macro = true`)".into(),
                    )
                } else {
                    (Risk::Dependency, format!("{how} `{key} = {value}`"))
                };
                flag(risk, p, what);
            }
        }
        if file == "Cargo.lock" {
            let old = base(p).map(|t| lock_packages(&t)).unwrap_or_default();
            for pkg in head(p).map(|t| lock_packages(&t)).unwrap_or_default() {
                if !old.contains(&pkg) {
                    flag(Risk::LockedPackage, p, format!("new or changed package {pkg}"));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn verb(status: char) -> &'static str {
    match status {
        'A' => "adds",
        'D' => "deletes",
        _ => "changes",
    }
}

/// Is `section` (a `[...]` header's name) a dependency table: the three
/// kinds, at top level, under `workspace.` or `target.<cfg>.`, a single
/// dependency's subtable (`dependencies.foo`), or a `patch.<source>` /
/// `replace` table (which substitutes a dependency's source)?
fn dependency_table(section: &str) -> bool {
    const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let parts: Vec<&str> = section.split('.').collect();
    let kind_at = |i: usize| parts.get(i).is_some_and(|k| KINDS.contains(k));
    parts[0] == "patch"
        || parts[0] == "replace"
        || kind_at(0)
        || (parts[0] == "workspace" && kind_at(1))
        || (parts[0] == "target" && parts.iter().skip(1).any(|k| KINDS.contains(k)))
}

/// A `#` comment cut off (outside a string), and the line trimmed.
fn strip_comment(line: &str) -> &str {
    let mut in_str = None;
    for (i, c) in line.char_indices() {
        match (in_str, c) {
            (None, '"' | '\'') => in_str = Some(c),
            (Some(q), _) if c == q => in_str = None,
            (None, '#') => return line[..i].trim(),
            _ => {}
        }
    }
    line.trim()
}

/// The entries of a `Cargo.toml` that [`classify`] compares, as
/// `<section>.<key>` → value with whitespace removed: every entry of a
/// dependency table ([`dependency_table`]), `package.build` and
/// `lib.proc-macro`. A multi-line inline table or array is joined.
pub fn manifest_entries(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = strip_comment(line);
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').replace([' ', '"', '\''], "");
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().trim_matches('"');
        let mut value: String = value.split_whitespace().collect();
        let open = |v: &str| v.matches(['{', '[']).count() > v.matches(['}', ']']).count();
        while open(&value) {
            let Some(more) = lines.next() else { break };
            value.extend(strip_comment(more).split_whitespace());
        }
        let wanted = dependency_table(&section)
            || (section == "package" && key == "build")
            || (section == "lib" && (key == "proc-macro" || key == "proc_macro"));
        if wanted {
            out.insert(format!("{section}.{key}"), value);
        }
    }
    out
}

/// The external packages of a `Cargo.lock` (those with a `source`), each
/// as `name version (source)` with the checksum when there is one. The
/// workspace's own crates have no source and are left out.
pub fn lock_packages(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for block in text.split("[[package]]").skip(1) {
        let field = |k: &str| {
            block.lines().find_map(|l| {
                let (key, v) = l.split_once('=')?;
                (key.trim() == k).then(|| v.trim().trim_matches('"').to_owned())
            })
        };
        let (Some(name), Some(source)) = (field("name"), field("source")) else { continue };
        let version = field("version").unwrap_or_default();
        let sum = field("checksum")
            .map(|c| format!(" sha256:{}", &c[..c.len().min(12)]))
            .unwrap_or_default();
        out.insert(format!("{name} {version} ({source}){sum}"));
    }
    out
}

/// `git diff --name-status --no-renames` output → changes.
pub fn parse_name_status(text: &str) -> Vec<Change> {
    text.lines()
        .filter_map(|l| {
            let (s, p) = l.split_once('\t')?;
            Some(Change { status: s.chars().next()?, path: p.trim().to_owned() })
        })
        .collect()
}

fn git(root: &Path, args: &[&str]) -> Result<String, Usage> {
    let o = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| error(format!("cannot run git: {e}")))?;
    if !o.status.success() {
        return Err(error(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The markdown job summary of `flags`.
pub fn summary(flags: &[Flag], base: &str, head: &str) -> String {
    let mut s = String::from("## Risky changes (`cargo xtask guard risky`)\n\n");
    if flags.is_empty() {
        let _ = writeln!(s, "None between `{base}` and `{head}`.");
        return s;
    }
    let _ = writeln!(
        s,
        "{} change(s) between `{base}` and `{head}` run code at build time or change CI; \
         review them before this merges (spec/harness.md \"Misbehaviour checks\"):\n",
        flags.len()
    );
    for f in flags {
        let _ = writeln!(s, "- **{}** `{}`: {}", f.risk.title(), f.file, f.detail);
    }
    s
}

/// A `::warning::` workflow command for `f` (GitHub's escaping of `%`,
/// CR and LF in the message, and also `,` and `:` in properties).
pub fn annotation(f: &Flag) -> String {
    let msg = |s: &str| s.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A");
    let prop = |s: &str| msg(s).replace(':', "%3A").replace(',', "%2C");
    format!(
        "::warning file={},title={}::{}",
        prop(&f.file),
        prop(&format!("Risky change: {}", f.risk.title())),
        msg(&f.detail)
    )
}

fn risky_main(mut args: impl Iterator<Item = String>) -> Result<bool, Usage> {
    let (mut base, mut head, mut strict) = (None, "HEAD".to_owned(), false);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--base" => base = Some(args.next().ok_or(Usage::Bad("--base <rev>".into()))?),
            "--head" => head = args.next().ok_or(Usage::Bad("--head <rev>".into()))?,
            "--strict" => strict = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(true);
            }
            other => return Err(Usage::Bad(format!("--base <rev>, not `{other}`"))),
        }
    }
    let base = base.ok_or(Usage::Bad("--base <rev>".into()))?;
    let root = crate::util::repo_root();
    let diff = git(&root, &["diff", "--name-status", "--no-renames", &base, &head, "--"])?;
    let changes = parse_name_status(&diff);
    let show = |rev: &str, p: &str| git(&root, &["show", &format!("{rev}:{p}")]).ok();
    let flags = classify(&changes, &|p| show(&base, p), &|p| show(&head, p));
    for f in &flags {
        println!("{}", annotation(f));
    }
    let text = summary(&flags, &base, &head);
    println!("xtask guard: {} changed files, {} flagged", changes.len(), flags.len());
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY").filter(|p| !p.is_empty()) {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| error(format!("cannot open $GITHUB_STEP_SUMMARY: {e}")))?;
        f.write_all(text.as_bytes())
            .map_err(|e| error(format!("cannot write the summary: {e}")))?;
    } else {
        print!("{text}");
    }
    Ok(!strict || flags.is_empty())
}

#[cfg(test)]
mod tests;
