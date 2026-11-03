// [expr.prim.id.general]/4: outside a SFINAE context, `decltype(C::f)` of a
// non-static member function names it without an object and is an error
// (the operand is a full-expression, [intro.execution]/5).
//@ verify
struct S {
  void f();
  int m;
};
using M = decltype(S::m);
using F = decltype(S::f); // expected-error {{call to non-static member function without an object argument}}
using G = decltype((S::f)); // expected-error {{call to non-static member function without an object argument}}
