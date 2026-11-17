// [expr.call]/12: a class lvalue passed through an ellipsis is copied
// ([conv.lval]/3.2) by the implicit copy constructor, which is an exact
// match and beats the variadic constructor `S(...)`; the variadic
// constructor is used only for an argument list that no other
// constructor takes. Companion of variadic-copy-own-ellipsis-constructor.cpp
// (found running GCC's C++ testsuite).
//@ expect: run
//@ stdout: 1 7 7
#include <cstdio>
int ellipsis_ctors;
struct S {
  int v;
  S(...) : v(7) { ++ellipsis_ctors; }
};
int pass(int n, ...) { return n; }
int main() {
  S a(1, 2);  // the variadic constructor
  S b(a);     // the implicit copy constructor
  S c = b;
  int v = pass(7, c);  // copied by the implicit copy constructor
  std::printf("%d %d %d\n", ellipsis_ctors, c.v, v);
}
