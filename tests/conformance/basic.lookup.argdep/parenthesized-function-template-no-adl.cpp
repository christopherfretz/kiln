// [basic.lookup.argdep]/1: when the postfix-expression of a call is a
// parenthesized name, argument-dependent lookup is not performed, also
// when the name denotes a function template and the call is dependent: at
// instantiation `(f)(t)` considers only the `f` found at the definition,
// not the associated namespace's. The shape of boost's
// `(iterator_arrow_result)(temp)` in move/detail/reverse_iterator.hpp,
// reached from folly's EvictingCacheMap. WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 1 1 3
#include <cstdio>
namespace a {
struct S {};
template <class T> int f(const T&) { return 2; }
}
namespace b {
template <class T> int f(const T&) { return 1; }
template <class T> int h(const T& t) { return (f)(t); }
template <class T> int g(const T& t) { return ((f))(t); }
int k(a::S s) { return (f)(s); }
template <class T> int both(const T& t) { return f(t) + (b::f)(t); }
}
int main() {
  a::S s;
  std::printf("%d %d %d\n", b::h(s), b::g(s), b::k(s) + b::both(1));
}
