// [dcl.attr.indet], [basic.indet] (C++26, P2795R5): erroneous behaviour; the
// `[[indeterminate]]` attribute opts a variable out of erroneous values.
// WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
void f() {
  int x [[indeterminate]];
  (void)x;
}
