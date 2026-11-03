// Argument checks of __builtin_elementwise_add_sat / _sub_sat at clang 19
// (SemaChecking SemaBuiltinElementwiseMath, then an integer element
// check): the operands take the usual unary conversions and, when both
// are arithmetic, the usual arithmetic conversions (so `int` with `long`
// is `long`, and bool, short and unscoped enumerations promote to int);
// the common type must be a vector, integer or floating type, then its
// element an integer. clang 19's wording; elementwise-sat-args-clang22.cpp
// has clang 22's (WP-432).
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:39 error "1st argument must be a vector of integers (was 'float')"
//@ diag: 29:40 error "too few arguments to function call, expected 2, have 1"
//@ diag: 30:45 error "too many arguments to function call, expected 2, have 3"
//@ diag: 33:39 error "1st argument must be a vector, integer or floating point type (was 'EC')"
//@ diag: 34:39 error "1st argument must be a vector of integers (was 'v4f' (vector of 4 'float' values))"
//@ diag: 35:39 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'int')"
//@ diag: 36:39 error "1st argument must be a vector, integer or floating point type (was 'S')"
//@ diag: 37:39 error "1st argument must be a vector, integer or floating point type (was 'int *')"
//@ diag: 43:39 error "1st argument must be a vector of integers (was 'float')"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
enum E { e1 };
enum class EC { x };
struct S {};
void f(int i, long l, short s, float x, bool b, E e, EC ec, v4i vi, v4f vf, S st, int *p, unsigned u, _BitInt(37) bi) {
  (void)__builtin_elementwise_add_sat(x, x);
  (void)__builtin_elementwise_add_sat(i, l);
  (void)__builtin_elementwise_add_sat(i);
  (void)__builtin_elementwise_add_sat(i, i, i);
  (void)__builtin_elementwise_add_sat(b, b);
  (void)__builtin_elementwise_add_sat(e, e);
  (void)__builtin_elementwise_add_sat(ec, ec);
  (void)__builtin_elementwise_add_sat(vf, vf);
  (void)__builtin_elementwise_add_sat(vi, i);
  (void)__builtin_elementwise_add_sat(st, st);
  (void)__builtin_elementwise_add_sat(p, p);
  (void)__builtin_elementwise_sub_sat(s, s);
  (void)__builtin_elementwise_sub_sat(s, i);
  (void)__builtin_elementwise_sub_sat(u, i);
  (void)__builtin_elementwise_sub_sat(bi, bi);
  (void)__builtin_elementwise_sub_sat(1, 2);
  (void)__builtin_elementwise_add_sat(i, x);
  (void)__builtin_elementwise_add_sat(e, i);
  (void)__builtin_elementwise_add_sat(b, i);
}
// Accepted: the promoted common type is the result.
int g(E a, short b) { return __builtin_elementwise_add_sat(a, b); }
long h(int a, long b) { return __builtin_elementwise_sub_sat(a, b); }
v4i m(v4i a) { return __builtin_elementwise_add_sat(a, a); }
