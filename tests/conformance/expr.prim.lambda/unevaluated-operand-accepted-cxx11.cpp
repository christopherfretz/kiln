// C++11 [expr.prim.lambda]/2: a lambda-expression in an unevaluated operand
// or a constant expression inside a template that is never instantiated is
// not diagnosed (clang 22; GCC 16.1 rejects at the definition, kiln follows
// clang), nor one in a default argument. Sibling of
// `unevaluated-operand-accepted-before-cxx20.cpp` (C++17). Found running
// GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
template <int N> struct X {};
template <class T> void f() { (void)sizeof([] {}); }
template <class T> struct V { static const int v = sizeof([] {}); };
template <class T> void g() { X<[] { return 1; }()> x; (void)x; }
void m(int (*p)() = [] { return 1; });
int a = [] { return (int)sizeof(int); }();
