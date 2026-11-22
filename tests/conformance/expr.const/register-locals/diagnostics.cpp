// [expr.const]/5.11, [basic.life]/1, [stmt.dcl]/3, [expr.prim.id.dtor]/2:
// reads the register-local escape test keeps in memory diagnose as
// clang's do (spec/oven.md "Register locals"): a local declared without
// an initializer and read before its assignment on some path (a later
// loop iteration, its own assignment, a jump past its declaration, an
// operand of std::move), a scalar's pseudo-destructor call, and a pointer
// to a local of an earlier iteration. read_after_scope keeps its locals in
// registers and is a constant.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:12 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'later_iteration(2)'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:7 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'own_assignment(1)'"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:12 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 59:15 note "in call to 'jump_past(1)'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:11 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 65:15 note "in call to 'moved_from_uninit()'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:7 note "destroying object 'x' whose lifetime has already ended"
//@ diag: 73:15 note "in call to 'pseudo_destructor(3)'"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 79:14 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 84:15 note "in call to 'previous_iteration()'"
#include <utility>
constexpr int later_iteration(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    int x;
    if (i > 0)
      s += x;
    x = i;
  }
  return s;
}
static_assert(later_iteration(2) == 0);
constexpr int own_assignment(int n) {
  int x;
  x = x + n;
  return x;
}
static_assert(own_assignment(1) == 1);
constexpr int jump_past(int n) {
  switch (n) {
    int x;
  case 0:
    x = 1;
    return x;
  case 1:
    return x;
  }
  return 0;
}
static_assert(jump_past(0) == 1);
static_assert(jump_past(1) == 0);
constexpr int moved_from_uninit() {
  int x;
  int y = std::move(x);
  return y;
}
static_assert(moved_from_uninit() == 0);
constexpr int pseudo_destructor(int n) {
  using T = int;
  int x = n;
  int y = x;
  x.~T();
  return y;
}
static_assert(pseudo_destructor(3) == 3);
constexpr int previous_iteration() {
  const int *p = nullptr;
  for (int i = 0; i < 2; ++i) {
    int x = i;
    if (p)
      return *p;
    p = &x;
  }
  return 0;
}
static_assert(previous_iteration() == 0);
constexpr int read_after_scope() {
  int s;
  {
    int x;
    x = 4;
    s = x;
  }
  return s;
}
static_assert(read_after_scope() == 4);
