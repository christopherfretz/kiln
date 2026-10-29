// [class.base.init]/2, /3, /6, /8: a mem-initializer-id names a non-static data
// member of the class or a direct or virtual base (else the class itself:
// delegation, which must be the only mem-initializer); a member, a base and a
// union's members are each initialized at most once; reference members bind
// no temporary ([class.base.init]/8); a constructor that delegates to itself
// (directly or not) is diagnosed at the end of the translation unit
// (clang's -Wdelegating-ctor-cycles, an error by default). Matches clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:13 error "member initializer 'z' does not name a non-static data member or base class"
//@ diag: 47:20 error "multiple initializations given for non-static member 'x'"
//@ diag: 47:14 note "previous initialization is here"
//@ diag: 48:18 error "multiple initializations given for base 'B'"
//@ diag: 48:12 note "previous initialization is here"
//@ diag: 49:13 error "an initializer for a delegating constructor must appear alone"
//@ diag: 50:20 error "an initializer for a delegating constructor must appear alone"
//@ diag: 51:15 error "type 'O' is not a direct or virtual base of 'S'"
//@ diag: 52:17 error "constructor initializer 'I' (aka 'int') does not name a class"
//@ diag: 53:13 error "member initializer 'st' does not name a non-static data member or base class"
//@ diag: 54:18 error "member initializer 'fn' does not name a non-static data member or base class"
//@ diag: 55:28 error "multiple initializations given for base 'V'"
//@ diag: 55:22 note "previous initialization is here"
//@ diag: 56:28 warning "field 'y' is uninitialized when used here"
//@ diag: 57:15 warning "field 'x' is uninitialized when used here"
//@ diag: 59:39 error "initializing multiple members of union"
//@ diag: 59:33 note "previous initialization is here"
//@ diag: 60:49 error "initializing multiple members of union"
//@ diag: 60:43 note "previous initialization is here"
//@ diag: 61:34 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 61:23 note "reference member declared here"
//@ diag: 61:51 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 61:23 note "reference member declared here"
//@ diag: 63:15 error "base class 'B' specified more than once as a direct base class"
//@ diag: 62:39 error "constructor for 'C1' creates a delegation cycle"
//@ diag: 62:13 note "it delegates to"
//@ diag: 62:29 note "which delegates to"
struct B { B(int = 0); };
struct V { V(int = 0); };
struct O {};
typedef int I;
struct M : virtual V {};
struct S : B, M {
  int x, y;
  static int st;
  void fn();
  S(char) : z(1) {}
  S(short) : x(1), x(2) {}
  S(int) : B(1), B(2) {}
  S(long) : S('a'), x(2) {}
  S(float) : x(1), S('a') {}
  S(double) : O() {}
  S(unsigned) : I(1) {}
  S(bool) : st(1) {}
  S(long long) : fn() {}
  S(unsigned char) : V(1), V(2) {}
  S(signed char) : y(x), x(y) {}
  S(int*) : x(x) {}
};
union U { int a; float b; U() : a(1), b(2) {} };
struct A { union { int p; int q; }; A() : p(1), q(2) {} };
struct R { const int &r; R() : r(1) {} R(int) : r(I(3)) {} };
struct C1 { C1() : C1(1) {} C1(int) : C1() {} };
struct D : B, virtual B {};
