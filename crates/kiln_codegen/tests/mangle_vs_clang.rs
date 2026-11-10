//! The Itanium mangling differential (spec/abi.md §"Mangling").
//!
//! Every `tests/mangle/*.cpp` is compiled by `clang++-19 -S -emit-llvm
//! -O0 -std=c++26 -fno-exceptions --target=<triple>` for each triple in [`TRIPLES`]; each
//! `_Z` symbol clang's IR names (definitions, declarations and
//! references, except the vtable/RTTI/thunk names WP-080 owns) must be
//! among the names kiln's mangler produces for the declarations of the
//! same file (every function in each constructor/destructor variant,
//! every variable, its guard and static-local form). The test fails
//! below [`MIN_SYMBOLS`] checked symbols per triple. Without `clang++-19`
//! it is skipped, unless `KILN_REQUIRE_LLVM=1` (GitHub CI), which makes a
//! missing `clang++-19` a failure.
//!
//! `cargo xtask ci` runs it as its own step,
//! `cargo test -p kiln_codegen --test mangle_vs_clang -- --nocapture`, so
//! the per-triple counts reach the log; `cargo xtask test tests/mangle` runs
//! it too (WP-1034, spec/harness.md "The mangling differential").
//!
//! `KILN_MANGLE_DUMP=1` prints kiln's names for each file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use kiln_abi::mangle::{self, Structor};
use kiln_abi::{Arch, Target};
use kiln_ast::decl::DeclKind;
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const TRIPLES: &[(&str, Arch)] =
    &[("aarch64-unknown-linux-gnu", Arch::AArch64), ("x86_64-unknown-linux-gnu", Arch::X86_64)];

/// The minimum number of distinct clang symbols checked per triple.
const MIN_SYMBOLS: usize = 800;

/// Name prefixes outside this differential: vtables, VTTs, RTTI,
/// construction vtables and thunks (WP-080).
const EXCLUDED: &[&str] = &["_ZTV", "_ZTT", "_ZTI", "_ZTS", "_ZTC", "_ZTh", "_ZTv", "_ZTc"];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mangle")
}

/// Kiln's names for every function and variable declared in `src`.
fn kiln_names(src: &str, arch: Arch) -> Result<BTreeSet<String>, String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(arch);
    let tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target,
        &ParseOptions::default(),
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    if !errors.is_empty() {
        return Err(format!("parse errors: {errors:?}"));
    }
    let ast = &tu.ast;
    let mut names = BTreeSet::new();
    // The oracle is clang 19 (member-like friend templates differ; WP-1146).
    let anon = &mut mangle::AnonIds::new().with_clang_compat(19);
    // A module interface unit's initializer (`_ZGI`).
    if let Some(u) = ast.modules.units.first()
        && u.kind.is_interface()
        && let Some(m) = &u.module
    {
        names.insert(mangle::mangle_module_initializer(ast, m, u.partition.as_deref()));
    }
    for d in ast.decls.ids() {
        match ast.decls.get(d).kind() {
            DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion => {
                names.insert(mangle::mangle_decl(ast, anon, d));
            }
            DeclKind::CXXConstructor => {
                names.insert(mangle::mangle_structor(ast, anon, d, Structor::Complete));
                names.insert(mangle::mangle_structor(ast, anon, d, Structor::Base));
            }
            DeclKind::CXXDestructor => {
                names.insert(mangle::mangle_structor(ast, anon, d, Structor::Complete));
                names.insert(mangle::mangle_structor(ast, anon, d, Structor::Base));
                names.insert(mangle::mangle_structor(ast, anon, d, Structor::Deleting));
            }
            DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                for i in 0..3 {
                    names.insert(mangle::mangle_static_local(ast, anon, d, i));
                    names.insert(mangle::mangle_guard(ast, anon, d, i));
                    for t in 0..3 {
                        names.insert(mangle::mangle_ref_temporary(ast, anon, d, i, t));
                    }
                }
            }
            DeclKind::TemplateParamObject => {
                names.insert(mangle::mangle_param_object(ast, anon, d));
            }
            _ => {}
        }
    }
    Ok(names)
}

/// The `_Z` symbols clang's IR for `file` names.
fn clang_names(file: &Path, triple: &str) -> Option<Result<BTreeSet<String>, String>> {
    let out = Command::new("clang++-19")
        .args(["-S", "-emit-llvm", "-O0", "-std=c++26", "-fno-exceptions"])
        .args([&format!("--target={triple}"), "-o", "-"])
        .arg(file)
        .output()
        .ok()?;
    if !out.status.success() {
        return Some(Err(String::from_utf8_lossy(&out.stderr).into_owned()));
    }
    let ir = String::from_utf8_lossy(&out.stdout);
    let mut names = BTreeSet::new();
    for (i, _) in ir.match_indices("@_Z") {
        let rest = &ir[i + 1..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if !EXCLUDED.iter().any(|p| name.starts_with(p)) {
            names.insert(name.to_owned());
        }
    }
    Some(Ok(names))
}

#[test]
fn mangling_matches_clang() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("tests/mangle")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    let mut failures = Vec::new();
    for &(triple, arch) in TRIPLES {
        let mut checked = 0usize;
        for f in &files {
            let stem = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let Some(c) = clang_names(f, triple) else {
                assert!(
                    std::env::var("KILN_REQUIRE_LLVM").as_deref() != Ok("1"),
                    "clang++-19 not found and KILN_REQUIRE_LLVM=1"
                );
                eprintln!("clang++-19 not found; skipping the mangling differential");
                return;
            };
            let theirs = match c {
                Ok(n) => n,
                Err(e) => {
                    failures.push(format!("{stem} [{triple}]: clang++-19 failed: {e}"));
                    continue;
                }
            };
            let src = std::fs::read_to_string(f).expect("read case");
            let ours = match kiln_names(&src, arch) {
                Ok(n) => n,
                Err(e) => {
                    failures.push(format!("{stem} [{triple}]: {e}"));
                    continue;
                }
            };
            if std::env::var("KILN_MANGLE_DUMP").is_ok() {
                eprintln!("==== {stem} [{triple}] kiln ====");
                for n in &ours {
                    eprintln!("{n}");
                }
            }
            for n in &theirs {
                if !ours.contains(n) {
                    failures.push(format!("{stem} [{triple}]: kiln does not produce {n}"));
                }
            }
            checked += theirs.len();
        }
        eprintln!(
            "mangle_vs_clang [{triple}]: {} files, {checked} clang symbols checked",
            files.len()
        );
        if checked < MIN_SYMBOLS {
            failures.push(format!("{triple}: only {checked} symbols (< {MIN_SYMBOLS})"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mangling mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
