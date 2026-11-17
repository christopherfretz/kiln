//! The completeness proof of `kiln_llvm`'s read view on real modules
//! (spec/llvm.md "Read view", WP-1558).
//!
//! Every C++ file of the test corpus's `tests/codegen` and
//! `tests/debuginfo` directories, and the files of `tests/conformance`
//! that exercise constructs with unusual IR (exceptions, atomics, inline
//! asm, coroutines, vectors, computed goto, aliases and IFUNCs, wide
//! integers, thread-local storage), is lowered by `kiln_codegen` without
//! and with `-g`, and printed twice: by `Module::to_text` and by the
//! second printer of `crates/kiln_llvm/tests/common/view_print.rs`, which
//! reads the module through the public API only. The texts must be equal.
//! Files that need a system header or that kiln rejects are skipped; the
//! test asserts how many modules it compared and that the compared text
//! covers each construct of the list above.
//!
//! `KILN_VIEW_FILE=<substring>` restricts the run to matching paths and
//! says why each skipped file was skipped.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use kiln_abi::{Arch, Target};
use kiln_codegen::{CodegenOptions, DebugInfoKind, DebugOptions, emit_with_debug};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

#[path = "../../kiln_llvm/tests/common/view_print.rs"]
mod view_print;

/// Words that select a conformance file (in its text) for the walk.
const CONFORMANCE_KEYS: &[&str] = &[
    "throw",
    "atomic",
    "asm",
    "co_await",
    "co_yield",
    "co_return",
    "goto *",
    "ifunc",
    "alias(",
    "vector_size",
    "ext_vector_type",
    "_BitInt",
    "__int128",
    "thread_local",
    "musttail",
    "__builtin_shufflevector",
];

/// Constructs the compared text must contain somewhere (so that the walk
/// demonstrably covers them).
const MUST_COVER: &[&str] = &[
    " invoke ",
    " landingpad ",
    " resume ",
    " atomicrmw ",
    " cmpxchg ",
    " fence ",
    "syncscope(",
    " asm ",
    " callbr ",
    "blockaddress(",
    " indirectbr ",
    " shufflevector ",
    " extractelement ",
    " insertelement ",
    " extractvalue ",
    " insertvalue ",
    " phi ",
    " switch ",
    " select ",
    " alias ",
    " ifunc ",
    " = comdat any",
    "#dbg_declare(",
    "!DILocation(",
    "!DIExpression(",
    "llvm.coro.",
    "thread_local",
    " freeze ",
    " personality ",
    "!llvm.module.flags",
    "attributes #0",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cpp_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            cpp_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "cpp") {
            out.push(p);
        }
    }
}

/// The `//@ args:` words of a corpus file.
fn args_of(src: &str) -> Vec<&str> {
    src.lines()
        .filter_map(|l| l.strip_prefix("//@ args:"))
        .flat_map(str::split_whitespace)
        .collect()
}

fn inputs() -> Vec<(PathBuf, String)> {
    let root = repo_root().join("tests");
    let mut files = Vec::new();
    cpp_files(&root.join("codegen"), &mut files);
    cpp_files(&root.join("debuginfo"), &mut files);
    let mut conf = Vec::new();
    cpp_files(&root.join("conformance"), &mut conf);
    let filter = std::env::var("KILN_VIEW_FILE").ok();
    let mut out = Vec::new();
    for (p, from_conformance) in
        files.into_iter().map(|p| (p, false)).chain(conf.into_iter().map(|p| (p, true)))
    {
        if filter.as_deref().is_some_and(|f| !p.to_string_lossy().contains(f)) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&p) else { continue };
        // No include paths here: only self-contained files.
        if src.contains("#include") || src.contains("import ") || src.contains("//@ module") {
            continue;
        }
        if from_conformance && !CONFORMANCE_KEYS.iter().any(|k| src.contains(k)) {
            continue;
        }
        out.push((p, src));
    }
    out
}

/// Lower `src` and return both texts; `Err` names why kiln rejected it.
fn texts(src: &str, debug: bool) -> Result<(String, String), String> {
    let args = args_of(src);
    let arch = if args.iter().any(|a| a.starts_with("--target=x86_64")) {
        Arch::X86_64
    } else {
        Arch::AArch64
    };
    let exceptions = !args.contains(&"-fno-exceptions");
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).map_err(|e| format!("{e:?}"))?;
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(arch);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { exceptions, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    if let Some(d) = sink.take().into_iter().find(|d| d.level >= Level::Error) {
        return Err(d.message);
    }
    let opts = CodegenOptions {
        source_filename: "t.cpp".to_owned(),
        exceptions,
        ident: "kiln".to_owned(),
        debug: if debug {
            DebugOptions {
                kind: DebugInfoKind::Constructor,
                main_file: "t.cpp".to_owned(),
                main_id: Some(file),
                comp_dir: "/view".to_owned(),
                ..DebugOptions::default()
            }
        } else {
            DebugOptions::default()
        },
        ..CodegenOptions::default()
    };
    let (m, _) = emit_with_debug(&mut tu.ast, &mut tu.oven, &target, &opts, Some(&sm));
    let m = m.map_err(|e| format!("codegen: {e}"))?;
    Ok((m.to_text(), view_print::print(&m)))
}

fn first_difference(a: &str, b: &str) -> String {
    let n = a.lines().zip(b.lines()).position(|(x, y)| x != y);
    match n {
        Some(i) => format!(
            "line {}:\n  to_text: {}\n  view:    {}",
            i + 1,
            a.lines().nth(i).unwrap_or_default(),
            b.lines().nth(i).unwrap_or_default()
        ),
        None => format!("lengths {} and {}", a.len(), b.len()),
    }
}

#[test]
fn view_printer_matches_to_text_on_corpus_modules() {
    let inputs = inputs();
    let verbose = std::env::var_os("KILN_VIEW_FILE").is_some();
    let next = AtomicUsize::new(0);
    let compared = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let covered = Mutex::new(vec![false; MUST_COVER.len()]);
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    std::thread::scope(|s| {
        for _ in 0..threads {
            std::thread::Builder::new()
                // Deep templates and long expressions recurse (the driver's
                // compile thread has 1 GiB).
                .stack_size(256 << 20)
                .spawn_scoped(s, || {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some((path, src)) = inputs.get(i) else { break };
                        for debug in [false, true] {
                            let (ours, view) = match texts(src, debug) {
                                Ok(t) => t,
                                Err(e) => {
                                    if verbose {
                                        eprintln!("skipped {}: {e}", path.display());
                                    }
                                    skipped.fetch_add(1, Ordering::Relaxed);
                                    continue;
                                }
                            };
                            compared.fetch_add(1, Ordering::Relaxed);
                            if ours == view {
                                let mut c = covered.lock().expect("lock");
                                for (k, hit) in MUST_COVER.iter().zip(c.iter_mut()) {
                                    *hit |= ours.contains(k);
                                }
                            } else {
                                failures.lock().expect("lock").push(format!(
                                    "{}{}: {}",
                                    path.display(),
                                    if debug { " (-g)" } else { "" },
                                    first_difference(&ours, &view)
                                ));
                            }
                        }
                    }
                })
                .expect("spawn");
        }
    });
    let failures = failures.into_inner().expect("lock");
    assert!(
        failures.is_empty(),
        "{} module(s) print differently through the read view:\n{}",
        failures.len(),
        failures.join("\n")
    );
    let compared = compared.into_inner();
    eprintln!(
        "read view: {compared} modules compared from {} files ({} lowerings skipped)",
        inputs.len(),
        skipped.into_inner()
    );
    if std::env::var_os("KILN_VIEW_FILE").is_none() {
        assert!(compared >= 1000, "only {compared} modules compared");
        let covered = covered.into_inner().expect("lock");
        let missing: Vec<&str> =
            MUST_COVER.iter().zip(covered).filter(|(_, c)| !c).map(|(k, _)| *k).collect();
        assert!(missing.is_empty(), "no compared module contains {missing:?}");
    }
}
