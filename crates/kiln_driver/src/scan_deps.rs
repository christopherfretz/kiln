//! `kiln-scan-deps`: the module dependency scanner build systems run
//! before compiling (CMake's `FILE_SET CXX_MODULES`), a mode of the
//! `kiln` binary picked by the program name, as `clang-scan-deps`
//! behaves (spec/modules.md §11.9, spec/driver.md "Dependency
//! scanning").
//!
//! `kiln-scan-deps [-format=p1689|make] [-o <file>] -- <compiler> <args>`
//! preprocesses the one input of the compile command exactly as the
//! compile would (same search list, predefines, feature answers), with
//! the module directives recorded instead of imported, then prints the
//! P1689r5 JSON (or a make rule) and writes the compile command's
//! `-MD`/`-MF`/`-MT` dependency file.

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

/// The program names that run the scanner: `kiln-scan-deps`, and the
/// aliases CMake's `find_program` looks for beside the compiler
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
}

/// The scanner's own options (before `--`).
struct ScanOpts {
    format: Format,
    output: Option<String>,
}

/// Parses the options before `--`; `Err` is the message to print.
fn parse_opts(prog: &str, opts: &[String]) -> Result<ScanOpts, String> {
    let mut o = ScanOpts { format: Format::Make, output: None };
    let mut i = 0;
    while i < opts.len() {
        let a = opts[i].as_str();
        i += 1;
        let flag = a.strip_prefix("--").map_or(a, |r| &a[a.len() - r.len() - 1..]);
        let (name, inline) = match flag.split_once('=') {
            Some((n, v)) => (n, Some(v)),
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
                o.format = match value(&mut i)?.as_str() {
                    "make" => Format::Make,
                    "p1689" => Format::P1689,
                    v @ "experimental-full" => {
                        return Err(format!("{prog}: -format={v} is not supported"));
                    }
                    v => {
                        return Err(format!(
                            "{prog}: for the --format option: Cannot find option named '{v}'"
                        ));
                    }
                }
            }
            "-o" => o.output = Some(value(&mut i)?),
            // Scheduling and caching knobs of clang's scanner: kiln scans
            // one command on one thread (documented no-ops).
            "-j" | "-mode" | "-resource-dir-recipe" => drop(value(&mut i)?),
            "-optimize-args"
            | "-eager-load-pcm"
            | "-reuse-filemanager"
            | "-skip-excluded-pp-ranges"
            | "-brief-result"
            | "-round-trip-args" => {}
            "-compilation-database" => {
                return Err(format!(
                    "{prog}: -compilation-database is not supported; pass the command after '--'"
                ));
            }
            _ if a.starts_with("-o") && a.len() > 2 && inline.is_none() => {
                o.output = Some(a[2..].to_owned());
            }
            _ => {
                return Err(format!(
                    "{prog}: Unknown command line argument '{a}'.  Try: '{prog} --help'"
                ));
            }
        }
    }
    Ok(o)
}

const HELP: &str = "\
OVERVIEW: kiln-scan-deps, kiln's module dependency scanner (clang-scan-deps compatible)

USAGE: kiln-scan-deps [options] -- <compiler> <compile arguments>

  -format=<p1689|make>  P1689r5 JSON, or a make rule (the default)
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

/// Runs the scanner: `argv` without the program name. Returns the exit
/// code: 0, or 1 after any error (clang-scan-deps's convention).
pub(crate) fn run(argv0: &str, argv: &[String], mut io: Io) -> i32 {
    let prog = crate::args::program_basename(argv0);
    let (opts, cmd) = match argv.iter().position(|a| a == "--") {
        Some(i) => (&argv[..i], &argv[i + 1..]),
        None => (argv, &[][..]),
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
    let Some((compiler, cargs)) = cmd.split_first() else {
        let _ = writeln!(
            io.stderr,
            "The compilation command line must be provided either via \
             '-compilation-database' or after '--'."
        );
        return 1;
    };
    let mut args = Args::parse_as(compiler, &crate::rsp::expand(cargs));
    args.scan_deps = true;
    // The make format lists every header whatever the command asks.
    let side_file = args.deps.side.is_some();
    if opts.format == Format::Make && args.deps.headers().is_none() {
        args.deps.side = Some(crate::args::DepHeaders::All);
    }
    let input = args.inputs.first().map_or_else(String::new, |i| i.path.clone());

    let capture = Capture::default();
    let diags = SharedWriter::new(capture.clone());
    let scanned = scan(&args, cmd, diags);
    let text = String::from_utf8_lossy(&capture.0.lock().unwrap_or_else(PoisonError::into_inner))
        .into_owned();
    if !text.is_empty() {
        let _ = write!(io.stderr, "Diagnostics while scanning dependencies for '{input}':\n{text}");
    }

    let out = match (&scanned, opts.format) {
        (Some(s), Format::P1689) => p1689(&args, &input, Some(s)),
        (None, Format::P1689) => p1689(&args, &input, None),
        (Some(s), Format::Make) => {
            let files = s.deps.as_ref().map(|d| d.borrow().files().to_vec()).unwrap_or_default();
            deps::deps_text(&args, &input, &files)
        }
        (None, Format::Make) => String::new(),
    };
    let mut rc = i32::from(scanned.is_none());
    if !out.is_empty() && !write_or_report(opts.output.as_deref(), &out, &mut io, prog) {
        rc = 1;
    }
    // The compile command's own `-MD` file, after a clean scan (clang
    // writes none after an error).
    if let Some(s) = &scanned
        && let Some(list) = &s.deps
        && side_file
    {
        let opts = DiagOptions {
            clang_compat: args.predef.clang_compat.major(),
            ..DiagOptions::default()
        };
        if !deps::write_side_file(&args, &input, &list.borrow(), &opts, &mut io) {
            rc = 1;
        }
    }
    rc
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

/// The P1689r5 document of the input, exactly as clang-scan-deps prints
/// it (llvm::json, two-space indent, keys sorted); an empty rule list
/// after an error.
fn p1689(args: &Args, input: &str, scanned: Option<&Scanned>) -> String {
    let mut s = String::from("{\n  \"revision\": 0,\n  \"rules\": [");
    if let Some(sc) = scanned {
        let primary = args.output.clone().unwrap_or_else(|| deps::default_target(input));
        s.push_str("\n    {\n      \"primary-output\": ");
        s.push_str(&json_quote(&primary));
        if let Some((name, Some(kind))) = &sc.unit {
            let interface = *kind != UnitKind::ImplementationPartition;
            s.push_str(",\n      \"provides\": [\n        {\n          \"is-interface\": ");
            s.push_str(if interface { "true" } else { "false" });
            s.push_str(",\n          \"logical-name\": ");
            s.push_str(&json_quote(name));
            s.push_str(",\n          \"source-path\": ");
            s.push_str(&json_quote(input));
            s.push_str("\n        }\n      ]");
        }
        if !sc.requires.is_empty() {
            s.push_str(",\n      \"requires\": [");
            for (i, r) in sc.requires.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str("\n        {\n          \"logical-name\": ");
                s.push_str(&json_quote(r));
                s.push_str("\n        }");
            }
            s.push_str("\n      ]");
        }
        s.push_str("\n    }\n  ");
    }
    s.push_str("],\n  \"version\": 1\n}\n");
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
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
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
        for n in ["kiln-scan-deps", "clang-scan-deps", "clang-scan-deps-22", "clang-scan-deps-22.1"]
        {
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
        let args = Args::parse_as("kiln++", &[]);
        assert_eq!(
            p1689(&args, "a.cpp", None),
            "{\n  \"revision\": 0,\n  \"rules\": [],\n  \"version\": 1\n}\n"
        );
    }
}
