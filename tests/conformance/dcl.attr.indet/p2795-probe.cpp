// [dcl.attr.indet], [basic.indet] (C++26, P2795R5): erroneous behaviour; the
// `[[indeterminate]]` attribute opts a variable out of erroneous values.
// WP-185 feature probe.
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 knows [[indeterminate]] (P2795)
//@ xfail(kiln): erroneous behaviour and [[indeterminate]] are not implemented (P2795)
void f() {
  int x [[indeterminate]];
  (void)x;
}
