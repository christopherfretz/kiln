// `__builtin_alloca_uninitialized` and
// `__builtin_alloca_with_align_uninitialized` (clang Builtins.td, NoThrow,
// WP-1104) are `__builtin_alloca` / `__builtin_alloca_with_align` that
// `-ftrivial-auto-var-init` leaves uninitialized: at -O0 the same
// dynamic `alloca` (GNU [dcl.attr] extension).
// Pinned to oracle 22 like every ir test here (clang 19 and 22 differ in
// target attributes); tests/codegen/lib_builtin_fwd.cpp covers oracle 19.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
void g(void *, void *);

void f(__SIZE_TYPE__ n) {
  void *p = __builtin_alloca_uninitialized(n);
  void *q = __builtin_alloca_with_align_uninitialized(n, 64);
  g(p, q);
}
