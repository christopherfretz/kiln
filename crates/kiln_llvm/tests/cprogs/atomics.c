// GNU atomic builtins: atomicrmw, cmpxchg, fences, atomic load/store.
int counter;
int bump(void) { return __atomic_fetch_add(&counter, 1, __ATOMIC_SEQ_CST); }
int get(void) { return __atomic_load_n(&counter, __ATOMIC_ACQUIRE); }
void set(int v) { __atomic_store_n(&counter, v, __ATOMIC_RELEASE); }
void barrier(void) { __atomic_thread_fence(__ATOMIC_ACQ_REL); }
int swap(int v) { return __atomic_exchange_n(&counter, v, __ATOMIC_RELAXED); }
