// [dcl.init.general]/7, [expr.const]/5.11: an uninitialized member read.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_uninit_member`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 14:15 note "in call to 'f()'"
struct S { int a; int b; };
constexpr int f() {
  S s;
  s.a = 1;
  return s.b;
}
static_assert(f() == 0);
