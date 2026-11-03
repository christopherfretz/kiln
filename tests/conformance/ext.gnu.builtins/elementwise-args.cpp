// Argument checks of __builtin_elementwise_abs / _max / _min at clang 19
// (SemaChecking PrepareBuiltinElementwiseMathOneArgCall /
// SemaBuiltinElementwiseMath): the operands take the usual unary
// conversions (bool and unscoped enumerations promote to int), max / min
// then the usual arithmetic conversions when both are arithmetic, and the
// two operand types must then agree. clang 19's wording;
// elementwise-args-clang22.cpp has clang 22's.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:29 error "1st argument must be a signed integer or floating point type (was 'unsigned int')"
//@ diag: 32:29 error "1st argument must be a signed integer or floating point type (was 'v4u' (vector of 4 'unsigned int' values))"
//@ diag: 33:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 34:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 35:29 error "too few arguments to function call, expected 1, have 0"
//@ diag: 36:32 error "too many arguments to function call, expected 1, have 2"
//@ diag: 38:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 39:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 40:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 41:29 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'v4u' (vector of 4 'unsigned int' values))"
//@ diag: 42:29 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'int')"
//@ diag: 43:29 error "1st argument must be a vector, integer or floating point type (was 'int *')"
//@ diag: 45:3 warning "ignoring return value of function declared with const attribute"
//@ diag: 46:3 warning "ignoring return value of function declared with const attribute"
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
  static_assert(same<decltype(__builtin_elementwise_max(s, (short)1)), int>);
  static_assert(same<decltype(__builtin_elementwise_max(ci, i)), int>);
  static_assert(same<decltype(__builtin_elementwise_abs(c)), int>);
}
