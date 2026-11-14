//! Back-end selection (`-fkiln-backend=llvm|hod`, `KILN_BACKEND`;
//! spec/driver.md "Back-end selection", WP-1371).
//!
//! hod (github.com/christopherfretz/hod) is a safe-Rust `-O0` back end
//! that stands in for `llc`. It is an external executable, like `llc`:
//! kiln has no dependency on it. Under `-fkiln-backend=hod` the driver
//! hands it the module before `opt` (when its `opt` triggers are a
//! subset of {`alwaysinline`, `coro`}: hod's "pre-opt modules") as
//! `hod --strict <the llc arguments> -o <obj>`, and falls back to `opt` +
//! `llc` when hod declines (status 2) or fails. hod's own bugs
//! (`internal-error`, `internal-panic`, a contained panic) are status 1,
//! a failure: a warning and the fallback, an error under
//! `KILN_HOD_STRICT=1`; never a silent decline. The route itself is in
//! `codegen_cmd::execute`; this module selects the back end, finds the
//! executable and reads hod's answer (hod's `spec/cli.md`, "Exit status"
//! and "Strict mode").
use std::path::PathBuf;

use crate::args::Args;
use crate::link::Command;
use crate::toolchain::{Tool, Toolchain};

/// The variable selecting the back end when no `-fkiln-backend=` is given.
pub const ENV_BACKEND: &str = "KILN_BACKEND";
/// The variable naming the hod executable when no `--hod=` is given.
pub const ENV_HOD: &str = "KILN_HOD";
/// `KILN_HOD_STRICT=1`: a hod failure (not a decline) is an error, not a
/// fallback.
pub const ENV_STRICT: &str = "KILN_HOD_STRICT";

/// The back end that turns kiln's IR into objects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// `opt` (when needed) and `llc`: the default.
    #[default]
    Llvm,
    /// hod first, for the cases it can take; LLVM otherwise.
    Hod,
}

impl Backend {
    /// The back end named `v` (`llvm`, `hod`).
    pub fn parse(v: &str) -> Option<Backend> {
        match v {
            "llvm" => Some(Backend::Llvm),
            "hod" => Some(Backend::Hod),
            _ => None,
        }
    }
}

/// The run's back end: `-fkiln-backend=`, else `KILN_BACKEND` (set and
/// not empty), else LLVM.
///
/// # Errors
///
/// `KILN_BACKEND` holds another value: `("KILN_BACKEND=<v>", v)`, as an
/// invalid flag value is reported.
pub fn selected(args: &Args) -> Result<Backend, (String, String)> {
    if let Some(b) = args.backend {
        return Ok(b);
    }
    let Some(v) = std::env::var_os(ENV_BACKEND).filter(|v| !v.is_empty()) else {
        return Ok(Backend::Llvm);
    };
    let v = v.to_string_lossy().into_owned();
    Backend::parse(&v).ok_or_else(|| (format!("{ENV_BACKEND}={v}"), v))
}

/// Whether `KILN_HOD_STRICT=1`.
pub fn strict() -> bool {
    std::env::var_os(ENV_STRICT).is_some_and(|v| v == "1")
}

/// The hod executable: `--hod=<path>`, else `KILN_HOD`, else `hod` next
/// to the `llc` kiln would use, else `hod` on `PATH`. A path named by
/// the flag or the variable is used only if it is a file (no further
/// search).
///
/// # Errors
///
/// None found: what was searched, in order, as the diagnostic shows it.
pub fn find(args: &Args, tc: &Toolchain) -> Result<PathBuf, String> {
    let named =
        args.hod.as_ref().map(|p| (p.clone(), "--hod=")).or_else(|| {
            std::env::var(ENV_HOD).ok().filter(|p| !p.is_empty()).map(|p| (p, ENV_HOD))
        });
    if let Some((p, from)) = named {
        let path = PathBuf::from(&p);
        return if path.is_file() { Ok(path) } else { Err(format!("'{p}' (from {from})")) };
    }
    let mut searched = Vec::new();
    let llc_dir = tc
        .find_tool(Tool::Llc)
        .and_then(|l| l.parent().map(std::path::Path::to_path_buf))
        .or_else(|| tc.llvm_dir().map(|d| d.join("bin")));
    if let Some(dir) = llc_dir {
        let p = dir.join("hod");
        if p.is_file() {
            return Ok(p);
        }
        searched.push(format!("'{}' (next to llc)", p.display()));
    }
    if let Some(p) = std::env::var_os("PATH")
        .and_then(|path| std::env::split_paths(&path).map(|d| d.join("hod")).find(|p| p.is_file()))
    {
        return Ok(p);
    }
    searched.push("PATH".to_owned());
    Err(searched.join(", "))
}

/// The hod command of an object: `hod --strict` then `llc`'s arguments,
/// unchanged (they end with `-o <obj> -`: the module on standard input).
pub fn command(hod: &std::path::Path, llc: &Command) -> Command {
    let mut args = Vec::with_capacity(llc.args.len() + 1);
    args.push("--strict".to_owned());
    args.extend(llc.args.iter().cloned());
    Command { tool: Tool::Hod, program: Some(hod.to_path_buf()), args, missing_llvm: None }
}

/// The `-o` file of a command made by [`command`].
pub fn output(cmd: &Command) -> Option<&str> {
    cmd.args.iter().position(|a| a == "-o").and_then(|i| cmd.args.get(i + 1)).map(String::as_str)
}

/// What hod did with a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Status 0: the object is written.
    Compiled,
    /// Status 2: hod declined the module; the reason it named.
    Declined(String),
    /// Any other status, a signal, or hod could not be run: the status as
    /// the warning shows it (`exit code 1`, `signal 9`, `not started`)
    /// and hod's message (its first standard-error line).
    Failed {
        /// `exit code <n>`, `signal <n>` or `not started`.
        status: String,
        /// The first non-empty line hod wrote on standard error.
        message: String,
    },
}

impl Outcome {
    /// The statistics log's `backend` value.
    pub fn backend(&self) -> &'static str {
        match self {
            Outcome::Compiled => "hod",
            Outcome::Declined(_) => "hod-declined",
            Outcome::Failed { .. } => "hod-failed",
        }
    }

    /// The statistics log's `hod_reason` value: a decline's reason; for
    /// a failure the reason its message names ([`failure_reason`]), else
    /// its status.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Outcome::Compiled => None,
            Outcome::Declined(r) => Some(r),
            Outcome::Failed { status, message } => Some(failure_reason(message).unwrap_or(status)),
        }
    }
}

/// Runs `cmd` (made by [`command`]) with the module `ir` on its standard
/// input; returns the outcome and hod's standard output (the object for
/// `-o -`, else empty). Its standard error is read for the reason.
pub fn run(cmd: &Command, ir: &[u8]) -> (Outcome, Vec<u8>) {
    let not_started =
        |message: String| Outcome::Failed { status: "not started".to_owned(), message };
    let Some(program) = &cmd.program else {
        return (not_started(String::new()), Vec::new());
    };
    let (status, out, err) = match crate::link::spawn_and_wait(program, &cmd.args, Some(ir)) {
        Ok(r) => r,
        Err(e) => return (not_started(e.to_string()), Vec::new()),
    };
    let err = String::from_utf8_lossy(&err);
    let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let outcome = match status.code() {
        Some(0) => Outcome::Compiled,
        Some(2) => Outcome::Declined(reason(line)),
        Some(c) => Outcome::Failed { status: format!("exit code {c}"), message: line.to_owned() },
        None => Outcome::Failed { status: signal_text(status), message: line.to_owned() },
    };
    (outcome, out)
}

#[cfg(unix)]
fn signal_text(status: std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal().map_or_else(|| "signal".to_owned(), |s| format!("signal {s}"))
}

#[cfg(not(unix))]
fn signal_text(_: std::process::ExitStatus) -> String {
    "signal".to_owned()
}

/// The reason in hod's status-2 line `hod: error: unsupported construct
/// '<reason>' (strict mode: no fallback)`; a command line hod rejected
/// (`unknown option '<o>'`, `unknown value ...`) is `unknown-option`, as
/// hod's own coverage log names it; any other text as written.
fn reason(line: &str) -> String {
    let msg = line.strip_prefix("hod: error: ").unwrap_or(line);
    let msg = msg.strip_suffix(" (strict mode: no fallback)").unwrap_or(msg);
    if let Some((r, _)) =
        msg.strip_prefix("unsupported construct '").and_then(|r| r.split_once('\''))
    {
        return r.to_owned();
    }
    if msg.starts_with("unknown option '") || msg.starts_with("unknown value '") {
        return "unknown-option".to_owned();
    }
    msg.to_owned()
}

/// The reason hod's failure line names (WP-1411), by hod's display of
/// its `CompileError` (hod `crates/hod/src/lib.rs`): `hod: error:
/// internal error: hod panicked: ...` is `internal-panic`, `internal
/// error: ...` `internal-error`, `malformed input: ...`
/// `malformed-input`, `invalid options: ...` `invalid-options`. Any
/// other line (usage, I/O, the coverage log): `None`.
pub fn failure_reason(line: &str) -> Option<&'static str> {
    let msg = line.strip_prefix("hod: error: ")?;
    [
        ("internal error: hod panicked: ", "internal-panic"),
        ("internal error: ", "internal-error"),
        ("malformed input: ", "malformed-input"),
        ("invalid options: ", "invalid-options"),
    ]
    .into_iter()
    .find_map(|(p, r)| msg.starts_with(p).then_some(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_of_hod_lines() {
        assert_eq!(
            reason(
                "hod: error: unsupported construct 'alwaysinline:frame-address' \
                 (strict mode: no fallback)"
            ),
            "alwaysinline:frame-address"
        );
        assert_eq!(
            reason("hod: error: unknown option '-foo' (strict mode: no fallback)"),
            "unknown-option"
        );
        assert_eq!(reason("something else"), "something else");
        assert_eq!(reason(""), "");
    }

    #[test]
    fn failure_reasons_of_hod_lines() {
        let r = |l: &str| failure_reason(l);
        assert_eq!(r("hod: error: internal error: function 'f': x"), Some("internal-error"));
        assert_eq!(
            r("hod: error: internal error: hod panicked: oops (strict mode: no fallback)"),
            Some("internal-panic")
        );
        assert_eq!(r("hod: error: malformed input: line 3:4: x"), Some("malformed-input"));
        assert_eq!(r("hod: error: invalid options: x"), Some("invalid-options"));
        assert_eq!(r("hod: error: cannot read '-': x"), None);
        assert_eq!(r("internal error: boom"), None);
        assert_eq!(r(""), None);
        let failed = |m: &str| Outcome::Failed { status: "exit code 1".into(), message: m.into() };
        assert_eq!(failed("hod: error: internal error: x").reason(), Some("internal-error"));
        assert_eq!(failed("").reason(), Some("exit code 1"));
    }

    #[test]
    fn parse_and_command() {
        assert_eq!(Backend::parse("hod"), Some(Backend::Hod));
        assert_eq!(Backend::parse("llvm"), Some(Backend::Llvm));
        assert_eq!(Backend::parse("LLVM"), None);
        let llc = Command {
            tool: Tool::Llc,
            program: Some(PathBuf::from("/x/llc")),
            args: ["-O0", "-filetype=obj", "-o", "a.o", "-"].map(str::to_owned).to_vec(),
            missing_llvm: None,
        };
        let h = command(std::path::Path::new("/y/hod"), &llc);
        assert_eq!(h.args, ["--strict", "-O0", "-filetype=obj", "-o", "a.o", "-"]);
        assert_eq!(h.tool, Tool::Hod);
        assert_eq!(output(&h), Some("a.o"));
    }
}
