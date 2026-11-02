// [expr.unary.op]/4: a pointer to member is formed only by `&` applied to
// a qualified-id not enclosed in parentheses. `&(C::f)`, `&(f)` and `&b.f`
// with an object (implicit or explicit) are "cannot create a non-constant
// pointer to member function" at the `&`; without one ([expr.prim.id.general]/4),
// the parenthesized name of an instance member function (or of a set of
// them) is "call to non-static member function without an object
// argument" at the name, and nothing else.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ args: -ferror-limit=0
//@ diag: 38:15 error "cannot create a non-constant pointer to member function"
//@ diag: 39:15 error "cannot create a non-constant pointer to member function"
//@ diag: 40:15 error "cannot create a non-constant pointer to member function"
//@ diag: 41:15 error "cannot create a non-constant pointer to member function"
//@ diag: 42:15 error "cannot create a non-constant pointer to member function"
//@ diag: 43:15 error "cannot create a non-constant pointer to member function"
//@ diag: 44:15 error "cannot create a non-constant pointer to member function"
//@ diag: 46:36 error "call to non-static member function without an object argument"
//@ diag: 48:26 error "call to non-static member function without an object argument"
//@ diag: 48:44 error "invalid use of non-static data member 'm'"
//@ diag: 48:63 error "call to non-static member function without an object argument"
//@ diag: 48:82 error "call to non-static member function without an object argument"
//@ diag: 52:15 error "cannot create a non-constant pointer to member function"
//@ diag: 53:15 error "cannot create a non-constant pointer to member function"
//@ diag: 54:15 error "cannot create a non-constant pointer to member function"
//@ diag: 55:10 error "variable 'p4' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 56:15 error "cannot create a non-constant pointer to member function"
//@ diag: 57:23 error "cannot create a non-constant pointer to member function"
//@ diag: 61:23 error "cannot create a non-constant pointer to member function"
//@ diag: 63:17 note "in instantiation of member function 'Y<C>::t' requested here"
//@ diag: 65:18 error "call to non-static member function without an object argument"
//@ diag: 67:18 error "call to non-static member function without an object argument"
//@ diag: 68:17 error "call to non-static member function without an object argument"
struct B { void g(); void o(); void o(int); static void s(); int m; ~B(); };
struct D : B {
  void t(B &b) {
    auto p1 = &(B::g);
    auto p2 = &(g);
    auto p3 = &this->g;
    auto p4 = &b.g;
    auto p5 = &(B::o);
    auto p6 = &((B::g));
    auto p7 = &(D::g);
  }
  static void st() { auto p = &(B::g); }
};
void f() { auto p = &(B::g); auto q = &(B::m); auto r = &((B::g)); auto o = &(B::o); }
struct C {
  void g(); void h(this C &); template<class T> void tf(); static void s(); static void s(int); void mix(); static void mix(int);
  void t(C &b) {
    auto p1 = &(h);
    auto p2 = &b.h;
    auto p3 = &(C::tf<int>);
    auto p4 = &(C::s);
    auto p5 = &(this->g);
    void (*p6)(int) = &(C::mix);
  }
};
template<class T> struct Y : T {
  void t() { auto p = &(T::g); }
};
template struct Y<C>;
void f2() {
  auto p1 = &(C::tf<int>);
  void (*p2)(int) = &(C::mix);
  auto p3 = &(C::h);
  decltype(&(C::g)) *p4;
}
