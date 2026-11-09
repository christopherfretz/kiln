// `__builtin_vfprintf` (NoThrow) and `__builtin_vfscanf` (not NoThrow),
// the va_list `<stdio.h>` aliases (Builtins.td `LibBuiltin<"stdio.h">`
// with `AddBuiltinPrefixedAlias`, WP-1104), lower to a `nounwind` call /
// an `invoke` of the library function (GNU [dcl.attr] extension).
// Pinned to oracle 22 like every ir test here (clang 19 and 22 differ in
// target attributes); tests/codegen/lib_builtin_fwd.cpp covers oracle 19.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
typedef struct _IO_FILE FILE;
struct D { ~D(); };

int f(FILE *in, FILE *out, const char *fmt, __builtin_va_list ap) {
  D d;
  __builtin_vfscanf(in, fmt, ap);
  return __builtin_vfprintf(out, fmt, ap);
}
