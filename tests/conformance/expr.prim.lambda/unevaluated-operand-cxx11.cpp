// C++11 [expr.prim.lambda]/2: a lambda-expression shall not appear in an
// unevaluated operand; clang 22 and GCC 16.1 reject each form below (clang
// at the lambda's `[`, in a template when instantiated), the unevaluated
// operand being reported before the constant expression a template argument
// is. Sibling of `unevaluated-operand-before-cxx20.cpp` (C++17). Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 17:16 error "lambda expression in an unevaluated operand"
//@ diag: 18:19 error "lambda expression in an unevaluated operand"
//@ diag: 19:10 error "lambda expression in an unevaluated operand"
//@ diag: 20:10 error "lambda expression in an unevaluated operand"
//@ diag: 21:45 error "lambda expression in an unevaluated operand"
//@ diag: 22:9 note "in instantiation of function template specialization 'ft<int>' requested here"
template <int N> struct X {};
int s = sizeof([] {});
bool n = noexcept([] {}());
decltype([] { return 1; }()) i = 0;
X<sizeof([] {})> x;
template <class T> int ft() { return sizeof([] {}); }
int y = ft<int>();
