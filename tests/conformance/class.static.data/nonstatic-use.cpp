// [class.static.data], [expr.prim.id.general]/4: a non-static data member
// named without an object outside a member function is ill-formed; a static
// one is not; a qualified name must name a member.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:14 error "invalid use of non-static data member 'x'"
//@ diag: 13:6 error "no member named 'v' in 'S'"
struct S { static int count; int x; };
int S::count;
void f(S s) {
  int a = S::x;
  s.count = a + S::count;
  S::v = 2;
}
