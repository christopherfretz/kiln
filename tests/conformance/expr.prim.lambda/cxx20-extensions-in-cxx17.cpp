// [expr.prim.lambda], [expr.prim.lambda.capture]: a lambda template
// parameter list, '[=, this]' and an init-capture pack are C++20; earlier
// they are extensions (clang -Wc++20-extensions). Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++17
//@ diag: 10:12 warning "explicit template parameter list for lambdas is a C++20 extension"
//@ diag: 12:27 warning "explicit capture of 'this' with a capture default of '=' is a C++20 extension"
//@ diag: 15:13 warning "initialized lambda pack captures are a C++20 extension"
auto a = []<class T>(T) {};
struct S {
  void f() { auto c = [=, this] {}; (void)c; }
};
template <class... A> void g(A... args) {
  auto d = [... xs = args] {};
  (void)d;
}
// Valid before C++20 without a warning.
struct R {
  void f() { auto c = [&, this] {}; auto e = [this] {}; (void)c; (void)e; }
};
template <class... A> void h(A... args) { auto d = [args...] {}; (void)d; }
