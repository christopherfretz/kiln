//! `cargo xtask clang-ast`: converts clang's `-Xclang -ast-dump=json` into
//! the kiln AST dump text format (`spec/ast-dump.md` §6), so clang can be
//! the oracle of `//@ expect: ast` goldens.
//!
//! The JSON carries almost everything a kiln line needs. What it lacks —
//! the `value:` of constexpr variables — comes from clang's text dump of the
//! same file (the "side channel", fetched lazily and aligned by
//! declaration order). Every node kind goes through the explicit mapping
//! table in [`table`]; anything the normalizer cannot render faithfully is
//! an error naming the construct, never a silently different line.

mod escape;
mod print;
mod side;
pub mod table;
#[cfg(test)]
mod tests;
mod values;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use crate::util::json::Json;
use crate::util::process;

const USAGE: &str = "\
usage: cargo xtask clang-ast <file> [options] [-- <extra clang args>]

Runs clang on <file> and prints its AST in the kiln dump format
(spec/ast-dump.md), as `kiln -fsyntax-only -fdump-ast` would.

options:
  --compiler=<clang>   the clang to run (default: clang++-19)
  --std=<std>          language standard (default: c++26)
  --implicit           include implicit declarations (-fdump-ast=implicit)
  --all-files          include top-level declarations from included files
                       (-fdump-ast=all-files)
  --filter=<substr>    dump each declaration whose qualified name contains
                       <substr> (-fdump-ast-filter=<substr>)
";

/// Default clang for the CLI (the harness passes its own).
const DEFAULT_CLANG: &str = "clang++-19";

/// The kiln `-fdump-ast` options the normalizer supports
/// (`spec/ast-dump.md` §1; `ids`, `locs` and `canonical` have no clang
/// equivalent).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DumpOptions {
    /// `implicit`: keep implicit declarations.
    pub implicit: bool,
    /// `all-files`: keep top-level declarations from included files.
    pub all_files: bool,
    /// `-fdump-ast-filter=`: dump matching declarations as separate trees.
    pub filter: Option<String>,
}

/// Stack for the normalizing thread: the JSON reader and the printers
/// recurse once per tree level, and deep expression trees (long `a + b +
/// ...` chains) are legitimate input.
const STACK_BYTES: usize = 256 << 20;
/// Nesting limit for the JSON reader (each AST level is two JSON levels).
const MAX_JSON_DEPTH: usize = 20_000;

/// Converts clang's JSON dump `json` of a translation unit whose main file
/// clang names `main_file` into kiln dump text. `text` produces clang's
/// text dump of the same compile; it is called at most once, and only when
/// a constexpr variable's value is needed.
pub fn normalize(
    json: &str,
    main_file: &str,
    opts: &DumpOptions,
    text: &mut (dyn FnMut() -> Result<String, String> + Send),
) -> Result<String, String> {
    std::thread::scope(|s| {
        let worker = std::thread::Builder::new()
            .name("clang-ast".into())
            .stack_size(STACK_BYTES)
            .spawn_scoped(s, || {
                let root = Json::parse_with_max_depth(json, MAX_JSON_DEPTH)
                    .map_err(|e| format!("clang-ast: clang's JSON dump: {e}"))?;
                print::dump(&root, main_file, opts, text)
            })
            .map_err(|e| format!("clang-ast: cannot start the worker thread: {e}"))?;
        worker.join().unwrap_or_else(|_| Err("clang-ast: the normalizer panicked".into()))
    })
}

/// A clang invocation: the compiler, its working directory, and the
/// arguments before the dump flags (`-std=`, user flags, the file).
pub struct ClangRun<'a> {
    pub clang: &'a Path,
    pub cwd: &'a Path,
    /// Arguments other than the dump mode, including the input file.
    pub args: &'a [String],
    pub timeout: Duration,
}

impl ClangRun<'_> {
    fn command(&self, mode: &str) -> Command {
        let mut c = Command::new(self.clang);
        c.current_dir(self.cwd);
        c.args(["-fsyntax-only", "-fno-color-diagnostics", "-Xclang", mode]);
        c.args(self.args);
        c
    }

    /// Runs one dump; returns stdout, or an error with clang's stderr.
    pub fn dump(&self, mode: &str) -> Result<String, String> {
        let mut cmd = self.command(mode);
        let shown = process::Shown(&cmd).to_string();
        let o = process::run(&mut cmd, self.timeout)
            .map_err(|e| format!("could not run `{shown}`: {e}"))?;
        if !o.success() {
            return Err(format!(
                "`{shown}` failed ({}):\n{}",
                o.describe_end(),
                o.stderr_str().trim_end()
            ));
        }
        String::from_utf8(o.stdout)
            .map_err(|_| format!("`{shown}` printed a dump that is not UTF-8"))
    }
}

struct Cli {
    file: String,
    compiler: PathBuf,
    std: String,
    opts: DumpOptions,
    extra: Vec<String>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Cli>, String> {
    let mut cli = Cli {
        file: String::new(),
        compiler: PathBuf::from(DEFAULT_CLANG),
        std: "c++26".into(),
        opts: DumpOptions::default(),
        extra: Vec::new(),
    };
    let mut file = None;
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        if a == "--" {
            cli.extra.extend(args.by_ref());
            break;
        }
        if a == "-h" || a == "--help" {
            return Ok(None);
        }
        if let Some(v) = a.strip_prefix("--compiler=") {
            cli.compiler = PathBuf::from(v);
        } else if let Some(v) = a.strip_prefix("--std=") {
            v.clone_into(&mut cli.std);
        } else if let Some(v) = a.strip_prefix("--filter=") {
            if v.is_empty() {
                return Err("--filter= needs a non-empty substring".into());
            }
            cli.opts.filter = Some(v.to_owned());
        } else if a == "--implicit" {
            cli.opts.implicit = true;
        } else if a == "--all-files" {
            cli.opts.all_files = true;
        } else if a.starts_with('-') {
            return Err(format!("unknown option `{a}`"));
        } else if file.replace(a).is_some() {
            return Err("more than one input file".into());
        }
    }
    cli.file = file.ok_or("no input file")?;
    Ok(Some(cli))
}

/// `cargo xtask clang-ast`.
pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let cli = match parse_args(args) {
        Ok(Some(c)) => c,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprint!("xtask clang-ast: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("xtask clang-ast: cannot read the working directory: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut args = vec![format!("-std={}", cli.std)];
    args.extend(cli.extra.iter().cloned());
    args.push(cli.file.clone());
    let run = ClangRun {
        clang: &cli.compiler,
        cwd: &cwd,
        args: &args,
        timeout: Duration::from_secs(120),
    };
    let result = run.dump("-ast-dump=json").and_then(|json| {
        let mut text = || run.dump("-ast-dump");
        normalize(&json, &cli.file, &cli.opts, &mut text)
    });
    match result {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            // Normalizer errors already name the tool.
            let e = e.strip_prefix("clang-ast: ").unwrap_or(&e);
            eprintln!("xtask clang-ast: {e}");
            ExitCode::FAILURE
        }
    }
}
