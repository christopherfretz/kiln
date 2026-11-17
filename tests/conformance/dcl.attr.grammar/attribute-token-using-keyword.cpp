// [dcl.attr.grammar]/1, /4: an attribute-using-prefix is `using
// attribute-namespace :`, so a `using` followed by `,`, `]`, `::` or `(`
// is an attribute-token (a keyword there is an identifier): `[[using]]` is
// an unknown attribute. GCC 16.1 accepts these; clang 22 reads every
// `using` after `[[` as a prefix ("expected identifier"). The text
// decides. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 reads every 'using' after '[[' as an attribute-using-prefix
//@ strict-diags
//@ diag: 19:3 warning "unknown attribute 'using' ignored"
//@ diag: 20:3 warning "unknown attribute 'using::x' ignored"
//@ diag: 20:13 warning "unknown attribute 'x::using' ignored"
//@ diag: 22:11 warning "unknown attribute 'using' ignored"
//@ diag: 22:18 warning "unknown attribute 'using::baz' ignored"
//@ diag: 22:30 warning "unknown attribute 'bar::using' ignored"
//@ diag: 22:42 warning "unknown attribute 'using::using' ignored"
[[using]] int a1;
[[using::x, x::using]] int a2;
int g() {
  int j [[using, using::baz, bar::using, using::using (2)]] = 0;
  return j;
}
