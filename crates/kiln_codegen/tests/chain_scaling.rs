//! Scaling of long postfix and operator chains in codegen ([expr.ref],
//! [expr.call], [class.temporary]; spec/codegen.md): lowering a chain of
//! N member calls `x.f().f()...` with debug information costs O(N)
//! (WP-2201: each call's debug location walked the chain below it, 19 s
//! at N = 8,000 by value with `-g` in a debug build).
//!
//! As `crates/kiln_parse/tests/chain_scaling.rs`, the test counts the steps
//! of the expression walks ([`kiln_ast::expr::walk_steps`]), not time, at
//! N and 4N, once each: a walk repeated per node of the chain makes the
//! count grow about 16x, a linear compile about 4x. Debug builds only.

use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_codegen::{CodegenOptions, DebugInfoKind, DebugOptions, emit_with_debug};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// 4x the chain may cost at most this many times the walk steps (the
/// bench scaling sweeps' bound, spec/bench.md "Scaling sweeps").
const MAX_RATIO: f64 = 4.6;

/// The chain length of the smaller compile.
const N: usize = 400;

/// Parses and lowers `src` with `-g` at `-fclang-compat=<compat>`; the
/// walk steps of both.
fn lower(src: &str, compat: u32) -> u64 {
    let before = kiln_ast::expr::walk_steps().unwrap_or(0);
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { clang_compat: compat, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let opts = CodegenOptions {
        source_filename: "t.cpp".to_owned(),
        exceptions: true,
        clang_compat: compat,
        debug: DebugOptions {
            kind: DebugInfoKind::Constructor,
            main_file: "t.cpp".to_owned(),
            main_id: Some(file),
            comp_dir: "/chain".to_owned(),
            ..DebugOptions::default()
        },
        ..CodegenOptions::default()
    };
    let (m, _) = emit_with_debug(&mut tu.ast, &mut tu.oven, &target, &opts, Some(&sm));
    m.expect("codegen");
    kiln_ast::expr::walk_steps().unwrap_or(0) - before
}

/// Asserts the chain `piece` repeated between `pre` and `post` lowers in
/// linear work, as clang 19 and as clang 22.
#[track_caller]
fn assert_linear(what: &str, pre: &str, piece: &str, post: &str) {
    if kiln_ast::expr::walk_steps().is_none() {
        return; // a release build counts nothing
    }
    for compat in [19, 22] {
        let run = |n: usize| {
            let src = format!("{pre}{}{post}", piece.repeat(n));
            // The parser and the lowering recurse per chain node: the
            // driver's stack, not the test harness's 2 MiB.
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || lower(&src, compat))
                .expect("spawn")
                .join()
                .expect("compile thread")
        };
        let (small, large) = (run(N), run(4 * N));
        #[allow(clippy::cast_precision_loss, reason = "a ratio of counts")]
        let ratio = large as f64 / small.max(1) as f64;
        eprintln!("{what} (clang {compat}): {small} -> {large} walk steps, {ratio:.2}x");
        assert!(
            ratio <= MAX_RATIO,
            "{what} (clang {compat}): {small} walk steps at N = {N}, {large} at 4N: \
             {ratio:.2}x (limit {MAX_RATIO}x)"
        );
    }
}

/// [expr.ref], [expr.call]: `x.f().f()...` returning a reference.
#[test]
fn member_call_chain() {
    assert_linear("x.f()", "struct X { X& f(); };\nvoid t(X x) { x", ".f()", "; }\n");
}

/// [class.temporary]/3: a chain of calls returning a class with a
/// non-trivial destructor (a temporary and a cleanup per call).
#[test]
fn temporary_call_chain() {
    assert_linear(
        "x.f() by value",
        "struct X { ~X(); X f() const; };\nvoid t(X x) { x",
        ".f()",
        "; }\n",
    );
}

/// [over.sub], [over.match.oper]: `a[0][0]...` and `o << 1 << 1 ...`.
#[test]
fn operator_chains() {
    assert_linear("a[0]", "struct A { A& operator[](int); };\nvoid t(A a) { a", "[0]", "; }\n");
    assert_linear("o << 1", "struct O { O& operator<<(int); };\nvoid t(O o) { o", " << 1", "; }\n");
}
