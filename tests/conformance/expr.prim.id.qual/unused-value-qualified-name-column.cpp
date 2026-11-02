// [stmt.expr]/1, [expr.prim.id.qual]: -Wunused-value on a qualified name used
// as an expression statement points at the start of the qualified-id
// (`N::x`, `::N::x`, `A::y`, `N::v<int>`), as clang's `DeclRefExpr` begin
// location, not at the unqualified name. A dependent `T::z` is not warned.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:3 warning "expression result unused"
//@ diag: 22:3 warning "expression result unused"
//@ diag: 23:3 warning "expression result unused"
//@ diag: 24:4 warning "expression result unused"
//@ diag: 25:3 warning "expression result unused"
//@ diag: 26:8 warning "expression result unused"
//@ diag: 27:3 warning "left operand of comma operator has no effect"
//@ diag: 27:9 warning "expression result unused"
//@ diag: 28:3 warning "left operand of comma operator has no effect"
//@ diag: 28:6 warning "expression result unused"
//@ diag: 30:31 warning "expression result unused"
namespace N { int x; template <class T> int v = 0; }
struct A { static int y; int m; };
void h(A a) {
  N::x;
  ::N::x;
  A::y;
  (N::x);
  N::v<int>;
  a.A::m;
  N::x, 1;
  1, N::x;
}
template <class T> void g() { N::x; T::z; }
