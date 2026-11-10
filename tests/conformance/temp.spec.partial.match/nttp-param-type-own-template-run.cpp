// [temp.spec.partial.match]/2, [temp.deduct.type]/20, [temp.over.link]/6:
// deducing a non-type parameter takes the type of the parameter of the
// template being deduced. Two templates whose non-type parameters sit at the
// same depth and index, typed `same_as<T> auto` and `C auto`, have
// equivalent canonical arguments (the type-constraint is not part of a
// parameter's profile, as in clang's `getUnconstrainedType`), so the
// canonical argument of a later partial specialization names the earlier
// template's parameter. Before WP-1217 kiln deduced from that parameter's
// type, `same_as<T> auto` dependent on `T`, and the type mismatch rejected
// every partial specialization and function template below (a CRTP
// `struct d final : qs<d, dq{}>` saw `qs<d, dq{}>` undefined).
//@ std: c++20
//@ expect: run
//@ stdout: Q 1 2 3
//@ stdout: F 1 2
//@ stdout: O 2 1
//@ stdout: C 1 0
template <class T, class U> concept same_as = __is_same(T, U);
template <class T> concept any = true;
template <class T> concept not_int = !__is_same(T, int);

// Formed first: its `V` is the canonical parameter at (0, 1).
template <typename T, same_as<T> auto V> consteval auto get() { return V; }
template <typename T, same_as<T> auto V> struct holder {};

struct dq { int n = 3; };
template <typename, auto...> struct qs { static constexpr int v = 0; };
template <typename Self, any auto Eq, not_int auto... Args>
struct qs<Self, Eq, Args...> {
  static constexpr int v = 1;
};
template <typename Self, auto Eq, auto Kind>
  requires(__is_same(decltype(Kind), char))
struct qs<Self, Eq, Kind> {
  static constexpr int v = 2;
};
// CRTP: `d` is incomplete when its base is instantiated.
inline constexpr struct d final : qs<d, dq{}> {} d;
inline constexpr struct e final : qs<e, 5, 'k'> {} e;

template <typename, auto> struct w {};
template <typename S, auto E> constexpr int f(w<S, E>) { return 1; }
template <typename S, any auto E> constexpr int g(w<S, E>) { return 2; }

template <typename, auto...> struct os { static constexpr int v = 0; };
template <typename S, auto E, auto... R> struct os<S, E, R...> { static constexpr int v = 1; };
template <typename S, auto E> struct os<S, E> { static constexpr int v = 2; };

template <typename T, any auto V> constexpr int c(holder<T, V> *) { return 1; }
template <typename T, auto V> constexpr int c(...) { return 0; }

extern "C" int printf(const char *, ...);
int main() {
  printf("Q %d %d %d\n", decltype(d)::v, decltype(e)::v, qs<int, dq{}, 1.5>::v + 2);
  printf("F %d %d\n", f(w<int, 5>{}), g(w<long, dq{}>{}));
  printf("O %d %d\n", os<int, 4>::v, os<int, 4, 5>::v);
  printf("C %d %d\n", c<int, 7>(nullptr), get<int, 0>());
}
