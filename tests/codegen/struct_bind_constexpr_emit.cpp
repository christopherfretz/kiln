// [dcl.struct.bind]/4, P2686R5: emission of constexpr tuple-like
// decompositions, as clang 22.
// - A block-scope `static constexpr` one's holding references are
//   constant-initialized globals (`@_ZZ1fvE1x = internal constant ptr
//   @_ZZ1fvEDC1x1yE`): no guard, no run-time `get` call.
// - A namespace-scope `e` with internal linkage that its bindings do not
//   refer to (`get` returns by value, so the holding references bind
//   temporaries) is still emitted: clang `DeclMustBeEmitted` requires a
//   decomposition whose holding variables must be emitted, and emits it
//   first (`@_ZDC2vx2vyE`, then `@_ZGR2vx_`, `@vx`).
// (clang 19 has no constexpr structured bindings.)
//@ min-oracle: 22
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> struct tuple_size<const T> {
  static constexpr size_t value = tuple_size<T>::value;
};
template <size_t I, class T> struct tuple_element<I, const T> {
  using type = const typename tuple_element<I, T>::type;
};
}
struct R {
  int a;
  long b;
};
template <std::size_t I> constexpr const auto &&get(const R &&r) {
  if constexpr (I == 0) return static_cast<const int &&>(r.a);
  else return static_cast<const long &&>(r.b);
}
template <> struct std::tuple_size<R> { static constexpr std::size_t value = 2; };
template <> struct std::tuple_element<0, R> { using type = int; };
template <> struct std::tuple_element<1, R> { using type = long; };
struct V {
  int x;
  long y;
  template <std::size_t I> constexpr auto get() const {
    if constexpr (I == 0) return x; else return y;
  }
};
template <> struct std::tuple_size<V> { static constexpr std::size_t value = 2; };
template <> struct std::tuple_element<0, V> { using type = int; };
template <> struct std::tuple_element<1, V> { using type = long; };

long f() {
  static constexpr auto [x, y] = R{1, 2};
  return x + y;
}
const long *g() {
  static constexpr auto [p, q] = R{3, 4};
  return &q;
}
constexpr auto [vx, vy] = V{5, 6};
namespace N { constexpr auto [wx, wy] = V{7, 8}; }
static constexpr auto [sx, sy] = V{9, 10};
constexpr auto [rx, ry] = R{11, 12};
int use() { return 0; }
