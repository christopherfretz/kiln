// clang's `annotate` attribute, clang 19 (WP-954): as
// annotate-attribute.cpp, but the argument-count and constant-expression
// errors name `[[clang::annotate]]` without its scope.
// [dcl.attr.grammar]/5, [expr.const].
//@ requires: clang<22
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:24 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 35:24 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 27:5 note "declared here"
//@ diag: 36:24 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 36:24 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 28:5 note "declared here"
//@ diag: 37:27 error "'annotate' attribute requires parameter 2 to be a constant expression"
//@ diag: 37:29 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 28:5 note "declared here"
//@ diag: 38:3 error "'annotate' attribute takes at least 1 argument"
//@ diag: 39:3 error "'annotate' attribute takes at least 1 argument"
//@ diag: 40:19 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 41:25 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 45:26 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 51:14 note "in instantiation of template class 'A<2, W>' requested here"
//@ diag: 45:26 note "read of non-const variable 'value' is not allowed in a constant expression"
//@ diag: 50:23 note "declared here"
constexpr int ci = 3;
int nc = 4;
int f();
constexpr int g() { return 1; }
struct S { int x; };

[[clang::annotate("a")]] int v1;
[[clang::annotate("b", 1, ci, g(), "s")]] int v2;
__attribute__((annotate("c", 2 + ci))) void fn();
[[clang::annotate("d", nc)]] int v3;
[[clang::annotate("e", f())]] int v4;
[[clang::annotate("f", 1, S{f()})]] int v5;
[[clang::annotate]] int v6;
[[clang::annotate()]] int v7;
[[clang::annotate(1)]] int v8;
__attribute__((annotate(ci))) int v9;

template <int N, class T> struct A {
  [[clang::annotate("t", N)]] void m();
  [[clang::annotate("u", T::value)]] void m2();
  [[clang::annotate("w", sizeof(T) + N)]] int x;
};
struct V { static constexpr int value = 5; };
A<1, V> a1;
struct W { static int value; };
void use() { A<2, W>().m2(); }
