// Clang extension: the atomic builtins at run time: constant and runtime
// memory orders, RMWs (and the *_fetch forms), strong/weak cmpxchg with the
// expected value written back, struct-valued load/store/exchange,
// test_and_set/clear, floating add/max, pointer arithmetic in units of the
// pointee, fences, 128-bit operands and the lock-free query.
//@ expect: run
//@ stdout: 1 3
//@ stdout: 7 8
//@ stdout: 0 7
//@ stdout: 1 7 9
//@ stdout: 15 15 6
//@ stdout: -3 100 -3
//@ stdout: 0 10 10
//@ stdout: 1 -3
//@ stdout: 3 4 7 8
//@ stdout: 0 1 0
//@ stdout: 3.5 1 4
//@ stdout: 6 1
//@ stdout: 5 1 3
//@ stdout: 1 1
extern "C" int printf(const char *, ...);
struct alignas(8) P { int a, b; };
_Atomic(int) ai = 1;
_Atomic(P) ap;
int order(int o) { return o; }
template <class T> T bump(_Atomic(T) *p, T v, int o) {
  T old = __c11_atomic_fetch_add(p, v, o);
  return old + __c11_atomic_load(p, 5);
}
int main() {
  int g = 10, e = 10, d = 20;
  int r0 = __c11_atomic_fetch_add(&ai, 2, 5);
  printf("%d %d\n", r0, __c11_atomic_load(&ai, order(2)));
  __c11_atomic_store(&ai, 7, order(3));
  printf("%d %d\n", __c11_atomic_exchange(&ai, 8, 4), __c11_atomic_fetch_sub(&ai, 1, order(5)));
  int exp = 5;
  bool ok = __c11_atomic_compare_exchange_strong(&ai, &exp, 9, 5, 5);
  printf("%d %d\n", ok, exp);
  ok = __c11_atomic_compare_exchange_strong(&ai, &exp, 9, order(4), order(2));
  printf("%d %d %d\n", ok, exp, __c11_atomic_load(&ai, 0));
  printf("%d %d %d\n", __atomic_add_fetch(&g, 5, 5), __atomic_fetch_and(&g, 6, 5), g);
  printf("%d %d %d\n", __atomic_nand_fetch(&g, 3, 5), __atomic_max_fetch(&g, 100, 5), __atomic_min_fetch(&g, -3, 5));
  printf("%d %d %d\n", __atomic_compare_exchange_n(&e, &d, 1, false, 5, 5), e, d);
  printf("%d %d\n", __atomic_compare_exchange(&e, &d, &g, true, 5, 0) || __atomic_compare_exchange(&e, &d, &g, false, 5, 0), e);
  P v = {3, 4};
  __c11_atomic_store(&ap, v, 5);
  P w = __c11_atomic_load(&ap, 5);
  P x = {5, 6}, y;
  __c11_atomic_exchange(&ap, x, 5);
  P z = {7, 8};
  __atomic_load(&z, &y, 5);
  printf("%d %d %d %d\n", w.a, w.b, y.a, y.b);
  bool flag = false;
  printf("%d %d", __atomic_test_and_set(&flag, 5), __atomic_test_and_set(&flag, order(2)));
  __atomic_clear(&flag, order(3));
  printf(" %d\n", flag);
  double dv = 1.5;
  __atomic_fetch_add(&dv, 2.0, 5);
  _Atomic(float) fv = 1.0f;
  printf("%g %g %g\n", dv, __c11_atomic_fetch_max(&fv, 4.0f, 5), __c11_atomic_load(&fv, 5));
  _Atomic(long) l = 1;
  _Atomic(int *) pp = &g;
  printf("%ld %d\n", bump<long>(&l, 4, order(5)), (int)(__c11_atomic_fetch_add(&pp, 1, 5) == &g));
  __atomic_thread_fence(5); __atomic_signal_fence(order(5)); __c11_atomic_thread_fence(order(2));
  long long big = 0;
  __int128 wide = 1;
  printf("%d %d %d\n", (int)__atomic_add_fetch(&big, 5, 5), (int)__atomic_fetch_add(&wide, 2, 5), (int)wide);
  printf("%d %d\n", __atomic_is_lock_free(4, &g), __c11_atomic_is_lock_free(sizeof(P)));
}
