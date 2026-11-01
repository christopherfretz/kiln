// [atomics.types.generic], clang 19 CGAtomic `UseLibcall`: an atomic
// operation whose size is not a power of two or exceeds 16 bytes is a call
// to the generic libatomic functions (size first, then the object, the
// operand/result buffers and the memory orders as given).
struct S3 { char c[3]; };
struct S20 { int a[5]; };
struct S32 { long a[4]; };
S3 g3;
S32 g32;
_Atomic(S20) a20;
_Atomic(S32) a32;

S3 load3() { S3 r; __atomic_load(&g3, &r, 5); return r; }
void store3(S3 v, int o) { __atomic_store(&g3, &v, o); }
void store32(S32 v) { __atomic_store(&g32, &v, 3); }
S32 xchg32(S32 v) { S32 r; __atomic_exchange(&g32, &v, &r, 5); return r; }
bool cas32(S32 *e, S32 d) { return __atomic_compare_exchange(&g32, e, &d, false, 5, 2); }
bool cas3(S3 *e, S3 *d, bool w, int s, int f) { return __atomic_compare_exchange(&g3, e, d, w, s, f); }

S20 c11load20(int o) { return __c11_atomic_load(&a20, o); }
void c11store20(S20 v) { __c11_atomic_store(&a20, v, 5); }
S32 c11xchg32(S32 v) { return __c11_atomic_exchange(&a32, v, 4); }
bool c11cas32(S32 *e, S32 d) {
  return __c11_atomic_compare_exchange_strong(&a32, e, d, 5, 5)
      && __c11_atomic_compare_exchange_weak(&a32, e, d, 0, 0);
}
void c11init(S20 v) { __c11_atomic_init(&a20, v); }
