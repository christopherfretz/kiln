// [expr.add]/5.3: pointers into different member arrays of one object.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_ptr_sub_subarrays`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:18 note "subtracted pointers are not elements of the same array"
//@ diag: 13:15 note "in call to 'f()'"
struct S { int a[2]; int b[2]; };
constexpr long f() {
  S s = {{1, 2}, {3, 4}};
  return &s.b[0] - &s.a[0];
}
static_assert(f() == 0);
