// [class.mfct]/1, [temp.expl.spec]/2: a member function is redeclared
// outside its class only by a definition (a function body, `= default` or
// `= delete`); an out-of-line declaration that is not one is ill-formed.
// clang reports it at the declarator-id as the default-error extension
// "out-of-line declaration of a member must be a definition"
// (-Wout-of-line-declaration), after the redeclaration's other checks
// (here: an attribute added after the explicitly defaulted definition).
// For member functions, constructors, destructors, conversion functions,
// operators, static member functions and member templates, of classes and
// class templates (also through a namespace qualifier). Not for an
// explicit specialization or an explicit instantiation of a member, a
// friend, or a function of a namespace; a qualified declaration in a class
// or a block, an implicitly declared or explicitly defaulted member and
// static data members, nested classes and enumerations have errors of
// their own. spec/sema.md §1.1, WP-1835.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 54:23 warning "attribute declaration must precede definition"
//@ diag: 53:4 note "previous definition is here"
//@ diag: 54:4 error "out-of-line declaration of a member must be a definition"
//@ diag: 55:4 error "out-of-line declaration of a member must be a definition"
//@ diag: 57:9 error "out-of-line declaration of a member must be a definition"
//@ diag: 58:9 error "out-of-line declaration of a member must be a definition"
//@ diag: 60:9 error "out-of-line declaration of a member must be a definition"
//@ diag: 61:28 error "out-of-line declaration of a member must be a definition"
//@ diag: 62:4 error "out-of-line declaration of a member must be a definition"
//@ diag: 63:8 error "out-of-line declaration of a member must be a definition"
//@ diag: 64:9 error "out-of-line declaration of a member must be a definition"
//@ diag: 70:31 error "out-of-line declaration of a member must be a definition"
//@ diag: 71:50 error "out-of-line declaration of a member must be a definition"
//@ diag: 72:26 error "out-of-line declaration of a member must be a definition"
//@ diag: 74:15 error "out-of-line declaration of a member must be a definition"
//@ diag: 76:12 error "out-of-line declaration of a member must be a definition"
//@ diag: 77:23 error "out-of-line declaration of a member must be a definition"
//@ diag: 78:44 error "non-friend class member 'f' cannot have a qualified name"
//@ diag: 79:21 error "definition or redeclaration of 'g' not allowed inside a function"
//@ diag: 81:4 error "definition of implicitly declared default constructor"
//@ diag: 83:4 error "definition of explicitly defaulted default constructor"
//@ diag: 84:8 error "forward declaration of struct cannot have a nested name specifier"
//@ diag: 85:6 error "forward declaration of enum cannot have a nested name specifier"
struct D {
  D();
  ~D();
  void f();
  void g();
  static void s();
  template <class T> void t(T);
  operator int();
  int operator+(int);
  void c() const;
};
D::D() = default;
D::D() __attribute__((cold));
D::~D();
void D::f() {}
void D::f();
void D::g();
void D::g() {}
void D::s();
template <class T> void D::t(T);
D::operator int();
int D::operator+(int);
void D::c() const;
template <class T> struct X {
  void f();
  template <class U> void g(U);
  X();
};
template <class T> void X<T>::f();
template <class T> template <class U> void X<T>::g(U);
template <class T> X<T>::X();
template <> struct X<long> { void f(); };
void X<long>::f();
namespace N { struct E { void f(); }; }
void N::E::f();
namespace N { void E::f(); }
struct A { struct B { void f(); }; void B::f(); };
void fn() { void D::g(); }
struct I {};
I::I();
struct J { J() = default; struct K; enum L : int; };
J::J();
struct J::K;
enum J::L : int;

// Accepted.
template <class T> void X<T>::f() {}
template <> void X<char>::f();
template <> template <class U> void X<int>::g(U);
template void X<short>::f();
template <> void D::t<int>(int);
struct F { friend void ff(); };
void ff();
namespace M { void q(); }
void M::q();
struct G { void h(); };
void G::h() {}
