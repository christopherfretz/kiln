//! hod as kiln's in-process back end (spec/driver.md "In-process back
//! end"; hod's design 0004, package K4, hod's half).
//!
//! [`Hod`] implements `kiln_driver`'s backend hook
//! ([`kiln_driver::backend_hook::InProcessBackend`]) over hod's library
//! facade, and [`register`] makes it the process's back end: the
//! `kiln-hod-driver` binary calls it and then runs the stock driver
//! ([`kiln_driver::entry::main`]). Under `-fkiln-backend=hod` the driver
//! then hands each module the in-process route takes to [`Hod`] before it
//! is printed, in place of running an external `hod` executable.
//!
//! This first form prints the module (`kiln_llvm::Module::to_text`, the
//! dialect the driver would have printed: the module's own
//! `ir_version`) and compiles the text in memory with `hod::compile_object`,
//! the function the `hod` executable compiles with, so that the object is
//! the external route's byte for byte. The bridge that builds `hod_ir`
//! from the module without the print and the parse (K3a) will live in
//! this crate too.
//!
//! What the `hod` executable reads besides the module comes from the same
//! place: the `llc` arguments through hod's own parser
//! (`hod::Options::from_llc_args`), threads, census and the coverage log
//! from the environment through `hod::Environment` (read once per
//! process). The object is appended to the driver's buffer; the driver
//! writes it. The back end opens a file only where the `hod` executable
//! would: the coverage log when `HOD_COVERAGE_LOG` names one, the make
//! jobserver's fifo when `MAKEFLAGS` names one (hod's own code), and the
//! `.dwo` file of `-split-dwarf-output` (hod's own code, as `llc`).
#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::sync::OnceLock;
use std::time::Instant;

use hod::coverage::{Log, Outcome, Record};
use hod::{CompileError, Environment, InputKind, Options, Report, Settings};
use kiln_driver::backend_hook::{Answer, InProcessBackend};

/// The hod revision this workspace pins (tag [`HOD_TAG`]): the `rev` of
/// `kiln_hod/Cargo.toml`, which a test checks against the lock.
pub const HOD_REV: &str = "d789a92ab0513dc330124645bd28d8f62532c23c";

/// The tag of [`HOD_REV`].
pub const HOD_TAG: &str = "v1.1.0";

/// The version of hod's producer surface this crate is written against
/// (hod's `spec/cli.md` "Embedding"): a pin that moves to a hod with
/// another value fails to build here, never silently (design 0004
/// section 10, answer 8).
pub const EMBED_API: u32 = 1;

const _: () = assert!(
    hod::EMBED_API == EMBED_API,
    "hod's EMBED_API changed: review kiln_hod against hod's spec/cli.md \"Embedding\""
);

/// The `--version` / `-v` line of the wrapper.
pub fn version_line() -> String {
    format!("In-process back end: hod {HOD_TAG} ({})", &HOD_REV[..12])
}

/// Registers [`Hod`] as the process's in-process back end and the
/// wrapper's `--version` line. Once, before the driver runs.
///
/// # Errors
///
/// A back end (or a version line) was registered already.
pub fn register() -> Result<(), &'static str> {
    kiln_driver::backend_hook::register(Box::new(Hod::default()))
        .map_err(|_| "an in-process back end is registered already")?;
    kiln_driver::version::set_built_in_backend(version_line())
        .map_err(|_| "a built-in back end's version line is set already")
}

/// What hod reads from the process's environment, once per process
/// (hod's `spec/cli.md` "The command line in the facade").
#[derive(Debug)]
struct Process {
    env: Environment,
    /// `env.settings()`: threads (`HOD_THREADS`, else the make jobserver
    /// of `MAKEFLAGS`, else one) and census (`HOD_CENSUS=1`), or the
    /// variable hod does not accept.
    settings: Result<Settings, String>,
}

/// hod, in process.
#[derive(Debug, Default)]
pub struct Hod {
    process: OnceLock<Process>,
}

impl Hod {
    fn process(&self) -> &Process {
        self.process.get_or_init(|| {
            let env = Environment::from_process();
            let settings = env.settings().map_err(|e| e.to_string());
            Process { env, settings }
        })
    }
}

/// The value of `-o` in `llc`'s arguments, as the coverage log names the
/// output (`-` when none).
fn output_of(llc_args: &[String]) -> String {
    llc_args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| llc_args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "-".to_owned())
}

/// The reason name of a compile error (hod's `spec/cli.md` "Reasons"), as
/// the `hod` executable's coverage line writes it.
fn reason_of(error: &CompileError) -> Cow<'static, str> {
    match error {
        CompileError::Unsupported(u) => u.construct.name().to_owned().into(),
        CompileError::MalformedInput(_) => "malformed-input".into(),
        CompileError::InvalidOptions(_) => "invalid-options".into(),
        CompileError::Internal(_) => hod::INTERNAL_ERROR.into(),
        CompileError::Panic(_) => hod::INTERNAL_PANIC.into(),
    }
}

impl InProcessBackend for Hod {
    fn compile(
        &self,
        module: &kiln_llvm::Module,
        llc_args: &[String],
        object: &mut Vec<u8>,
    ) -> Answer {
        let start = Instant::now();
        let process = self.process();
        let mut record = Record {
            outcome: Outcome::Error,
            reason: None,
            input_kind: None,
            input_bytes: None,
            output: output_of(llc_args),
            target: None,
            hod_time: std::time::Duration::ZERO,
            fallback_time: None,
            conversion_time: None,
            fallback_status: None,
            reasons: None,
        };
        let answer = compile(process, module, llc_args, object, &mut record);
        record.hod_time = start.elapsed();
        // The coverage log, as the `hod` executable writes it (its input
        // kind `module`: in process). A log that cannot be written is the
        // executable's status 1: a failure, whatever the answer was.
        if let Some(path) = &process.env.coverage_log {
            let written = Log::open(path).and_then(|mut log| log.append(&record));
            if let Err(e) = written {
                object.clear();
                return Answer::Failed(format!(
                    "cannot write coverage log '{}': {e}",
                    path.display()
                ));
            }
        }
        answer
    }
}

/// One compile: the answer, with `record` filled as the `hod` executable
/// fills its coverage line (strict mode: a decline is an `error` line).
fn compile(
    process: &Process,
    module: &kiln_llvm::Module,
    llc_args: &[String],
    object: &mut Vec<u8>,
    record: &mut Record,
) -> Answer {
    let fail = |record: &mut Record, reason: Cow<'static, str>, message: String| {
        record.reason = Some(reason);
        Answer::Failed(message)
    };
    let settings = match &process.settings {
        Ok(s) => s,
        Err(message) => return fail(record, "usage".into(), message.clone()),
    };
    // hod's own parser over `llc`'s arguments. An option hod does not
    // know is a decline (`unknown-option`: `llc` may know it), as the
    // executable answers it in strict mode; any other mistake an error.
    let mut options = match Options::from_llc_args(llc_args) {
        Ok(o) => o,
        Err(e) => {
            let reason = e.reason();
            record.reason = Some(reason.into());
            return if reason == "unknown-option" {
                Answer::Declined(reason.to_owned())
            } else {
                Answer::Failed(e.to_string())
            };
        }
    };
    // As the executable: `HOD_COMPRESS_DEBUG_SECTIONS` when the command
    // line does not say (kiln's own `-gz` runs `llvm-objcopy` after).
    options.debug_compression = process.env.compress_debug_sections.unwrap_or_default();
    record.target.clone_from(&options.triple);

    let text = module.to_text().into_bytes();
    // The record is the executable's for the same text (kind `text`, its
    // size): hod reads exactly the bytes `llc` would have (`module` is
    // hod's kind for a module built in process, which this is not).
    record.input_kind = Some(InputKind::Text);
    record.input_bytes = Some(text.len());
    if record.target.is_none() {
        record.target = hod::effective_triple(&text, &options).map(str::to_owned);
    }
    let mut report = Report::default();
    let compiled = hod::compile_object(&mut Cow::Owned(text), &options, settings, &mut report);
    if report.target.is_some() {
        record.target = report.target;
    }
    if settings.census {
        record.reasons = Some(report.reasons);
    }
    let error = match compiled.map(|file| file.write_to(object)) {
        Ok(Ok(Ok(_))) => {
            record.outcome = Outcome::Hod;
            return Answer::Done;
        }
        // Writing into a vector does not fail; if it ever did, it is the
        // executable's `output-unwritable`.
        Ok(Ok(Err(e))) => {
            object.clear();
            return fail(record, "output-unwritable".into(), format!("cannot write object: {e}"));
        }
        Ok(Err(e)) | Err(e) => e,
    };
    object.clear();
    let reason = reason_of(&error);
    record.reason = Some(reason.clone());
    match error {
        CompileError::Unsupported(_) => Answer::Declined(reason.into_owned()),
        // `internal error: ...`, `internal error: hod panicked: ...`,
        // `malformed input: ...`, `invalid options: ...`: the driver
        // reads the reason back from the display, as from the
        // executable's line.
        other => Answer::Failed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names() {
        let a = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(output_of(&a(&["-O0", "-o", "x.o", "-"])), "x.o");
        assert_eq!(output_of(&a(&["-O0", "-"])), "-");
    }

    /// The pin moves in one commit: the manifest's `rev`, the lock and
    /// [`HOD_REV`] agree.
    #[test]
    fn the_pin_is_the_lock() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(manifest.contains(&format!("rev = \"{HOD_REV}\"")), "Cargo.toml's hod rev");
        let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
        let sources: Vec<&str> =
            lock.lines().filter(|l| l.starts_with("source = \"git+")).collect();
        assert!(!sources.is_empty(), "no git source in Cargo.lock");
        for s in sources {
            assert!(s.ends_with(&format!("#{HOD_REV}\"")), "Cargo.lock: {s}");
        }
    }
}
