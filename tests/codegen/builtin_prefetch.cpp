// GNU __builtin_prefetch(addr [, rw [, locality]]) (clang
// EmitBuiltinExpr, BI__builtin_prefetch): `llvm.prefetch.p0(addr, rw,
// locality, 1)`, `rw` 0 and `locality` 3 by default, the trailing 1 the
// data cache. The address converts to `const void *` (WP-610).
void f(const void *p) {
  __builtin_prefetch(p);
  __builtin_prefetch(p, 1);
  __builtin_prefetch(p, 0, 3);
  __builtin_prefetch(p, 1, 0);
}
struct S { int a[8]; long b; };
enum Hint { Read, Write };
constexpr int kLocality = 2;
void g(S *s, int i) {
  __builtin_prefetch(s);
  __builtin_prefetch(&s->a[i], Write, kLocality);
  __builtin_prefetch(&s->b, (char)1, true);
  __builtin_prefetch(s + i + 1, 0, 1);
  __builtin_prefetch(nullptr);
}
template <int RW, int L, class T> void h(const T *p) { __builtin_prefetch(p, RW, L); }
template void h<1, 2>(const S *);
template void h<0, 0>(const long *);
