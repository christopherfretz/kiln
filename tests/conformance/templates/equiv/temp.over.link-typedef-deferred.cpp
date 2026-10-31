// [dcl.typedef]/4, [temp.res.general]/6: a typedef redefinition whose
// types are dependent is not diagnosed in a template that is never
// instantiated (as clang).
template <class T> void f() {
  typedef int A[sizeof(T)];
  typedef int A[sizeof(T) + 1];
}
