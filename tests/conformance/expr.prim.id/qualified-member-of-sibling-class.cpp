// [expr.prim.id.general]/3: `C::m` in a member function of `B` is an
// implicit member access only when `C` is `B` or a base of `B`. `C` and
// `B` both derive from `A`, so `C::m()` and `C::d` name members without an
// object; an unevaluated operand may still name one. Found running GCC's
// C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:23 error "call to non-static member function without an object argument"
//@ diag: 15:23 error "invalid use of non-static data member 'd'"

struct A { int m(); int d; };
struct C : A {};
struct B : A {
  int f() { return C::m(); }
  int g() { return C::d; }
  int h() { return sizeof(C::d) + A::m() + B::d; }
};
