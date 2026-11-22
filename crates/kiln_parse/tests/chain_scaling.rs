//! Scaling of long postfix and operator chains ([expr.post], [expr.ref],
//! [expr.call], [expr.sub], [over.match.oper]): a chain of N member calls
//! `x.f().f()...` (generated code: builders, fluent interfaces) costs
//! O(N), as in clang (WP-2201, reported by a downstream user: 23.6 s at
//! N = 20,000 before the fix).
//!
//! The tests count work, not time, so a loaded host cannot make them
//! flaky: [`kiln_ast::expr::walk_steps`] counts the steps of every
//! expression walk (listing a node's children, one step of a begin
//! location walk in sema or the Oven). Each shape is compiled at N and
//! 4N, once; a walk repeated per node of the chain makes the count grow
//! about 16x, a linear front end about 4x. The bound,
//! [`MAX_RATIO`], is the one of the bench scaling sweeps
//! (spec/bench.md "Scaling sweeps"). Debug builds only (the counter is
//! not compiled into a release build).

mod common;
use common::{Parsed, opts19, parse_with};
use kiln_parse::ParseOptions;

/// 4x the chain may cost at most this many times the walk steps.
const MAX_RATIO: f64 = 4.6;

/// The chain length of the smaller compile.
const N: usize = 400;

/// Compiles `pre + piece * n + post` with `opts` on a thread with the
/// driver's stack (the dump and the parser recurse per chain node); the
/// walk steps it took and the result.
fn steps(pre: &str, piece: &str, post: &str, n: usize, opts: ParseOptions) -> (u64, Parsed) {
    let src = format!("{pre}{}{post}", piece.repeat(n));
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let before = kiln_ast::expr::walk_steps().unwrap_or(0);
            let p = parse_with(&src, &opts);
            (kiln_ast::expr::walk_steps().unwrap_or(0) - before, p)
        })
        .expect("spawn")
        .join()
        .expect("parse thread")
}

/// Asserts the chain `piece` repeated between `pre` and `post` compiles
/// cleanly and in linear work, as clang 19 and as clang 22 (their sema
/// paths differ: the clang 19 one checks more call operands).
#[track_caller]
fn assert_linear(what: &str, pre: &str, piece: &str, post: &str) {
    if kiln_ast::expr::walk_steps().is_none() {
        return; // a release build counts nothing
    }
    for opts in [opts19(), ParseOptions::default()] {
        let compat = opts.clang_compat;
        let (small, p) = steps(pre, piece, post, N, opts.clone());
        p.assert_clean();
        let (large, p) = steps(pre, piece, post, 4 * N, opts);
        p.assert_clean();
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

/// [expr.ref], [expr.call]: `x.f().f()...`, each call's object the
/// previous call (the reported shape).
#[test]
fn member_call_chain() {
    assert_linear("x.f()", "struct X { X& f(); };\nvoid t(X x) { x", ".f()", "; }\n");
}

/// [expr.ref]/2: `p->f()->f()...`.
#[test]
fn arrow_call_chain() {
    assert_linear("p->f()", "struct X { X* f(); };\nvoid t(X* p) { p", "->f()", "; }\n");
}

/// [class.temporary]/2: a chain of calls returning a trivial class by
/// value materializes a temporary per call (the object of the next).
#[test]
fn prvalue_call_chain() {
    assert_linear(
        "x.f() by trivial value",
        "struct X { X f(); };\nvoid t(X x) { x",
        ".f()",
        "; }\n",
    );
}

/// [dcl.fct.default]: a chain of calls each using a default argument
/// (a by-value class one: a temporary too), of a plain member and of a
/// template's member (instantiated per call, [temp.inst]/12).
#[test]
fn default_arg_call_chain() {
    assert_linear(
        "x.f() default argument",
        "struct D { ~D(); };\nstruct X { X& f(int = 0, D = D()); };\nvoid t(X x) { x",
        ".f()",
        "; }\n",
    );
    assert_linear(
        "x.f() instantiated default argument",
        "template <class T> struct X { X& f(T = T()); };\nvoid t(X<int> x) { x",
        ".f()",
        "; }\n",
    );
}

/// [class.temporary]/3: a chain of calls returning a class with a
/// non-trivial destructor binds a temporary per call.
#[test]
fn temporary_call_chain() {
    assert_linear(
        "x.f() by value",
        "struct X { ~X(); X f() const; };\nvoid t(X x) { x",
        ".f()",
        "; }\n",
    );
}

/// [dcl.init.ref]: a reference initialized from a chain (its initializer
/// is evaluated as a potential constant, which compiles it in the Oven).
#[test]
fn reference_init_chain() {
    assert_linear(
        "X& r = x.f()...",
        "struct X { X& f(); };\nvoid t(X x) { X& r = x",
        ".f()",
        "; (void)r; }\n",
    );
}

/// [expr.const]: a chain evaluated in a constant expression (the Oven
/// compiles every node of it).
#[test]
fn constexpr_call_chain() {
    assert_linear(
        "constexpr x.f()",
        "struct X { int v; constexpr X& f() { ++v; return *this; } };\n\
         constexpr int t() { X x{0}; x",
        ".f()",
        "; return x.v; }\nstatic_assert(t() > 0);\n",
    );
}

/// [temp.inst]: a chain in a template instantiated for a class.
#[test]
fn instantiated_call_chain() {
    assert_linear(
        "template x.f()",
        "struct X { X& f(); };\ntemplate <class T> void t(T x) { x",
        ".f()",
        "; }\ntemplate void t<X>(X);\n",
    );
}

/// [expr.ref]: `x.a.a...` of a reference member.
#[test]
fn data_member_chain() {
    assert_linear("x.a", "struct X { X& a; };\nvoid t(X x) { (void)x", ".a", "; }\n");
}

/// [over.call]: `f()()...` of a function object.
#[test]
fn call_operator_chain() {
    assert_linear("f()()", "struct F { F& operator()(); };\nvoid t(F f) { f", "()", "; }\n");
}

/// [over.sub]: `a[0][0]...` of a class subscript.
#[test]
fn subscript_chain() {
    assert_linear("a[0]", "struct A { A& operator[](int); };\nvoid t(A a) { a", "[0]", "; }\n");
}

/// [over.match.oper]: `o << 1 << 1 ...` (the generated `std::cout << ...`
/// form), and the built-in `<<` on `int`.
#[test]
fn shift_chain() {
    assert_linear("o << 1", "struct O { O& operator<<(int); };\nvoid t(O o) { o", " << 1", "; }\n");
    assert_linear("int << 1", "void t(int o) { (void)(o", " << 1", "); }\n");
}
