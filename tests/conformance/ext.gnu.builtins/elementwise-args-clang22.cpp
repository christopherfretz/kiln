// Argument checks of __builtin_elementwise_abs / _max / _min at clang 22
// (SemaChecking BuiltinElementwiseMath): abs takes a signed integer,
// floating or vector of them; max / min take two operands of the same
// integer, floating or vector type (no conversion; bool and enumerations
// are rejected). The operand keeps its type (no promotion). clang 22's
// wording; tests/conformance/ext.gnu.builtins/elementwise-args.cpp has
// clang 19's.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:29 error "1st argument must be a scalar or vector of signed integer or floating-point types (was 'unsigned int')"
//@ diag: 32:29 error "1st argument must be a scalar or vector of signed integer or floating-point types (was 'v4u' (vector of 4 'unsigned int' values))"
//@ diag: 33:29 error "1st argument must be a scalar or vector of signed integer or floating-point types (was 'bool')"
//@ diag: 34:29 error "1st argument must be a scalar or vector of signed integer or floating-point types (was 'E')"
//@ diag: 35:29 error "too few arguments to function call, expected 1, have 0"
//@ diag: 36:32 error "too many arguments to function call, expected 1, have 2"
//@ diag: 38:29 error "arguments are of different types ('short' vs 'long long')"
//@ diag: 39:29 error "arguments are of different types ('unsigned int' vs 'int')"
//@ diag: 40:29 error "arguments are of different types ('float' vs 'double')"
//@ diag: 41:29 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'v4u' (vector of 4 'unsigned int' values))"
//@ diag: 42:29 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'int')"
//@ diag: 43:29 error "1st argument must be a vector, integer or floating-point type (was 'int *')"
//@ diag: 45:29 error "1st argument must be a vector, integer or floating-point type (was 'bool')"
//@ diag: 46:29 error "1st argument must be a vector, integer or floating-point type (was 'E')"
//@ diag: 47:35 error "too many arguments to function call, expected 2, have 3"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned v4u __attribute__((ext_vector_type(4)));
enum E { e1 };
void f(short s, unsigned u, float fl, double d, bool b, E e, int *p, v4i vi, v4u vu, long long l, signed char c) {
  auto a1 = __builtin_elementwise_abs(s);
  __builtin_elementwise_abs(u);
  __builtin_elementwise_abs(vu);
  __builtin_elementwise_abs(b);
  __builtin_elementwise_abs(e);
  __builtin_elementwise_abs();
  __builtin_elementwise_abs(s, s);
  auto m1 = __builtin_elementwise_max(s, s);
  __builtin_elementwise_max(s, l);
  __builtin_elementwise_max(u, 1);
  __builtin_elementwise_max(fl, d);
  __builtin_elementwise_max(vi, vu);
  __builtin_elementwise_max(vi, 1);
  __builtin_elementwise_max(p, p);
  auto m2 = __builtin_elementwise_min(c, c);
  __builtin_elementwise_min(b, b);
  __builtin_elementwise_min(e, e);
  __builtin_elementwise_max(1, 2, 3);
  (void)a1; (void)m1; (void)m2;
}
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
void g(short s, const int ci, int i, signed char c) {
  static_assert(same<decltype(__builtin_elementwise_max(s, (short)1)), short>);
  static_assert(same<decltype(__builtin_elementwise_max(ci, i)), int>);
  static_assert(same<decltype(__builtin_elementwise_abs(c)), signed char>);
}
