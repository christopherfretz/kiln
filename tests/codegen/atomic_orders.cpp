// [atomics.order] support: a memory order that is not a constant becomes a
// switch over the valid orders (clang CGAtomic), for loads, stores, RMWs,
// both cmpxchg orders, test_and_set/clear and fences; volatile pointees
// make the operations volatile.
_Atomic(int) gi = 5;
_Atomic(short) gs;
int ld(int o) { return __c11_atomic_load(&gi, o); }
void st(int o, int v) { __c11_atomic_store(&gi, v, o); }
int rmw(int o) { return __c11_atomic_fetch_sub(&gi, 2, o) + __atomic_fetch_or((short *)&gs, 1, o); }
bool cx(int *e, int o, int f) { return __c11_atomic_compare_exchange_strong(&gi, e, 3, o, f); }
bool cw(int *e, int f) { return __c11_atomic_compare_exchange_weak(&gi, e, 3, 5, f); }
bool tas(bool *b, int o) { __atomic_clear(b, 5); return __atomic_test_and_set(b, 5) | __atomic_test_and_set(b, o); }
void clr(volatile bool *b, int o) { __atomic_clear(b, o); }
void fn(int o) { __atomic_thread_fence(o); __atomic_signal_fence(2); }
int vld(volatile _Atomic(int) *p, int o) { return __c11_atomic_load(p, o); }
