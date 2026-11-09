// `__builtin_malloc`, `__builtin_calloc`, `__builtin_realloc` and
// `__builtin_free` are clang's `__builtin_`-prefixed aliases of the C
// library functions (Builtins.td `LibBuiltin<"stdlib.h">` with
// `AddBuiltinPrefixedAlias`, NoThrow; GNU [dcl.attr] extension, WP-1104):
// usable without a declaration, each lowers to a `nounwind` call of the
// library function, with no `allocsize` and no `noalias` return.
// Pinned to oracle 22 like every ir test here (clang 19 and 22 differ in
// target attributes); tests/codegen/lib_builtin_fwd.cpp covers oracle 19.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
struct D { ~D(); };

void *f(__SIZE_TYPE__ n) {
  D d;
  void *p = __builtin_malloc(n);
  void *q = __builtin_calloc(n, 4);
  p = __builtin_realloc(p, 2 * n);
  __builtin_free(q);
  static_assert(noexcept(__builtin_malloc(1)));
  static_assert(__is_same(decltype(__builtin_calloc(1, 1)), void *));
  return p;
}
