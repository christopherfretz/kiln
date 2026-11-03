//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
// [dcl.pre]/6, [dcl.struct.bind] (C++26, P2686R5): a structured binding
// declaration may be constexpr, static constexpr or constinit. The
// bindings of a constexpr one are usable in constant expressions
// ([expr.const]/3): bindings to a class's members, to an array's elements
// and of a tuple-like type, whose get calls and holding variables are
// evaluated at compile time; the P1061 pack form composes.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct S { int a, b; };
struct T2 {
  int x;
  long y;
  template <unsigned long I> constexpr auto get() const {
    if constexpr (I == 0) return x; else return y;
  }
};
template <> struct std::tuple_size<T2> { static constexpr unsigned long value = 2; };
template <> struct std::tuple_element<0, T2> { using type = int; };
template <> struct std::tuple_element<1, T2> { using type = long; };

constexpr S cs{1, 2};
constexpr auto [a1, b1] = cs;
static_assert(a1 == 1 && b1 == 2);
constinit auto [a2, b2] = cs;
static constexpr auto [a3, b3] = S{3, 4};
static_assert(a3 + b3 == 7);
constexpr int arr[3] = {5, 6, 7};
constexpr auto [e0, e1, e2] = arr;
static_assert(e0 + e1 + e2 == 18);
constexpr auto& [r0, r1, r2] = arr;
static_assert(&r1 == &arr[1]);
constexpr auto [t0, t1] = T2{8, 9};
static_assert(t0 == 8 && t1 == 9);

template <int N> struct Tag { static constexpr int v = N; };
constexpr int local() {
  constexpr auto [x, y] = S{12, 13};
  static_assert(x == 12);
  static constexpr auto [p, q] = T2{14, 15};
  static_assert(q == 15);
  return Tag<y>::v + p;
}
static_assert(local() == 13 + 14);

template <class U> constexpr int dep(U u) {
  constexpr auto [x, y] = U{16, 17};
  static_assert(x + y == 33);
  static constexpr auto [...zs] = U{1, 2};
  static_assert(sizeof...(zs) == 2 && (zs + ...) == 3);
  constexpr auto [h, ...rest] = arr;
  static_assert(sizeof...(rest) == 2);
  return x + (zs + ...) + (h + ... + rest) + u.a;
}
static_assert(dep(S{1, 0}) == 16 + 3 + 18 + 1);
