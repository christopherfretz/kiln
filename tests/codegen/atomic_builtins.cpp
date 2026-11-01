// [atomics] support: the __c11_atomic_* and __atomic_* builtins lowered as
// clang CGAtomic does: load/store atomic, atomicrmw (including the
// *_fetch forms recomputed from the old value), cmpxchg with the expected
// value written back on failure, test_and_set/clear on i8, fences, and the
// lock-free query libcall.
_Atomic(int) ai;
_Atomic(int*) ap;
struct S { int a, b; };
_Atomic(S) as;
int g(int *p) { return __atomic_load_n(p, 2) + __c11_atomic_fetch_add(&ai, 1, 5); }
int *pex(int **pp, int *v) { return __atomic_exchange_n(pp, v, 5); }
int *padd(int **q) { return __c11_atomic_fetch_add(&ap, 3, 5) + (long)__atomic_fetch_add(q, 8, 5); }
S sl() { return __c11_atomic_load(&as, 5); }
void ss(S v) { __c11_atomic_store(&as, v, 5); }
void sgl(S *p, S *r) { __atomic_load(p, r, 5); __atomic_store(p, r, 5); }
bool lf(int *p, long n) { return __atomic_is_lock_free(n, p) + __c11_atomic_is_lock_free(n); }
int mn(unsigned *u, int *i) {
  return __atomic_max_fetch(u, 1u, 5) + __atomic_min_fetch(i, 2, 5) + __atomic_nand_fetch(i, 3, 5);
}
bool gcas(int *p, int *e, int d, bool w) { return __atomic_compare_exchange_n(p, e, d, w, 5, 2); }
bool gw(int *p, int *e, int *d, bool w) { return __atomic_compare_exchange(p, e, d, w, 5, 2); }
bool cw(int *e) { return __c11_atomic_compare_exchange_weak(&ai, e, 3, 4, 0); }
void init(_Atomic(int) *p) { __c11_atomic_init(p, 7); }
void fl(double *p, double v) { __atomic_fetch_add(p, v, 5); __atomic_sub_fetch(p, v, 5); }
float ff(_Atomic(float) *p) { return __c11_atomic_fetch_max(p, 1.0f, 5) + __c11_atomic_fetch_min(p, 2.0f, 5); }
__int128 big(__int128 *p, __int128 v) { return __atomic_fetch_add(p, v, 5) + __atomic_load_n(p, 5); }
template <class T> T tl(_Atomic(T) *p, T v) { __c11_atomic_store(p, v, 3); return __c11_atomic_exchange(p, v, 4); }
long tli(_Atomic(long) *p) { return tl<long>(p, 4) + tl<char>((_Atomic(char) *)p, 1); }
void fences() {
  __c11_atomic_thread_fence(5); __c11_atomic_signal_fence(3);
  __atomic_thread_fence(0); __atomic_signal_fence(4);
}
bool tas(volatile char *c) { __atomic_clear(c, 5); return __atomic_test_and_set(c, 2); }
