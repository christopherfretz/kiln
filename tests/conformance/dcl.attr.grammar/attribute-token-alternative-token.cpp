// [dcl.attr.grammar]/5: an alternative token in an attribute-token is an
// identifier, so `[[and, bitor, compl]]` names three (unknown, ignored)
// attributes (clang 22; GCC 16.1 warns too). Found running GCC's C++
// testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 14:11 warning "unknown attribute 'and' ignored"
//@ diag: 14:16 warning "unknown attribute 'bitor' ignored"
//@ diag: 14:23 warning "unknown attribute 'xor_eq' ignored"
//@ diag: 14:31 warning "unknown attribute 'compl' ignored"
//@ diag: 14:38 warning "unknown attribute 'bitand' ignored"
int foo() {
  int i [[and, bitor, xor_eq, compl, bitand]];
  i = 0;
  return i;
}
