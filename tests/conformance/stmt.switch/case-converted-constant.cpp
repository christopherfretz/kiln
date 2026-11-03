// [stmt.switch]/2, [expr.const]/12: a case value is a converted constant
// expression of the (promoted) condition type. A floating-integral
// conversion is not allowed there, so `case 1.5:` and `case d:` with a
// `const double d` are errors (not a silently truncating conversion), and
// an integral value outside the condition type narrows (a
// `-Wc++11-narrowing` error by default). Each is reported at the value's
// first token and names the value's type as written. clang is the oracle.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:8 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 19:8 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 21:8 error "case value evaluates to 300000000000, which cannot be narrowed to type 'int'"
//@ diag: 29:8 error "conversion from 'const double' to 'long' is not allowed in a converted constant expression"
//@ diag: 36:8 error "case value evaluates to 4294967296, which cannot be narrowed to type 'unsigned int'"
//@ diag: 46:8 error "conversion from 'float' to 'int' is not allowed in a converted constant expression"
int f(int x) {
  switch (x) {
  case 1.5: return 1;
  case 2.0: return 2;
  case 'a': return 3;
  case 300000000000LL: return 4;
  case -1: return 5;
  }
  return 0;
}
constexpr double d = 3.0;
int g(long x) {
  switch (x) {
  case d: return 1;
  case (int)d + 1: return 2;
  }
  return 0;
}
int h(unsigned x) {
  switch (x) {
  case 4294967296: return 1;
  case 7: return 2;
  }
  return 0;
}
// An enumerator converts; a `float` does not.
enum Color { Red, Green };
int k(int x) {
  switch (x) {
  case Green: return 1;
  case 1e0f: return 2;
  }
  return 0;
}
