// [stmt.return]/4, [dcl.attr.fallthrough], [basic.scope.block] (WP-234):
// a warning promoted to an error by -Werror makes the compile fail but
// does not stop the flow analyses that an error stops (clang
// `hasUncompilableErrorOccurred` / `hasUnrecoverableErrorOccurred`
// count only diagnostics that are errors by default): -Wreturn-type,
// -Wimplicit-fallthrough and the unused-variable warnings of the same
// scope still follow it.
//@ expect: compile-fail
//@ args: -Wall -Wimplicit-fallthrough -Werror=unused-variable
//@ strict-diags
//@ diag: 18:16 error "unused variable 'unused'"
//@ diag: 22:1 warning "non-void function does not return a value in all control paths"
//@ diag: 27:3 warning "unannotated fall-through between switch labels"
//@ diag: 27:3 note "insert 'break;' to avoid fall-through"
//@ diag: 32:7 error "unused variable 'y'"
//@ diag: 33:7 warning "variable 'z' set but not used"
int g(int);
void a() { int unused; }
int b(int x) {
  if (x)
    return 1;
}
void c(int x) {
  switch (x) {
  case 0:
    g(0);
  case 1:
    break;
  }
}
void d() {
  int y;
  int z;
  z = 1;
}
