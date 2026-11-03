// [temp.names]: in a template-id written in a template, a class substituted
// for a parameter prints unscoped among the arguments (clang's
// ElaboratedType policy) and scoped elsewhere (libc++ tuple get<T>).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "no viable conversion from 'int' to 'Box<N::Q, int>'"
//@ diag: 23:15 note "in instantiation of function template specialization 'use<N::Q>' requested here"
//@ diag: 17:36 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Box<N::Q, int> &' for 1st argument"
//@ diag: 17:36 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Box<N::Q, int> &&' for 1st argument"
//@ diag: 20:6 error "cannot initialize a variable of type 'N::Q *' with an rvalue of type 'int'"
//@ diag: 21:15 error "no viable conversion from 'int' to 'Box<N::Q *, N::Q>'"
//@ diag: 17:36 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Box<N::Q *, N::Q> &' for 1st argument"
//@ diag: 17:36 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Box<N::Q *, N::Q> &&' for 1st argument"
//@ requires: clang>=22
// clang 22 spelling of substituted-args-unscoped.cpp (WP-394).
namespace N { struct Q {}; }
template <class T, class U> struct Box {};
template <class T> void use() {
  Box<T, int> b = 1;
  T *p = 1;
  Box<T *, T> c = 2;
}
template void use<N::Q>();
