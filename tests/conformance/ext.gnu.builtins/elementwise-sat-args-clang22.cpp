// Argument checks of __builtin_elementwise_add_sat / _sub_sat at clang 22
// (SemaChecking BuiltinElementwiseMath with the integer restriction): two
// arguments, decayed and not promoted; the first must be a scalar or
// vector of integer type (bool and an unscoped enumeration are integer
// types), then the second must have the same unqualified type. The result
// has that type. elementwise-sat-args.cpp has clang 19 (WP-432).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:39 error "1st argument must be a scalar or vector of integer types (was 'float')"
//@ diag: 31:39 error "arguments are of different types ('int' vs 'long')"
//@ diag: 32:40 error "too few arguments to function call, expected 2, have 1"
//@ diag: 33:45 error "too many arguments to function call, expected 2, have 3"
//@ diag: 36:39 error "1st argument must be a scalar or vector of integer types (was 'EC')"
//@ diag: 37:39 error "1st argument must be a scalar or vector of integer types (was 'v4f' (vector of 4 'float' values))"
//@ diag: 38:39 error "arguments are of different types ('v4i' (vector of 4 'int' values) vs 'int')"
//@ diag: 39:39 error "1st argument must be a scalar or vector of integer types (was 'S')"
//@ diag: 40:39 error "1st argument must be a scalar or vector of integer types (was 'int *')"
//@ diag: 42:39 error "arguments are of different types ('short' vs 'int')"
//@ diag: 43:39 error "arguments are of different types ('unsigned int' vs 'int')"
//@ diag: 46:39 error "arguments are of different types ('int' vs 'float')"
//@ diag: 47:39 error "arguments are of different types ('E' vs 'int')"
//@ diag: 48:39 error "arguments are of different types ('bool' vs 'int')"
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
// Accepted: the same integer type (bool, an unscoped enumeration, a
// `_BitInt`), the result of that type.
E g(E a) { return __builtin_elementwise_add_sat(a, a); }
bool h(bool a, bool b) { return __builtin_elementwise_sub_sat(a, b); }
short k(short a, short b) { return __builtin_elementwise_sub_sat(a, b); }
v4i m(v4i a) { return __builtin_elementwise_add_sat(a, a); }
