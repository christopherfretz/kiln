// The gnu_printf / gnu_scanf format archetypes (WP-186; GNU [dcl.attr]
// extension): clang 22 accepts them as printf / scanf (clang 19 reports
// them as not supported). Verified against clang 22.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ diag: 13:13 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 14:16 warning "data argument not used by format string"
void p(const char *f, ...) __attribute__((format(gnu_printf, 1, 2)));
void q(const char *f, ...) __attribute__((format(__gnu_printf__, 1, 2)));
void r(const char *f, ...) __attribute__((format(gnu_scanf, 1, 2)));
void t(int i) {
  p("%s\n", i);
  q("%d\n", i, i);
  r("%d", &i);
}
