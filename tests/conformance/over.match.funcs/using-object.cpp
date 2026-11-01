// [over.match.funcs.general]/4: a non-conversion member function nominated
// by a using-declaration is considered a member of the derived class for
// the type of its implicit object parameter, so P::h(int) is an exact
// match on the object of a Q and beats Q::h(long) const; the call still
// adjusts `this` to the base subobject (P at a non-zero offset).
//@ expect: run
//@ stdout: P 7
//@ stdout: Q 8
#include <stdio.h>
struct A { int pad[3]; };
struct P { int tag = 7; int h(int) { printf("P %d\n", tag); return 1; } };
struct Q : A, P {
  using P::h;
  int h(long) const { printf("Q %d\n", tag + 1); return 2; }
};
int main() {
  Q q;
  const Q &cq = q;
  return q.h(0) + cq.h(0) == 3 ? 0 : 1;
}
