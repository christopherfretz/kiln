// `__builtin_scanf`, `__builtin_sscanf`, `__builtin_vscanf`,
// `__builtin_vsscanf`, `__builtin_fscanf` and `__builtin_vfscanf` carry
// clang's implicit format(scanf, F, A) attribute (Builtins.td
// `ScanfFormat`; WP-1104), so -Wformat checks their arguments, and
// `__builtin_fprintf` / `__builtin_vfprintf` the printf one. The `FILE`
// forms are not declared while no `FILE` type is (clang's
// `GE_Missing_stdio`), so the use on line 25 names nothing (GNU
// [dcl.attr] extension).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:29 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 21:25 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 22:24 warning "more '%' conversions than data arguments"
//@ diag: 25:9 error "use of undeclared identifier '__builtin_fscanf'"
//@ diag: 30:30 warning "format specifies type 'int *' but the argument has type 'float *'"
//@ diag: 31:40 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 32:37 warning "data argument not used by format string"
void f(const char *s, float *x, int *i) {
  __builtin_sscanf(s, "%d", i);
  __builtin_sscanf(s, "%d", x);
  __builtin_scanf("%d", x);
  __builtin_scanf("%d %d", i);
}
// Before any FILE type: the name is not declared.
int r = __builtin_fscanf(0, "");
// clang's GetBuiltinType finds FILE at each lookup, so the builtin
// exists from here on.
typedef struct _IO_FILE FILE;
void h(FILE *in, FILE *out, float *x, int *i) {
  __builtin_fscanf(in, "%d", x);
  __builtin_fprintf(out, "%d %s\n", 1, 2);
  __builtin_fprintf(out, "%d\n", 1, *i);
}
