// [expr.ref]/4, [class.mem.general]/7, [basic.types.general]/5: before its
// closing `}` a class is incomplete, so member access into it outside a
// member function body is "member access into incomplete type". Each such
// use is reported until a member of incomplete type (of the class itself or
// of another incomplete class) makes the class invalid; after that, uses
// that need the class complete (member access, another such member,
// `sizeof`) are not reported again, while unrelated errors still are.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:39 error "member access into incomplete type 'A'"
//@ diag: 24:8 note "definition of 'A' is not complete until the closing '}'"
//@ diag: 27:39 error "member access into incomplete type 'A'"
//@ diag: 24:8 note "definition of 'A' is not complete until the closing '}'"
//@ diag: 28:5 error "field has incomplete type 'A'"
//@ diag: 24:8 note "definition of 'A' is not complete until the closing '}'"
//@ diag: 33:17 error "static assertion failed"
//@ diag: 32:14 error "use of undeclared identifier 'undeclared'"
//@ diag: 39:5 error "field has incomplete type 'E'"
//@ diag: 38:10 note "forward declaration of 'D::E'"
//@ diag: 45:39 error "member access into incomplete type 'F'"
//@ diag: 43:8 note "definition of 'F' is not complete until the closing '}'"
//@ diag: 46:10 error "cannot initialize a member subobject of type 'F *' with an rvalue of type 'int'"

struct A {
  static const int v = sizeof(((A *)0)->m());
  int m();
  static const int w = sizeof(((A *)0)->m());
  A a;
  A a2;
  int z[sizeof(((A *)0)->m())];
  static const int s = sizeof(A);
  void f() { undeclared(); }
  static_assert(false);
};
A x;
int k = x.m();
struct D {
  struct E;
  E e;
  int m();
  static const int v = sizeof(((D *)0)->m());
};
struct F {
  int m();
  static const int v = sizeof(((F *)0)->m());
  F *p = ((F *)0)->m();
};
