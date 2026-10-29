// [expr.mptr.oper]/2-3: the right operand must be a pointer to member and
// the left operand an object of its class or of a class derived from it.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): needs WP-065 parser classes
//@ diag: 12:11 error "left hand operand to .* must be a class compatible with the right hand operand, but is 'B'"
//@ diag: 12:18 error "right hand operand to .* has non-pointer-to-member type 'int'"
struct A { int x; };
struct B { int y; };
int use(A a, B b, int i) {
  int A::*p = &A::x;
  return b.*p + a.*i;
}
