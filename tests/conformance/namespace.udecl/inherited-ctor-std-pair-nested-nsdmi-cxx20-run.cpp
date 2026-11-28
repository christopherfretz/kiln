// [namespace.udecl]/14, [temp.deduct.general]/8, [except.spec]/7: C++20
// variant of inherited-ctor-std-pair-nested-nsdmi-cxx17-run.cpp; in C++20
// libstdc++ `std::pair::pair()` is also constrained with
// `is_default_constructible_v`, which hits `__is_constructible(M::Data)` at
// `std::pair`'s `}` (reduced from real code, regression since 1f2951a7b,
// WP-3685).
//@ expect: run
//@ std: c++20
#include <utility>
template <class K, class V> struct DP : std::pair<K, V> { using std::pair<K, V>::pair; };
template <class K, class V> struct Map { char s[sizeof(DP<K, V>)]; };
struct M {
  struct Data { bool H = false; unsigned ID = 0; };
  Map<const int *, Data> Info;
};
int main() {
  M m;
  M::Data d;
  return (d.H || d.ID != 0) ? 1 : 0;
}
