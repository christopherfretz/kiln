// [dcl.ambig.res]/2, [expr.cast]: `(T())` is an ambiguous type-id; followed
// by `++` or `--` and then no operand it is a parenthesized expression with
// a postfix operator, not a cast (`(T())++;`); with an operand it is a cast
// of a prefix increment (`(int)++a`). GCC 16.1, clang 22. Found running
// GCC's C++ testsuite (WP-1466).
//@ expect: run
//@ std: c++17
//@ strict-diags
struct T { int n = 0; int operator++(int) { return 1; } };
struct U { int operator--(int) { return 2; } };
int main() {
  int r = (T())++;
  r += (U())--;
  int a = 0;
  r += (int)++a;
  r += (int)--a;
  return r == 1 + 2 + 1 + 0 ? 0 : 1;
}
