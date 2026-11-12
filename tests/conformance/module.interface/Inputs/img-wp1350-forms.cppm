export module imgwf;
export namespace wf {
struct In { int c = 1; int d = 2; };
struct Out { int a = 0; In b; int e = 5; };
template <class T> Out mk(T x) { return Out{.a = int(x), .b = {.d = int(x) + 1}}; }
template <class T> T dep(T x) { return T{.a = x.a + 1, .e = 9}; }
auto abbrev(auto x) { return Out{.b = {.c = int(x), .d = 4}}; }
template <class T> T reorder(int x) { return T{.e = x, .a = 1}; }

using size_t = decltype(sizeof 0);
template <class T, size_t N> using vec __attribute__((__ext_vector_type__(N))) = T;
template <class T, size_t N> T sum(vec<T, N> v) {
  T s = 0;
  for (size_t i = 0; i < N; ++i) s += v[i];
  return s;
}
template <class T> constexpr size_t lanes = 16 / sizeof(T);
template <class T> vec<T, lanes<T>> splat(T x) { return vec<T, lanes<T>>(x); }
template <class T, size_t N> long widen(vec<T, N> v) {
  vec<long, N> w = __builtin_convertvector(v, vec<long, N>);
  return w[0] + w[N - 1];
}

template <class T> struct rr { using type = T; };
template <class T> using rr_t = typename rr<T>::type;
template <class T> struct hp { static constexpr bool value = false; };
template <class T, class D, class R = rr_t<D>, bool = hp<R>::value> struct ptr_of { using type = T*; };
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <class T, class U> struct conv { static constexpr bool value = __is_convertible(T, U); };
template <class T, class D = int> struct up {
  typedef D deleter_type;
  using pointer = typename ptr_of<T, deleter_type>::type;
  pointer p = nullptr;
  template <class UP, class U>
  using EnMove = typename en<conv<typename UP::pointer, pointer>::value && !__is_array(U)>::type;
  up() = default;
  explicit up(pointer q) : p(q) {}
  template <class U, class E, class = EnMove<up<U, E>, U>> up(up<U, E>&& o) : p(o.p) { o.p = nullptr; }
};
struct B { int v = 4; };
struct Dd : B {};

struct Al { int id = 7; };
struct Str {
  int n = 0, a = 0;
  template <class It> Str(It b, It e, const Al& al = Al()) : n(int(e - b)), a(al.id) {}
};
inline Str mks(const char* s) { return Str(s, s + 3); }
template <class T> int g(T x, int k = sizeof(T)) { return int(x) + k; }
inline int use_g() { return g(1.0) + g<char>(2); }
}  // namespace wf
