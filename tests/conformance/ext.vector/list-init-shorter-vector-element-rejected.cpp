// A vector initializer list whose leading element is a vector
// copy-initializes the whole vector from it (clang `InitListChecker::
// CheckVectorType`, C++ rules); the OpenCL rule that splits a shorter
// vector into lanes does not apply, so `v4{v2, v2}` is an error in
// clang 19 and 22 alike. WP-797 listed it as a kiln rejection to fix;
// WP-802 found kiln already agrees with clang, so nothing changed
// (the leading-vector-element rule in spec/sema.md).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:30 error "cannot initialize a value of type 'v4' (vector of 4 'float' values) with an lvalue of type 'v2' (vector of 2 'float' values)"
//@ diag: 16:33 error "cannot initialize a value of type 'v4' (vector of 4 'float' values) with an lvalue of type 'v2' (vector of 2 'float' values)"
typedef float v2 __attribute__((ext_vector_type(2)));
typedef float v4 __attribute__((ext_vector_type(4)));
v4 ok(float a, v4 w) { return v4{a, a, 1, 2} + v4{w}; }
v4 f(v2 a, v2 b) { return v4{a, b}; }
v4 h(v2 a, float c) { return v4{a, c, 3}; }
