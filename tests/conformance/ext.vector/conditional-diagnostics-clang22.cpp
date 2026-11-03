// Errors in the GNU element-wise conditional (clang
// CheckVectorConditionalTypes) and in a scalar-condition conditional with
// vector arms (CheckVectorOperands); texts and locations are clang 19's,
// including the stray `}` of err_conditional_vector_mismatched (WP-173).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 43:12 error "vector operands to the vector conditional must be the same type ('g4i' (vector of 4 'int' values) and 'g4u' (vector of 4 'unsigned int' values))}"
//@ diag: 44:12 error "vector operands to the vector conditional must be the same type ('g4i' (vector of 4 'int' values) and 'v4i' (vector of 4 'int' values))}"
//@ diag: 45:13 error "cannot mix vectors and extended vectors in a vector conditional"
//@ diag: 46:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type 'g8s' (vector of 8 'short' values) do not have the same number of elements"
//@ diag: 47:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type 'g2l' (vector of 2 'long' values) do not have the same number of elements"
//@ diag: 48:12 error "enumeration type 'E' is not allowed in a vector conditional"
//@ diag: 49:14 error "GNU vector conditional operand cannot be void"
//@ diag: 50:14 error "GNU vector conditional operand cannot be a throw expression"
//@ diag: 51:18 error "GNU vector conditional operand cannot be a throw expression"
//@ diag: 52:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type '__attribute__((__vector_size__(4 * sizeof(double)))) double' (vector of 4 'double' values) do not have elements of the same size"
//@ diag: 53:12 error "vector operands to the vector conditional must be the same type ('g4i' (vector of 4 'int' values) and 'g2i' (vector of 2 'int' values))}"
//@ diag: 54:12 error "vector operands to the vector conditional must be the same type ('g4i' (vector of 4 'int' values) and 'g8s' (vector of 8 'short' values))}"
//@ diag: 55:10 error "value of type 'g4f' (vector of 4 'float' values) is not contextually convertible to 'bool'"
//@ diag: 56:12 error "cannot convert between vector type 'g2i' (vector of 2 'int' values) and vector type 'g4i' (vector of 4 'int' values) as implicit conversion would cause truncation"
//@ diag: 57:12 error "invalid operands to binary expression ('v4b' (vector of 4 'bool' values) and 'bool')"
//@ diag: 59:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type '__attribute__((__vector_size__(4 * sizeof(long)))) long' (vector of 4 'long' values) do not have elements of the same size"
//@ diag: 60:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type 'g4l' (vector of 4 'long' values) do not have elements of the same size"
//@ diag: 61:12 error "vector condition type 'g4i' (vector of 4 'int' values) and result type '__attribute__((__vector_size__(4 * sizeof(int *)))) int *' (vector of 4 'int *' values) do not have elements of the same size"
//@ diag: 66:64 error "vector condition type '__attribute__((__vector_size__(4 * sizeof(int)))) int' (vector of 4 'int' values) and result type '__attribute__((__vector_size__(4 * sizeof(double)))) double' (vector of 4 'double' values) do not have elements of the same size"
//@ diag: 67:13 note "in instantiation of function template specialization 'sel<__attribute__((__vector_size__(4 * sizeof(int)))) int, double>' requested here"
//@ requires: clang>=22
// clang 22 spelling of conditional-diagnostics.cpp (WP-394).
typedef int g4i __attribute__((vector_size(16)));
typedef unsigned g4u __attribute__((vector_size(16)));
typedef float g4f __attribute__((vector_size(16)));
typedef short g8s __attribute__((vector_size(16)));
typedef long g2l __attribute__((vector_size(16)));
typedef long g4l __attribute__((vector_size(32)));
typedef int g2i __attribute__((vector_size(8)));
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef bool v4b __attribute__((ext_vector_type(4)));
enum E { e0 };
void f(g4i c, g4i a, g4u u, g4f fl, g8s s, g2l l, v4i ec, v4f ef, g2i n,
       v4b b, int i, E en, float x, long li, g4l vl, int *p) {
  (void)(c ? a : u);
  (void)(c ? a : ec);
  (void)(ec ? a : a);
  (void)(c ? s : s);
  (void)(c ? l : l);
  (void)(c ? en : en);
  (void)(c ? (void)0 : (void)0);
  (void)(c ? throw 1 : a);
  (void)(c ? a : throw 1);
  (void)(c ? 1.0 : 2.0);
  (void)(c ? a : n);
  (void)(c ? a : s);
  (void)(fl ? a : a);
  (void)(i ? a : n);
  (void)(i ? b : true);
  (void)(b ? i : i);
  (void)(c ? li : 1);
  (void)(c ? vl : 1);
  (void)(c ? p : p);
  (void)(c ? i : en);
  (void)(n ? i : i);
  (void)(c ? i : 'c');
}
template <class C, class T> auto sel(C k, T p, T q) { return k ? p : q; }
auto inst = sel(g4i{}, 1.0, 2.0);
