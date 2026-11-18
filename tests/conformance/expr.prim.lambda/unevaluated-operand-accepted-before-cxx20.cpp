// C++11-17 [expr.prim.lambda]/2: what the rule against lambdas in
// unevaluated operands and (C++17) template-arguments leaves valid, as clang
// 22 accepts it: a template never instantiated is not checked (GCC 16.1
// rejects these at the definition; kiln follows clang), a default argument
// and an unused default template argument are neither (a used one is
// substituted as a template-argument), and a lambda named through a
// variable or written in another lambda's body is not written in one.
// Rejected siblings: `unevaluated-operand-before-cxx20.cpp`,
// `template-argument-cxx17.cpp`. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
template <int N> struct X {};
template <class T> void f() { (void)sizeof([] {}); }
template <class T> struct V { static const int v = sizeof([] {}); };
template <class T> void g() { X<[] { return 1; }()> x; (void)x; }
template <int N = [] { return 1; }()> struct Z {};
constexpr auto l = [] { return 1; };
X<l()> x;
void m(int (*p)() = [] { return 1; });
int a = [] { return (int)sizeof(int); }();
auto k = [] { return X<sizeof(int)>{}; };
