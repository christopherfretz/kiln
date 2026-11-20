// [dcl.array]/1, [expr.sizeof]/1, [basic.start.dynamic]: GNU variable
// length arrays outside a function. A type-id whose bound is not a
// constant expression is variably modified at namespace and class scope
// too: `sizeof(int[m])` is evaluated by the dynamic initializer (and by
// the constructor, for a default member initializer). A parameter of a
// function template may be variably modified as well (a call's
// substitution then fails, [temp.deduct.general]/11). Found running
// GCC's C++ testsuite.
//@ expect: run
//@ stdout: 3 9 3
#include <cstdio>

int m = 3;
int t = sizeof(int[m]) / sizeof(int);
int u = sizeof(int[m][m]) / sizeof(int);

struct S {
  int k = sizeof(int[m]) / sizeof(int);
};

template <class T> T last(int n, T (*a)[n]);
template <class T> T first(int n, T a[n]) { return a[0]; }

int main() {
  S s;
  std::printf("%d %d %d\n", t, u, s.k);
}
