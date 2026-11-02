// [temp.names]: in a template-id written in a template, a class substituted
// for a parameter prints unscoped among the arguments (clang's
// ElaboratedType policy) and scoped elsewhere (libc++ tuple get<T>).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:15 error "no viable conversion from 'int' to 'Box<Q, int>'"
//@ diag: 21:15 note "in instantiation of function template specialization 'use<N::Q>' requested here"
//@ diag: 15:36 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Box<Q, int> &' for 1st argument"
//@ diag: 15:36 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Box<Q, int> &&' for 1st argument"
//@ diag: 18:6 error "cannot initialize a variable of type 'N::Q *' with an rvalue of type 'int'"
//@ diag: 19:15 error "no viable conversion from 'int' to 'Box<Q *, Q>'"
//@ diag: 15:36 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const Box<Q *, Q> &' for 1st argument"
//@ diag: 15:36 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'Box<Q *, Q> &&' for 1st argument"
namespace N { struct Q {}; }
template <class T, class U> struct Box {};
template <class T> void use() {
  Box<T, int> b = 1;
  T *p = 1;
  Box<T *, T> c = 2;
}
template void use<N::Q>();
