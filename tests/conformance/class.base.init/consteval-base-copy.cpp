// [class.base.init]/7, CWG2403 (WP-158): a base initialized from a
// prvalue of its own type is not elided; the prvalue is materialized and
// the base copy- or move-constructed from it, so a consteval copy/move
// constructor is an immediate invocation there. In the implicit move
// constructor of a derived class it makes the constructor immediate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:19 error "call to consteval function 'B2::B2' is not a constant expression"
//@ diag: 33:22 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 33:11 note "declared here"
//@ diag: 41:16 error "call to consteval function 'B4::B4' is not a constant expression"
//@ diag: 41:22 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 41:11 note "declared here"
//@ diag: 46:31 error "call to consteval function 'B4::B4' is not a constant expression"
//@ diag: 46:38 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 46:26 note "declared here"
//@ diag: 47:20 error "call to immediate function 'D4::D4' is not a constant expression"
//@ diag: 40:8 note "'D4' is an immediate constructor because its body contains a call to a consteval constructor 'B4' and that call is not a constant expression"
//@ diag: 47:41 note "function parameter 'd' with unknown value cannot be used in a constant expression"
//@ diag: 47:12 note "declared here"
struct B {
  int a;
  constexpr B(int x) : a(x) {}
  consteval B(const B &o) : a(o.a) {}
};
consteval B mk(int x) { return B(x); }
struct D : B {
  D() : B(mk(3)) {}
};
int g;
struct B2 { int *p; consteval B2(int *q) : p(q) {} };
struct D2 : B2 {
  D2(int *q) : B2(B2(q)) {}
};
struct B4 {
  int *p;
  constexpr B4(int *q) : p(q) {}
  consteval B4(B4 &&o) : p(o.p) {}
};
struct D4 : B4 {
  D4(int *q) : B4(B4(q)) {}
  constexpr D4() : B4(B4(nullptr)) {}
  D4(char) : D4(D4()) {}
};
constexpr B4 mk4(int *q) { return B4(q); }
struct E4 : B4 { E4(int *q) : B4(mk4(q)) {} E4() : B4{B4{nullptr}} {} };
void k(D4 &d) { D4 e(static_cast<D4 &&>(d)); }
D4 h() { return D4(); }
