// [dcl.array]/1, [basic.align]/1: GNU `aligned` on an enumeration (on its
// definition, before or after the braces, or on a typedef of it) raises
// its alignment but not its size, so an array of it whose element size is
// not a multiple of that alignment is rejected (clang
// `err_array_element_alignment`, GCC 16.1 "alignment of array elements is
// greater than element size"), also through a template argument (the
// canonical enumeration keeps the alignment). The values are in
// over-aligned-enum.cpp. Found running GCC's C++ testsuite.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:4 error "size of array element of type 'E' (1 bytes) isn't a multiple of its alignment (8 bytes)"
//@ diag: 24:5 error "size of array element of type 'AE' (aka 'enum F') (4 bytes) isn't a multiple of its alignment (8 bytes)"
//@ diag: 25:4 error "size of array element of type 'G' (4 bytes) isn't a multiple of its alignment (16 bytes)"
//@ diag: 26:5 error "size of array element of type 'TH' (aka 'enum H') (4 bytes) isn't a multiple of its alignment (8 bytes)"
//@ diag: 27:36 error "size of array element of type 'E' (1 bytes) isn't a multiple of its alignment (8 bytes)"
//@ diag: 28:8 note "in instantiation of template class 'Arr<E>' requested here"
enum __attribute__((aligned(8))) E : char { e };
typedef enum __attribute__((aligned(8))) F { f } AE;
enum G { g } __attribute__((aligned(16)));
enum H { h }; typedef enum H TH __attribute__((aligned(8)));
E a[2];
AE b[2];
G c[2];
TH d[2];
template <class T> struct Arr { T t[2]; };
Arr<E> x;
