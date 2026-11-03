// [dcl.type.splice] (P2996R13): `typename [: r :]` names the type r
// represents, for r from a constexpr variable, `^^` directly, or a
// consteval call, in a declaration, a template argument ([temp.arg.type]:
// GCC 16.1 requires the `typename` there) and an alias-declaration; the
// spliced type is the same type ([temp.type]).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ stdout: 42 2.5 7 k 3 0.5
//@ stdout: 2 4 8
//@ stdout: 1 1 1 1
#include <cstdio>
using info = decltype(^^int);
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
template <class T> struct Box { T v; };
struct S { short a; double b; };
constexpr info ri = ^^int;
consteval info pick(bool c) { return c ? ^^char : ^^short; }
typename [: ri :] x = 41;
typename [: ^^double :] y = 2.5;
Box<typename [: ^^long long :]> b{7};
using CF = typename [: ^^const float :];
typename [: pick(true) :] c = 'k';
int main() {
  typename [: ^^S :] local{3, 0.5};
  std::printf("%d %.1f %lld %c %d %.1f\n", x + 1, y, b.v, c, local.a, local.b);
  std::printf("%zu %zu %zu\n", sizeof(typename [: pick(false) :]), sizeof(Box<typename [: ri :]>),
              sizeof(b));
  std::printf("%d %d %d %d\n", same<decltype(x), int>::value, same<CF, const float>::value,
              same<Box<typename [: ri :]>, Box<int>>::value, same<typename [: ^^S :], S>::value);
  return x == 41 ? 0 : 1;
}
