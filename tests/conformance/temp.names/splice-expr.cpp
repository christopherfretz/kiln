// P2996R13 [expr.prim.splice]/3, [temp.names]/1: `template [: r :] <
// args >` in an expression names a variable template specialization, a
// concept-id or a function template specialization; `template [: r :]`
// alone is a function template's overload set, its arguments deduced by
// the call. A dependent r is resolved at instantiation. Compared with
// GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 8 8 10 1 0 12 42 8 1 2 6
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
extern "C" int printf(const char*, ...);

template <class T> constexpr int var = sizeof(T) * 2;
template <class T> constexpr T twice(T x) { return x + x; }
template <class T> concept Small = sizeof(T) <= 4;
template <class... Ts> constexpr int count = sizeof...(Ts);

// Non-dependent.
constexpr int v1 = template [: ^^var :]<int>;
constexpr int v2 = template [: ^^twice :]<int>(4);
constexpr int v3 = template [: ^^twice :](5);
constexpr bool v4 = template [: ^^Small :]<int>;
constexpr bool v5 = template [: ^^Small :]<double>;
constexpr int (*pf)(int) = template [: ^^twice :]<int>;

// Dependent.
template <std::meta::info R, class T> constexpr int value() { return template [: R :]<T>; }
template <std::meta::info R, class T> constexpr T call(T x) { return template [: R :]<T>(x); }
template <std::meta::info R> constexpr int deduce() { return template [: R :](21); }
template <std::meta::info R, class T>
  requires (template [: R :]<T>)
constexpr int pick() { return 1; }
template <std::meta::info R, class T> constexpr int pick() { return 2; }
template <std::meta::info R, class... Ts> constexpr int spread() {
  return template [: R :]<Ts..., int>;
}

int main() {
  printf("%d %d %d %d %d %d %d %d %d %d %d\n", v1, value<^^var, int>(), v3, v4, v5,
         pf(6), deduce<^^twice>(), call<^^twice>(4), pick<^^Small, int>(),
         pick<^^Small, double>(), v2 - 2 + spread<^^count, char>() - 2);
  return 0;
}
