// [basic.scope.scope]/5 (P2169R4): before C++26 a redeclaration of a
// name-independent `_` is accepted as an extension (clang's
// -Wc++26-extensions), for a variable, a structured binding, an
// init-capture and a non-static data member; the first `_` is not.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ diag: 14:23 warning "placeholder variables are a C++2c extension"
//@ diag: 17:7 warning "placeholder variables are a C++2c extension"
//@ diag: 18:9 warning "placeholder variables are a C++2c extension"
//@ diag: 19:20 warning "placeholder variables are a C++2c extension"
//@ diag: 24:5 warning "placeholder variables are a C++2c extension"
struct P { int a, b; };
struct S { int _; int _; };
int f() {
  int _ = 1;
  int _ = 2;
  auto [_, b] = P{1, 2};
  auto l = [_ = 1, _ = 2] { return 0; };
  return b + l();
}
template <class T> T g(T t) {
  T _ = t;
  T _ = t;
  return t;
}
int h = g(1);
