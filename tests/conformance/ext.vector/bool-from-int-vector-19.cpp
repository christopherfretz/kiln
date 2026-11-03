// Clang ext vectors, clang 19 (Sema IsVectorConversion): an int ext
// vector does not convert to a `bool` ext vector, implicitly or by a
// cast; clang 20+ accepts the same-lane-count conversion
// (bool-from-int-vector-clang22.cpp). WP-453.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ requires: clang<22
//@ diag: 20:6 error "cannot initialize a variable of type 'b4' (vector of 4 'bool' values) with an rvalue of type 'int __attribute__((ext_vector_type(4)))' (vector of 4 'int' values)"
//@ diag: 21:7 error "assigning to 'b4' (vector of 4 'bool' values) from incompatible type 'uc4' (vector of 4 'unsigned char' values)"
//@ diag: 22:3 error "no matching function for call to 'use'"
//@ diag: 18:6 note "candidate function not viable: no known conversion from 'i4' (vector of 4 'int' values) to 'b4' (vector of 4 'bool' values) for 1st argument"
//@ diag: 23:10 error "static_cast from 'i4' (vector of 4 'int' values) to 'b4' (vector of 4 'bool' values) is not allowed"
//@ diag: 24:10 error "cannot initialize return object of type 'b4' (vector of 4 'bool' values) with an rvalue of type 'i4' (vector of 4 'int' values)"
typedef int i4 __attribute__((ext_vector_type(4)));
typedef unsigned char uc4 __attribute__((ext_vector_type(4)));
typedef bool b4 __attribute__((ext_vector_type(4)));
void use(b4);
b4 f(i4 I, uc4 U) {
  b4 x = I < 4;
  x = U;
  use(I);
  b4 y = static_cast<b4>(I);
  return I;
}
