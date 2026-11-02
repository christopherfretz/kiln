//@ expect: ast
//@ ast-filter: test
//@ requires: clang>=22
// [dcl.struct.bind]/4 (C++26 /7): without a member get template, the
// initializer is `get<i>(xe)` with get found only by argument-dependent
// lookup ([basic.lookup.argdep]); the call is marked `adl`.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
namespace n {
struct R { int a; long b; };
template <unsigned long I> auto &get(R &r) {
  if constexpr (I == 0) return r.a; else return r.b;
}
}
template <> struct std::tuple_size<n::R> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, n::R> { using type = int; };
template <> struct std::tuple_element<1, n::R> { using type = long; };
long test_adl(n::R r) {
  auto &[a, b] = r;
  return a + b;
}
