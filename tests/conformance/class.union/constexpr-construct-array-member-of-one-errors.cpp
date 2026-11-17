// [class.union.general]/6, [expr.const]/5.10: after an array member of a
// union is created as an array of one array (`new (p) T[1]()`), the
// previously active member's lifetime has ended, so reading it is not
// constant; and constructing a subobject of an inactive member that is
// not an array element (`&u.s.b`) does not activate the member.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ requires: clang>=22
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:10 note "read of member 'x' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 36:15 note "in call to 'old_member()'"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:3 note "construction of subobject of member 's' of union with active member 'x' is not allowed in a constant expression"
//@ diag: 37:15 note "in call to 'member_subobject()'"
#include <new>
struct P {
  int a;
  int b;
};
union U {
  long long x = -1;
  int arr[4];
  P s;
};
constexpr long long old_member() {
  U u;
  ::new ((void *)&u.arr) int[1][4]();
  return u.x;
}
constexpr int member_subobject() {
  U u;
  ::new (&u.s.b) int(5);
  return 0;
}
static_assert(old_member() == -1);
static_assert(member_subobject() == 0);
