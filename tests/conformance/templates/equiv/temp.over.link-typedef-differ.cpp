//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "typedef redefinition with different types ('int[5]' vs 'int[4]')"
//@ diag: 13:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 9:15 note "previous definition is here"
// [dcl.typedef]/4, [temp.res.general]/6: a redefinition with a dependent
// type is checked at instantiation (clang's `isIncompatibleTypedef`).
template <class T> int f() {
  typedef int A[sizeof(T)];
  typedef int A[sizeof(T) + 1];
  return 0;
}
int r = f<int>();
