// [expr.new], [temp.res.general]/6: a new-expression with a dependent
// allocated type or dependent operands is checked when it is instantiated.
//@ expect: compile-ok
void *operator new(unsigned long, void *p) noexcept;
template <class T> T &dv();
template <class T> void *voidify(T &r) {
  return const_cast<void *>(static_cast<const volatile void *>(&r));
}
template <class T, class... A, class = decltype(::new (dv<void *>()) T(dv<A>()...))>
constexpr T *construct_at(T *p, A &&...a) {
  return ::new (voidify(*p)) T(static_cast<A &&>(a)...);
}
template <class T> T *make_array(unsigned long n) { return new T[n]; }
template <class T, int N> T (*make_2d(unsigned long n))[N] { return new T[n][N]; }
template <class T> T *make_list(T a, T b) { return new T[2]{a, b}; }
template <class T> auto make_auto(T t) { return new auto(t); }
template <class T, class... A> T *make_brace(A... a) { return new T{a...}; }
template <class T> T *make_global() { return ::new T(); }

struct S { int a; S(int x) : a(x) {} };
struct P { int x, y; };
struct N {};
template <class T, class = decltype(new T(0))> constexpr bool can(int) { return true; }
template <class T> constexpr bool can(...) { return false; }
static_assert(can<int>(0) && can<S>(0) && !can<N>(0));

void f() {
  S s(1);
  construct_at(&s, 5);
  int i;
  construct_at(&i, 3);
  double *d = make_array<double>(4);
  int(*m)[3] = make_2d<int, 3>(2);
  long *l = make_list(1L, 2L);
  char *c = make_auto('x');
  P *p = make_brace<P>(1, 2);
  N *n = make_global<N>();
  (void)d, (void)m, (void)l, (void)c, (void)p, (void)n;
}
