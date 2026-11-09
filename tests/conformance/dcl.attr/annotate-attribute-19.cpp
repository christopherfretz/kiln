// clang's `annotate` attribute, clang 19 (WP-954): as
// annotate-attribute.cpp, but the argument-count and constant-expression
// errors name `[[clang::annotate]]` without its scope.
// [dcl.attr.grammar]/5, [expr.const].
//@ requires: clang<22
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:24 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 40:24 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 32:5 note "declared here"
//@ diag: 41:24 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 41:24 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 33:5 note "declared here"
//@ diag: 42:27 error "'annotate' attribute requires parameter 2 to be a constant expression"
//@ diag: 42:29 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 33:5 note "declared here"
//@ diag: 43:3 error "'annotate' attribute takes at least 1 argument"
//@ diag: 44:3 error "'annotate' attribute takes at least 1 argument"
//@ diag: 45:19 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 46:25 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 50:26 error "'annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 56:14 note "in instantiation of template class 'A<2, W>' requested here"
//@ diag: 50:26 note "read of non-const variable 'value' is not allowed in a constant expression"
//@ diag: 55:23 note "declared here"
//@ diag: 60:60 error "'annotate' attribute requires parameter 2 to be a constant expression"
//@ diag: 61:7 note "in instantiation of template class 'Q<nc>' requested here"
//@ diag: 60:60 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 32:5 note "declared here"
//@ diag: 62:56 error "expression contains unexpanded parameter pack 'Is'"
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
template <int... Is> struct P { [[clang::annotate("p", Is...)]] void f(); };
P<1, 2> p2;
P<> p0;
template <int &...Rs> struct Q { [[clang::annotate("q", 0, Rs...)]] void f(); };
Q<nc> q;
template <int... Is> struct U { [[clang::annotate("u", Is)]] void f(); };
