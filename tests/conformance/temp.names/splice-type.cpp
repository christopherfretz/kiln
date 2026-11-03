// P2996R13 [temp.names]/1, [dcl.type.splice]/1: a splice-specialization-
// specifier `[: r :] < args >` in a type position names the
// specialization of the class or alias template r designates, after
// `typename` or in a type-only context; a dependent r is resolved at
// instantiation. Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 3 4 8 2 1 7
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
extern "C" int printf(const char*, ...);

template <class T> struct Box { T v; };
template <class T> using Ptr = T*;
template <class... Ts> struct List { static constexpr int n = sizeof...(Ts); };

constexpr std::meta::info box = ^^Box;

// Non-dependent: after `typename`, in a type-only context, as a
// template argument after `typename`.
typename [: ^^Box :]<int> b1{3};
using B2 = [: box :]<long>;
Box<typename [: ^^Ptr :]<int>> b3{nullptr};
static_assert(sizeof(B2) == sizeof(long));

// Dependent: an alias, a member typedef, a pack, a return type.
template <std::meta::info R, class T> using Apply = typename [: R :]<T>;
template <std::meta::info R, class T> struct Holder {
  using type = typename [: R :]<T>;
  type value;
};
template <std::meta::info R, class... Ts> using Spread = typename [: R :]<Ts..., int>;
template <std::meta::info R, class T> typename [: R :]<T> make(T x) { return {x}; }

int main() {
  Apply<^^Box, short> s{4};
  Holder<^^Ptr, int> h{nullptr};
  typename [: ^^Box :]<char> local{7};
  printf("%d %d %d %d %d %d\n", b1.v, s.v, (int)sizeof(Apply<^^Box, double>),
         Spread<^^List, char>::n, h.value == nullptr && b3.v == nullptr,
         make<^^Box>(local.v).v);
  return 0;
}
