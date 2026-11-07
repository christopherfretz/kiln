// [over.over]/1, [over.match.copy], [over.match.ctor], [over.best.ics]:
// a class type is not an [over.over] target; an overload set (or the
// address of one) initializing a class is the argument of each
// constructor, and each constructor's parameter type selects from the set.
// Copy-, direct- and list-initialization, a return, a by-value argument
// (also against a non-class overload), a member-pointer set, a set with a
// template, and a dependent context.
//@ expect: run
//@ stdout: 15 1 30 2 5 5 4
#include <cstdio>
int f() { return 1; }
int f(int) { return 2; }
template <class T> int g(T) { return 3; }
int g(double, double) { return 4; }
struct S { int m() { return 5; } int m(int) { return 6; } };
struct P { int v; P(int (*p)(int)) : v(p(0)) {} };
struct T { int v; template <class F> T(F*) : v(30) {} };
struct E { int v; explicit E(int (*p)()) : v(p()) {} };
struct W { int v; W(int (*p)(char)) : v(p('a')) {} };
struct M { int v; M(int (S::*p)()) : v((S{}.*p)()) {} };
P ret() { return f; }
int byval(P p) { return p.v; }
int ovl(P p) { return p.v; }
int ovl(long) { return -1; }
template <class U> int dep() { P p = &f; const P& r = f; return p.v + r.v; }
int main() {
  P a = f;
  P b(f);
  P c{f};
  W w = g;
  int r = a.v + b.v + c.v + ret().v + byval(f) + ovl(f) + w.v;
  E e(f);
  T t = g<int>;
  P pa = &f;
  M m = &S::m;
  const M& mr = &S::m;
  std::printf("%d %d %d %d %d %d %d\n", r, e.v, t.v, pa.v, m.v, mr.v, dep<int>());
}
