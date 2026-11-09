// [stmt.expand], [stmt.break]/1, [stmt.return]/4 (GNU statement
// expressions): the flow analyses see where a header jump goes. A break in
// the init-statement leaves the enclosing loop, so control can fall off
// the end (-Wreturn-type). A break in an expansion-init-list element leaves
// only the expansion statement, and the enclosing for(;;) loops again, so
// there is no warning. GCC 16.1 (when compiling, not with -fsyntax-only)
// warns at the same place and only there; clang 22 has no P1306 and
// clang's CFG does not model expansion statements. WP-1081; spec/sema.md
// §7.6a.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 19:1 warning "non-void function does not return a value in all control paths"
int init_break(int n) {
  for (;;) {
    template for (int i = ({ if (n) break; 0; }); auto x : {1}) { return x + i; }
  }
}
int element_break(int n) {
  for (;;) {
    template for (auto x : {({ if (n) break; 0; })}) { return x; }
  }
}
