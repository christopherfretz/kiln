// WP-165: padded `_Atomic` types (size rounded up to a power of two) and
// operands whose memory size differs from the atomic width
// (clang `convertToAtomicIntPointer`, `EmitAtomicInit`).
struct S3 { char c[3]; };
struct S6 { short s[3]; };
_Atomic(S3) a3;
_Atomic(S6) a6;
_Atomic(long double) ald;
long double ldv;
int sizes() {
  return sizeof(a3) + alignof(_Atomic(S3)) * 100 + sizeof(a6) * 1000 +
         alignof(_Atomic(S6)) * 10000;
}
S3 load3() { return __c11_atomic_load(&a3, 5); }
S6 load6(int o) { return __c11_atomic_load(&a6, o); }
void store3(S3 v) { __c11_atomic_store(&a3, v, 5); }
void store6(S6 v) { __c11_atomic_store(&a6, v, 3); }
S3 xchg3(S3 v) { return __c11_atomic_exchange(&a3, v, 5); }
bool cas6(S6 *e, S6 d) {
  return __c11_atomic_compare_exchange_strong(&a6, e, d, 5, 5);
}
void init3(S3 v) { __c11_atomic_init(&a3, v); }
void locals(S3 v) {
  _Atomic(long double) x = 1.0L;
  _Atomic(S3) y = v;
  __c11_atomic_init(&x, 2.0L);
  __c11_atomic_store(&x, ldv, 5);
  __atomic_store(&ldv, &ldv, 5);
  __c11_atomic_store(&y, v, 5);
}
long double loadld() { return __c11_atomic_load(&ald, 5); }
void storeld(long double v) { __c11_atomic_store(&ald, v, 5); }
