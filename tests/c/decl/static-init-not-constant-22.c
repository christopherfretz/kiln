// C23 6.7.11p4, 6.6p9 (GNU labels as values): clang 22 converts a label
// address difference only to an integer of 32 bits or more in a static
// initializer; a `short` one is not a constant (clang 19 truncates it,
// tests/c/run/static-local-label-difference-short-19.c).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ args: -w
//@ diag: 11:20 error "initializer element is not a compile-time constant"
void f(void) {
  static short y = &&l1 - &&l2;
  (void)y;
l1:
l2:;
}
