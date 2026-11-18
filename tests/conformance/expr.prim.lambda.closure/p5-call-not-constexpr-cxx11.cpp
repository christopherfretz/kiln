// [expr.prim.lambda.closure]/5, /8 at C++11 (C++11 [expr.const]/2: a
// lambda-expression is not a core constant expression): as at C++14
// (`p5-call-not-constexpr-before-cxx17.cpp`), the call operator and the
// conversion function are not constexpr and a lambda-expression in a
// constant-evaluated context is an error. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 26:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 26:19 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-cxx11.cpp:26:19)' cannot be used in a constant expression"
//@ diag: 28:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 28:19 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 27:10 note "declared here"
//@ diag: 29:17 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 29:24 note "non-constexpr function 'operator int (*)()' cannot be used in a constant expression"
//@ diag: 27:10 note "declared here"
//@ diag: 30:15 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:15 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-cxx11.cpp:30:15)' cannot be used in a constant expression"
//@ diag: 31:16 warning "'constexpr' on lambda expressions is a C++17 extension"
//@ diag: 32:17 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 32:24 note "non-constexpr function 'operator int (*)()' cannot be used in a constant expression"
//@ diag: 31:11 note "declared here"

constexpr int a = []{ return 1; }();
auto l = []{ return 1; };
constexpr int c = l();
constexpr int (*p)() = l;
static_assert([]{ return true; }(), "");
auto cl = []() constexpr { return 1; };
constexpr int (*q)() = cl;
