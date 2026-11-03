// [dcl.attr.indet]/1 (C++26, P2795R5): on a parameter, `[[indeterminate]]`
// must appear on the first declaration of the function; a later
// declaration may repeat it. [dcl.attr.grammar]/4 (P2156R1): the same
// attribute may appear more than once in one attribute-list.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 12:23 error "'[[indeterminate]]' attribute on parameter 'b' must appear on the first declaration of the function"
//@ diag: 10:6 note "previous declaration is here"
void p(int a [[indeterminate]], int b);
void p(int a [[indeterminate]], int b);
void p(int a, int b [[indeterminate]]);
void q(int x [[indeterminate, indeterminate]]) { (void)x; }
void r(int x [[indeterminate]]) {
  int y [[indeterminate]] [[indeterminate]];
  (void)x;
  (void)y;
}
