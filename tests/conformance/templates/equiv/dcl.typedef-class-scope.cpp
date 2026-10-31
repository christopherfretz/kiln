//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:39 error "redefinition of 'I'"
//@ diag: 10:24 note "previous definition is here"
//@ diag: 13:14 error "redefinition of 'B'"
//@ diag: 12:13 note "previous definition is here"
// [dcl.typedef]/4, CWG56/424: a typedef-name may not be redeclared in
// class scope, even as the same type; a class template's is not
// re-diagnosed at instantiation.
struct S { typedef int I; typedef int I; };
template <class T> struct U {
  typedef T B;
  typedef T *B;
};
U<int> u;
struct V { struct A {}; typedef A A; };
