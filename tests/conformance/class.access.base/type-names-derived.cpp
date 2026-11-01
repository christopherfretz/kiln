// [class.access.base]: nested types named from derived classes, through
// private and protected bases (the injected-class-name is a public member of
// its class), and base-specifiers (checked at once, as clang does).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:15 error "'B' is a protected member of 'A'"
//@ diag: 18:10 note "declared protected here"
//@ diag: 29:27 error "'Priv' is a private member of 'A'"
//@ diag: 16:10 note "implicitly declared private here"
//@ diag: 31:28 error "'A' is a private member of 'A'"
//@ diag: 30:13 note "constrained by private inheritance here"
//@ diag: 15:7 note "member is declared here"
//@ diag: 34:17 error "'B' is a protected member of 'A'"
//@ diag: 18:10 note "declared protected here"
class A {
  struct Priv { };
protected:
  struct B { };
  struct C { struct In { }; };
  typedef int PI;
  static int s;
};
struct D : A::B, A {
  A::C::In in;
  PI pi;
  C c;
  void k() { A::B b; B b2; (void)b; (void)b2; }
};
struct E : A { void k() { Priv p; (void)p; } };
struct F2 : private A { };
struct G : F2 { void k() { A::B b; (void)b; } };
struct H : protected A { };
struct I : H { void k() { B b; C::In i; (void)b; (void)i; } };
void out() { H::B b; (void)b; }
struct G2 : F2 { void k() { ::A::B b; (void)b; } };
