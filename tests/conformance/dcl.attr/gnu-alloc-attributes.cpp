// GNU allocation attributes (WP-170, clang handleRestrictAttr /
// handleAllocSizeAttr / AddAllocAlignAttr): malloc and alloc_size need a
// pointer result (alloc_align: or a reference), indices are one-based
// integer constants naming integer parameters, and index 1 of a member
// function is the implicit object argument.
//@ expect: compile-fail
//@ diag: 27:24 warning "'malloc' attribute only applies to return values that are pointers"
//@ diag: 29:22 warning "'malloc' attribute only applies to functions"
//@ diag: 30:30 error "'alloc_size' attribute parameter 1 is out of bounds"
//@ diag: 31:44 error "'alloc_size' attribute argument may only refer to a function parameter of integer type"
//@ diag: 32:40 error "'alloc_size' attribute parameter 2 is out of bounds"
//@ diag: 33:28 warning "'alloc_size' attribute only applies to return values that are pointers"
//@ diag: 34:30 error "'alloc_size' attribute parameter 1 is out of bounds"
//@ diag: 36:30 error "'alloc_size' attribute requires parameter 1 to be an integer constant"
//@ diag: 37:30 error "'alloc_size' attribute takes at least 1 argument"
//@ diag: 38:35 error "'alloc_size' attribute takes no more than 2 arguments"
//@ diag: 39:22 warning "'alloc_size' attribute only applies to non-K&R-style functions"
//@ diag: 40:40 error "'alloc_size' attribute is invalid for the implicit this argument"
//@ diag: 41:30 error "'alloc_align' attribute parameter 1 is out of bounds"
//@ diag: 42:45 error "'alloc_align' attribute argument may only refer to a function parameter of integer type"
//@ diag: 43:28 warning "'alloc_align' attribute only applies to return values that are pointers or references"
//@ diag: 44:30 error "'alloc_align' attribute requires parameter 1 to be an integer constant"
//@ diag: 45:35 error "'alloc_align' attribute takes one argument"
//@ diag: 46:22 warning "'alloc_align' attribute only applies to non-K&R-style functions"
//@ diag: 48:29 warning "'alloc_size' attribute only applies to return values that are pointers"
//@ diag: 50:55 error "'alloc_size' attribute argument may only refer to a function parameter of integer type"
int f() __attribute__((malloc));
void *g() __attribute__((malloc));
int x __attribute__((malloc));
void *a1(int) __attribute__((alloc_size(2)));
void *a2(double) __attribute__((alloc_size(1)));
void *a3(int, int, int) __attribute__((alloc_size(1, 4)));
int a4(int) __attribute__((alloc_size(1)));
void *a5(int) __attribute__((alloc_size(0)));
int n;
void *a6(int) __attribute__((alloc_size(n)));
void *a7(int) __attribute__((alloc_size()));
void *a8(int, int) __attribute__((alloc_size(1, 2, 1)));
int v __attribute__((alloc_size(1)));
struct S { void *m(int) __attribute__((alloc_size(1))); void *k(int) __attribute__((alloc_size(2))); };
void *b1(int) __attribute__((alloc_align(2)));
void *b2(double) __attribute__((alloc_align(1)));
int b3(int) __attribute__((alloc_align(1)));
void *b4(int) __attribute__((alloc_align(n)));
void *b5(int, int) __attribute__((alloc_align(1, 2)));
int y __attribute__((alloc_align(1)));
int &b6(int) __attribute__((alloc_align(1)));
int &a9(int) __attribute__((alloc_size(1)));
void *ok(long, unsigned) __attribute__((malloc, alloc_size(2, 1), alloc_align(1)));
template <class T> T *tp(T) __attribute__((alloc_size(1)));
