// A scalar converted to a vector converts to the element type and splats
// ([conv] per lane; clang `CheckImplicitConversion`): a floating literal
// that loses its value is -Wliteral-conversion and an integer constant that
// does not fit the element is -Wconstant-conversion, both naming the vector.
// clang measures a floating value against the whole vector's width, so
// 1e20 into a vector of 4 'int' is not diagnosed. WP-187.
//@ strict-diags
//@ diag: 19:9 warning "implicit conversion from 'double' to 'v4i' (vector of 4 'int' values) changes value from 2.5 to 2"
//@ diag: 20:9 warning "implicit conversion from 'int' to 'v4c' (vector of 4 'char' values) changes value from 300 to 44"
//@ diag: 24:20 warning "implicit conversion from 'double' to 'v4i' (vector of 4 'int' values) changes value from 3.5 to 3"
//@ diag: 25:27 warning "implicit conversion from 'int' to 'v4c' (vector of 4 'char' values) changes value from 1000 to 232"
//@ diag: 26:28 warning "implicit conversion from 'double' to 'v4i' (vector of 4 'int' values) changes value from 1.5 to 1"
//@ diag: 27:9 warning "implicit conversion from 'double' to 'v4b' (vector of 4 'bool' values) changes value from 2.5 to 2"
//@ diag: 28:9 warning "implicit conversion from 'double' to 'v4i' (vector of 4 'int' values) changes value from -0 to 0"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef char v4c __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef bool v4b __attribute__((ext_vector_type(4)));
v4i a = 2.5;
v4c b = 300;
v4i c = 1e20;
v4c d = 'a';
v4f e = 1e300;
v4i g(v4i x) { x = 3.5; return x; }
v4c h(v4c x) { return x + 1000; }
v4i k(bool q) { return q ? 1.5 : 2; }
v4b m = 2.5;
v4i n = -0.0;
