// [temp.names]/1, [temp.arg.general] (P2996R13): a splice-specifier that
// forms a whole template-argument is ill-formed, whatever the parameter
// kind and also in a dependent context; it must be written
// `typename [: r :]`, `template [: r :]` or `([: r :])`.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 20:3 error "unparenthesized splice expression cannot be used as a template argument"
//@ diag: 21:3 error "unparenthesized splice expression cannot be used as a template argument"
//@ diag: 22:3 error "unparenthesized splice expression cannot be used as a template argument"
//@ diag: 24:5 error "unparenthesized splice expression cannot be used as a template argument"
template <class T> struct X {};
template <int N> struct Y {};
template <template <class> class TT> struct W {};
constexpr int k = 3;

X<[: ^^int :]> x1;
Y<[: ^^k :]> y1;
W<[: ^^X :]> w1;
template <class T> struct Z {
  X<[: ^^T :]> m;
};
X<int> x2;
