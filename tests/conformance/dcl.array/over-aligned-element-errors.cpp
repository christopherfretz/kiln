// [dcl.array]/1, [basic.align]/1, [expr.sizeof]/2: the elements of an
// array are contiguous, so an element type whose size is not a multiple
// of its alignment (a typedef whose GNU `aligned` attribute raises the
// alignment above the size) cannot be an array element: clang's
// `err_array_element_alignment` (GCC 16.1: "alignment of array elements
// is greater than element size"), wherever the array type is formed. The
// accepted side is over-aligned-element.cpp. Found running GCC's C++
// testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:4 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 28:13 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 29:13 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 30:11 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 31:15 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 32:12 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 33:13 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 34:10 error "size of array element of type 'const I' (aka 'const int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 35:8 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 37:4 error "size of array element of type 'J' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 39:8 error "size of array element of type 'C2' (aka 'char') (1 bytes) isn't a multiple of its alignment (2 bytes)"
//@ diag: 42:6 error "size of array element of type 'EE' (aka 'E') (1 bytes) isn't a multiple of its alignment (8 bytes)"
//@ diag: 43:13 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 44:31 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
typedef int I __attribute__((aligned(16)));
I a[2];
typedef I A2[2];
using A3 = I[3];
void f(I p[2]);
struct M { I m[2]; };
I *n = new I[3];
I *pp = new I[1];
const I c[2] = {};
I m2[2][3];
typedef I J;
J j[2];
typedef char C2 __attribute__((aligned(2)));
C2 bad2[2];
struct E {};
typedef E EE __attribute__((aligned(8)));
EE ee[2];
extern I ext[];
void g(int k) { (void)new I[k][2]; }
