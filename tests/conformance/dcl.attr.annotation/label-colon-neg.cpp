// [stmt.label], [stmt.pre]/1, [dcl.attr.annotation]/1: an
// attribute-specifier-seq after a label's colon starts the labeled
// sub-statement and appertains to it, so an annotation there is an error
// unless the sub-statement is a declaration (label-colon.cpp); it is never
// dropped silently. GCC 16.1 ignores these with a warning; kiln follows
// the draft (WP-830), so this test has no GCC oracle.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 16:5 error "an annotation can appertain only to a declaration"
//@ diag: 21:5 error "an annotation can appertain only to a declaration"
//@ diag: 25:5 error "an annotation can appertain only to a declaration"
int main() {
lbl:
  [[=1]];
  return 0;
}
void g() {
lbl:
  [[=2]] return;
}
void h(int y) {
again:
  [[=3]] if (y) goto again;
}
