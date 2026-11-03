// [expr.log.and]/1, [expr.log.or]/1 (clang -Wconstant-logical-operand, on
// by default): a non-bool integer left operand of `&&` with a right operand
// that folds to an integer (clang `EvaluateAsInt`) warns, with notes
// suggesting `&` and removing the constant. In C++ any folded value warns
// unless the right operand is `bool` or comes from a macro, where only a
// value other than 0 or 1 does. Not for a bool, floating or constant-only
// mismatch of types, an operator spelled by a macro, a template
// instantiation (the definition warns), a value-dependent operand, or an
// enumerator operand whose value is not 0 or 1.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 70:47 warning "use of logical '&&' with constant operand"
//@ diag: 70:47 note "use '&' for a bitwise operation"
//@ diag: 70:47 note "remove constant to silence this warning"
//@ diag: 75:10 warning "use of logical '&&' with constant operand"
//@ diag: 75:10 note "use '&' for a bitwise operation"
//@ diag: 75:10 note "remove constant to silence this warning"
//@ diag: 76:10 warning "use of logical '&&' with constant operand"
//@ diag: 76:10 note "use '&' for a bitwise operation"
//@ diag: 76:10 note "remove constant to silence this warning"
//@ diag: 77:10 warning "use of logical '&&' with constant operand"
//@ diag: 77:10 note "use '&' for a bitwise operation"
//@ diag: 77:10 note "remove constant to silence this warning"
//@ diag: 78:10 warning "use of logical '&&' with constant operand"
//@ diag: 78:10 note "use '&' for a bitwise operation"
//@ diag: 78:10 note "remove constant to silence this warning"
//@ diag: 79:10 warning "use of logical '&&' with constant operand"
//@ diag: 79:10 note "use '&' for a bitwise operation"
//@ diag: 79:10 note "remove constant to silence this warning"
//@ diag: 80:10 warning "use of logical '&&' with constant operand"
//@ diag: 80:10 note "use '&' for a bitwise operation"
//@ diag: 80:10 note "remove constant to silence this warning"
//@ diag: 81:10 warning "use of logical '&&' with constant operand"
//@ diag: 81:10 note "use '&' for a bitwise operation"
//@ diag: 81:10 note "remove constant to silence this warning"
//@ diag: 82:10 warning "use of logical '&&' with constant operand"
//@ diag: 82:10 note "use '&' for a bitwise operation"
//@ diag: 82:10 note "remove constant to silence this warning"
//@ diag: 83:10 warning "use of logical '&&' with constant operand"
//@ diag: 83:10 note "use '&' for a bitwise operation"
//@ diag: 83:10 note "remove constant to silence this warning"
//@ diag: 84:10 warning "use of logical '&&' with constant operand"
//@ diag: 84:10 note "use '&' for a bitwise operation"
//@ diag: 84:10 note "remove constant to silence this warning"
//@ diag: 85:10 warning "use of logical '&&' with constant operand"
//@ diag: 85:10 note "use '&' for a bitwise operation"
//@ diag: 85:10 note "remove constant to silence this warning"
//@ diag: 86:10 warning "use of logical '&&' with constant operand"
//@ diag: 86:10 note "use '&' for a bitwise operation"
//@ diag: 86:10 note "remove constant to silence this warning"
//@ diag: 87:10 warning "use of logical '&&' with constant operand"
//@ diag: 87:10 note "use '&' for a bitwise operation"
//@ diag: 87:10 note "remove constant to silence this warning"
//@ diag: 88:10 warning "use of logical '&&' with constant operand"
//@ diag: 88:10 note "use '&' for a bitwise operation"
//@ diag: 88:10 note "remove constant to silence this warning"
//@ diag: 89:27 warning "use of logical '&&' with constant operand"
//@ diag: 89:27 note "use '&' for a bitwise operation"
//@ diag: 89:27 note "remove constant to silence this warning"
//@ diag: 90:10 warning "use of logical '&&' with constant operand"
//@ diag: 90:10 note "use '&' for a bitwise operation"
//@ diag: 90:10 note "remove constant to silence this warning"
//@ diag: 91:10 warning "use of logical '&&' with constant operand"
//@ diag: 91:10 note "use '&' for a bitwise operation"
//@ diag: 91:10 note "remove constant to silence this warning"
#define ONE 1
#define FIVE 5
#define AND &&
enum E { e0, e1, e5 = 5 };
template <class T> int g(T, int x) { return x && 5; }
template <int N> int h(int x) { return x && N; }
int f(int x, double d, bool b, unsigned u, long l, E e) {
  const int k = 5;
  int r = 0;
  r += x && 5;
  r += x && 1;
  r += x && 0;
  r += x && (5);
  r += x && -1;
  r += u && 3u;
  r += l && 'a';
  r += e && 2;
  r += x && k;
  r += x && sizeof(int);
  r += x && (1 + 1);
  r += x && e1;
  r += x && FIVE;
  r += x && __builtin_isnan(1.0);
  r += __builtin_isnan(d) && __builtin_fpclassify(1, 2, 3, 4, 5, 1.0);
  r += x && __builtin_popcount(7);
  r += x && 1 && 5;
  r += x && ONE;
  r += x AND 5;
  r += x && true;
  r += x && (bool)5;
  r += x && !5;
  r += b && 5;
  r += d && 5;
  r += 5 && x;
  r += x && e5;
  r += e5 && 2;
  r += x && __builtin_isnan(d);
  r += x && __builtin_constant_p(x);
  r += __builtin_isnan(d) && __builtin_fpclassify(1, 2, 3, 4, 5, d);
  return r + g(0, x) + h<4>(x);
}
