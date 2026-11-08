// [temp.param]/10: a non-type template parameter whose type, once prior
// arguments are substituted, is a function or array type has the adjusted
// pointer type, in a member template of a class template too. WP-811
// (gRPC's `ParseValue<F, G>::Parse<F f, G g>` with `F` a function type).
//@ expect: run
//@ strict-diags
//@ stdout: 1042 10 9
extern "C" int printf(const char *, ...);
template <typename F, typename G>
struct PV {
  template <F f, G g>
  static auto parse(int v) -> decltype(g(f(v, true))) { return g(f(v, true)); }
};
int twice(int x, bool b) { return b ? 2 * x : x; }
long widen(int x) { return x + 1000L; }
template <typename T, T t> struct H { using type = decltype(t); static constexpr auto p = t; };
template <typename T, T a> struct A { static constexpr auto p = a; };
const int arr[3] = {7, 8, 9};
int main() {
  long r = PV<int(int, bool), long(int)>::parse<twice, widen>(21);
  static_assert(__is_same(H<int(int, bool), twice>::type, int (*)(int, bool)));
  static_assert(__is_same(decltype(A<const int[3], arr>::p), const int *const));
  printf("%ld %d %d\n", r, H<int(int, bool), twice>::p(5, true), A<const int[3], arr>::p[2]);
}
