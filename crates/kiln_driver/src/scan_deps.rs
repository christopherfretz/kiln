//! `kiln-scan-deps`: the module dependency scanner build systems run
//! before compiling (`CMake`'s `FILE_SET CXX_MODULES`), a mode of the
//! `kiln` binary picked by the program name, as `clang-scan-deps`
//! behaves (spec/modules.md §11.9, spec/driver.md "Dependency
//! scanning").
//!
//! `kiln-scan-deps [-format=p1689|make|experimental-full] [-o <file>]
//! -- <compiler> <args>` preprocesses the one input of the compile
//! command exactly as the compile would (same search list, predefines,
//! feature answers), with the module directives recorded instead of
//! imported, then prints the P1689r5 JSON (a make rule, or the
//! per-translation-unit JSON) and writes the compile command's
//! `-MD`/`-MF`/`-MT` dependency file. `-compilation-database <file>`
//! scans every command of a JSON compilation database instead, each in
//! its entry's directory, into one document.

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions};
use kiln_lex::Interner;
use kiln_lex::TokKind;
use kiln_pp::bmi::UnitKind;
use kiln_source::{SourceManager, Span};

use crate::args::{Args, Lang};
use crate::pp_cmd::Job;
use crate::{Io, SharedWriter, deps, make_engine};

#[path = "scan_deps_cdb.rs"]
mod cdb;

/// The program names that run the scanner: `kiln-scan-deps`, and the
/// aliases `CMake`'s `find_program` looks for beside the compiler
/// (`clang-scan-deps`, `clang-scan-deps-<major>[.<minor>]`).
#[must_use]
pub fn is_scanner_name(prog: &str) -> bool {
    if prog == "kiln-scan-deps" || prog == "clang-scan-deps" {
        return true;
    }
    prog.strip_prefix("clang-scan-deps-")
        .is_some_and(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

/// The output format (`-format=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    /// A make rule (clang-scan-deps's default).
    Make,
    /// P1689r5 JSON.
    P1689,
    /// clang-scan-deps's `experimental-full` JSON: one object per
    /// translation unit with its command, file and named-module deps.
    Full,
}

/// Which clang-scan-deps a program name stands for: under
/// `clang-scan-deps-19[.<minor>]` (what `CMake` finds beside a compiler
/// that says `__clang_major__ == 19`, `-fclang-compat=19`) the scanner
/// prints clang 19's error header and `experimental-full` shape; under
/// every other name clang 22's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavour {
    V19,
    V22,
}

impl Flavour {
    fn of(prog: &str) -> Flavour {
        match prog.strip_prefix("clang-scan-deps-19") {
            Some(rest) if rest.is_empty() || rest.starts_with('.') => Flavour::V19,
            _ => Flavour::V22,
        }
    }
}

/// The scanner's own options (before `--`).
struct ScanOpts {
    format: Format,
    output: Option<String>,
    /// `-compilation-database <file>`.
    database: Option<String>,
}

/// Parses the options before `--`; `Err` is the message to print.
fn parse_opts(prog: &str, opts: &[String]) -> Result<ScanOpts, String> {
    let mut parsed = ScanOpts { format: Format::Make, output: None, database: None };
    let mut i = 0;
    while i < opts.len() {
        let arg = opts[i].as_str();
        i += 1;
        let flag = arg.strip_prefix("--").map_or(arg, |r| &arg[arg.len() - r.len() - 1..]);
        let (name, inline) = match flag.split_once('=') {
            Some((key, val)) => (key, Some(val)),
            None => (flag, None),
        };
        let value = |i: &mut usize| -> Result<String, String> {
            if let Some(v) = inline {
                return Ok(v.to_owned());
            }
            let v = opts
                .get(*i)
                .cloned()
                .ok_or_else(|| format!("{prog}: for the {name} option: requires a value!"))?;
            *i += 1;
            Ok(v)
        };
        match name {
            "-format" => {
                parsed.format = match value(&mut i)?.as_str() {
                    "make" => Format::Make,
                    "p1689" => Format::P1689,
                    "experimental-full" => Format::Full,
                    v => {
                        return Err(format!(
                            "{prog}: for the --format option: Cannot find option named '{v}'"
                        ));
                    }
                }
            }
            "-o" => parsed.output = Some(value(&mut i)?),
            "-compilation-database" => parsed.database = Some(value(&mut i)?),
            // Scheduling and caching knobs of clang's scanner: kiln scans
            // the commands in order on one thread (documented no-ops).
            "-j" | "-mode" | "-resource-dir-recipe" => drop(value(&mut i)?),
            "-optimize-args"
            | "-eager-load-pcm"
            | "-reuse-filemanager"
            | "-skip-excluded-pp-ranges"
            | "-brief-result"
            | "-round-trip-args" => {}
            _ if arg.starts_with("-o") && arg.len() > 2 && inline.is_none() => {
                parsed.output = Some(arg[2..].to_owned());
            }
            _ => {
                return Err(format!(
                    "{prog}: Unknown command line argument '{arg}'.  Try: '{prog} --help'"
                ));
            }
        }
    }
    Ok(parsed)
}

const HELP: &str = "\
OVERVIEW: kiln-scan-deps, kiln's module dependency scanner (clang-scan-deps compatible)

USAGE: kiln-scan-deps [options] -- <compiler> <compile arguments>
       kiln-scan-deps [options] -compilation-database <compile_commands.json>

  -format=<p1689|make|experimental-full>
                        P1689r5 JSON, a make rule (the default), or
                        clang-scan-deps's per-translation-unit JSON
  -compilation-database <file>
                        scan every command of a JSON compilation database
  -o <file>             write the result to <file> instead of standard output

See spec/modules.md section 11.9.
";

/// A [`Write`]r that keeps what it is given (the diagnostics, printed
/// under clang-scan-deps's header once the scan is over).
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// What the scan of the input found.
struct Scanned {
    /// The main unit's module declaration (see
    /// [`kiln_pp::Preprocessor::main_module_unit`]).
    unit: Option<(String, Option<UnitKind>)>,
    /// The named modules it imports.
    requires: Vec<String>,
    /// The `-MD` dependency list, when the command asks for one.
    deps: Option<crate::pp_cmd::SharedDeps>,
}

/// One compile command to scan: the one after `--`, or an entry of the
/// compilation database.
struct Command {
    /// The entry's `directory` (`None` after `--`: the working directory).
    directory: Option<String>,
    /// The entry's `file` (`None` after `--`: the command's input).
    file: Option<String>,
    /// The entry's `output`.
    output: Option<String>,
    /// The command line; `[0]` is the compiler.
    words: Vec<String>,
}

/// A P1689 rule (one per scanned command).
struct Rule {
    primary: String,
    /// The module the unit provides: name, `is-interface`, source path.
    provides: Option<(String, bool, String)>,
    /// The imported modules, each with the source path of the rule that
    /// provides it, when one does.
    requires: Vec<(String, Option<String>)>,
}

/// One `experimental-full` command (one per translation unit).
struct FullCommand {
    context_hash: String,
    named_module: Option<String>,
    module_deps: Vec<String>,
    command_line: Vec<String>,
    executable: String,
    file_deps: Vec<String>,
    input_file: String,
}

/// The results of the scanned commands, in input order.
#[derive(Default)]
struct Results {
    make: String,
    rules: Vec<Rule>,
    /// `None` for a command whose scan failed (`"commands": []`).
    units: Vec<Option<FullCommand>>,
}

/// Runs the scanner: `words` is its argv without the program name.
/// Returns the exit code: 0, or 1 after any error (clang-scan-deps's
/// convention).
pub(crate) fn run(argv0: &str, words: &[String], mut io: Io) -> i32 {
    let prog = crate::args::program_basename(argv0);
    let flavour = Flavour::of(prog);
    let (opts, cmd) = match words.iter().position(|a| a == "--") {
        Some(i) => (&words[..i], &words[i + 1..]),
        None => (words, &[][..]),
    };
    if opts.iter().any(|a| a == "-help" || a == "--help") {
        let _ = io.stdout.write_all(HELP.as_bytes());
        return 0;
    }
    let opts = match parse_opts(prog, opts) {
        Ok(o) => o,
        Err(msg) => {
            let _ = writeln!(io.stderr, "{msg}");
            return 1;
        }
    };
    let commands = match (&opts.database, cmd.is_empty()) {
        (Some(path), true) => match cdb::load(path) {
            Ok(entries) => entries
                .into_iter()
                .map(|e| Command {
                    directory: Some(e.directory),
                    file: Some(e.file),
                    output: e.output,
                    words: e.command,
                })
                .collect(),
            Err(msg) => {
                let _ = writeln!(io.stderr, "{msg}");
                return 1;
            }
        },
        (None, false) => {
            vec![Command { directory: None, file: None, output: None, words: cmd.to_vec() }]
        }
        _ => {
            let _ = writeln!(
                io.stderr,
                "The compilation command line must be provided either via \
                 '-compilation-database' or after '--'."
            );
            return 1;
        }
    };

    // Each database entry is scanned in its own directory; the scanner
    // is one thread of a process of its own, so it changes the process's
    // working directory, and restores it before writing its result.
    let cwd = std::env::current_dir().ok();
    let mut results = Results::default();
    let mut rc = 0;
    for c in &commands {
        if !scan_command(c, opts.format, flavour, cwd.as_deref(), &mut io, &mut results) {
            rc = 1;
        }
    }
    if let Some(dir) = &cwd
        && opts.database.is_some()
    {
        let _ = std::env::set_current_dir(dir);
    }

    let out = match opts.format {
        Format::Make => std::mem::take(&mut results.make),
        Format::P1689 => p1689(results.rules),
        Format::Full => full(&results.units, flavour),
    };
    if !out.is_empty() && !write_or_report(opts.output.as_deref(), &out, &mut io, prog) {
        rc = 1;
    }
    rc
}

/// Scans one command into `results`; false after an error (reported).
fn scan_command(
    c: &Command,
    format: Format,
    flavour: Flavour,
    cwd: Option<&std::path::Path>,
    io: &mut Io,
    results: &mut Results,
) -> bool {
    // The base of the absolute `file-deps` of the full format: clang's
    // working directory, `.` after `--`.
    let dir = c.directory.as_deref().unwrap_or(".");
    let base = cwd.map_or_else(|| std::path::PathBuf::from(dir), |w| w.join(dir));
    let fail = |results: &mut Results| {
        if format == Format::Full {
            results.units.push(None);
        }
        false
    };
    if c.directory.is_some()
        && let Err(e) = std::env::set_current_dir(&base)
    {
        let name = c.file.as_deref().unwrap_or_default();
        let msg = e.to_string();
        let msg = msg.split(" (os error").next().unwrap_or(&msg);
        header(io, flavour, name, &format!("error: cannot change to directory '{dir}': {msg}\n"));
        return fail(results);
    }
    let (compiler, cargs) = match c.words.split_first() {
        Some((compiler, rest)) => (compiler.as_str(), rest),
        None => ("", &[][..]),
    };
    let mut args = Args::parse_as(compiler, &crate::rsp::expand(cargs));
    args.scan_deps = true;
    // The make and full formats list every header whatever the command
    // asks.
    let side_file = args.deps.side.is_some();
    if format != Format::P1689 && args.deps.headers().is_none() {
        args.deps.side = Some(crate::args::DepHeaders::All);
    }
    let input = args.inputs.first().map_or_else(String::new, |i| i.path.clone());
    let name = c.file.clone().unwrap_or_else(|| input.clone());

    let capture = Capture::default();
    let diags = SharedWriter::new(capture.clone());
    let scanned = scan(&args, &c.words, diags);
    let text = String::from_utf8_lossy(&capture.0.lock().unwrap_or_else(PoisonError::into_inner))
        .into_owned();
    header(io, flavour, &name, &text);
    let Some(s) = scanned else {
        return fail(results);
    };
    let files = || s.deps.as_ref().map(|d| d.borrow().files().to_vec()).unwrap_or_default();
    match format {
        Format::Make => {
            // A database entry's files are named from its directory.
            let files: Vec<String> = match &c.directory {
                Some(d) => files().iter().map(|f| join(d, f)).collect(),
                None => files(),
            };
            results.make.push_str(&deps::deps_text(&args, &input, &files));
        }
        Format::P1689 => {
            let primary = if c.directory.is_some() {
                c.output.clone().unwrap_or_default()
            } else {
                args.output.clone().unwrap_or_else(|| deps::default_target(&input))
            };
            let provides = match &s.unit {
                Some((n, Some(kind))) => {
                    Some((n.clone(), *kind != UnitKind::ImplementationPartition, name.clone()))
                }
                _ => None,
            };
            let requires = s.requires.iter().map(|r| (r.clone(), None)).collect();
            results.rules.push(Rule { primary, provides, requires });
        }
        Format::Full => {
            let base = base.to_string_lossy();
            let input_paths: Vec<&str> = args.inputs.iter().map(|i| i.path.as_str()).collect();
            results.units.push(Some(FullCommand {
                context_hash: context_hash(cargs, &input_paths),
                named_module: match &s.unit {
                    Some((n, Some(_))) => Some(n.clone()),
                    _ => None,
                },
                module_deps: s.requires.clone(),
                command_line: cargs.to_vec(),
                executable: resolve_executable(compiler),
                file_deps: files().iter().map(|f| join(&base, f)).collect(),
                input_file: name.clone(),
            }));
        }
    }
    // The compile command's own `-MD` file, after a clean scan (clang
    // writes none after an error).
    if let Some(list) = &s.deps
        && side_file
    {
        let opts = DiagOptions {
            clang_compat: args.predef.clang_compat.major(),
            ..DiagOptions::default()
        };
        if !deps::write_side_file(&args, &input, &list.borrow(), &opts, io) {
            return false;
        }
    }
    true
}

/// Prints the diagnostics `text` of the scan of `name` (if any) under
/// the header of the clang-scan-deps `flavour` stands for.
fn header(io: &mut Io, flavour: Flavour, name: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    let _ = match flavour {
        Flavour::V19 => write!(io.stderr, "Error while scanning dependencies for {name}:\n{text}"),
        Flavour::V22 => {
            write!(io.stderr, "Diagnostics while scanning dependencies for '{name}':\n{text}")
        }
    };
}

/// `path` named from `dir` (unchanged when absolute).
fn join(dir: &str, path: &str) -> String {
    if path.starts_with('/') || dir.is_empty() {
        path.to_owned()
    } else if dir.ends_with('/') {
        format!("{dir}{path}")
    } else {
        format!("{dir}/{path}")
    }
}

/// The compiler of a command as clang-scan-deps reports it: a bare name
/// looked up on `PATH`, anything else as written.
fn resolve_executable(prog: &str) -> String {
    if prog.is_empty() || prog.contains('/') {
        return prog.to_owned();
    }
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join(prog)).find(|c| c.is_file()))
        .map_or_else(|| prog.to_owned(), |p| p.to_string_lossy().into_owned())
}

/// The `clang-context-hash` of a command: commands that differ only in
/// their input, output and dependency-file options share it (as clang's
/// do), rendered in base 36 as clang renders its hash. FNV-1a over the
/// kiln version and the remaining arguments.
fn context_hash(args: &[String], inputs: &[&str]) -> String {
    const VALUED: [&str; 4] = ["-o", "-MF", "-MT", "-MQ"];
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |s: &str| {
        for b in s.bytes().chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(crate::version::VERSION);
    let mut it = args.iter();
    while let Some(w) = it.next() {
        let w = w.as_str();
        if VALUED.contains(&w) {
            it.next();
        } else if !(w == "-MD"
            || w == "-MMD"
            || inputs.contains(&w)
            || VALUED.iter().any(|p| w.len() > p.len() && w.starts_with(p)))
        {
            feed(w);
        }
    }
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut out = Vec::new();
    loop {
        out.push(char::from(digits[usize::try_from(h % 36).unwrap_or(0)]));
        h /= 36;
        if h == 0 {
            break;
        }
    }
    out.iter().rev().collect()
}

/// Writes `text` to `path` (standard output without one); false after
/// reporting an error.
fn write_or_report(path: Option<&str>, text: &str, io: &mut Io, prog: &str) -> bool {
    match crate::write_output(path, text, &mut io.stdout) {
        Ok(()) => true,
        Err(e) => {
            let _ =
                writeln!(io.stderr, "{prog}: error: cannot open '{}': {e}", path.unwrap_or("-"));
            false
        }
    }
}

/// Preprocesses the input of `args` with modules recorded; `None` after
/// reporting an error to `diags`.
fn scan(args: &Args, cmd: &[String], diags: SharedWriter) -> Option<Scanned> {
    let mut diag_opts = DiagOptions::from_args(&args.diag_args);
    diag_opts.clang_compat = args.predef.clang_compat.major();
    // clang-scan-deps reports errors only (its scanner drops `#warning`).
    diag_opts.ignore_warnings = true;
    // Nor does it print the source line of a diagnostic.
    diag_opts.show_carets = false;
    let dsm = SourceManager::new();
    let drv = make_engine(&dsm, &diag_opts, diags.clone(), None);
    for e in &args.errors {
        crate::emit_arg_error(e, &drv);
    }
    let tc = crate::toolchain_of(args, &drv);
    let inputs: Vec<_> = args
        .inputs
        .iter()
        .filter(|i| {
            let ok = i.path == "-" || std::path::Path::new(&i.path).exists();
            if !ok {
                Diag::new(DiagId::drv_no_such_file, Span::INVALID).arg(&i.path).emit(&drv);
            }
            ok
        })
        .collect();
    match inputs.as_slice() {
        [_] if args.inputs.len() == 1 => {}
        _ => {
            if inputs.is_empty() {
                Diag::new(DiagId::drv_no_input_files, Span::INVALID).emit(&drv);
            }
            Diag::new(DiagId::drv_scan_one_job, Span::INVALID).arg(cmd.join(" ")).emit(&drv);
        }
    }
    let lang = inputs.first().map_or(Lang::Cxx, |i| args.input_lang(i));
    let std = if drv.error_count() == 0 {
        match crate::input_std(args, lang) {
            Ok(s) => Some(s),
            Err(d) => {
                (*d).emit(&drv);
                None
            }
        }
    } else {
        None
    };
    drv.flush();
    let std = std.filter(|_| drv.error_count() == 0)?;
    drop(drv);
    let input = inputs[0];
    let stdin_text = (input.path == "-").then(|| {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut s);
        s
    });
    let job = Job {
        args,
        toolchain: &tc,
        std,
        path: &input.path,
        stdin_text,
        diag_opts: &diag_opts,
        stderr: diags.clone(),
    };
    let sm = SourceManager::new();
    let interner = Interner::new();
    let engine = make_engine(&sm, &diag_opts, diags, None);
    let found = crate::crash::catching(&input.path, job.stdin_text.as_deref(), &sm, || {
        drain(&job, &sm, &engine, &interner)
    });
    engine.flush();
    found.filter(|_| engine.error_count() == 0)
}

/// Runs the preprocessor of `job` to the end of its input.
fn drain<'a>(
    job: &Job<'_>,
    sm: &'a SourceManager,
    engine: &DiagEngine<'a>,
    interner: &'a Interner,
) -> Option<Scanned> {
    let inputs = job.pp_inputs(sm, engine)?;
    let (mut pp, deps) = job.build_preprocessor(sm, engine, interner, inputs, true)?;
    while pp.next().kind != TokKind::Eof {}
    Some(Scanned { unit: pp.main_module_unit(), requires: pp.scanned_imports().to_vec(), deps })
}

/// The P1689r5 document of the scanned commands, exactly as
/// clang-scan-deps prints it (`llvm::json`, two-space indent, keys
/// sorted): the rules sorted by primary output (stably: ties keep the
/// command order), each import given the source path of the rule that
/// provides it.
fn p1689(mut rules: Vec<Rule>) -> String {
    let sources: std::collections::HashMap<String, String> = rules
        .iter()
        .filter_map(|r| r.provides.as_ref())
        .filter(|(_, _, path)| !path.is_empty())
        .map(|(name, _, path)| (name.clone(), path.clone()))
        .collect();
    for r in &mut rules {
        for (name, path) in &mut r.requires {
            path.clone_from(&sources.get(name).cloned());
        }
    }
    rules.sort_by(|a, b| a.primary.cmp(&b.primary));
    let mut s = String::from("{\n  \"revision\": 0,\n  \"rules\": [");
    for (i, r) in rules.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str("    {\n      \"primary-output\": ");
        s.push_str(&json_quote(&r.primary));
        if let Some((name, interface, path)) = &r.provides {
            s.push_str(",\n      \"provides\": [\n        {\n          \"is-interface\": ");
            s.push_str(if *interface { "true" } else { "false" });
            s.push_str(",\n          \"logical-name\": ");
            s.push_str(&json_quote(name));
            s.push_str(",\n          \"source-path\": ");
            s.push_str(&json_quote(path));
            s.push_str("\n        }\n      ]");
        }
        if !r.requires.is_empty() {
            s.push_str(",\n      \"requires\": [");
            for (i, (name, path)) in r.requires.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str("\n        {\n          \"logical-name\": ");
                s.push_str(&json_quote(name));
                if let Some(p) = path {
                    s.push_str(",\n          \"source-path\": ");
                    s.push_str(&json_quote(p));
                }
                s.push_str("\n        }");
            }
            s.push_str("\n      ]");
        }
        s.push_str("\n    }");
    }
    if !rules.is_empty() {
        s.push_str("\n  ");
    }
    s.push_str("],\n  \"version\": 1\n}\n");
    s
}

/// A field of an `experimental-full` command object.
enum Field<'a> {
    Str(&'a str),
    List(&'a [String]),
}

/// The `experimental-full` document, in the shape of the clang-scan-deps
/// `flavour` stands for: 22 streams its keys in a fixed order, adds
/// `named-module`/`named-module-deps`, and ends without a newline; 19
/// builds an `llvm::json` object (keys sorted, no named-module fields)
/// and ends with one. `modules` (clang modules) is always empty: kiln
/// has no clang module maps.
fn full(units: &[Option<FullCommand>], flavour: Flavour) -> String {
    let mut s = String::from("{\n  \"modules\": [],\n  \"translation-units\": [");
    for (i, unit) in units.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str("    {\n      \"commands\": [");
        if let Some(u) = unit {
            let mut fields = vec![("clang-context-hash", Field::Str(&u.context_hash))];
            if flavour == Flavour::V22 {
                if let Some(m) = &u.named_module {
                    fields.push(("named-module", Field::Str(m)));
                }
                if !u.module_deps.is_empty() {
                    fields.push(("named-module-deps", Field::List(&u.module_deps)));
                }
            }
            fields.extend([
                ("clang-module-deps", Field::List(&[])),
                ("command-line", Field::List(&u.command_line)),
                ("executable", Field::Str(&u.executable)),
                ("file-deps", Field::List(&u.file_deps)),
                ("input-file", Field::Str(&u.input_file)),
            ]);
            s.push_str("\n        {");
            for (j, (key, val)) in fields.iter().enumerate() {
                s.push_str(if j == 0 { "\n" } else { ",\n" });
                s.push_str("          ");
                s.push_str(&json_quote(key));
                s.push_str(": ");
                match val {
                    Field::Str(v) => s.push_str(&json_quote(v)),
                    Field::List([]) => s.push_str("[]"),
                    Field::List(items) => {
                        s.push('[');
                        for (k, item) in items.iter().enumerate() {
                            s.push_str(if k == 0 { "\n" } else { ",\n" });
                            s.push_str("            ");
                            s.push_str(&json_quote(item));
                        }
                        s.push_str("\n          ]");
                    }
                }
            }
            s.push_str("\n        }\n      ");
        }
        s.push_str("]\n    }");
    }
    if !units.is_empty() {
        s.push_str("\n  ");
    }
    s.push_str("]\n}");
    if flavour == Flavour::V19 {
        s.push('\n');
    }
    s
}

/// A JSON string as `llvm::json` quotes it: `"` and `\` escaped, `\t`,
/// `\n`, `\r` short, other control characters `\u00xx`.
fn json_quote(v: &str) -> String {
    let mut s = String::with_capacity(v.len() + 2);
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\t' => s.push_str("\\t"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = std::fmt::Write::write_fmt(&mut s, format_args!("\\u{:04x}", c as u32));
            }
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_names() {
        for n in [
            "kiln-scan-deps",
            "clang-scan-deps",
            "clang-scan-deps-19",
            "clang-scan-deps-22",
            "clang-scan-deps-22.1",
        ] {
            assert!(is_scanner_name(n), "{n}");
        }
        for n in ["kiln", "kiln++", "clang-scan-deps-", "clang-scan-deps-x", "clang++"] {
            assert!(!is_scanner_name(n), "{n}");
        }
    }

    #[test]
    fn quoting() {
        assert_eq!(json_quote("a\"b\\c\td\u{1}"), "\"a\\\"b\\\\c\\td\\u0001\"");
    }

    #[test]
    fn options() {
        let v = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let o = parse_opts("p", &v("-format=p1689 -o out.json -j 4")).unwrap();
        assert_eq!(o.format, Format::P1689);
        assert_eq!(o.output.as_deref(), Some("out.json"));
        assert_eq!(parse_opts("p", &v("--format make")).unwrap().format, Format::Make);
        assert_eq!(
            parse_opts("p", &v("-format=bogus")).err().unwrap(),
            "p: for the --format option: Cannot find option named 'bogus'"
        );
        assert!(parse_opts("p", &v("-frob")).is_err());
    }

    #[test]
    fn empty_document() {
        assert_eq!(
            p1689(Vec::new()),
            "{\n  \"revision\": 0,\n  \"rules\": [],\n  \"version\": 1\n}\n"
        );
        assert_eq!(
            full(&[], Flavour::V22),
            "{\n  \"modules\": [],\n  \"translation-units\": []\n}"
        );
        assert_eq!(
            full(&[None], Flavour::V19),
            "{\n  \"modules\": [],\n  \"translation-units\": [\n    {\n      \"commands\": []\n    }\n  ]\n}\n"
        );
    }

    #[test]
    fn flavours() {
        for n in ["clang-scan-deps-19", "clang-scan-deps-19.1"] {
            assert_eq!(Flavour::of(n), Flavour::V19, "{n}");
        }
        for n in ["clang-scan-deps-22", "clang-scan-deps-190", "clang-scan-deps", "kiln-scan-deps"]
        {
            assert_eq!(Flavour::of(n), Flavour::V22, "{n}");
        }
    }

    #[test]
    fn rules_sorted_and_linked() {
        let rule = |p: &str, provides: Option<&str>, requires: &[&str]| Rule {
            primary: p.to_owned(),
            provides: provides.map(|m| (m.to_owned(), true, format!("{m}.cppm"))),
            requires: requires.iter().map(|r| ((*r).to_owned(), None)).collect(),
        };
        let doc = p1689(vec![
            rule("main.o", None, &["m", "x"]),
            rule("", Some("m"), &[]),
            rule("", Some("n"), &[]),
        ]);
        // Stable: the two empty primaries keep their order.
        let m =
            doc.find("\"logical-name\": \"m\",\n          \"source-path\": \"m.cppm\"").unwrap();
        assert!(doc.find("\"n.cppm\"").unwrap() > m);
        assert!(doc.find("main.o").unwrap() > doc.find("\"n.cppm\"").unwrap());
        // `m` is provided by a rule, `x` by none.
        assert!(doc.contains(
            "\"logical-name\": \"m\",\n          \"source-path\": \"m.cppm\"\n        },"
        ));
        assert!(doc.contains("\"logical-name\": \"x\"\n        }"));
    }

    #[test]
    fn hashes_ignore_inputs_and_outputs() {
        let v = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let a = context_hash(&v("-std=c++20 -c a.cpp -o a.o -MD -MF a.d"), &["a.cpp"]);
        let b = context_hash(&v("-std=c++20 -c b.cpp -ob.o"), &["b.cpp"]);
        let c = context_hash(&v("-std=c++23 -c b.cpp"), &["b.cpp"]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()), "{a}");
    }

    #[test]
    fn joins() {
        assert_eq!(join("/d", "a.h"), "/d/a.h");
        assert_eq!(join("/d/", "a.h"), "/d/a.h");
        assert_eq!(join(".", "../a.h"), "./../a.h");
        assert_eq!(join("/d", "/usr/a.h"), "/usr/a.h");
    }

    #[test]
    fn executables() {
        assert_eq!(resolve_executable("./c++"), "./c++");
        assert_eq!(resolve_executable("no-such-compiler-x"), "no-such-compiler-x");
    }
}
