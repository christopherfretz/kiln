// [dcl.meaning.general]/2 (CWG482): a member declared in its class with a
// nested-name-specifier naming that class is an error (the qualifier is
// dropped); a namespace member redeclared in its namespace with a
// qualifier naming it gets -Wextra-qualification; a qualifier naming an
// unrelated class or no prior declaration does not match; a member
// declaration that does not match says "declaration", a definition
// "definition"
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
// clang 22 notes the class a qualified declarator did not match in.
//@ diag: 42:11 error "extra qualification on member 'f'"
//@ diag: 43:10 error "extra qualification on member 'x'"
//@ diag: 44:17 error "extra qualification on member 'y'"
//@ diag: 45:6 error "extra qualification on member 'S'"
//@ diag: 46:6 error "extra qualification on member '~S'"
//@ diag: 47:18 error "extra qualification on member 'T'"
//@ diag: 49:11 error "extra qualification on member 'h'"
//@ diag: 50:9 error "extra qualification on member 'operator='"
//@ diag: 52:37 error "extra qualification on member 'f'"
//@ diag: 53:38 error "extra qualification on member 'f'"
//@ diag: 53:54 error "extra qualification on member 'g'"
//@ diag: 54:20 error "non-friend class member 'g' cannot have a qualified name"
//@ diag: 56:9 error "out-of-line declaration of 'f' does not match any declaration in 'A'"
//@ diag: 55:8 note "A defined here"
//@ diag: 57:9 error "out-of-line definition of 'g' does not match any declaration in 'A'"
//@ diag: 55:8 note "A defined here"
//@ diag: 59:11 warning "extra qualification on member 'f'"
//@ diag: 59:11 error "out-of-line declaration of 'f' does not match any declaration in namespace 'M'"
//@ diag: 60:11 warning "extra qualification on member 'g'"
//@ diag: 60:11 error "out-of-line definition of 'g' does not match any declaration in namespace 'M'"
//@ diag: 61:28 warning "extra qualification on member 'h'"
//@ diag: 61:28 error "out-of-line declaration of 'h' does not match any declaration in namespace 'M::I'"
//@ diag: 61:41 warning "extra qualification on member 'k'"
//@ diag: 61:41 error "out-of-line declaration of 'k' does not match any declaration in namespace 'M::I'"
//@ diag: 62:22 error "extra qualification on member 'm'"
//@ diag: 64:11 warning "extra qualification on member 'q'"
//@ diag: 66:29 warning "extra qualification on member 't'"
//@ diag: 67:22 warning "extra qualification on member 't'"
//@ diag: 73:8 warning "extra qualification on member 'z'"
struct S {
  void S::f();
  int S::x;
  static int S::y;
  S::S();
  S::~S();
  typedef int S::T;
  void g();
  void S::h() {}
  S& S::operator=(const S&);
};
namespace N { struct T { void N::T::f(); }; }
template<class U> struct W { void W::f(); void W<U>::g(); };
struct Q { void S::g(); };
struct A {};
void A::f();
void A::g() {}
namespace M {
  void M::f();
  void M::g() {}
  namespace I { void M::I::h(); void I::k(); }
  struct R { void R::m(); };
  void q();
  void M::q() {}
  template<class T> void t();
  template<class T> void M::t() {}
  template<> void M::t<int>() {}
  struct U { void g(); static int s; };
  void U::g() {}
  int U::s;
}
void z();
void ::z() {}
