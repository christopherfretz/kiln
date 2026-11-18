// C++14 [expr.const]/2.6 (C++11 [expr.const]/2): an expression whose
// evaluation would evaluate a lambda-expression is not a core constant
// expression, even when its value is discarded. GCC 16.1 rejects these
// ("temporary of non-literal type"); clang 22 accepts the first and reports
// only that the lambda-expression may not appear in the second
// (docs/status/deviations.md). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ args: -Wno-unused-value
//@ skip(clang): clang 22 evaluates a discarded lambda-expression as a constant
//@ diag: 18:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 18:20 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-discarded-lambda-not-constant-before-cxx17.cpp:18:20)' cannot be used in a constant expression"
//@ diag: 19:16 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:16 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-discarded-lambda-not-constant-before-cxx17.cpp:19:16)' cannot be used in a constant expression"

constexpr int d = ([]{}, 0);
static_assert(([]{ return true; }, true), "");
