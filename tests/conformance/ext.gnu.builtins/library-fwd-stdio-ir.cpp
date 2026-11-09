// `__builtin_fprintf` (NoThrow) and `__builtin_fscanf` (not NoThrow) are
// clang's `__builtin_` aliases of the `<stdio.h>` functions (Builtins.td
// `LibBuiltin<"stdio.h">` with `AddBuiltinPrefixedAlias`, WP-1104): their
// type names `FILE`, so they exist once a `FILE` type is declared, and
// lower to a `nounwind` call / an `invoke` of the library function
// (GNU [dcl.attr] extension). The va_list forms are in
// library-fwd-stdio-va-ir.cpp.
// Pinned to oracle 22 like every ir test here (clang 19 and 22 differ in
// target attributes); tests/codegen/lib_builtin_fwd.cpp covers oracle 19.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
typedef struct _IO_FILE FILE;
struct D { ~D(); };

int f(FILE *in, FILE *out) {
  D d;
  int x = 0;
  __builtin_fscanf(in, "%d", &x);
  return __builtin_fprintf(out, "%d\n", x);
}
