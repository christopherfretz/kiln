// C23 6.7.6.2, 6.2.8 (as [dcl.array]/1 in C++): an array whose element
// type is a GNU `aligned` typedef with a size that is not a multiple of
// its alignment is rejected (clang `err_array_element_alignment`, shared
// with C++); `_Alignas` on an object does not change its type. Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:4 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 13:15 error "size of array element of type 'I' (aka 'int') (4 bytes) isn't a multiple of its alignment (16 bytes)"
typedef int I __attribute__((aligned(16)));
I a[2];
_Alignas(16) int b[2];
struct S { I x[2]; };
typedef int I4 __attribute__((aligned(4)));
I4 ok[2];
