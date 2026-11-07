// clang `BI__builtin_memcpy_inline`: `call void
// @llvm.memcpy.inline.p0.p0.i64(dst, src, n, i1 false)` with the
// operands' known alignments; `BI__builtin_assume_separate_storage`:
// `call void @llvm.assume(i1 true) ["separate_storage"(ptr a, ptr b)]`
// (WP-726).
struct S { long a[4]; };
void f(char *d, const char *s, S *x, const S *y) {
  __builtin_memcpy_inline(d, s, 16);
  __builtin_memcpy_inline(x, y, sizeof(S));
  __builtin_memcpy_inline(d, s, 0);
}
template <unsigned N> void g(void *d, const void *s) { __builtin_memcpy_inline(d, s, N); }
template void g<3>(void *, const void *);
void h(char *a, const volatile int *b, const S *c) {
  __builtin_assume_separate_storage(a, b);
  __builtin_assume_separate_storage(c, a + 1);
}
