// array-size-unknown-empty-parens.cpp before C++20: `new T[]()` is
// rejected the same way, and other parenthesized arguments are "array
// 'new' cannot have initialization arguments" (WP-1518).
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "cannot determine allocated array size from initializer"
//@ diag: 12:11 error "array 'new' cannot have initialization arguments"
//@ diag: 13:13 error "cannot determine allocated array size from initializer"
struct A { int a; };
int *p1 = new int[]();
int *p2 = new int[](1, 2);
A *p3 = new A[]();
int *ok1 = new int[]{};
int *ok2 = new int[]{1, 2};
int *ok3 = new int[2]();
