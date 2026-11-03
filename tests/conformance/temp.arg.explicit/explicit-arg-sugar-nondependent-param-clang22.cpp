// [temp.arg.explicit]/6, [temp.deduct.call]/4 (CWG1391): a parameter
// the explicit template arguments make non-dependent is checked for an
// implicit conversion; clang substitutes those arguments as written, so
// the note keeps their sugar (`W<s::pl>`, aka `W<s::plain>`). A member
// of a class template specialization is substituted with canonical
// arguments and prints `W<plain>` with no `aka`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:3 error "no matching function for call to 'f'"
//@ diag: 22:25 note "candidate function template not viable: no known conversion from 'int' to 'W<s::pl>' (aka 'W<plain>') for 1st argument"
//@ diag: 29:7 error "no viable conversion from 'int' to 'W<s::plain>'"
//@ diag: 21:27 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const W<s::plain> &' for 1st argument"
//@ diag: 21:27 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'W<s::plain> &&' for 1st argument"
//@ diag: 24:14 note "passing argument to parameter here"
//@ requires: clang>=22
// clang 22 spelling of explicit-arg-sugar-nondependent-param.cpp (WP-394).
namespace s {
struct plain {};
using pl = plain;
}
template <class T> struct W {};
template <class T> void f(W<T>, int);
template <class T> struct V {
  void m(W<T>);
};
void h() {
  f<s::pl>(1, 2);
  V<s::pl> v;
  v.m(1);
}
