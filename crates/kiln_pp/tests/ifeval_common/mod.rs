//! Helpers for the WP-015 tests (`ifeval*.rs`, `features*.rs`): run kiln
//! with a chosen target, standard, diagnostic flags and hooks, and run
//! `clang++-19 -E` on the same source.
#![allow(dead_code, reason = "each test binary uses a subset")]

use std::io::Write as _;
use std::process::{Command, Stdio};

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{IfEval, MemoryResolver, PpOptions, Preprocessor, PrintOptions, print_preprocessed};
use kiln_source::SourceManager;

/// The reference compiler.
pub const CLANG: &str = "clang++-19";

/// `clang++-19` is installed.
pub fn clang_available() -> bool {
    Command::new(CLANG)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// clang's `-std=` spelling of `std`.
pub fn std_flag(std: Std) -> &'static str {
    match std {
        Std::Cxx98 => "-std=c++98",
        Std::Cxx03 => "-std=c++03",
        Std::Cxx11 => "-std=c++11",
        Std::Cxx14 => "-std=c++14",
        Std::Cxx17 => "-std=c++17",
        Std::Cxx20 => "-std=c++20",
        Std::Cxx23 => "-std=c++23",
        Std::Cxx26 => "-std=c++26",
    }
}

/// clang 19's predefines for `triple` (`aarch64-unknown-linux-gnu` or
/// `x86_64-unknown-linux-gnu`) and `std` (C++17 ones before C++17).
pub fn predefines(triple: &str, std: Std) -> &'static str {
    macro_rules! p {
        ($t:literal, $s:literal) => {
            include_str!(concat!("../../../../resource/predefines/", $t, ".c++", $s, ".txt"))
        };
    }
    let x86 = triple.starts_with("x86_64");
    match (x86, std) {
        (false, Std::Cxx26) => p!("aarch64-unknown-linux-gnu", "26"),
        (false, Std::Cxx23) => p!("aarch64-unknown-linux-gnu", "23"),
        (false, Std::Cxx20) => p!("aarch64-unknown-linux-gnu", "20"),
        (false, _) => p!("aarch64-unknown-linux-gnu", "17"),
        (true, Std::Cxx26) => p!("x86_64-unknown-linux-gnu", "26"),
        (true, Std::Cxx23) => p!("x86_64-unknown-linux-gnu", "23"),
        (true, Std::Cxx20) => p!("x86_64-unknown-linux-gnu", "20"),
        (true, _) => p!("x86_64-unknown-linux-gnu", "17"),
    }
}

/// One kiln run.
#[derive(Debug)]
pub struct Out {
    /// The `-E -P` output.
    pub text: String,
    /// `line:col: level: message [option]`, notes as separate entries.
    pub diags: Vec<String>,
}

fn render(sm: &SourceManager, d: &Diagnostic, out: &mut Vec<String>) {
    let at = sm
        .presumed_line_col(d.span.lo)
        .map_or_else(String::new, |p| format!("{}:{}: ", p.line, p.col));
    let opt = d.option.as_ref().map_or_else(String::new, |o| format!(" [{o}]"));
    out.push(format!("{at}{}: {}{opt}", d.level.text_name(), d.message));
    for n in &d.notes {
        render(sm, n, out);
    }
}

/// Runs kiln on `src` (main file `t.cpp`) for `triple` and `std`, with
/// diagnostic arguments `args`, the [`IfEval`] for `triple`, and `setup`
/// applied to the preprocessor (to install feature tables).
pub fn run_kiln(
    src: &str,
    triple: &str,
    std: Std,
    args: &[&str],
    r: MemoryResolver,
    setup: impl FnOnce(&mut Preprocessor<'_, '_>),
) -> Out {
    run_kiln_with(src, triple, std, true, args, r, setup)
}

/// [`run_kiln`], without clang's predefined macros if `!predefs`
/// (`clang -undef`).
pub fn run_kiln_with(
    src: &str,
    triple: &str,
    std: Std,
    predefs: bool,
    args: &[&str],
    r: MemoryResolver,
    setup: impl FnOnce(&mut Preprocessor<'_, '_>),
) -> Out {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let sink = MemorySink::new();
    let mut dopts = DiagOptions::from_args(args.iter().copied());
    dopts.parse_arg("-ferror-limit=0");
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut opts = PpOptions::new(std);
    if predefs {
        predefines(triple, std).clone_into(&mut opts.predefines);
    }
    opts.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    pp.set_include_resolver(Box::new(r));
    pp.set_if_evaluator(Box::new(IfEval::for_triple(triple)));
    setup(&mut pp);
    pp.enter_main_file(main);
    let mut text = String::new();
    print_preprocessed(&mut pp, PrintOptions::default(), &mut text);
    drop(pp);
    let mut diags = Vec::new();
    for d in sink.take() {
        render(&sm, &d, &mut diags);
    }
    Out { text, diags }
}

/// Runs `clang++-19 -E -P` on `src` (from stdin) for `triple` and `std`
/// with the extra arguments `args`. Diagnostics in the [`Out`] format;
/// macro-expansion notes are dropped. `None` if clang cannot be run.
pub fn run_clang(src: &str, triple: &str, std: Std, args: &[&str]) -> Option<Out> {
    let mut child = Command::new(CLANG)
        .args(["-x", "c++", "-E", "-P", "-ferror-limit=0", "-fno-caret-diagnostics"])
        .args(["-fno-color-diagnostics", "-nostdinc", "-nostdinc++"])
        .arg(std_flag(std))
        .arg(format!("--target={triple}"))
        .args(args)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // Feed stdin from a thread: a large output would otherwise fill the
    // stdout pipe while we are still writing.
    let mut stdin = child.stdin.take()?;
    let input = src.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()).is_ok());
    let out = child.wait_with_output().ok()?;
    if !writer.join().unwrap_or(false) {
        return None;
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let diags = stderr
        .lines()
        .filter_map(|l| l.strip_prefix("<stdin>:"))
        .filter(|l| !l.contains(": note: expanded from macro"))
        // Spelling suggestions for `\\N{...}` are not implemented.
        .filter(|l| !l.contains(": note: did you mean "))
        .map(str::to_owned)
        .collect();
    Some(Out { text: String::from_utf8_lossy(&out.stdout).into_owned(), diags })
}

/// The output's tokens, whitespace and line structure ignored.
pub fn words(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}
