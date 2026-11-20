// [namespace.udecl]/8: at block scope a using-declarator may name a class
// member only if it is an enumerator. One with a dependent qualifier and
// an operator-function-id or conversion-function-id (`using
// T::operator+;`) is checked when instantiated (clang's
// UnresolvedUsingValueDecl): never an enumerator, so "using declaration
// cannot refer to class member"; an uninstantiated template (m) is not,
// a generic lambda's is at the lambda's instantiation (clang 22 wording).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:40 error "using declaration cannot refer to class member"
//@ diag: 29:12 note "in instantiation of function template specialization 'f<S>' requested here"
//@ diag: 22:40 error "using declaration cannot refer to class member"
//@ diag: 29:20 note "in instantiation of function template specialization 'g<S>' requested here"
//@ diag: 23:40 error "using declaration cannot refer to class member"
//@ diag: 29:28 note "in instantiation of function template specialization 'h<S>' requested here"
//@ diag: 26:44 error "using declaration cannot refer to class member"
//@ diag: 27:3 note "in instantiation of function template specialization 'l()::(lambda)::operator()<S>' requested here"
//@ diag: 29:36 note "in instantiation of function template specialization 'l<S>' requested here"
struct S { int operator+(int) const; operator int() const; operator S *(); };
template <class T> void f() { using T::operator+; }
template <class T> void g() { using T::operator int; }
template <class T> void h() { using T::operator T *; }
template <class T> void m() { using T::operator+; }
template <class T> void l() {
  auto q = [](auto x) { using decltype(x)::operator+; };
  q(T());
}
void u() { f<S>(); g<S>(); h<S>(); l<S>(); }
