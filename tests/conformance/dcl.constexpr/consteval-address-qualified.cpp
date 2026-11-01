// [expr.const]/16 (WP-137): taking the address of a consteval member with
// a qualified name is reported at the qualifier, also inside a member
// function (where `&C::m` builds an implicit member reference first).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:13 error "cannot take address of consteval function 'm' outside of an immediate invocation"
//@ diag: 17:17 note "declared here"
//@ diag: 23:13 error "cannot take address of consteval function 's' outside of an immediate invocation"
//@ diag: 18:24 note "declared here"
//@ diag: 24:13 error "cannot take address of consteval function 's' outside of an immediate invocation"
//@ diag: 18:24 note "declared here"
//@ diag: 26:22 error "cannot take address of consteval function 'm' outside of an immediate invocation"
//@ diag: 17:17 note "declared here"
//@ diag: 26:38 error "cannot take address of consteval function 's' outside of an immediate invocation"
//@ diag: 18:24 note "declared here"
struct C {
  consteval int m() const { return 1; }
  static consteval int s() { return 2; }
  void g();
};
void C::g() {
  auto p = &C::m;
  auto q = &C::s;
  auto r = &s;
}
void h() { auto p = &C::m; auto q = &C::s; }
