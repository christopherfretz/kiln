//! The `kiln` driver. See `docs/ARCHITECTURE.md` ("Driver") and `spec/driver.md`.
//!
//! M2 supports `-fsyntax-only [-fdump-ast[=opts]] [-fdump-ast-filter=s]`
//! on one C++ file; code generation is not wired yet.
#![forbid(unsafe_code)]

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use kiln_ast::dump::{DumpOptions, Filter, dump};
use kiln_diag::{DiagEngine, DiagOptions, Format, JsonSink, TextSink};
use kiln_lex::Std;
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;

/// The parsed command line.
#[derive(Default)]
struct Args {
    input: Option<PathBuf>,
    syntax_only: bool,
    dump: Option<String>,
    dump_filter: Option<String>,
    std: Option<Std>,
    target: Option<String>,
    diag: DiagOptions,
    errors: Vec<String>,
}

/// `-std=` values ([`Std`]); GNU modes are accepted as their ISO base.
fn parse_std(s: &str) -> Option<Std> {
    let base = s.strip_prefix("gnu++").or_else(|| s.strip_prefix("c++"))?;
    Some(match base {
        "98" | "03" => Std::Cxx03,
        "11" | "0x" => Std::Cxx11,
        "14" | "1y" => Std::Cxx14,
        "17" | "1z" => Std::Cxx17,
        "20" | "2a" => Std::Cxx20,
        "23" | "2b" => Std::Cxx23,
        "26" | "2c" => Std::Cxx26,
        _ => return None,
    })
}

fn parse_args(argv: impl IntoIterator<Item = String>) -> Args {
    let mut a = Args::default();
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        if a.diag.parse_arg(&arg) {
            continue;
        }
        match arg.as_str() {
            "-fsyntax-only" => a.syntax_only = true,
            "-fdump-ast" => a.dump = Some(String::new()),
            "-o" => {
                let _ = it.next();
            }
            "-target" => a.target = it.next(),
            "-x" => match it.next().as_deref() {
                Some("c++" | "c++-header") => {}
                Some(l) => a.errors.push(format!("language '{l}' is not supported")),
                None => a.errors.push("argument to '-x' is missing".into()),
            },
            _ => {
                if let Some(v) = arg.strip_prefix("-fdump-ast=") {
                    a.dump = Some(v.to_owned());
                } else if let Some(v) = arg.strip_prefix("-fdump-ast-filter=") {
                    a.dump_filter = Some(v.to_owned());
                } else if let Some(v) = arg.strip_prefix("-std=") {
                    match parse_std(v) {
                        Some(s) => a.std = Some(s),
                        None => a.errors.push(format!("invalid value '{v}' in '{arg}'")),
                    }
                } else if let Some(v) =
                    arg.strip_prefix("--target=").or_else(|| arg.strip_prefix("-target="))
                {
                    a.target = Some(v.to_owned());
                } else if arg.starts_with('-') && arg != "-" {
                    a.errors.push(format!("unknown argument: '{arg}'"));
                } else if a.input.is_some() {
                    a.errors.push("only one input file is supported".into());
                } else {
                    a.input = Some(PathBuf::from(arg));
                }
            }
        }
    }
    a
}

/// The dump options of `-fdump-ast=<opts>`.
fn dump_options<'a>(
    opts: &str,
    filter: Option<String>,
    sm: &'a SourceManager,
    main: kiln_source::FileId,
) -> Result<DumpOptions<'a>, String> {
    let mut d = DumpOptions {
        filter: filter.map(|name| Filter { name: Some(name), kind: None }),
        sources: Some(sm),
        main_file: Some(main),
        ..DumpOptions::default()
    };
    for o in opts.split(',').filter(|o| !o.is_empty()) {
        match o {
            "ids" => d.ids = true,
            "locs" => d.locs = true,
            "canonical" => d.canonical_types = true,
            "implicit" => d.implicit = true,
            "all-files" => d.all_files = true,
            _ => return Err(format!("unknown -fdump-ast option '{o}'")),
        }
    }
    Ok(d)
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("kiln: error: {msg}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "--version") {
        println!("kiln version {} (C++26 frontend, LLVM IR backend)", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let args = parse_args(argv);
    if let Some(e) = args.errors.first() {
        return fail(e);
    }
    let Some(input) = args.input.clone() else {
        return fail("no input files");
    };
    if !args.syntax_only {
        return fail("code generation is not implemented yet; use -fsyntax-only");
    }
    let target = match &args.target {
        None => Target::for_arch(Arch::AArch64),
        Some(t) => match Target::from_triple(t) {
            Ok(t) => t,
            Err(_) => return fail(&format!("unknown target triple '{t}'")),
        },
    };
    let sm = SourceManager::new();
    let file = match sm.load_file(&input) {
        Ok(f) => f,
        Err(e) => {
            return fail(&format!("no such file or directory: '{}' ({e})", input.display()));
        }
    };
    let dump_opts = match &args.dump {
        Some(o) => match dump_options(o, args.dump_filter.clone(), &sm, file) {
            Ok(d) => Some(d),
            Err(e) => return fail(&e),
        },
        None => None,
    };
    let engine = match args.diag.format {
        Format::Json => {
            DiagEngine::new(&sm, &args.diag, JsonSink::new(std::io::stderr(), &args.diag))
        }
        _ => DiagEngine::new(&sm, &args.diag, TextSink::new(std::io::stderr(), &args.diag)),
    };
    let popts = ParseOptions { std: args.std.unwrap_or(Std::Cxx26) };
    let ast = parse_file(&sm, file, &engine, target, &popts);
    if let Some(d) = dump_opts {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        if dump(&ast, &d, &mut out).and_then(|()| out.flush()).is_err() {
            return fail("cannot write the AST dump");
        }
    }
    if let Some(s) = engine.summary() {
        eprintln!("{s}");
    }
    if engine.error_count() > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
