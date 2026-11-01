// [dcl.array]/1, /3 (clang BuildArrayType, WP-166): an array of an
// incomplete class type is formable in C++; it is incomplete until the
// class is completed, and each use that needs a complete type is diagnosed
// with clang's spelling (the element type for variables and fields).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:12 error "subscript of pointer to incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 32:12 error "subscript of pointer to incomplete type 'A3' (aka 'I[3]')"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 33:13 error "arithmetic on a pointer to an incomplete type 'A3' (aka 'I[3]')"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 34:21 error "field has incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 36:3 error "cannot throw pointer to object of incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 37:5 error "variable has incomplete type 'I[2]'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 38:5 error "variable has incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 40:19 error "invalid application of 'sizeof' to an incomplete type 'A3' (aka 'I[3]')"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 41:20 error "invalid application of 'alignof' to an incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
//@ diag: 42:18 error "allocation of incomplete type 'I'"
//@ diag: 27:8 note "forward declaration of 'I'"
struct I;
using A3 = I[3];
extern I ext[];
I* p = &ext[0];
A3* pa;
auto q = pa[1];
auto r = pa + 1;
struct F { int a; I arr[2]; };
void f() {
  throw (I*)nullptr;
  I vv[2] = {};
  I uu[2];
}
unsigned long s = sizeof(A3);
unsigned long al = alignof(A3);
I (*np)[3] = new A3;
struct I { int x; };
A3 ok;
static_assert(sizeof(A3) == 3 * sizeof(int));
