// A call to a function whose friend declaration carries GNU `noreturn`
// (a bit of its function type) is a `noreturn` call followed by
// `unreachable`, through a later redeclaration and through a hidden
// friend found by argument-dependent lookup alike. kiln dropped the
// attribute from the friend's type, so the calls returned.
//@ expect: ir
//@ strict-diags
struct C {
  friend void d1() __attribute__((noreturn));
  friend void h(C) __attribute__((noreturn));
};
void d1();
int r1() { d1(); }
int r2(C c) { h(c); }
