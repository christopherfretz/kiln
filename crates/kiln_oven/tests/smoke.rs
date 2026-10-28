//! First end-to-end checks of the Oven through the test DSL.

mod common;
use common::*;

/// [expr.const]/5: a constant expression evaluates arithmetic.
#[test]
fn expr_const_arith() {
    ok("static_assert(1 + 2 == 3);");
}

/// [dcl.constexpr]/3: a constexpr function may recurse.
#[test]
fn dcl_constexpr_recursion() {
    ok("constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
        static_assert(fib(10) == 55);");
}
