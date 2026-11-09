// C11 6.5.1.1p3 (_Generic in C++, a clang extension): the controlling
// expression is not evaluated, only the selected association is (code is
// emitted for it alone); the result is that expression with its value
// category (an assignable lvalue, a reference binding, a class lvalue
// without a copy, a class prvalue); the expression operand undergoes
// lvalue, array-to-pointer and function-to-pointer conversion, a class
// keeping its cv-qualifiers; dependent expression and type operands are
// selected at instantiation.
//@ expect: run
//@ stdout: 10 0
//@ stdout: 42 0
//@ stdout: 10
//@ stdout: 7
//@ stdout: int long str other
//@ stdout: 1 2 3
//@ stdout: 2
//@ stdout: 3
//@ stdout: 2.5
//@ stdout: 7
#include <cstdio>
struct S { int m = 7; S() = default; S(const S&) { std::puts("copy"); } };
int calls;
int side() { ++calls; return 1; }
int x = 5;
const S cs;
template <class T> const char *name(T v) { return _Generic(v, int: "int", long: "long", const char *: "str", default: "other"); }
template <class T> int pick() { return _Generic(T, int: 1, S: 2, default: 3); }
int main() {
  // Controlling expression is not evaluated.
  int r = _Generic(side(), int: 10, default: 20);
  std::printf("%d %d\n", r, calls);
  // Only the selected association is evaluated.
  int r2 = _Generic(x, long: side(), int: 42);
  std::printf("%d %d\n", r2, calls);
  // lvalue result: assignable.
  _Generic(x, int: x) = 9;
  int &ref = _Generic(x, int: x, default: r);
  ref += 1;
  std::printf("%d\n", x);
  // class keeps its cv-qualifiers; no copy.
  std::printf("%d\n", _Generic(cs, const S: cs.m, S: 0));
  std::printf("%s %s %s %s\n", name(1), name(2L), name("s"), name(1.0));
  std::printf("%d %d %d\n", pick<int>(), pick<S>(), pick<char>());
  // array and function decay
  int arr[2] = {1, 2};
  std::printf("%d\n", _Generic(arr, int *: arr[1], default: 0));
  std::printf("%d\n", _Generic(main, int (*)(): 3, default: 0));
  double d = _Generic(1.0f, float: 2.5, default: 0.0);
  std::printf("%g\n", d);
  S s2 = _Generic(1, int: S(), default: S());
  std::printf("%d\n", s2.m);
  return _Generic(x, int: 0, default: 1);
}
