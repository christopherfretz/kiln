// [expr.const]/5.8: reading a member through a past-the-end struct pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_index_past_end_read_struct`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:13 note "cannot access field of pointer past the end of object"
//@ diag: 14:15 note "in call to 'f()'"
struct S { int a; };
constexpr int f() {
  S s[1] = {{1}};
  S *p = s + 1;
  return p->a;
}
static_assert(f() == 1);
