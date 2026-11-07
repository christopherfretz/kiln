// [temp.inst]/4, [dcl.init.list]/3.8, [dcl.init.general]/16: the deferred
// in-class initializer of a static data member of a class template keeps its
// written form when it is instantiated: `E e{v}` is direct-list-
// initialization, which converts an integer to an enumeration with a fixed
// underlying type and may choose an explicit constructor; `= {v}` is
// copy-list-initialization, which does neither.
//@ expect: run
//@ std: c++20
//@ stdout: 1 5 2
#include <cstdio>
enum class P : unsigned char { a = 0, b = 1, c = 2 };
struct E { explicit E(int v) : v(v) {} int v; };
template<class X> struct S {
  static constexpr P q{X::i};
  static inline E e{X::i + 4};
  static constexpr P r[2]{P{X::i}, P{X::i + 1}};
};
struct T { static constexpr int i = 1; };
static_assert(S<T>::q == P::b);
int main() { std::printf("%d %d %d\n", (int)S<T>::q, S<T>::e.v, (int)S<T>::r[1]); }
