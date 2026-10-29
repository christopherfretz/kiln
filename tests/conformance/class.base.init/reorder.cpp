// [class.base.init]/13: mem-initializers run in declaration order (virtual
// bases, direct bases, members), not written order: clang's -Wreorder-ctor,
// one warning for one swap, else a note per out-of-order initializer. Unnamed
// bit-fields are skipped; anonymous union members are in their union's place.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wreorder-ctor
//@ diag: 26:12 warning "field 'b' will be initialized after field 'a'"
//@ diag: 27:13 warning "initializer order does not match the declaration order"
//@ diag: 27:13 note "field 'c' will be initialized after field 'b'"
//@ diag: 27:19 note "field 'b' will be initialized after field 'a'"
//@ diag: 28:13 warning "field 'a' will be initialized after base 'B1'"
//@ diag: 29:14 warning "initializer order does not match the declaration order"
//@ diag: 29:14 note "base class 'B2' will be initialized after base 'B1'"
//@ diag: 29:21 note "base class 'B1' will be initialized after base 'V'"
//@ diag: 31:15 warning "field 'd' will be initialized after field 'u2'"
//@ diag: 35:28 warning "field 'y' will be initialized after field 'x'"
struct B1 { B1(int = 0); };
struct B2 { B2(int = 0); };
struct V { V(int = 0); };
struct S : B1, virtual V, B2 {
  int a, b, c;
  union { int u1; float u2; };
  int : 3;
  int d;
  S(int) : b(1), a(2) {}
  S(char) : c(1), b(2), a(3) {}
  S(long) : a(1), B1(2) {}
  S(short) : B2(1), B1(2), V(3) {}
  S(float) : a(1), b(2), c(3), u1(4), d(5) {}
  S(double) : d(1), u2(2) {}
  S(bool) : V(1), B1(2), B2(3), a(4), b(5) {}
  S(unsigned) : S(1) {}
};
struct T { int x, y; T() : y(0), x(0) {} T(int) : x(0), y(0) {} };
