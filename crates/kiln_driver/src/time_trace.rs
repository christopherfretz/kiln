//! `-ftime-trace[=<path>]`, `-ftime-trace-granularity=<n>` and
//! `-ftime-trace-verbose` (spec/driver.md "Time trace").
//!
//! The driver decides where each input's trace goes (clang's
//! `handleTimeTrace`), starts the thread's recorder
//! ([`kiln_source::trace`]) around the input's compile, folds in the
//! traces `opt` and `llc` write for their part (`--time-trace`), and
//! writes the JSON when every step of the input succeeded.
use crate::args::{Action, Args};
use crate::codegen_cmd::Phase;
use crate::link::Command;
use crate::toolchain::Tool;
use kiln_source::trace::{self, Span};
use std::time::Instant;

/// The trace settings of a run that writes traces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `-ftime-trace-granularity=` (µs; default 500).
    pub granularity: u64,
    /// `-ftime-trace-verbose`.
    pub verbose: bool,
}

/// Whether this run writes time traces: `-ftime-trace` with an action
/// clang runs a compiler job for (`-c`, `-S`, linking, `-E`, `-M`,
/// `--precompile`, a header precompile), not `-fsyntax-only` (clang:
/// "argument unused").
pub fn writes(args: &Args) -> bool {
    args.time_trace.on
        && matches!(
            args.action,
            Action::Compile | Action::Link | Action::Preprocess | Action::Precompile
        )
}

/// The flags of the family as written when the run writes no trace:
/// clang's `argument unused during compilation` list (granularity and
/// verbose are claimed only with `-ftime-trace`).
pub fn unused(args: &Args) -> &[String] {
    if writes(args) { &[] } else { &args.time_trace.written }
}

/// The settings, when the run writes traces; `Err((flag, value))` for a
/// granularity that is not an unsigned integer (clang's `invalid integral
/// value`).
///
/// # Errors
///
/// The granularity is not a non-negative integer.
pub fn settings(args: &Args) -> Result<Option<Settings>, (String, String)> {
    if !writes(args) {
        return Ok(None);
    }
    let granularity = match &args.time_trace.granularity {
        None => trace::DEFAULT_GRANULARITY_US,
        Some(v) => {
            v.parse::<u64>().map_err(|_| (format!("-ftime-trace-granularity={v}"), v.clone()))?
        }
    };
    Ok(Some(Settings { granularity, verbose: args.time_trace.verbose }))
}

/// `path` with its extension replaced by `json` (LLVM
/// `sys::path::replace_extension`: a dot in a directory name is not an
/// extension; a name with none gets one).
fn json_name(path: &str) -> String {
    let name_at = path.rfind('/').map_or(0, |i| i + 1);
    match path[name_at..].rfind('.') {
        Some(i) if i > 0 || name_at == path.len() => format!("{}.json", &path[..name_at + i]),
        _ => format!("{path}.json"),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Where the trace of `input` goes (clang `handleTimeTrace`):
/// - `-ftime-trace=<dir>` (an existing directory): `<dir>/<output file
///   name>` with the extension `.json`;
/// - `-ftime-trace=<file>`: that file;
/// - `-ftime-trace` when linking: `<-o or a>-<input file name>`, the
///   extension `.json` (clang's implicit `-dumpdir`);
/// - otherwise the output with the extension `.json` (`x.o` -> `x.json`;
///   `-` for `-E` to standard output -> `-.json`).
///
/// `output` is the job's output as the driver names it (the `-o` file,
/// or the default one); with `phase` [`Phase::Link`] it is the object
/// file name the input compiles to.
pub fn path(args: &Args, phase: Option<Phase>, input: &str, output: &str) -> String {
    if let Some(p) = &args.time_trace.path {
        if std::path::Path::new(p).is_dir() {
            let sep = if p.ends_with('/') { "" } else { "/" };
            return format!("{p}{sep}{}", json_name(file_name(output)));
        }
        return p.clone();
    }
    if phase == Some(Phase::Link) {
        let dump = args.output.as_deref().unwrap_or("a");
        return json_name(&format!("{dump}-{}", file_name(input)));
    }
    json_name(output)
}

/// One input's trace while it compiles: the recorder runs from
/// [`InputTrace::start`] to [`InputTrace::finish`].
#[derive(Debug)]
pub struct InputTrace {
    path: String,
    settings: Settings,
    execute: Option<Span>,
    backend: Option<Span>,
    children: Vec<String>,
    /// `-E` / `-M`: clang's preprocessor-only job records no `Source`
    /// events (its `Source` spans come from Sema's callbacks).
    pub no_sources: bool,
    /// A step of the input failed: no trace is written (as clang, whose
    /// failing compiler job writes none).
    pub failed: bool,
}

impl InputTrace {
    /// Starts the recorder and the input's `ExecuteCompiler` span; the
    /// trace will go to `path`.
    pub fn start(settings: Settings, path: String) -> InputTrace {
        trace::start(settings.granularity, settings.verbose);
        let execute = trace::begin_on("ExecuteCompiler");
        InputTrace {
            path,
            settings,
            execute,
            backend: None,
            children: Vec::new(),
            no_sources: false,
            failed: false,
        }
    }

    /// Opens the `Backend` span (the IR passes and code generation after
    /// the frontend).
    pub fn begin_backend(&mut self) {
        if self.backend.is_none() {
            self.backend = trace::begin_on("Backend");
        }
    }

    /// `cmd` asked to write its own trace to `file` when it is `opt` or
    /// `llc` (`--time-trace`); the file is read back at the end. Other
    /// tools run as they are.
    pub fn child(&mut self, cmd: &Command, file: String) -> Command {
        if !matches!(cmd.tool, Tool::Opt | Tool::Llc) {
            return cmd.clone();
        }
        let mut c = cmd.clone();
        c.arg("--time-trace")
            .arg(format!("--time-trace-granularity={}", self.settings.granularity))
            .arg(format!("--time-trace-file={file}"));
        self.children.push(file);
        c
    }

    /// Runs `cmd` (made by [`InputTrace::child`]) inside its clang phase
    /// span: `Optimizer` for `opt`, `CodeGenPasses` for `llc` and hod. The same
    /// two clock readings are the child's time in the compile statistics
    /// log (any tool, by its name; [`trace::phase_add`]).
    pub fn run_child<T>(cmd: &Command, run: impl FnOnce() -> T) -> T {
        // hod (`-fkiln-backend=hod`) stands in for `llc`: the same span,
        // `hod` as its detail. It writes no trace of its own.
        let name = match cmd.tool {
            Tool::Opt => Some("Optimizer"),
            Tool::Llc | Tool::Hod => Some("CodeGenPasses"),
            _ => None,
        };
        let stats = trace::phases_on();
        if name.is_none() && !stats {
            return run();
        }
        let start = Instant::now();
        let r = run();
        let end = Instant::now();
        if let Some(name) = name {
            let hod = cmd.tool == Tool::Hod;
            trace::complete(
                name,
                start,
                end,
                || if hod { "hod".to_owned() } else { String::new() },
            );
        }
        trace::phase_add(cmd.tool.name(), start, end);
        r
    }

    /// Ends the spans, stops the recorder and, when nothing failed,
    /// writes the trace (merging the children's).
    ///
    /// # Errors
    ///
    /// The trace file could not be written: its path and the error.
    pub fn finish(mut self, program: &str) -> Result<(), (String, String)> {
        if let Some(b) = self.backend.take() {
            trace::end(b, String::new);
        }
        if let Some(e) = self.execute.take() {
            trace::end(e, String::new);
        }
        let Some(mut t) = trace::finish() else { return Ok(()) };
        if self.failed {
            return Ok(());
        }
        if self.no_sources {
            t.events.retain(|e| e.name != "Source");
            t.totals.retain(|e| e.name != "Source");
        }
        for c in &self.children {
            // A child that wrote nothing (or garbage) loses its events,
            // not the trace.
            if let Ok(text) = std::fs::read_to_string(c) {
                let _ = t.merge_child(&text);
            }
        }
        let thread = crate::args::program_basename(program);
        let json = t.to_json(std::process::id(), "kiln", thread);
        std::fs::write(&self.path, json).map_err(|e| (self.path.clone(), e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Args {
        Args::parse(&v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn json_names_replace_the_extension() {
        assert_eq!(json_name("x.o"), "x.json");
        assert_eq!(json_name("out/o1.o"), "out/o1.json");
        assert_eq!(json_name("a.out-x.cpp"), "a.out-x.json");
        assert_eq!(json_name("-"), "-.json");
        assert_eq!(json_name("d.x/obj"), "d.x/obj.json");
        assert_eq!(json_name(".hidden"), ".hidden.json");
    }

    #[test]
    fn paths_follow_clang() {
        let a = args(&["-c", "-ftime-trace", "x.cpp"]);
        assert_eq!(path(&a, Some(Phase::Object), "x.cpp", "x.o"), "x.json");
        let a = args(&["-c", "-ftime-trace", "src/x.cpp", "-o", "out/o1.o"]);
        assert_eq!(path(&a, Some(Phase::Object), "src/x.cpp", "out/o1.o"), "out/o1.json");
        let a = args(&["-ftime-trace", "src/x.cpp", "-o", "out/a.out"]);
        assert_eq!(path(&a, Some(Phase::Link), "src/x.cpp", "x.o"), "out/a.out-x.json");
        let a = args(&["-ftime-trace", "x.cpp"]);
        assert_eq!(path(&a, Some(Phase::Link), "x.cpp", "x.o"), "a-x.json");
        let a = args(&["-c", "-ftime-trace=t/f.json", "x.cpp"]);
        assert_eq!(path(&a, Some(Phase::Object), "x.cpp", "x.o"), "t/f.json");
        let a = args(&["-E", "-ftime-trace", "x.cpp"]);
        assert_eq!(path(&a, None, "x.cpp", "-"), "-.json");
    }

    #[test]
    fn unused_without_a_compiler_job() {
        let a = args(&["-fsyntax-only", "-ftime-trace", "-ftime-trace-verbose", "x.cpp"]);
        assert!(!writes(&a));
        assert_eq!(unused(&a), ["-ftime-trace", "-ftime-trace-verbose"]);
        let a = args(&["--precompile", "-ftime-trace", "m.cppm"]);
        assert!(writes(&a));
        let a = args(&["-c", "-ftime-trace-granularity=3", "x.cpp"]);
        assert_eq!(unused(&a), ["-ftime-trace-granularity=3"]);
        let a = args(&["-c", "-ftime-trace", "-ftime-trace-granularity=3", "x.cpp"]);
        assert!(unused(&a).is_empty());
        assert_eq!(settings(&a), Ok(Some(Settings { granularity: 3, verbose: false })));
        let a = args(&["-c", "-ftime-trace", "-ftime-trace-granularity=-5", "x.cpp"]);
        assert_eq!(settings(&a), Err(("-ftime-trace-granularity=-5".into(), "-5".into())));
    }
}
