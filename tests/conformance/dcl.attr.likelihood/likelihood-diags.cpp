//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:5 error "'likely' attribute cannot be applied to a declaration"
//@ diag: 23:24 error "'unlikely' and 'likely' attributes are not compatible"
//@ diag: 23:13 note "conflicting attribute is here"
//@ diag: 24:23 error "'likely' and 'unlikely' attributes are not compatible"
//@ diag: 24:13 note "conflicting attribute is here"
//@ diag: 26:12 warning "conflicting attributes 'likely' are ignored"
//@ diag: 26:34 note "conflicting attribute is here"
//@ diag: 27:12 warning "conflicting attributes 'unlikely' are ignored"
//@ diag: 27:36 note "conflicting attribute is here"
//@ diag: 29:11 error "attribute 'likely' cannot have an argument list"
//@ diag: 30:13 error "attribute 'unlikely' cannot have an argument list"
// [dcl.attr.likelihood]/1 (WP-182): a likelihood attribute applies to a
// statement, not a declaration, and `likely` and `unlikely` exclude each
// other; equal likelihood on both branches of an if is ignored with a
// warning (clang `ActOnIfStmt`); a standard attribute without arguments
// takes no argument list.
int g(int);
int f(int x) {
  [[likely]] int y = 0;
  switch (x) {
  case 2: [[likely]] [[unlikely]] g(4); break;
  case 3: [[unlikely, likely]] g(4); break;
  }
  if (x) [[likely]] g(1); else [[likely]] g(2);
  if (x) [[unlikely]] g(1); else [[unlikely]] { g(2); }
  if (x) [[likely]] g(1); else [[unlikely]] g(2);
  [[likely(1)]] g(1);
  [[unlikely()]] g(1);
  return y;
}
