// [dcl.init.aggr]/5: a braced list initializes every member of a class
// that was incomplete where a function taking it was declared
// (`void f(J)` before `struct J {...}`); the members not named are
// value-initialized, also through a parameter of reference type and
// through a function referenced while the class was incomplete.
//@ expect: run
//@ stdout: 5 0 0 0 7 0 0 0
#include <cstdio>
struct J;
void f(J, int);
void g(const J&);
void (*pf)(J, int) = f;
struct J { int x; int y; };
int seen[8], n;
void f(J j, int) { seen[n++] = j.x; seen[n++] = j.y; }
void g(const J& j) { seen[n++] = j.x; seen[n++] = j.y; }
int main() {
  f(J{5}, 1);
  g(J{});
  pf(J{7}, 2);
  g(J());
  for (int i = 0; i < n; ++i) std::printf(i ? " %d" : "%d", seen[i]);
  std::printf("\n");
}
