// Clang ext vectors, [clang 20+] (clang Sema IsVectorConversion,
// ICK_Boolean_Conversion): only an integer ext vector converts implicitly
// to a `bool` ext vector, and only with the same lane count; a float, GNU
// or wider vector does not, a `bool` vector does not convert back, list
// initialization from an int vector narrows, and the conversion is not a
// constant expression (clang's VectorExprEvaluator has no case for it).
// WP-453.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ requires: clang>=22
//@ diag: 34:6 error "cannot initialize a variable of type 'b4' (vector of 4 'bool' values) with an lvalue of type 'f4' (vector of 4 'float' values)"
//@ diag: 35:6 error "cannot initialize a variable of type 'b4' (vector of 4 'bool' values) with an lvalue of type 'i8' (vector of 8 'int' values)"
//@ diag: 36:6 error "cannot initialize a variable of type 'b4' (vector of 4 'bool' values) with an lvalue of type 'gv4' (vector of 4 'int' values)"
//@ diag: 37:6 error "cannot initialize a variable of type 'i4' (vector of 4 'int' values) with an lvalue of type 'b4' (vector of 4 'bool' values)"
//@ diag: 38:11 error "C-style cast from vector 'b4' (vector of 4 'bool' values) to vector 'i4' (vector of 4 'int' values) of different size"
//@ diag: 39:11 error "C-style cast from vector 'f4' (vector of 4 'float' values) to vector 'b4' (vector of 4 'bool' values) of different size"
//@ diag: 40:9 error "type 'i4' (vector of 4 'int' values) cannot be narrowed to 'b4' (vector of 4 'bool' values) in initializer list"
//@ diag: 40:9 note "insert an explicit cast to silence this issue"
//@ diag: 41:12 error "type 'i4' (vector of 4 'int' values) cannot be narrowed to 'b4' (vector of 4 'bool' values) in initializer list"
//@ diag: 41:12 note "insert an explicit cast to silence this issue"
//@ diag: 44:19 error "constexpr variable 'cx' must be initialized by a constant expression"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:10 note "subexpression not valid in a constant expression"
//@ diag: 50:15 note "in call to 'g()'"
typedef int i4 __attribute__((ext_vector_type(4)));
typedef float f4 __attribute__((ext_vector_type(4)));
typedef int i8 __attribute__((ext_vector_type(8)));
typedef int gv4 __attribute__((vector_size(16)));
typedef bool b4 __attribute__((ext_vector_type(4)));
void n(b4);
void n(bool);
void f(i4 I, f4 F, i8 E, gv4 G, b4 B) {
  b4 x1 = F;
  b4 x2 = E;
  b4 x3 = G;
  i4 x4 = B;
  i4 x5 = (i4)B;
  b4 x6 = (b4)F;
  b4 x7{I};
  b4 x8 = {I};
  n(I);
}
constexpr b4 cx = i4{1, 0, 2, 0};
constexpr bool g() {
  i4 I = {1, 0, 2, 0};
  b4 x = I;
  return x[0];
}
static_assert(g());
