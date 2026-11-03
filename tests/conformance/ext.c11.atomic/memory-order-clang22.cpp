// Clang extension: a constant memory order that the operation does not
// allow warns -Watomic-memory-ordering (the success order of loads, stores
// and RMWs; the failure order of cmpxchg); valid orders do not.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:25 warning "memory order argument to atomic operation is invalid"
//@ diag: 19:29 warning "memory order argument to atomic operation is invalid"
//@ diag: 20:22 warning "memory order argument to atomic operation is invalid"
//@ diag: 21:26 warning "memory order argument to atomic operation is invalid"
//@ diag: 22:53 warning "failure memory order argument to atomic operation is invalid"
//@ diag: 23:50 warning "failure memory order argument to atomic operation is invalid"
//@ diag: 24:21 warning "memory order argument to atomic operation is invalid"
//@ diag: 25:33 warning "memory order argument to atomic operation is invalid"
//@ diag: 26:32 warning "memory order argument to atomic operation is invalid"
//@ requires: clang>=22
// clang 22 spelling of memory-order.cpp (WP-394).
void f(_Atomic(int) *ap, int *p, int *e, bool *b) {
  __c11_atomic_load(ap, 3);
  __c11_atomic_store(ap, 1, 2);
  __atomic_load_n(p, 4);
  __atomic_store_n(p, 1, 1);
  __c11_atomic_compare_exchange_strong(ap, e, 1, 5, 3);
  __atomic_compare_exchange_n(p, e, 1, false, 5, 4);
  __atomic_clear(b, 2);
  __c11_atomic_fetch_add(ap, 1, 7);
  __c11_atomic_exchange(ap, 1, -1);
  __c11_atomic_load(ap, 0); __c11_atomic_store(ap, 1, 3); __atomic_fetch_add(p, 1, 4);
}
