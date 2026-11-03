//@ args: -O1
// [atomics.types.generic], [basic.lval]/11: TBAA on atomic accesses at
// -O1 (clang CGAtomic). A load or store of an `_Atomic` lvalue (`EmitAtomicLoad`,
// `EmitAtomicStore`, its initialization `EmitAtomicInit`) carries the
// lvalue's tag, whose access type `_Atomic(T)` is `omnipotent char` (also
// as a class member, on its struct path); the `__atomic_*` / `__c11_atomic_*`
// builtin operation itself is untagged, the result read back from its
// temporary has the value type's tag, and an aggregate result is copied
// with `!tbaa.struct`. `atomic<T>` is the libc++ shape (an `_Atomic(T)`
// member driven by the `__c11_atomic_*` builtins). `optnone` keeps the bodies.
#define ON __attribute__((optnone))
_Atomic(int) ai; _Atomic(float) af; _Atomic(long*) ap;
struct S { int a, b; };
_Atomic(S) as;
ON int g1(int *p) { return __atomic_load_n(p, 2); }
ON void g2(int *p, int v) { __atomic_store_n(p, v, 5); }
ON void g3(int *p, int *r) { __atomic_load(p, r, 5); __atomic_store(p, r, 5); }
ON int c1() { return __c11_atomic_load(&ai, 5); }
ON void c2(int v) { __c11_atomic_store(&ai, v, 5); }
ON float c3() { return __c11_atomic_load(&af, 5); }
ON S c4() { return __c11_atomic_load(&as, 5); }
ON void c5(S v) { __c11_atomic_store(&as, v, 5); }
ON int l1() { ai = 3; af = 1.5f; ap = 0; return ai; }
ON int x1(int *p, int v) { return __atomic_exchange_n(p, v, 5) + __atomic_fetch_add(p, 1, 5); }
ON bool x2(int *p, int *e, int d) { return __atomic_compare_exchange_n(p, e, d, false, 5, 2); }
ON void loc() { _Atomic(int) x = 5; x = 7; int y = x; (void)y; }
_Atomic(bool) ab; _Atomic(short) gs; _Atomic(double) gd;
struct M { int k; _Atomic(int) m; _Atomic(S) sa; };
ON bool b1(bool x) { ab = x; return ab; }
ON int m1(M *p) { p->m = 4; _Atomic(S) s; s = p->sa; p->sa = s; S v{1,2}; p->sa = v; return p->m; }
ON void cm() { gs *= 2; gd += 1.5; ab = !ab; ++gs; }
ON int ref(_Atomic(int) &r) { r += 2; return r++; }
ON void a32(_Atomic(S) &d, _Atomic(S) &e) { d = e; }
template <class T> struct atomic {
  _Atomic(T) v;
  ON T load() const { return __c11_atomic_load(const_cast<_Atomic(T) *>(&v), 5); }
  ON void store(T x) { __c11_atomic_store(&v, x, 5); }
  ON T fetch_add(T x) { return __c11_atomic_fetch_add(&v, x, 5); }
};
ON int sa(atomic<int> &a, atomic<long> &b) { a.store(1); b.store(2); return a.load() + a.fetch_add(3) + int(b.load()); }
