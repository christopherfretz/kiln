// [temp.spec.general]/1, [temp.inst]: a class template specialization prints with its
// canonical arguments, so a type written with a dependent member typedef shows the specialization
// as its 'aka' and an instantiation note names 'X<int>' for 'X<I>'
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:44 error "static assertion failed due to requirement 'sizeof(int) == 0': X"
//@ diag: 25:8 note "in instantiation of template class 'X<int>' requested here"
//@ diag: 20:54 note "expression evaluates to '4 == 0'"
//@ diag: 26:3 error "no matching function for call to 'f2'"
//@ diag: 21:6 note "candidate function not viable: no known conversion from 'int' to 'pr<int, typename id<int>::type>' (aka 'pr<int, int>') for 1st argument"
//@ diag: 17:76 error "no viable conversion from 'pr<int, typename id<int>::type>' (aka 'pr<int, int>') to 'int'"
//@ diag: 23:12 note "in instantiation of member function 'W<int>::g' requested here"
//@ diag: 18:65 error "no viable conversion from 'pr<int, typename id<int>::type>' (aka 'pr<int, int>') to 'int'"
//@ diag: 24:3 note "in instantiation of function template specialization 'h<int>' requested here"
template<class A, class B> struct pr {};
template<class T> struct id { using type = T; };
template<class T> struct W { pr<T, typename id<T>::type> m; void g() { int x = m; } };
template<class T> void h() { pr<T, typename id<T>::type> v; int y = v; }
using I = int;
template<class T> struct X { static_assert(sizeof(T) == 0, "X"); };
void f2(pr<int, typename id<int>::type>);
void t() {
  W<int>().g();
  h<int>();
  X<I> x;
  f2(1);
}
