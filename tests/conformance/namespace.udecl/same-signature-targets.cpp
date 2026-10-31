// [namespace.udecl]/11 (its example): two using-declarators in one block
// scope may introduce functions and function templates with the same
// parameter-type-list; a call that reaches both is ambiguous, and only a
// function declared in the scope conflicts with a using-declared one.
// WP-128.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "call to 'f' is ambiguous"
//@ diag: 18:20 note "candidate function"
//@ diag: 19:20 note "candidate function"
//@ diag: 25:8 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 19:20 note "target of using declaration"
//@ diag: 22:12 note "using declaration"
//@ diag: 30:10 error "call to 'g' is ambiguous"
//@ diag: 18:67 note "candidate function"
//@ diag: 19:81 note "candidate function"

namespace B { void f(int); void f(double); template <class T> int g(T) { return 1; } }
namespace C { void f(int); void f(double); void f(char); template <class T> int g(T) { return 2; } }
void h() {
  using B::f;
  using C::f;
  f('h');
  f(1);
  void f(int);
}
int k() {
  using B::g;
  using C::g;
  return g(1);
}
