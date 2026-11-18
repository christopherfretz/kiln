// C++17 [expr.prim.lambda]/2 (removed by P0315R4 for C++20): a
// lambda-expression shall not appear in a template-argument, even where
// C++17 makes it a constant expression. clang 22 and GCC 16.1 reject each
// form below at C++17 ("cannot appear in this context", at the lambda's
// `[`; in a template when instantiated; in a default template argument when
// it is used). A lambda named through a variable or nested in another
// lambda's body is not written in one (sibling
// `unevaluated-operand-accepted-before-cxx20.cpp`). Before C++17 the same
// forms are not constant expressions (`expr.prim.lambda.closure/
// p5-call-not-constexpr-before-cxx17.cpp`); C++20 accepts them
// (`template-argument.cpp`). Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 25:3 error "a lambda expression cannot appear in this context"
//@ diag: 26:4 error "a lambda expression cannot appear in this context"
//@ diag: 27:18 error "a lambda expression cannot appear in this context"
//@ diag: 29:19 error "a lambda expression cannot appear in this context"
//@ diag: 30:3 error "a lambda expression cannot appear in this context"
//@ diag: 31:34 error "a lambda expression cannot appear in this context"
//@ diag: 32:15 note "in instantiation of function template specialization 'ft<int>' requested here"
//@ diag: 33:19 error "a lambda expression cannot appear in this context"
//@ diag: 34:1 note "in instantiation of default argument for 'Z<>' required here"
template <int N> struct X {};
X<[] { return 1; }()> a;
X<([] { return 1; })()> b;
int c = sizeof(X<[] { return 1; }()>);
template <int N> void fn(X<N>) {}
void use() { fn(X<[] { return 2; }()>{}); }
X<[] { return [] { return 1; }(); }()> d;
template <class T> void ft() { X<[] { return 1; }()> x; (void)x; }
template void ft<int>();
template <int N = [] { return 1; }()> struct Z {};
Z<> z;
