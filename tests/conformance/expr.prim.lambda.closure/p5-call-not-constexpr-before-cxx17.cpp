// [expr.prim.lambda.closure]/5, /8 (C++11/14 wording, changed by P0170R1),
// C++14 [expr.const]/2.6 (C++11 [expr.const]/2): before C++17 a closure's
// call operator and conversion function are not constexpr (a written
// `constexpr` is a C++17 extension and still leaves the conversion
// non-constexpr), a lambda-expression is never a core constant expression
// (clang 22 names the closure type, not literal then), and one written in a
// constant-evaluated context is an error of its own (outside templates; at
// instantiation in one). C++17 and later accept all of these (sibling
// `p5-call-constexpr-cxx17.cpp`). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ args: -Wno-unused-value
//@ diag: 53:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 53:19 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:53:19)' cannot be used in a constant expression"
//@ diag: 54:25 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 54:29 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:54:29)' cannot be used in a constant expression"
//@ diag: 56:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 56:19 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 55:10 note "declared here"
//@ diag: 57:17 error "constexpr variable 'p1' must be initialized by a constant expression"
//@ diag: 57:25 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:57:25)' cannot be used in a constant expression"
//@ diag: 58:17 error "constexpr variable 'p2' must be initialized by a constant expression"
//@ diag: 58:25 note "non-constexpr function 'operator int (*)()' cannot be used in a constant expression"
//@ diag: 55:10 note "declared here"
//@ diag: 59:17 error "constexpr variable 'p3' must be initialized by a constant expression"
//@ diag: 59:26 note "non-constexpr function 'operator int (*)()' cannot be used in a constant expression"
//@ diag: 55:10 note "declared here"
//@ diag: 60:15 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:15 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:60:15)' cannot be used in a constant expression"
//@ diag: 61:14 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 61:14 error "expression is not an integral constant expression"
//@ diag: 61:14 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:61:14)' cannot be used in a constant expression"
//@ diag: 62:29 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 62:29 error "case value is not a constant expression"
//@ diag: 62:29 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:62:29)' cannot be used in a constant expression"
//@ diag: 64:3 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 64:3 error "non-type template argument is not a constant expression"
//@ diag: 64:3 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:64:3)' cannot be used in a constant expression"
//@ diag: 65:18 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 66:16 warning "'constexpr' on lambda expressions is a C++17 extension"
//@ diag: 68:17 error "constexpr variable 'p4' must be initialized by a constant expression"
//@ diag: 68:25 note "non-constexpr function 'operator int (*)()' cannot be used in a constant expression"
//@ diag: 66:11 note "declared here"
//@ diag: 69:32 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 70:15 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 69:32 error "non-type template argument is not a constant expression"
//@ diag: 69:32 note "non-literal type '(lambda at tests/conformance/expr.prim.lambda.closure/p5-call-not-constexpr-before-cxx17.cpp:69:32)' cannot be used in a constant expression"
//@ diag: 71:41 error "a lambda expression may not appear inside of a constant expression"
//@ diag: 72:15 note "in instantiation of function template specialization 'm<int>' requested here"

constexpr int a = []{ return 1; }();
int f() { constexpr int b = []{ return 1; }(); return b; }
auto l = []{ return 1; };
constexpr int c = l();
constexpr int (*p1)() = []{ return 1; };
constexpr int (*p2)() = l;
constexpr int (*p3)() = +l;
static_assert([]{ return true; }(), "");
enum E { e = []{ return 1; }() };
int g() { switch (1) { case []{ return 1; }(): return 1; } return 0; }
template<int N> struct X {};
X<[]{ return 1; }()> x;
template<int N = []{ return 1; }()> struct Y {};
auto cl = []() constexpr { return 1; };
constexpr int d = cl();
constexpr int (*p4)() = cl;
template<class T> void h() { X<[]{ return 1; }()> x; (void)x; }
template void h<int>();
template<class T> void m() { enum { e = []{ return 1; }() }; }
template void m<int>();
template<class T> void k() { static_assert([]{ return true; }(), ""); }
