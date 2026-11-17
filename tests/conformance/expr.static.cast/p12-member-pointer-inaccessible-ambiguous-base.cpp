// [expr.static.cast]/12: static_cast from "pointer to member of D" to
// "pointer to member of B" needs B to be an unambiguous, accessible,
// non-virtual base of D, as the implicit conversion the other way
// ([conv.mem]/2) does; a C-style cast ignores only the access
// ([expr.cast]/4), so the ambiguous base stays an error there.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:32 error "cannot cast 'D' to its private base class 'B'"
//@ diag: 22:12 note "declared private here"
//@ diag: 28:41 error "cannot cast 'D' to its private base class 'B'"
//@ diag: 22:12 note "declared private here"
//@ diag: 29:33 error "cannot cast 'P' to its protected base class 'B'"
//@ diag: 23:12 note "declared protected here"
//@ diag: 30:47 error "cannot cast private base class 'B' to 'D'"
//@ diag: 22:12 note "declared private here"
//@ diag: 31:38 error "ambiguous conversion from pointer to member of derived class 'Amb' to pointer to member of base class 'B':"
//@ diag: 32:55 error "ambiguous conversion from pointer to member of base class 'B' to pointer to member of derived class 'Amb':"
//@ diag: 33:42 error "ambiguous conversion from pointer to member of derived class 'Amb' to pointer to member of base class 'B':"
//@ diag: 34:50 error "ambiguous conversion from pointer to member of base class 'B' to pointer to member of derived class 'Amb':"
struct B { int m; int f(); };
struct D : private B {};
struct P : protected B {};
struct A1 : B {};
struct A2 : B {};
struct Amb : A1, A2 {};
int B::*up(int D::*p) { return static_cast<int B::*>(p); }
int (B::*upf(int (D::*p)()))() { return static_cast<int (B::*)()>(p); }
int B::*upp(int P::*p) { return static_cast<int B::*>(p); }
int D::*down() { return static_cast<int D::*>(&B::m); }
int B::*amb_up(int Amb::*p) { return static_cast<int B::*>(p); }
int Amb::*amb_down() { return static_cast<int Amb::*>(&B::m); }
int B::*amb_cstyle(int Amb::*p) { return (int B::*)p; }
int Amb::*amb_cstyle_down() { return (int Amb::*)&B::m; }
// Accepted: a C-style cast through the private base, reinterpret_cast, and
// static_cast where the base is accessible.
int B::*cs(int D::*p) { return (int B::*)p; }
int D::*cs_down() { return (int D::*)&B::m; }
int B::*re(int D::*p) { return reinterpret_cast<int B::*>(p); }
struct F : private B {
  static int B::*in(int F::*p) { return static_cast<int B::*>(p); }
};
