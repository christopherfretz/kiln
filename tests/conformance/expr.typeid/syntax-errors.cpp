// typeid syntax errors ([expr.typeid]/1): a missing '(' and unbalanced
// parentheses; clang skips to the ')' it expected.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "expected '(' after 'typeid'"
//@ diag: 16:16 error "expected expression"
//@ diag: 17:19 error "expected ')'"
//@ diag: 17:15 note "to match this '('"
//@ diag: 18:20 error "expected ')'"
//@ diag: 18:15 note "to match this '('"
//@ diag: 19:3 warning "expression result unused"
namespace std { class type_info; }
int x;
void f() {
  (void)typeid;
  (void)typeid(;
  (void)typeid(int;
  (void)typeid(int x);
  typeid(x);
}
