// C++11-17 [expr.prim.lambda]/2 (removed by P0315R4 for C++20): a
// lambda-expression shall not appear in an unevaluated operand. clang 22 and
// GCC 16.1 reject each form below at C++11, 14 and 17; clang reports it at
// the lambda's `[`, in a template when instantiated. C++20 accepts them
// (sibling `unevaluated.cpp`); lambdas in templates never instantiated are
// accepted (sibling `unevaluated-operand-accepted-before-cxx20.cpp`). Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 31:16 error "lambda expression in an unevaluated operand"
//@ diag: 32:19 error "lambda expression in an unevaluated operand"
//@ diag: 33:27 error "lambda expression in an unevaluated operand"
//@ diag: 34:12 error "lambda expression in an unevaluated operand"
//@ diag: 35:21 error "lambda expression in an unevaluated operand"
//@ diag: 36:18 error "lambda expression in an unevaluated operand"
//@ diag: 37:30 error "lambda expression in an unevaluated operand"
//@ diag: 38:22 error "lambda expression in an unevaluated operand"
//@ diag: 39:17 error "lambda expression in an unevaluated operand"
//@ diag: 40:28 error "lambda expression in an unevaluated operand"
//@ diag: 41:23 error "lambda expression in an unevaluated operand"
//@ diag: 42:39 error "lambda expression in an unevaluated operand"
//@ diag: 43:10 error "lambda expression in an unevaluated operand"
//@ diag: 44:29 error "lambda expression in an unevaluated operand"
//@ diag: 45:27 error "lambda expression in an unevaluated operand"
//@ diag: 48:59 error "lambda expression in an unevaluated operand"
//@ diag: 49:9 note "in instantiation of template class 'V<int>' requested here"
//@ diag: 46:45 error "lambda expression in an unevaluated operand"
//@ diag: 47:9 note "in instantiation of function template specialization 'ft<int>' requested here"
template <class T> struct Y {};
int s = sizeof([] {});
bool n = noexcept([] {}());
int al = alignof(decltype([] {}));
Y<decltype([] {})> y;
using T1 = decltype([] {});
typedef decltype([] {}) T2;
template <class T = decltype([] {})> struct W {};
auto f() -> decltype([] { return 1; }());
void g(decltype([] {}) p);
void h() noexcept(noexcept([] {}));
void k(int a = sizeof([] {}));
template <class T> using A = decltype([] {});
decltype([] { return 1; }()) i = 0;
auto l = [] { return sizeof([] {}); };
struct M { int m = sizeof([] {}); };
template <class T> int ft() { return sizeof([] {}); }
int x = ft<int>();
template <class T> struct V { static const int v = sizeof([] {}); };
int z = V<int>::v;
