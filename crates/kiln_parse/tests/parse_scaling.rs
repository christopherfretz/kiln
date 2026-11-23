//! Scaling of the parse of long runs and deep nests, as tooling clients
//! that parse large real code with kiln as a library meet them (WP-2478):
//! a run of labels ([stmt.label]) and of case labels ([stmt.switch]), a
//! deep member-call chain ([expr.ref]), an `else if` chain
//! ([stmt.if]), nested classes ([class.mem]), a chain of functions each
//! returning a closure of the previous one ([expr.prim.lambda]), and a
//! nested designated initializer list in a template ([dcl.init.aggr],
//! [temp.inst]). Each costs O(N), as in clang.
//!
//! The tests count work, not time, so a loaded host cannot make them
//! flaky: [`kiln_ast::expr::walk_steps`] (expression walks) plus
//! [`kiln_ast::expr::search_steps`] (label and case-value searches, the
//! scopes the unqualified-lookup walk visits, the lookup-stamp
//! fingerprints, rebuilds of the scope stack's walk index), and for the
//! designated lists the number of expression nodes. Each shape is
//! compiled at N and 4N, once; work repeated per element of the run
//! grows the count about 16x, a linear front end about 4x. The bound,
//! [`MAX_RATIO`], is the one of the bench scaling sweeps (spec/bench.md
//! "Scaling sweeps"). The counters are debug-build only; the node count
//! is checked in every build.

use std::fmt::Write as _;
use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// 4x the run may cost at most this many times the work.
const MAX_RATIO: f64 = 4.6;

/// What one compile cost.
struct Cost {
    /// Walk steps plus search steps (0 in a release build).
    steps: u64,
    /// Expression nodes in the AST.
    exprs: usize,
}

fn counted() -> u64 {
    kiln_ast::expr::walk_steps().unwrap_or(0) + kiln_ast::expr::search_steps().unwrap_or(0)
}

/// Compiles `src` with `opts` on a thread with the driver's stack (the
/// parser recurses per nesting level); its cost, after asserting it
/// compiled without a diagnostic.
fn cost(what: &str, src: String, opts: ParseOptions) -> Cost {
    let what = what.to_owned();
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let before = counted();
            let sm = SourceManager::new();
            let file = sm.add_virtual("t.cpp", src).expect("location space");
            let interner = Arc::new(Interner::new());
            let sink = MemorySink::new();
            let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
            let engine = DiagEngine::new(&sm, &dopts, sink.clone());
            let ast =
                parse_file(&sm, &interner, file, &engine, Target::for_arch(Arch::X86_64), &opts);
            let steps = counted() - before;
            let diags = sink.take();
            assert!(diags.is_empty(), "{what}: unexpected diagnostics: {:?}", diags[0].message);
            Cost { steps, exprs: ast.exprs.len() }
        })
        .expect("spawn")
        .join()
        .expect("parse thread")
}

/// The ratio of two counts.
#[allow(clippy::cast_precision_loss, reason = "a ratio of counts")]
fn ratio(small: u64, large: u64) -> f64 {
    large as f64 / small.max(1) as f64
}

/// Asserts that `generate(4 * n)` costs at most [`MAX_RATIO`] times
/// `generate(n)`, as clang 19 and as clang 22: in counted steps (debug
/// builds) and, when `nodes`, in expression nodes.
#[track_caller]
fn assert_linear(what: &str, n: usize, nodes: bool, generate: impl Fn(usize) -> String) {
    for compat in [19, 22] {
        let opts = ParseOptions { clang_compat: compat, ..ParseOptions::default() };
        let small = cost(what, generate(n), opts.clone());
        let large = cost(what, generate(4 * n), opts);
        if kiln_ast::expr::walk_steps().is_some() {
            let r = ratio(small.steps, large.steps);
            eprintln!("{what} (clang {compat}): {} -> {} steps, {r:.2}x", small.steps, large.steps);
            assert!(
                r <= MAX_RATIO,
                "{what} (clang {compat}): {} steps at N = {n}, {} at 4N: {r:.2}x (limit {MAX_RATIO}x)",
                small.steps,
                large.steps
            );
        }
        if nodes {
            let r = ratio(small.exprs as u64, large.exprs as u64);
            eprintln!("{what} (clang {compat}): {} -> {} nodes, {r:.2}x", small.exprs, large.exprs);
            assert!(
                r <= MAX_RATIO,
                "{what} (clang {compat}): {} expression nodes at N = {n}, {} at 4N: {r:.2}x \
                 (limit {MAX_RATIO}x)",
                small.exprs,
                large.exprs
            );
        }
    }
}

/// [stmt.label], [stmt.goto]: a run of N labels, each the target of a
/// `goto` (a label search per label and per `goto`).
#[test]
fn label_run() {
    assert_linear("labels", 500, false, |n| {
        let mut s = String::from("void f(int i) {\n");
        for k in 0..n {
            let _ = writeln!(s, "l{k}: if (i == {k}) goto l{};", (k + 1) % n);
        }
        s.push_str("}\n");
        s
    });
}

/// [stmt.switch]: a switch of N case labels (a duplicate check per
/// case), and of N case ranges (the GNU extension).
#[test]
fn case_run() {
    assert_linear("cases", 500, false, |n| {
        let mut s = String::from("int f(int x) {\n  switch (x) {\n");
        for k in 0..n {
            let _ = writeln!(s, "  case {k}: return {k};");
        }
        s.push_str("  }\n  return -1;\n}\n");
        s
    });
    assert_linear("case ranges", 500, false, |n| {
        let mut s = String::from("int f(int x) {\n  switch (x) {\n");
        for k in 0..n {
            let _ = writeln!(s, "  case {} ... {}: return {k};", 4 * k, 4 * k + 2);
        }
        s.push_str("  }\n  return -1;\n}\n");
        s
    });
}

/// [expr.ref], [expr.call]: `a.f().f()...` (the chain tests are
/// `chain_scaling.rs`; this one counts the searches too).
#[test]
fn member_call_chain() {
    assert_linear("a.f()", 400, false, |n| {
        format!("struct A {{ A& f(); }};\nvoid g(A a) {{ a{}; }}\n", ".f()".repeat(n))
    });
}

/// [stmt.if]: an `else if` chain of N branches, each a scope one deeper
/// (every name lookup in it used to walk, and every lookahead memo
/// fingerprint used to hash, all N enclosing scopes).
#[test]
fn else_if_chain() {
    assert_linear("else if", 200, false, |n| {
        let mut s = String::from("int f(int x) {\n  if (x == 0) return 0;\n");
        for k in 1..n {
            let _ = writeln!(s, "  else if (x == {k}) return {k};");
        }
        s.push_str("  return -1;\n}\n");
        s
    });
}

/// [class.mem], [class.nest]: N nested classes, the innermost with a
/// member (the class-head name of each was looked up through every
/// enclosing class, and each class scope rescanned the scopes below it).
#[test]
fn nested_classes() {
    assert_linear("nested classes", 100, false, |n| {
        let mut s = String::new();
        for k in 0..n {
            let _ = writeln!(s, "struct C{k} {{");
        }
        s.push_str("int x;\n");
        s.push_str(&"};".repeat(n));
        s.push('\n');
        s
    });
}

/// [expr.prim.lambda], [dcl.spec.auto]: N functions, each returning a
/// closure that calls the previous one.
#[test]
fn closure_chain() {
    assert_linear("closures", 100, false, |n| {
        let mut s = String::from("auto f0() { return [] { return 0; }; }\n");
        for k in 1..n {
            let _ = writeln!(s, "auto f{k}() {{ return [] {{ return f{}(); }}; }}", k - 1);
        }
        s
    });
}

/// [dcl.init.aggr]/3, [temp.inst]: a designated initializer list nested
/// N deep in a class template, instantiated: one written form per
/// written list, N + 1 in all, not (N + 1)(N + 2)/2 (each level's
/// written form rebuilt every inner one).
#[test]
fn nested_designated_lists() {
    assert_linear("nested designated lists", 40, true, |n| {
        let mut s = String::from("struct N0 { int x; };\n");
        for k in 1..=n {
            let _ = writeln!(s, "struct N{k} {{ N{} a; }};", k - 1);
        }
        let mut init = String::from("{.x = 1}");
        for _ in 0..n {
            init = format!("{{.a = {init}}}");
        }
        let _ = writeln!(s, "template <class T> struct A {{ static inline N{n} w = {init}; }};");
        let _ = writeln!(s, "int use = A<int>::w{}.x;", ".a".repeat(n));
        s
    });
}
