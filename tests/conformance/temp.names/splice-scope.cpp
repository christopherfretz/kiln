// P2996R13 [expr.prim.id.qual]/1: a splice-scope-specifier `template [:
// r :] < args > ::` names the class template specialization as a scope,
// in an expression, after `typename`, and as the leading decl-specifier
// of a declaration at namespace and block scope; a dependent r is
// resolved at instantiation. Compared with GCC 16.1 (spec/harness.md
// "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 4 2 8 8 3 5
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
extern "C" int printf(const char*, ...);

template <class T> struct Box {
  static constexpr int k = sizeof(T);
  using type = T;
  static int get(int x) { return x + k; }
};

// Non-dependent.
constexpr int k1 = template [: ^^Box :]<char>::k;
typename template [: ^^Box :]<int>::type n1 = 4;
template [: ^^Box :]<short>::type n2 = 2;
struct S {
  typename template [: ^^Box :]<double>::type d;
};

// Dependent.
template <std::meta::info R, class T> struct Of {
  static constexpr int k = template [: R :]<T>::k;
  using type = typename template [: R :]<T>::type;
};
template <std::meta::info R> int get(int x) { return template [: R :]<char>::get(x); }

int main() {
  template [: ^^Box :]<int>::type local = 3;
  printf("%d %d %d %d %d %d %d\n", k1, n1, n2, (int)sizeof(S), Of<^^Box, double>::k,
         (int)(Of<^^Box, long>::type)local, get<^^Box>(4));
  return 0;
}
