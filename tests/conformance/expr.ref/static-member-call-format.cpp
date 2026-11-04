// [expr.ref], [dcl.fct.default]/1: a static member function named by a
// class member access is checked like a call by its name: its
// `format` attribute checks the arguments (`-Wformat`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:15 warning "format specifies type 'int' but the argument has type 'const char *'"
//@ diag: 11:16 warning "format specifies type 'int' but the argument has type 'const char *'"
struct S { static int log(const char*, ...) __attribute__((format(printf, 1, 2))); };
void use(S s, S* p) {
  s.log("%d", "x");
  p->log("%d", "x");
}
