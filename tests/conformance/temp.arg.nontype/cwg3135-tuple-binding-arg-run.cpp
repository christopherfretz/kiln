//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 keeps the reference model (the binding names a lifetime-extended temporary) and rejects the arguments
//@ stdout: 8 8 4 4 3 4
//@ stdout: 1 1 1
// [temp.arg.nontype]/6, [dcl.struct.bind]/7 (CWG 3135), [expr.const]/14:
// a tuple-like binding whose get<i> returns a prvalue names the variable
// `T_i r = get<i>(e);`, not a temporary, so at namespace scope (constexpr
// or not) its object is a valid argument for a template parameter of
// reference or pointer type. The argument mangles as that variable
// (`L_Z1bE`, GCC 16.1's spelling; the asm labels below link only if the
// explicit instantiations have exactly these names), and arguments naming
// the same binding are the same.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <class T> struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element;
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 3; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
extern "C" int printf(const char *, ...);

template <int& R> int tm() { return R; }
template <const int& R> int tc() { return R; }
template <int* P> int tp() { return *P; }
template <int& R> struct C { static int* addr() { return &R; } };

auto [a, b] = X{};
constexpr auto [c, d] = X{};
namespace n { auto [e, f] = X{}; }

template int tm<b>();
template int tc<d>();
template int tp<&a>();
template int tm<n::f>();
int probe_b() __asm__("_Z2tmIL_Z1bEEiv");
int probe_d() __asm__("_Z2tcIL_Z1dEEiv");
int probe_a() __asm__("_Z2tpIXadL_Z1aEEEiv");
int probe_f() __asm__("_Z2tmIL_ZN1n1fEEEiv");

static_assert(__is_same(C<b>, C<b>) && !__is_same(C<a>, C<b>));

int main() {
  b = 8;
  int x = tm<b>();
  int y = probe_b();
  int z = tc<d>();
  int w = probe_d();
  int u = probe_a();
  int v = probe_f();
  printf("%d %d %d %d %d %d\n", x, y, z, w, u, v);
  int same = C<b>::addr() == &b;
  int other = C<a>::addr() != C<b>::addr();
  int ptr = tp<&a>() == a;
  printf("%d %d %d\n", same, other, ptr);
}
