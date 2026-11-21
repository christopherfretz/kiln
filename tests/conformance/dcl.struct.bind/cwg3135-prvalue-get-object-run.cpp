//@ expect: run
//@ skip(clang): clang 22 implements neither CWG 3135 nor P2686R5 references to automatic objects
//@ stdout: 102 1
//@ stdout: 40 44
// [dcl.struct.bind]/7 (CWG 3135), [expr.const.init]/2: constexpr pointers
// to bindings of automatic objects and to tuple-like bindings whose get<i>
// returns a prvalue designate the objects of the current invocation at run
// time (and stay writable through them); a const int binding of a constant
// get is a constant array bound; a namespace-scope one has a constant
// address though it is dynamically initialized.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct S { int x, y; };
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 10; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
struct Z {
  int m;
  template <std::size_t I> constexpr int get() const { return m + int(I); }
};
template <> struct std::tuple_size<Z> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, Z> { using type = int; };
struct N { template <std::size_t I> constexpr int get() const { return int(I) + 20; } };
template <> struct std::tuple_size<const N> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, const N> { using type = int; };
extern "C" int printf(const char *, ...);
Z gz{1};
auto [ge, gf] = gz;
constexpr int *pgf = &gf;

int use(int k) {
  S l{k, k + 1};
  auto [lx, ly] = l;
  constexpr int *p = &ly;
  *p += 10;
  Z z{k};
  auto [e, f] = z;
  constexpr int *pf = &f;
  *pf *= 2;
  const auto [c, d] = X{};
  int arr[d];
  constexpr const int *pd = &d;
  return ly + f + int(sizeof arr / sizeof arr[0]) + *pd + l.y;
}

int main() {
  *pgf += 100;
  printf("%d %d\n", gf, ge);
  printf("%d %d\n", use(1), use(2));
}
