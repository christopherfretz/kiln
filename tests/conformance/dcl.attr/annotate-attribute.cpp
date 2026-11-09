// clang's `annotate` attribute (WP-954; clang `handleAnnotateAttr` and
// `ConstantFoldAttrArgs`, [dcl.attr.grammar]/5): at least one argument, a
// string literal first; each other argument is converted as a function
// argument and must be a constant expression ([expr.const]), else
// `requires parameter N to be a constant expression` with the
// evaluation's notes. A value-dependent argument is checked when its
// template is instantiated (A<2, W>); an argument may be a pack expansion
// (P, Q; [temp.variadic]/5), and a pack outside one is an error (U,
// [temp.variadic]/12). Clang 19 names the attribute
// without its scope: annotate-attribute-19.cpp.
//@ requires: clang>=22
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:24 error "'clang::annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 46:24 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 38:5 note "declared here"
//@ diag: 47:24 error "'clang::annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 47:24 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 39:5 note "declared here"
//@ diag: 48:27 error "'clang::annotate' attribute requires parameter 2 to be a constant expression"
//@ diag: 48:29 note "non-constexpr function 'f' cannot be used in a constant expression"
//@ diag: 39:5 note "declared here"
//@ diag: 49:3 error "'clang::annotate' attribute takes at least 1 argument"
//@ diag: 50:3 error "'clang::annotate' attribute takes at least 1 argument"
//@ diag: 51:19 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 52:25 error "expected string literal as argument of 'annotate' attribute"
//@ diag: 56:26 error "'clang::annotate' attribute requires parameter 1 to be a constant expression"
//@ diag: 62:14 note "in instantiation of template class 'A<2, W>' requested here"
//@ diag: 56:26 note "read of non-const variable 'value' is not allowed in a constant expression"
//@ diag: 61:23 note "declared here"
//@ diag: 66:60 error "'clang::annotate' attribute requires parameter 2 to be a constant expression"
//@ diag: 67:7 note "in instantiation of template class 'Q<nc>' requested here"
//@ diag: 66:60 note "read of non-const variable 'nc' is not allowed in a constant expression"
//@ diag: 38:5 note "declared here"
//@ diag: 68:56 error "expression contains unexpanded parameter pack 'Is'"
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
