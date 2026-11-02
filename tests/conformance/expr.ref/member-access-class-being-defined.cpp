// [expr.ref]/4, [class.mem.general]/8: a class is incomplete before its
// closing '}' outside a complete-class context; member access into it in a
// static data member initializer, array bound, alias, decltype or
// static_assert is ill-formed. Inside a trailing return type, noexcept,
// default argument, NSDMI or body it is complete.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:39 error "member access into incomplete type 'A'"
//@ diag: 19:8 note "definition of 'A' is not complete until the closing '}'"
//@ diag: 24:24 error "member access into incomplete type 'B'"
//@ diag: 23:8 note "definition of 'B' is not complete until the closing '}'"
//@ diag: 28:30 error "member access into incomplete type 'C'"
//@ diag: 27:8 note "definition of 'C' is not complete until the closing '}'"
//@ diag: 32:20 error "member access into incomplete type 'D'"
//@ diag: 31:8 note "definition of 'D' is not complete until the closing '}'"
//@ diag: 36:32 error "member access into incomplete type 'E'"
//@ diag: 35:8 note "definition of 'E' is not complete until the closing '}'"

struct A {
  static const int v = sizeof(((A *)0)->m());
  int m();
};
struct B {
  int a[sizeof(((B *)0)->m())];
  int m();
};
struct C {
  using T = decltype(((C *)0)->m());
  int m();
};
struct D {
  decltype(((D *)0)->m()) x;
  int m();
};
struct E {
  static_assert(sizeof(((E *)0)->m()) == 4);
  int m();
};

struct Ok {
  int m();
  int a = ((Ok *)0)->m();
  void f(int = ((Ok *)0)->m());
  auto g() -> decltype(((Ok *)0)->m());
  void h() noexcept(noexcept(((Ok *)0)->m()));
  void b() { (void)((Ok *)0)->m(); }
};
