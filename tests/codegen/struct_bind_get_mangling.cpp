// [dcl.struct.bind]/4, [temp.arg.nontype]/2: the index of a tuple-like
// binding's `get<i>` call is a converted constant expression of the type
// of `get`'s template parameter, not `std::size_t`: `template <int I>
// get` is called as `get<0>` with an `int` argument and mangles `ILi0E`,
// `template <unsigned char I>` as `ILh0E` (both targets: no `size_t`
// spelling in the symbol).
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
}
namespace N {
struct P { int a, b; };
template <int I> int &get(P &p) { return I == 0 ? p.a : p.b; }
struct M {
  int a;
  long b;
  template <unsigned char I> auto &get() {
    if constexpr (I == 0) return a; else return b;
  }
};
struct L { short s; };
template <long long I> short &&get(L &&l) { return static_cast<short &&>(l.s); }
}
template <> struct std::tuple_size<N::P> { static constexpr std::size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, N::P> { using type = int; };
template <> struct std::tuple_size<N::M> { static constexpr std::size_t value = 2; };
template <> struct std::tuple_element<0, N::M> { using type = int; };
template <> struct std::tuple_element<1, N::M> { using type = long; };
template <> struct std::tuple_size<N::L> { static constexpr std::size_t value = 1; };
template <> struct std::tuple_element<0, N::L> { using type = short; };

int f(N::P p, N::M m) {
  auto &[x, y] = p;
  auto &[u, v] = m;
  auto [s] = N::L{3};
  return x + y + u + (int)v + s;
}
