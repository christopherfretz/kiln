// WP-165: atomic builtins clang reports under -Watomic-alignment
// (misaligned and oversized libcalls; the warnings are checked by
// tests/conformance/ext.c11.atomic/alignment-warning-run.cpp).
struct B { char c[32]; };
struct S3 { char c[3]; };
struct __attribute__((packed)) P { char x; long l; };
B gb; S3 g3; P gp; _Atomic(B) ab; _Atomic(int) ai;
void f(B *pb, S3 *p3, long double *ld) {
  B t;
  __atomic_load(pb, &t, 5);
  __atomic_store(&gb, &t, 5);
  S3 u; __atomic_load(p3, &u, 5);
  long v = __atomic_load_n(&gp.l, 5);
  __atomic_fetch_add(&gp.l, 1, 5);
  long double w; __atomic_load(ld, &w, 5);
  B r = __c11_atomic_load(&ab, 5);
  ab = t;
  __c11_atomic_store(&ai, 1, 5);
}
