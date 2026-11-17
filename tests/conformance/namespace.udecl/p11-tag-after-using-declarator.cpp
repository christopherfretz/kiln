// [namespace.udecl]/11, [basic.scope.scope]/4: a class or enumeration
// declared (or defined) in the scope of a using-declarator whose target is a
// typedef-name, an alias, a class or variable template or a concept
// corresponds to that target and declares another entity, so the program is
// ill-formed, at namespace, block and class scope (clang 22 reports it at the
// class-key). In a class a member function hides only a function the
// using-declarator named ([namespace.udecl]/14): beside a type, a data member
// or an enumerator it conflicts. clang 19 and 22 agree; GCC 16.1 rejects the
// typedef-names and the class scope's functions, accepts the templates and
// the concept. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 58:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 50:15 note "target of using declaration"
//@ diag: 58:12 note "using declaration"
//@ diag: 59:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 51:9 note "target of using declaration"
//@ diag: 59:12 note "using declaration"
//@ diag: 60:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 52:28 note "target of using declaration"
//@ diag: 60:12 note "using declaration"
//@ diag: 61:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 53:25 note "target of using declaration"
//@ diag: 61:12 note "using declaration"
//@ diag: 62:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 54:29 note "target of using declaration"
//@ diag: 62:12 note "using declaration"
//@ diag: 63:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 55:10 note "target of using declaration"
//@ diag: 63:12 note "using declaration"
//@ diag: 66:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 50:15 note "target of using declaration"
//@ diag: 66:12 note "using declaration"
//@ diag: 67:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 52:28 note "target of using declaration"
//@ diag: 67:12 note "using declaration"
//@ diag: 76:15 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 70:28 note "target of using declaration"
//@ diag: 76:12 note "using declaration"
//@ diag: 77:20 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 71:15 note "target of using declaration"
//@ diag: 77:12 note "using declaration"
//@ diag: 78:20 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 72:7 note "target of using declaration"
//@ diag: 78:12 note "using declaration"
//@ diag: 79:20 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 73:10 note "target of using declaration"
//@ diag: 79:12 note "using declaration"
namespace N {
  typedef int A;
  using B = int;
  template<class T> struct C {};
  template<class T> int E = 0;
  template<class T> concept F = true;
  struct G {};
}
namespace M {
  using N::A; struct A {};
  using N::B; enum B { b };
  using N::C; struct C;
  using N::E; enum class E;
  using N::F; struct F {};
  using N::G; struct G;
}
void f() {
  using N::A; struct A {};
  using N::C; struct C;
}
struct H {
  template<class T> struct C {};
  typedef int T;
  int v;
  enum { e };
};
struct J : H {
  using H::C; struct C {};
  using H::T; void T();
  using H::v; void v();
  using H::e; void e(int);
};
