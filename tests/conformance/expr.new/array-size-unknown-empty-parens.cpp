// [expr.new]/7: an omitted first bound is given by the number of initial
// elements of the new-initializer; `()` value-initializes and supplies
// none ([dcl.init.general]/16.4), so clang's "cannot determine allocated
// array size from initializer" at the end of the element type, in every
// language mode, also for a parenthesized type-id and at instantiation.
// GCC 16.1 accepts these (an array of no elements): the text gives no
// bound, clang's reading is followed. Found running GCC's C++ testsuite
// (WP-1518). The forms with a bound are array-size-unknown-init-ok.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "cannot determine allocated array size from initializer"
//@ diag: 23:27 error "cannot determine allocated array size from initializer"
//@ diag: 24:20 error "cannot determine allocated array size from initializer"
//@ diag: 25:16 error "cannot determine allocated array size from initializer"
//@ diag: 26:34 error "cannot determine allocated array size from initializer"
//@ diag: 27:27 error "cannot determine allocated array size from initializer"
//@ diag: 28:13 error "cannot determine allocated array size from initializer"
//@ diag: 29:40 error "cannot determine allocated array size from initializer"
//@ diag: 30:11 note "in instantiation of function template specialization 'f<int>' requested here"
struct A { int a; };
int *p1 = new int[]();
const int *p2 = new const int[]();
int **p3 = new int *[]();
int *p4 = new (int[])();
unsigned long *p5 = new unsigned long[]();
int (*p6)[2] = new int[][2]();
A *p7 = new A[]();
template <class T> T *f() { return new T[](); }
int *p8 = f<int>();
