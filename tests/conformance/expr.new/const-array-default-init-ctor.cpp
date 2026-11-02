// [expr.new]/24, [dcl.init.general]/7-8: `new const T[n]` default-
// initializes each element; that is fine when T is const-default-
// constructible (a user-provided default constructor, or every member
// has a default member initializer) and ill-formed otherwise (libc++
// unique_ptr<const A[]>::reset).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:22 error "default initialization of an object of const type 'const P[]'"
struct A { A() {} int x; };
struct M { int x = 1; };
struct P { int x; };
void f(int n) {
  delete[] new const A[n];
  delete[] new const M[n];
  delete[] new const P[n];
  delete[] new const A[2][3];
}
