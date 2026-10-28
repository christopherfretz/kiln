// [lex.string]/9: string literal objects are const; modification is undefined.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_modify_string_literal`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:8 note "assignment to object '\"ab\"[0]' whose value is not known"
//@ diag: 14:15 note "in call to 'f()'"
//@ diag: 10:21 note "temporary created here"
constexpr char f() {
  char *p = (char *)"ab";
  p[0] = 'x';
  return p[0];
}
static_assert(f() == 'x');
