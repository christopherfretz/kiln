// [temp.mem]/1, [dcl.fct]/22, [temp.over.link]/6: an out-of-line
// abbreviated member's invented parameter list must match the declared
// member's, type-constraint included.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:30 error "out-of-line definition of 'e' does not match any declaration in 'A"
template <class T> concept C = sizeof(T) >= 4;
template <class T> concept D = sizeof(T) >= 2;
template <class T> struct A {
  int e(D auto x);
  int f(C auto x);
};
template <class T> int A<T>::e(C auto x) { return 0; }
template <class T> int A<T>::f(C auto x) { return 1; }
