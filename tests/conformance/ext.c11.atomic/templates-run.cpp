// Clang extension: _Atomic(T) members and the atomic builtins in templates
// (libc++ __cxx_atomic_base_impl): dependent operands and orders are
// checked and lowered at instantiation; ++/-- on a deduced _Atomic(T)&.
//@ expect: run
//@ stdout: 5 14 7
//@ stdout: 1 7 9
//@ stdout: 1 1
//@ stdout: 15 9
//@ stdout: 5 4 3 3
extern "C" int printf(const char *, ...);
template <class T> struct Base {
  _Atomic(T) v;
  Base(T x) { __c11_atomic_init(&v, x); }
  T load(int o) const volatile { return __c11_atomic_load(const_cast<_Atomic(T) *>(&v), o); }
  void store(T x, int o) { __c11_atomic_store(&v, x, o); }
  T add(T x) { return __c11_atomic_fetch_add(&v, x, 5); }
  bool cas(T &e, T d) { return __c11_atomic_compare_exchange_strong(&v, &e, d, 5, 5); }
  static constexpr bool lock_free = __c11_atomic_is_lock_free(sizeof(T));
};
template <class T> T gnu(T *p, T v) { __atomic_store_n(p, v, 5); return __atomic_exchange_n(p, v + v, 4) + __atomic_load_n(p, 2); }
template <class T> auto bump(T &x) { ++x; auto y = x++; return y + x; }
template <class T> T *padv(_Atomic(T *) *p) { return __c11_atomic_fetch_add(p, 2, 5); }
template <int O> int ordered(_Atomic(int) *p) { return __c11_atomic_load(p, O); }
int main() {
  Base<int> bi(3);
  Base<long> bl(4);
  Base<char *> bp(nullptr);
  bi.store(5, 3);
  printf("%d %ld %d\n", bi.add(2), bl.add(6) + bl.load(5), bi.load(2));
  int e = 7;
  bool ok = bi.cas(e, 9);
  printf("%d %d %d\n", ok, e, bi.load(5));
  char buf[4];
  bp.store(buf, 5);
  printf("%d %d\n", (int)(padv<char>(&bp.v) == buf), (int)(bp.load(5) == buf + 2));
  long g = 1;
  short s = 2;
  printf("%ld %d\n", gnu(&g, 5L), gnu(&s, (short)3));
  _Atomic(int) ai = 1;
  _Atomic(double) ad = 0.5;
  int r1 = bump(ai);
  double r2 = bump(ad);
  printf("%d %g %d %d\n", r1, r2, ordered<5>(&ai), ordered<0>(&ai));
  static_assert(Base<int>::lock_free && Base<long>::lock_free);
}
