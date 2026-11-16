// [expr.new]/23 with [dcl.init.general]/16.5: before C++20 (P0960) an array
// new-expression takes only `()` or a braced-init-list; clang's "array 'new'
// cannot have initialization arguments" at the expression's start, also for
// an array typedef and in a template definition. Found running GCC's C++
// testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:11 error "array 'new' cannot have initialization arguments"
//@ diag: 21:11 error "array 'new' cannot have initialization arguments"
//@ diag: 22:9 error "array 'new' cannot have initialization arguments"
//@ diag: 23:9 error "array 'new' cannot have initialization arguments"
//@ diag: 24:11 error "array 'new' cannot have initialization arguments"
//@ diag: 25:11 error "array 'new' cannot have initialization arguments"
//@ diag: 26:39 error "array 'new' cannot have initialization arguments"
//@ diag: 27:41 error "array 'new' cannot have initialization arguments"
struct S { int a; };
struct C { C(int); };
typedef int A2[2];
int *p1 = new int[2](1, 2);
int *p2 = new int[](1, 2);
S *p3 = new S[2]({1}, {2});
C *p4 = new C[2](1, 2);
int *p5 = ::new int[2](1);
int *p6 = new A2(1, 2);
template <class T> T *f(T x) { return new T[2](x, x); }
template <class T> int *g(T x) { return new int[2](x); }
